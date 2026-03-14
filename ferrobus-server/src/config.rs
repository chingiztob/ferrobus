use std::{
    error::Error,
    fmt, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use chrono::NaiveDate;
use clap::Parser;
use serde::Deserialize;

const DEFAULT_MAX_TRANSFER_TIME: u32 = 1200;

#[derive(Parser, Debug, Clone)]
pub struct ServerCli {
    #[arg(long)]
    pub config: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub osm_path: PathBuf,
    pub gtfs_dirs: Vec<PathBuf>,
    pub date: Option<NaiveDate>,
    pub max_transfer_time: u32,
    pub bind: SocketAddr,
}

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    MissingField(&'static str),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    f,
                    "failed to read config file '{}': {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => {
                write!(
                    f,
                    "failed to parse TOML config '{}': {source}",
                    path.display()
                )
            }
            Self::MissingField(field) => write!(f, "missing required option '{field}'"),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::MissingField(_) => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct FileConfig {
    osm_path: Option<PathBuf>,
    gtfs_dirs: Option<Vec<PathBuf>>,
    date: Option<NaiveDate>,
    max_transfer_time: Option<u32>,
    bind: Option<SocketAddr>,
}

impl ServerCli {
    pub fn resolve(self) -> Result<ServerConfig, ConfigError> {
        let file_cfg = read_file_config(&self.config)?;

        let osm_path = file_cfg
            .osm_path
            .ok_or(ConfigError::MissingField("osm_path"))?;

        let gtfs_dirs = file_cfg.gtfs_dirs.unwrap_or_default();
        if gtfs_dirs.is_empty() {
            return Err(ConfigError::MissingField("gtfs_dirs"));
        }

        let bind = file_cfg
            .bind
            .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 3000)));

        Ok(ServerConfig {
            osm_path,
            gtfs_dirs,
            date: file_cfg.date,
            max_transfer_time: file_cfg
                .max_transfer_time
                .unwrap_or(DEFAULT_MAX_TRANSFER_TIME),
            bind,
        })
    }
}

fn read_file_config(path: &Path) -> Result<FileConfig, ConfigError> {
    let content = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&content).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_toml_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos();
        path.push(format!("ferrobus_server_cfg_{}_{}.toml", process::id(), ts));
        path
    }

    #[test]
    fn resolve_reads_toml_only() {
        let path = temp_toml_path();
        fs::write(
            &path,
            r#"
osm_path = "D:/data/file.osm.pbf"
gtfs_dirs = ["D:/data/file_feed"]
max_transfer_time = 900
bind = "127.0.0.1:4000"
"#,
        )
        .expect("config file should be written");

        let cli = ServerCli {
            config: path.clone(),
        };

        let resolved = cli.resolve().expect("config should resolve");
        assert_eq!(resolved.osm_path, PathBuf::from("D:/data/file.osm.pbf"));
        assert_eq!(resolved.gtfs_dirs, vec![PathBuf::from("D:/data/file_feed")]);
        assert_eq!(resolved.max_transfer_time, 900);
        assert_eq!(resolved.bind, "127.0.0.1:4000".parse().unwrap());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn resolve_requires_mandatory_fields() {
        let path = temp_toml_path();
        fs::write(&path, "").expect("config file should be written");

        let cli = ServerCli {
            config: path.clone(),
        };

        let err = cli.resolve().expect_err("missing fields should fail");
        assert!(matches!(err, ConfigError::MissingField("osm_path")));

        let _ = fs::remove_file(path);
    }
}
