use hashbrown::HashMap;
use log::info;
use petgraph::graph::NodeIndex;
use rayon::prelude::*;

use crate::{RaptorStopId, Time, TransitModel, model::Transfer, routing::dijkstra};

/// Calculate transfers between stops using the street network
pub(crate) fn calculate_transfers(graph: &mut TransitModel) {
    let max_transfer_time = graph.meta.max_transfer_time;
    let stop_count = graph.transit_data.stops.len();

    info!("Calculating transfers between {stop_count} stops");

    // Snap all transit stops to street network nodes (Some = snapped, None = too far)
    let stop_nodes = snap_stops_to_network(graph);
    // Calculate transfers for all stops that could be snapped
    let stop_transfers = calculate_stop_transfers(graph, &stop_nodes, max_transfer_time);

    update_transit_model_with_transfers(graph, stop_transfers, &stop_nodes);
}

/// Snap transit stops to their nearest street network nodes
/// Returns None for stops that are too far from any street (> max_transfer_time walking distance)
fn snap_stops_to_network(graph: &TransitModel) -> Vec<Option<NodeIndex>> {
    let max_snap_distance = graph.meta.max_transfer_time;

    graph
        .transit_data
        .stops
        .iter()
        .map(|stop| {
            if let Some((node, walking_time)) = graph.street_graph.nearest_node(&stop.geometry) {
                if walking_time <= max_snap_distance {
                    Some(node)
                } else {
                    log::trace!(
                        "Stop at {:?} is {}s walk from nearest street (max: {}s) - excluding from transfers",
                        stop.geometry, walking_time, max_snap_distance
                    );
                    None
                }
            } else {
                log::trace!("Stop at {:?} has no nearby streets - excluding from transfers", stop.geometry);
                None
            }
        })
        .collect()
}

/// Calculate transfers for all stops using parallel processing
fn calculate_stop_transfers(
    graph: &TransitModel,
    stop_nodes: &[Option<NodeIndex>],
    max_transfer_time: Time,
) -> Vec<(RaptorStopId, Vec<Transfer>)> {
    (0..stop_nodes.len())
        .into_par_iter()
        .filter_map(|source_idx| {
            // Skip stops that couldn't be snapped to streets
            let source_node = stop_nodes[source_idx]?;

            let transfers = find_transfers_from_stop(
                graph,
                stop_nodes,
                source_idx,
                source_node,
                max_transfer_time,
            );

            if transfers.is_empty() {
                None
            } else {
                Some((source_idx, transfers))
            }
        })
        .collect()
}

/// Find all valid transfers from a single stop
fn find_transfers_from_stop(
    graph: &TransitModel,
    stop_nodes: &[Option<NodeIndex>],
    source_idx: usize,
    source_node: NodeIndex,
    max_transfer_time: Time,
) -> Vec<Transfer> {
    // Get reachable nodes within time limit
    let reachable = dijkstra::dijkstra_path_weights(
        &graph.street_graph,
        source_node,
        None,
        Some(f64::from(max_transfer_time)),
    );

    stop_nodes
        .iter()
        .enumerate()
        .filter_map(|(target_idx, target_node_opt)| {
            // Skip self-transfers
            if source_idx == target_idx {
                return None;
            }

            // Skip stops that couldn't be snapped to streets
            let target_node = (*target_node_opt)?;

            // Check if target is reachable within time limit
            reachable
                .get(&target_node)
                .filter(|&&time| time <= max_transfer_time)
                .map(|&time| Transfer {
                    target_stop: target_idx,
                    duration: time,
                })
        })
        .collect()
}

/// Update the transit model with calculated transfers
fn update_transit_model_with_transfers(
    graph: &mut TransitModel,
    stop_transfers: Vec<(RaptorStopId, Vec<Transfer>)>,
    stop_nodes: &[Option<NodeIndex>],
) {
    // Flatten transfers and build index
    let mut all_transfers = Vec::new();
    let mut transfer_indices = HashMap::new();

    for (stop_id, transfers) in stop_transfers {
        let start_idx = all_transfers.len();
        let count = transfers.len();

        all_transfers.extend(transfers);
        transfer_indices.insert(stop_id, (start_idx, count));
    }

    // Update stop transfer indices
    for (stop_id, (start, count)) in transfer_indices {
        let stop = &mut graph.transit_data.stops[stop_id];
        stop.transfers_start = start;
        stop.transfers_len = count;
    }

    // Update node-to-stop mapping (only for stops that were successfully snapped)
    for (stop_idx, node_opt) in stop_nodes.iter().enumerate() {
        if let Some(node) = node_opt {
            graph.transit_data.node_to_stop.insert(*node, stop_idx);
        }
    }

    // Store all transfers
    graph.transit_data.transfers = all_transfers;
}
