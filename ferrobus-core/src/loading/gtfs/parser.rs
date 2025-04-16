use std::fs::File;
use std::path::Path;

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
pub fn parse_time(time_str: &str) -> u32 {
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
