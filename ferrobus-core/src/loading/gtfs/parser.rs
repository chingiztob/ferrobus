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
    let parts: Vec<u32> = time_str
        .split(':')
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect();

    parts[0] * 3600 + parts[1] * 60 + parts[2]
}
