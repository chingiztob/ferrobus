use std::fs::File;
use std::path::Path;

use serde::Deserialize;

pub fn deserialize_gtfs_file<T>(path: &Path) -> Result<Vec<T>, std::io::Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let file = File::open(path)?;
    Ok(csv::Reader::from_reader(file)
        .deserialize()
        .filter_map(Result::ok)
        .collect::<Vec<T>>())
}

/// Parse time string in HH:MM:SS format to seconds since midnight
fn parse_time(time_str: &str) -> u32 {
    let mut parts = time_str.split(':');
    let hours = parts
        .next()
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);
    let minutes = parts
        .next()
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);
    let seconds = parts
        .next()
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);

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
