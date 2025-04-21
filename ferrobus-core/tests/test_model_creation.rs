use ferrobus_core::{Error, TransitModel, TransitModelConfig, create_transit_model};
use std::path::PathBuf;

fn get_test_data_dir() -> PathBuf {
    PathBuf::from("..").join("tests").join("test-data")
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
    assert!(matches!(model_result, Err(Error::InvalidData(_))));
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
