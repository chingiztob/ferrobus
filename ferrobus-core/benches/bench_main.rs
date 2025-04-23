use std::path::PathBuf;

use divan::{Bencher, bench};
use ferrobus_core::{TransitModel, model::TransitPoint, multimodal_routing};

static TRANSIT_DATA: std::sync::LazyLock<(TransitModel, TransitPoint, TransitPoint, u32, usize)> =
    std::sync::LazyLock::new(|| {
        let config = ferrobus_core::TransitModelConfig {
            max_transfer_time: 1200, // 20 minutes max transfer time
            osm_path: PathBuf::from("/home/chingiz/Rust/osm/roads_SZ.pbf"),
            gtfs_dirs: vec![
                PathBuf::from("/home/chingiz/Rust/py_rust/cascade/scripts/files/SPB2"),
                PathBuf::from("/home/chingiz/Rust/py_rust/cascade/scripts/files/spb-metro"),
            ],
            date: chrono::NaiveDate::from_ymd_opt(2025, 4, 10),
        };

        let transit_graph = ferrobus_core::create_transit_model(&config).unwrap();

        let departure_time = 43500;
        let max_transfers = 4;
        let max_walking_time = 1200;

        let start_point = TransitPoint::new(
            geo::Point::new(30.397364, 60.013049),
            &transit_graph,
            max_walking_time,
            10,
        )
        .unwrap();

        let end_point = TransitPoint::new(
            geo::Point::new(30.268505, 59.887109),
            &transit_graph,
            max_walking_time,
            10,
        )
        .unwrap();

        (
            transit_graph,
            start_point,
            end_point,
            departure_time,
            max_transfers,
        )
    });

#[bench(sample_count = 1000)]
fn raptor_routing(bencher: Bencher) {
    let (transit_graph, start_point, end_point, departure_time, max_transfers) = &*TRANSIT_DATA;

    bencher.bench(|| {
        let _ = multimodal_routing(
            transit_graph,
            start_point,
            end_point,
            *departure_time,
            *max_transfers,
        )
        .unwrap();
    });
}

fn main() {
    divan::main();
}
