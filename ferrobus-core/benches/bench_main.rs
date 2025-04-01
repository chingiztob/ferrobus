use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use ferrobus_core::{TransitModel, model::TransitPoint, multimodal_routing};

fn raptor_route(
    transit_graph: &TransitModel,
    start_point: &TransitPoint,
    end_point: &TransitPoint,
    departure_time: u32,
    max_transfers: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = multimodal_routing(
        transit_graph,
        start_point,
        end_point,
        departure_time,
        max_transfers,
    )?;

    Ok(())
}

fn criterion_benchmark(c: &mut Criterion) {
    use ferrobus_core::{TransitModelConfig, create_transit_model, model::TransitPoint};

    let config = TransitModelConfig {
        max_transfer_time: 1200, // 20 minutes max transfer time
        osm_path: PathBuf::from("/home/chingiz/Rust/osm/roads_SZ.pbf"),
        gtfs_dirs: vec![
            PathBuf::from("/home/chingiz/Rust/py_rust/cascade/scripts/files/SPB"),
            PathBuf::from("/home/chingiz/Rust/py_rust/cascade/scripts/files/spb-metro"),
        ],
        date: chrono::NaiveDate::from_ymd_opt(2025, 4, 10)
    };

    let transit_graph = create_transit_model(&config).unwrap();

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

    c.bench_function("raptor routing", |b| {
        b.iter(|| {
            raptor_route(
                &transit_graph,
                &start_point,
                &end_point,
                departure_time,
                max_transfers,
            )
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
