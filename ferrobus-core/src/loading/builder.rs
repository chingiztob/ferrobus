use log::info;
use std::sync::mpsc::{self, RecvError};

use super::config::TransitModelConfig;
use super::gtfs::transit_model_from_gtfs;
use super::osm::create_street_graph;
use super::transfers::calculate_transfers;
use crate::{Error, TransitModel};

/// Creates a transit model based on the provided configuration
///
/// # Errors
///
/// Returns an error if there are problems reading or processing data
pub fn create_transit_model(config: &TransitModelConfig) -> Result<TransitModel, Error> {
    info!(
        "Processing street data (OSM): {}",
        config.osm_path.display()
    );

    if config.gtfs_dirs.is_empty() {
        return Err(Error::InvalidData(
            "No GTFS directories provided in the configuration".to_string(),
        ));
    }

    let (street_sender, street_receiver) = mpsc::channel();

    // Start OSM data processing in a separate thread
    let osm_path = config.osm_path.clone();
    let graph_handle = std::thread::spawn(move || {
        let result = create_street_graph(osm_path);
        let _ = street_sender.send(result);
    });

    info!("Processing public transit data (GTFS)");
    let transit_data = transit_model_from_gtfs(config)?;

    let street_graph = match street_receiver.recv() {
        Ok(Ok(graph)) => graph,
        Ok(Err(error)) => return Err(error),
        Err(RecvError) => {
            return Err(Error::UnrecoverableError(
                "Unrecoverable error while processing OSM data",
            ));
        }
    };

    // Wait for the thread to finish
    let _ = graph_handle.join();

    let mut graph = TransitModel::with_transit(
        street_graph,
        transit_data,
        crate::model::TransitModelMeta {
            max_transfer_time: config.max_transfer_time,
        },
    );

    calculate_transfers(&mut graph)?;
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
            log::error!("Memory trimming failed");
        }
    };
    Ok(graph)
}
