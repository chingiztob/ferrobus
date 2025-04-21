use ferrobus_core::{
    TransitModel, TransitModelConfig, TransitPoint, create_transit_model,
    routing::multimodal_routing::{multimodal_routing, multimodal_routing_one_to_many},
    routing::pareto::range_multimodal_routing,
};
use geo::Point;
use std::path::PathBuf;

// Helper functions
fn get_test_data_dir() -> PathBuf {
    PathBuf::from("..").join("tests").join("test-data")
}

fn create_test_model() -> TransitModel {
    let test_data_dir = get_test_data_dir();
    let config = TransitModelConfig {
        osm_path: test_data_dir.join("roads_zhelez.pbf"),
        gtfs_dirs: vec![test_data_dir.join("zhelez")],
        date: None,
        max_transfer_time: 1200,
    };

    create_transit_model(&config).expect("Failed to create test model")
}

fn create_transit_point(model: &TransitModel, lat: f64, lon: f64) -> TransitPoint {
    let point = Point::new(lon, lat);
    TransitPoint::new(point, model, 600, 10).expect("Failed to create transit point")
}

#[test]
fn test_find_route() {
    let model = create_test_model();

    let start = create_transit_point(&model, 56.256657, 93.533561);
    let end = create_transit_point(&model, 56.242574, 93.499159);

    let result = multimodal_routing(&model, &start, &end, 43200, 2)
        .expect("Routing failed")
        .expect("Must return a route");

    assert_eq!(result.travel_time, 1566);
    assert!(result.transfers <= 2);
}

#[test]
fn test_find_routes_one_to_many() {
    let model = create_test_model();

    let start = create_transit_point(&model, 56.256657, 93.533561);
    let ends = vec![
        create_transit_point(&model, 56.242574, 93.499159),
        create_transit_point(&model, 56.231878, 93.552460),
    ];

    let results = multimodal_routing_one_to_many(&model, &start, &ends, 43200, 2)
        .expect("Batch routing failed");

    assert_eq!(results.len(), 2);

    // Check individual route results based on Python test values
    assert_eq!(results[0].as_ref().unwrap().travel_time, 1524);
    assert_eq!(results[1].as_ref().unwrap().travel_time, 735);
}

#[test]
fn test_range_multimodal_routing() {
    let model = create_test_model();

    let start = create_transit_point(&model, 56.256657, 93.533561);
    let end = create_transit_point(&model, 56.242574, 93.499159);

    let departure_range = (43200, 44000);
    let result = range_multimodal_routing(&model, &start, &end, departure_range, 2)
        .expect("Range routing failed");

    assert!(!result.journeys.is_empty());
    assert!(result.median_travel_time() > 0);
    assert_eq!(result.journeys.len(), result.travel_times().len());
    assert_eq!(result.journeys.len(), result.departure_times().len());
}

#[test]
fn test_walking_route() {
    let model = create_test_model();

    // Two very close points, where walking must be the best option
    let start = create_transit_point(&model, 56.252619, 93.532134);
    let end = create_transit_point(&model, 56.252819, 93.532334);

    let result = multimodal_routing(&model, &start, &end, 43200, 0)
        .expect("Walking route failed")
        .expect("Must return a route");

    // This should be a pure walking route with no transfers
    assert_eq!(result.transfers, 0);

    assert!(
        result.travel_time < 600,
        "Walking route should be under 10 minutes"
    );
}

#[test]
fn test_routing_with_transfer_limit() {
    let model = create_test_model();

    let start = create_transit_point(&model, 56.256657, 93.533561);
    let end = create_transit_point(&model, 56.242574, 93.499159);

    let result1 = multimodal_routing(&model, &start, &end, 43200, 1)
        .expect("Routing with 1 transfer failed")
        .expect("Must return a route");
    let result2 = multimodal_routing(&model, &start, &end, 43200, 2)
        .expect("Routing with 2 transfers failed")
        .expect("Must return a route");

    assert!(result1.travel_time >= result2.travel_time);

    assert!(result1.transfers <= 1);
    assert!(result2.transfers <= 2);
}
