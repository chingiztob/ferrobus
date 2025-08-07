use geo::Point;
use hashbrown::HashMap;
use log::info;
use rayon::prelude::*;

use crate::{Error, RaptorStopId, Time, TransitModel, model::Transfer, routing::dijkstra};

/// Calculate transfers between stops using the street network
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::needless_range_loop)]
pub(crate) fn calculate_transfers(graph: &mut TransitModel) -> Result<(), Error> {
    // Structure for each stop's result
    type StopResult = (RaptorStopId, Vec<Transfer>);

    let max_transfer_time = graph.meta.max_transfer_time;

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
    let stop_nodes = stops_geometry
        .iter()
        .map(|geometry| {
            rtree
                .nearest_neighbor(geometry)
                .map(|n| n.data)
                .ok_or(Error::NoPointsFound)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let transit_data = &mut graph.transit_data;

    // Collect stop node indices first to avoid borrow
    let stop_nodes_indices: Vec<_> = stop_nodes.clone();
    let stop_count = transit_data.stops.len();

    // For each stop, compute walking distance to all other stops in parallel
    let all_results: Vec<StopResult> = (0..stop_count)
        .into_par_iter()
        .filter_map(|source_idx| {
            let source_node = stop_nodes_indices[source_idx];

            // find paths to all other stops within cutoff
            let reachable = dijkstra::dijkstra_path_weights(
                &graph.street_graph,
                source_node,
                None,
                Some(f64::from(max_transfer_time)),
            );

            let mut local_transfers = Vec::new();

            for target_idx in 0..stop_count {
                if source_idx == target_idx {
                    continue; // Skip self-transfers
                }

                let target_node = stop_nodes_indices[target_idx];

                // If the target is reachable within our time limit
                if let Some(&time) = reachable.get(&target_node)
                    && time <= max_transfer_time
                {
                    local_transfers.push(Transfer {
                        target_stop: target_idx,
                        duration: time as Time,
                    });
                }
            }

            // Only return results if there are transfers
            if local_transfers.is_empty() {
                None
            } else {
                Some((source_idx, local_transfers))
            }
        })
        .collect();

    // Merge the results
    let mut transfers = Vec::with_capacity(all_results.iter().map(|(_, t)| t.len()).sum());
    let mut transfer_indices =
        HashMap::<RaptorStopId, (usize, usize)>::with_capacity(all_results.len());

    for (source_idx, local_transfers) in all_results {
        let count = local_transfers.len();
        let start_idx = transfers.len();
        transfers.extend(local_transfers);
        transfer_indices.insert(source_idx, (start_idx, count));
    }

    for (stop_id, (start, count)) in &transfer_indices {
        transit_data.stops[*stop_id].transfers_start = *start;
        transit_data.stops[*stop_id].transfers_len = *count;
    }

    for (stop_idx, stop_point) in stop_nodes.iter().enumerate() {
        transit_data.node_to_stop.insert(*stop_point, stop_idx);
    }

    transit_data.transfers.clone_from(&transfers);

    Ok(())
}
