use crate::Time;
use std::path::PathBuf;

/// Configuration for creating a transit model
#[derive(Debug, Clone)]
pub struct TransitModelConfig {
    /// Directories containing GTFS data
    pub gtfs_dirs: Vec<PathBuf>,
    /// OSM file with road network data
    pub osm_path: PathBuf,
    /// Maximum transfer time in seconds
    pub max_transfer_time: Time,
    /// Day of week for trips filtering
    pub date: Option<chrono::NaiveDate>,
}

impl Default for TransitModelConfig {
    fn default() -> Self {
        Self {
            max_transfer_time: 1200, // 20 minutes
            osm_path: PathBuf::new(),
            gtfs_dirs: vec![PathBuf::new()],
            date: None,
        }
    }
}

impl TransitModelConfig {
    pub fn new(osm_path: PathBuf, max_transfer_time: Time) -> Self {
        Self {
            max_transfer_time,
            osm_path,
            gtfs_dirs: Vec::new(),
            date: None,
        }
    }

    #[must_use]
    pub fn add_gtfs_dir(mut self, dir: PathBuf) -> Self {
        self.gtfs_dirs.push(dir);
        self
    }
}
