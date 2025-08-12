use std::fs::File;
use std::path::Path;

use crate::Error;

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
fn parse_time(time_str: &str) -> Result<u32, Error> {
    let time_str = time_str.trim();
    let bytes = time_str.as_bytes();

    if bytes.len() == 8 && bytes[2] == b':' && bytes[5] == b':' {
        if !(bytes[0].is_ascii_digit()
            && bytes[1].is_ascii_digit()
            && bytes[3].is_ascii_digit()
            && bytes[4].is_ascii_digit()
            && bytes[6].is_ascii_digit()
            && bytes[7].is_ascii_digit())
        {
            return Err(Error::InvalidTimeFormat(time_str.to_string()));
        }

        let hours = u32::from(bytes[0] - b'0') * 10 + u32::from(bytes[1] - b'0');
        let minutes = u32::from(bytes[3] - b'0') * 10 + u32::from(bytes[4] - b'0');
        let seconds = u32::from(bytes[6] - b'0') * 10 + u32::from(bytes[7] - b'0');
        return Ok(hours * 3600 + minutes * 60 + seconds);
    }

    Err(Error::InvalidTimeFormat(time_str.to_string()))
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
    parse_time(&time_str).map_err(serde::de::Error::custom)
}
