use std::fs::File;
use std::path::Path;

use crate::Error;

use serde::Deserialize;

fn format_csv_error(path: &Path, err: &csv::Error) -> Error {
    let position = err.position().map_or_else(
        || "position unknown".to_string(),
        |pos| {
            let line = pos.line();
            let record = pos.record();
            format!("line {line}, record {record}")
        },
    );

    Error::InvalidData(format!(
        "Failed to deserialize GTFS file '{}' at {position}: {err}",
        path.display()
    ))
}

pub fn deserialize_gtfs_file<T>(path: &Path) -> Result<Vec<T>, Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let file = File::open(path).map_err(|e| {
        Error::IoError(std::io::Error::new(
            e.kind(),
            format!("Failed to open file '{}': {}", path.display(), e),
        ))
    })?;

    let mut rows = Vec::new();
    let mut reader = csv::Reader::from_reader(file);
    for record in reader.deserialize() {
        let row = record.map_err(|err| format_csv_error(path, &err))?;
        rows.push(row);
    }

    Ok(rows)
}

pub fn deserialize_optional_gtfs_file<T>(path: &Path) -> Vec<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    match deserialize_gtfs_file(path) {
        Ok(rows) => rows,
        Err(err) => {
            log::warn!("Skipping optional GTFS file '{}': {err}", path.display());
            Vec::new()
        }
    }
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
