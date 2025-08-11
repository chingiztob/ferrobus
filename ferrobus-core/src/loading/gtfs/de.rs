use std::fs::File;
use std::path::Path;

use serde::Deserialize;

pub fn deserialize_gtfs_file<T>(path: &Path) -> Result<Vec<T>, std::io::Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let file = File::open(path).map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("Failed to open file '{}': {}", path.display(), e),
        )
    })?;
    Ok(csv::Reader::from_reader(file)
        .deserialize()
        .filter_map(Result::ok)
        .collect::<Vec<T>>())
}

/// Parse time string in HH:MM:SS format to seconds since midnight
fn parse_time(time_str: &str) -> u32 {
    let bytes = time_str.as_bytes();

    // Fast path for HH:MM:SS format
    if bytes.len() >= 8 && bytes[2] == b':' && bytes[5] == b':' {
        let hours = u32::from(bytes[0] - b'0') * 10 + u32::from(bytes[1] - b'0');
        let minutes = u32::from(bytes[3] - b'0') * 10 + u32::from(bytes[4] - b'0');
        let seconds = u32::from(bytes[6] - b'0') * 10 + u32::from(bytes[7] - b'0');
        return hours * 3600 + minutes * 60 + seconds;
    }

    // Fallback for other formats
    parse_time_fallback(time_str)
}

fn parse_time_fallback(time_str: &str) -> u32 {
    let mut parts = time_str.split(':');
    let hours = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minutes = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let seconds = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    hours * 3600 + minutes * 60 + seconds
}

pub(super) fn deserialize_gtfs_date<'de, D>(
    deserializer: D,
) -> Result<Option<chrono::NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let date_str = String::deserialize(deserializer)?;
    if date_str.is_empty() {
        Ok(None)
    } else {
        chrono::NaiveDate::parse_from_str(&date_str, "%Y%m%d")
            .map(Some)
            .map_err(serde::de::Error::custom)
    }
}

pub(super) fn deserialize_gtfs_time<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let time_str = String::deserialize(deserializer)?;
    Ok(parse_time(&time_str))
}
