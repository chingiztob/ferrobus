use ferrobus_core::{Error, TransitModel, TransitModelConfig, create_transit_model};
use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

fn get_test_data_dir() -> PathBuf {
    PathBuf::from("..").join("tests").join("test-data")
}

fn temp_dir(name: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be valid")
        .as_nanos();
    std::env::temp_dir().join(format!("ferrobus_core_{name}_{}_{}", process::id(), ts))
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("destination directory should be created");
    for entry in fs::read_dir(src).expect("source directory should be readable") {
        let entry = entry.expect("directory entry should be readable");
        let file_type = entry.file_type().expect("file type should be readable");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&from, &to);
        } else {
            fs::copy(&from, &to).expect("file should be copied");
        }
    }
}

fn create_valid_config() -> TransitModelConfig {
    let test_data_dir = get_test_data_dir();
    TransitModelConfig {
        osm_path: test_data_dir.join("roads_zhelez.pbf"),
        gtfs_dirs: vec![test_data_dir.join("zhelez")],
        date: None,
        max_transfer_time: 1200,
    }
}

//  config with custom parameters
fn create_custom_config(
    custom_osm: Option<PathBuf>,
    custom_gtfs: Option<Vec<PathBuf>>,
    custom_date: Option<chrono::NaiveDate>,
    custom_transfer_time: Option<u32>,
) -> TransitModelConfig {
    let mut config = create_valid_config();

    if let Some(osm) = custom_osm {
        config.osm_path = osm;
    }

    if let Some(gtfs) = custom_gtfs {
        config.gtfs_dirs = gtfs;
    }

    config.date = custom_date;

    if let Some(time) = custom_transfer_time {
        config.max_transfer_time = time;
    }

    config
}

fn try_create_model(config: &TransitModelConfig) -> Result<TransitModel, Error> {
    create_transit_model(config)
}

fn create_and_verify_model(config: &TransitModelConfig) -> TransitModel {
    let model_result = create_transit_model(config);
    assert!(model_result.is_ok());
    model_result.unwrap()
}

#[test]
fn test_model_creation_valid() {
    let config = create_valid_config();
    let model = create_and_verify_model(&config);

    assert_eq!(model.transit_data.stops.len(), 194, "Stop count mismatch");
    assert_eq!(model.transit_data.routes.len(), 18, "Route count mismatch");
}

#[test]
fn test_model_creation_invalid_osm() {
    let config = create_custom_config(Some(PathBuf::from("invalid_path.pbf")), None, None, None);

    let model_result = try_create_model(&config);
    assert!(model_result.is_err());
    assert!(matches!(model_result, Err(Error::IoError(_))));
}

#[test]
fn test_model_creation_invalid_gtfs() {
    let config = create_custom_config(
        None,
        Some(vec![PathBuf::from("/invalid/gtfs/dir")]),
        None,
        None,
    );

    let model_result = try_create_model(&config);
    assert!(model_result.is_err());
    assert!(matches!(model_result, Err(Error::IoError(_))));
}

#[test]
fn test_model_creation_with_date_filtering_in_calendar() {
    let config = create_custom_config(
        None,
        None,
        chrono::NaiveDate::from_ymd_opt(2024, 5, 1),
        None,
    );

    let model = create_and_verify_model(&config);
    assert_eq!(model.transit_data.stops.len(), 194);
    assert_eq!(model.transit_data.routes.len(), 17);
    assert_eq!(model.transit_data.stop_times.len(), 12235);
}

#[test]
fn test_model_creation_with_date_filtering_not_in_calendar() {
    let config = create_valid_config();
    let model = create_and_verify_model(&config);

    assert_eq!(model.transit_data.stops.len(), 194);
    assert_eq!(model.transit_data.routes.len(), 18);
    assert_eq!(model.transit_data.stop_times.len(), 34860);
}

#[test]
fn test_model_creation_with_empty_gtfs_dirs() {
    let config = create_custom_config(None, Some(vec![]), None, None);

    let model_result = try_create_model(&config);
    assert!(model_result.is_err());
}

#[test]
fn test_model_creation_fails_on_malformed_required_gtfs_row() {
    let test_data_dir = get_test_data_dir();
    let working_dir = temp_dir("malformed_gtfs");
    let gtfs_dir = working_dir.join("zhelez");

    fs::create_dir_all(&working_dir).expect("working directory should be created");
    fs::copy(
        test_data_dir.join("roads_zhelez.pbf"),
        working_dir.join("roads_zhelez.pbf"),
    )
    .expect("osm file should be copied");
    copy_dir_all(&test_data_dir.join("zhelez"), &gtfs_dir);

    let stops_path = gtfs_dir.join("stops.txt");
    let mut stops_contents = fs::read_to_string(&stops_path).expect("stops.txt should be readable");
    stops_contents.push_str("\nBROKEN,,,,not_a_float,93.0,,,,,\n");
    fs::write(&stops_path, stops_contents).expect("broken stops.txt should be written");

    let config = TransitModelConfig {
        osm_path: working_dir.join("roads_zhelez.pbf"),
        gtfs_dirs: vec![gtfs_dir],
        date: None,
        max_transfer_time: 1200,
    };

    let result = create_transit_model(&config);
    assert!(matches!(result, Err(Error::InvalidData(_))));
    if let Err(Error::InvalidData(message)) = result {
        assert!(message.contains("stops.txt"));
    }

    let _ = fs::remove_dir_all(working_dir);
}
