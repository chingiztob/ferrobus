use std::{env, path::PathBuf};

use criterion::{Criterion, criterion_group, criterion_main};
use dotenvy::dotenv;
use ferrobus_core::{TransitModel, model::TransitPoint, multimodal_routing};

static TRANSIT_DATA: std::sync::LazyLock<(TransitModel, TransitPoint, TransitPoint, u32, usize)> =
    std::sync::LazyLock::new(|| {
        dotenv().ok();

        let config = ferrobus_core::TransitModelConfig {
            max_transfer_time: 1200, // 20 minutes max transfer time
            osm_path: required_env_path("FERROBUS_BENCH_OSM_PATH"),
            gtfs_dirs: required_env_paths("FERROBUS_BENCH_GTFS_DIRS"),
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

fn required_env_path(name: &str) -> PathBuf {
    PathBuf::from(env::var(name).unwrap_or_else(|_| panic!("missing required env var: {name}")))
}

fn required_env_paths(name: &str) -> Vec<PathBuf> {
    let paths: Vec<_> = env::var(name)
        .unwrap_or_else(|_| panic!("missing required env var: {name}"))
        .split(':')
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .collect();

    assert!(!paths.is_empty(), "env var {name} must not be empty");
    paths
}

fn raptor_routing(c: &mut Criterion) {
    let (transit_graph, start_point, end_point, departure_time, max_transfers) = &*TRANSIT_DATA;

    c.bench_function("raptor_routing", |b| {
        b.iter(|| {
            let _ = multimodal_routing(
                transit_graph,
                start_point,
                end_point,
                *departure_time,
                *max_transfers,
            )
            .unwrap();
        });
    });
}

criterion_group!(benches, raptor_routing);
criterion_main!(benches);
