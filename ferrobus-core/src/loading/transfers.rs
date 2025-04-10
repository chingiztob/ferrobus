use std::sync::{Arc, Mutex};

use geo::Point;
use hashbrown::HashMap;
use log::info;
use petgraph::graph::NodeIndex;
use rayon::prelude::*;

use crate::{
    Error, RaptorStopId, Time, TransitModel, model::transit::types::Transfer, routing::dijkstra,
};

/// Calculate transfers between stops using the street network
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::needless_range_loop)]
pub fn calculate_transfers(graph: &mut TransitModel, max_transfer_time: Time) -> Result<(), Error> {
    // Get the stop count first before mutably borrowing transit_data
    let stop_count = graph.transit_data.stops.len();

    info!("Calculating transfers between {stop_count} stops");

    // First, snap all transit stops to the street network
    let stops_geometry: Vec<Point> = graph
        .transit_data
        .stops
        .iter()
        .map(|stop| stop.geometry)
        .collect();

    let rtree = graph.rtree_ref();
    let stop_nodes: Vec<NodeIndex> = stops_geometry
        .into_iter()
        .map(|geometry| {
            rtree
                .nearest_neighbor(&geometry)
                .map(|rtree_item| rtree_item.data)
                .ok_or(Error::NoPointsFound)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let transit_data = &mut graph.transit_data;

    let transfers = Arc::new(Mutex::new(Vec::new()));
    let transfer_indices = Arc::new(Mutex::new(HashMap::<RaptorStopId, (usize, usize)>::new()));

    // Collect stop node indices first to avoid borrow
    let stop_nodes_indices: Vec<_> = stop_nodes.clone();
    let stop_count = transit_data.stops.len();

    // For each stop, compute walking distance to all other stops in parallel
    (0..stop_count).into_par_iter().for_each(|source_idx| {
        let source_node = stop_nodes_indices[source_idx];

        // Use Dijkstra to find paths to all other stops within cutoff
        let reachable = dijkstra::dijkstra_paths(
            &graph.street_graph,
            source_node,
            None,
            Some(f64::from(max_transfer_time)),
        );

        let mut local_transfers = Vec::new();
        let mut count = 0;

        for target_idx in 0..stop_count {
            if source_idx == target_idx {
                continue; // Skip self-transfers
            }

            let target_node = stop_nodes_indices[target_idx];

            // If the target is reachable within our time limit
            if let Some(path) = reachable.get(&target_node) {
                let time = path.duration() as u32;
                let geometry = path.geometry();
                let geometry = Box::new(geometry);

                if time <= max_transfer_time {
                    local_transfers.push(Transfer {
                        target_stop: target_idx,
                        duration: time as Time,
                        geometry,
                    });
                    count += 1;
                }
            }
        }

        // Save the range of transfers for this stop
        if count > 0 {
            let start_idx;
            {
                let mut transfers_guard = transfers.lock().unwrap();
                start_idx = transfers_guard.len();
                transfers_guard.extend(local_transfers);
            }
            transfer_indices
                .lock()
                .unwrap()
                .insert(source_idx, (start_idx, count));
        }
    });

    // Update the transfer data in transit_data
    for (stop_id, (start, count)) in transfer_indices.lock().unwrap().iter() {
        transit_data.stops[*stop_id].transfers_start = *start;
        transit_data.stops[*stop_id].transfers_len = *count;
    }

    // Update the stop to node mapping
    for (stop_idx, stop_point) in stop_nodes.iter().enumerate() {
        transit_data.node_to_stop.insert(*stop_point, stop_idx);
    }

    // Update the transfers vector in transit_data
    transit_data
        .transfers
        .clone_from(&transfers.lock().unwrap());

    Ok(())
}
