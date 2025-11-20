use geo::{ConvexHull, Intersects, MultiPoint};
use log::info;

use super::config::TransitModelConfig;
use super::gtfs::transit_model_from_gtfs;
use super::osm::create_street_graph;
use super::transfers::calculate_transfers;
use crate::{Error, PublicTransitData, TransitModel, model::StreetGraph};

/// Creates a transit model based on the provided configuration
///
/// # Errors
///
/// Returns an error if there are problems reading or processing data
pub fn create_transit_model(config: &TransitModelConfig) -> Result<TransitModel, Error> {
    validate_config(config)?;

    info!(
        "Processing street data (OSM): {}",
        config.osm_path.display()
    );

    // Start OSM data processing in a separate thread
    let osm_path = config.osm_path.clone();
    let graph_handle = std::thread::spawn(move || create_street_graph(osm_path));

    info!("Processing public transit data (GTFS)");
    let transit_data = transit_model_from_gtfs(config)?;

    let street_graph = graph_handle
        .join()
        .map_err(|_| Error::UnrecoverableError("OSM processing thread panicked"))??;

    validate_graph_transit_overlap(&street_graph, &transit_data);

    let mut graph = TransitModel::with_transit(
        street_graph,
        transit_data,
        crate::model::TransitModelMeta {
            max_transfer_time: config.max_transfer_time,
        },
    );

    calculate_transfers(&mut graph);
    info!(
        "Calculated {} transfers between stops",
        &graph.transit_data.transfers.len()
    );

    info!("Transit model created successfully");
    // While processing OSM protobuf data, and during CSV deserialization
    // large amounts of memory are allocated. This memory is not always
    // released back to the system. This call will release all free memory
    // from the tail of the heap back to the system.
    //
    // # Safety
    //
    // This call is safe to use on linux with glibc implementation
    // which is checked by the cfg attribute in compile time.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        if libc::malloc_trim(0) == 0 {
            log::warn!("Memory trimming failed - continuing anyway");
        } else {
            log::debug!("Successfully trimmed unused heap memory");
        }
    }
    Ok(graph)
}

fn validate_config(config: &TransitModelConfig) -> Result<(), Error> {
    if !config.osm_path.exists() {
        return Err(Error::InvalidData(format!(
            "OSM file not found: {}",
            config.osm_path.display()
        )));
    }

    if config.gtfs_dirs.is_empty() {
        return Err(Error::InvalidData(
            "No GTFS directories provided in the configuration".to_string(),
        ));
    }

    for dir in &config.gtfs_dirs {
        if !dir.exists() {
            return Err(Error::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("GTFS directory not found: {}", dir.display()),
            )));
        }
    }

    Ok(())
}

#[allow(clippy::cast_precision_loss)]
fn validate_graph_transit_overlap(streets: &StreetGraph, transit: &PublicTransitData) {
    let graph_nodes: MultiPoint = streets
        .graph
        .node_weights()
        .map(|node| node.geometry)
        .collect();
    let graph_hull = graph_nodes.convex_hull();

    let stops_outside_hull = transit
        .stops
        .iter()
        .filter(|stop| !stop.geometry.intersects(&graph_hull))
        .count();

    let total_stops = transit.stops.len();

    let percentage = (stops_outside_hull as f64 / total_stops as f64) * 100.0;
    if stops_outside_hull > 0 {
        log::warn!(
            "{stops_outside_hull} of {total_stops} transit stops ({percentage:.1}%) are outside \
        the street network coverage area. These stops may be unreachable for routing. \
        Consider using a larger OSM dataset that covers all transit stops."
        );
    }
}
