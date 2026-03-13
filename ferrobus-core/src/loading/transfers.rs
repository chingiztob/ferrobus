use hashbrown::HashMap;
use log::{info, warn};
use petgraph::graph::NodeIndex;
use rayon::prelude::*;

use crate::{RaptorStopId, Time, TransitModel, model::Transfer, routing::dijkstra};

/// Calculate transfers between stops using the street network
/// Merges with GTFS-defined transfers (GTFS takes priority)
pub(crate) fn calculate_transfers(graph: &mut TransitModel) {
    let max_transfer_time = graph.meta.max_transfer_time;
    let stop_count = graph.transit_data.stops.len();

    info!("Calculating transfers between {stop_count} stops");

    // Snap all transit stops to street network nodes (Some = snapped, None = too far)
    let stop_nodes = snap_stops_to_network(graph);
    // Calculate transfers for all stops that could be snapped
    let computed_transfers = calculate_stop_transfers(graph, &stop_nodes, max_transfer_time);
    // Add zero-time links for stops snapped to the same node, so routing can
    // still use co-located stops.
    let synthetic_colocated_transfers = create_colocated_stop_transfers(&stop_nodes);

    if !synthetic_colocated_transfers.is_empty() {
        let synthetic_count: usize = synthetic_colocated_transfers
            .iter()
            .map(|(_, t)| t.len())
            .sum();
        info!("Generated {synthetic_count} synthetic co-located transfers");
    }

    let gtfs_transfers_raw = std::mem::take(&mut graph.transit_data.gtfs_transfers);

    let stop_id_map: HashMap<String, RaptorStopId> = graph
        .transit_data
        .stops
        .iter()
        .enumerate()
        .map(|(i, s)| (s.stop_id.clone(), i))
        .collect();

    // Convert GTFS transfers to internal format
    let gtfs_transfers =
        convert_gtfs_transfers_to_internal(&gtfs_transfers_raw, &stop_id_map, max_transfer_time);

    if !gtfs_transfers.is_empty() {
        let gtfs_count: usize = gtfs_transfers.iter().map(|(_, t)| t.len()).sum();
        info!("Loaded {gtfs_count} GTFS-defined transfers");
    }

    // Merge order:
    // 1. computed street transfers
    // 2. synthetic co-located transfers
    // 3. GTFS transfers (highest precedence)
    let computed_with_colocated =
        merge_transfers(computed_transfers, synthetic_colocated_transfers);
    let merged_transfers = merge_transfers(computed_with_colocated, gtfs_transfers);

    update_transit_model_with_transfers(graph, merged_transfers, &stop_nodes);
}

fn convert_gtfs_transfers_to_internal(
    gtfs_transfers: &[crate::loading::FeedTransfer],
    stop_id_map: &HashMap<String, RaptorStopId>,
    max_transfer_time: Time,
) -> Vec<(RaptorStopId, Vec<Transfer>)> {
    let mut transfers_by_stop: HashMap<RaptorStopId, Vec<Transfer>> = HashMap::new();

    for transfer in gtfs_transfers {
        // "Transfers are not possible between routes at the location" link
        // https://gtfs.org/documentation/schedule/reference/#transferstxt
        if transfer.transfer_type == 3 {
            continue;
        }

        let Some(duration) = transfer.min_transfer_time else {
            continue;
        };

        if duration > max_transfer_time {
            continue;
        }

        // GTFS Stop IDs to internal indices of raptor flat model
        let Some(&from_idx) = stop_id_map.get(&transfer.from_stop_id) else {
            warn!(
                "GTFS transfer: unknown from_stop_id '{}', skipping",
                transfer.from_stop_id
            );
            continue;
        };

        let Some(&to_idx) = stop_id_map.get(&transfer.to_stop_id) else {
            warn!(
                "GTFS transfer: unknown to_stop_id '{}', skipping",
                transfer.to_stop_id
            );
            continue;
        };

        if from_idx == to_idx {
            continue;
        }

        transfers_by_stop
            .entry(from_idx)
            .or_default()
            .push(Transfer {
                target_stop: to_idx,
                duration,
            });
    }

    transfers_by_stop.into_iter().collect()
}

fn merge_transfers(
    computed: Vec<(RaptorStopId, Vec<Transfer>)>,
    gtfs: Vec<(RaptorStopId, Vec<Transfer>)>,
) -> Vec<(RaptorStopId, Vec<Transfer>)> {
    // from_stop, to_stop
    let mut merged: HashMap<RaptorStopId, HashMap<RaptorStopId, Transfer>> = HashMap::new();

    for (from_stop, transfers) in computed {
        let entry = merged.entry(from_stop).or_default();
        for transfer in transfers {
            entry.insert(transfer.target_stop, transfer);
        }
    }

    // Override with GTFS transfers
    for (from_stop, transfers) in gtfs {
        let entry = merged.entry(from_stop).or_default();
        for transfer in transfers {
            entry.insert(transfer.target_stop, transfer);
        }
    }

    merged
        .into_iter()
        .map(|(from_stop, transfers_map)| {
            let transfers: Vec<Transfer> = transfers_map.into_values().collect();
            (from_stop, transfers)
        })
        .filter(|(_, transfers)| !transfers.is_empty())
        .collect()
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

fn group_stops_by_node(stop_nodes: &[Option<NodeIndex>]) -> HashMap<NodeIndex, Vec<RaptorStopId>> {
    // Build a temporary reverse index so we can detect multiple GTFS stops snapped
    // to the same street node.
    let mut grouped: HashMap<NodeIndex, Vec<RaptorStopId>> = HashMap::new();

    for (stop_idx, node_opt) in stop_nodes.iter().enumerate() {
        if let Some(node) = node_opt {
            grouped.entry(*node).or_default().push(stop_idx);
        }
    }

    grouped
}

fn create_colocated_stop_transfers(
    stop_nodes: &[Option<NodeIndex>],
) -> Vec<(RaptorStopId, Vec<Transfer>)> {
    // `node_to_stop` keeps one canonical stop per node for fast lookup; these
    // synthetic zero-cost links preserve reachability to other co-located stops.
    let grouped = group_stops_by_node(stop_nodes);
    let mut transfers_by_stop: HashMap<RaptorStopId, Vec<Transfer>> = HashMap::new();

    for stops in grouped.into_values() {
        if stops.len() < 2 {
            continue;
        }

        for &from_stop in &stops {
            let entry = transfers_by_stop.entry(from_stop).or_default();
            entry.reserve(stops.len().saturating_sub(1));
            for &to_stop in &stops {
                if to_stop != from_stop {
                    entry.push(Transfer {
                        target_stop: to_stop,
                        duration: 0,
                    });
                }
            }
        }
    }

    transfers_by_stop.into_iter().collect()
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

#[cfg(test)]
mod tests {
    use petgraph::graph::NodeIndex;

    use super::*;

    fn to_sorted_targets(
        transfers: Vec<(RaptorStopId, Vec<Transfer>)>,
    ) -> HashMap<RaptorStopId, Vec<(RaptorStopId, Time)>> {
        let mut map: HashMap<RaptorStopId, Vec<(RaptorStopId, Time)>> = HashMap::new();
        for (from, ts) in transfers {
            let mut vals = ts
                .into_iter()
                .map(|t| (t.target_stop, t.duration))
                .collect::<Vec<_>>();
            vals.sort_unstable();
            map.insert(from, vals);
        }
        map
    }

    #[test]
    fn creates_pairwise_synthetic_transfers_for_colocated_stops() {
        let n0 = NodeIndex::new(0);
        let n1 = NodeIndex::new(1);
        let stop_nodes = vec![Some(n0), Some(n0), Some(n1), Some(n0), None];

        let grouped = create_colocated_stop_transfers(&stop_nodes);
        let as_map = to_sorted_targets(grouped);

        assert_eq!(as_map.get(&0), Some(&vec![(1, 0), (3, 0)]));
        assert_eq!(as_map.get(&1), Some(&vec![(0, 0), (3, 0)]));
        assert_eq!(as_map.get(&3), Some(&vec![(0, 0), (1, 0)]));
        assert!(!as_map.contains_key(&2));
        assert!(!as_map.contains_key(&4));
    }

    #[test]
    fn gtfs_overrides_synthetic_for_same_pair() {
        let computed = vec![(
            0,
            vec![Transfer {
                target_stop: 1,
                duration: 120,
            }],
        )];
        let synthetic = vec![(
            0,
            vec![
                Transfer {
                    target_stop: 1,
                    duration: 0,
                },
                Transfer {
                    target_stop: 2,
                    duration: 0,
                },
            ],
        )];
        let gtfs = vec![(
            0,
            vec![Transfer {
                target_stop: 1,
                duration: 42,
            }],
        )];

        let with_synthetic = merge_transfers(computed, synthetic);
        let merged = merge_transfers(with_synthetic, gtfs);
        let as_map = to_sorted_targets(merged);

        assert_eq!(as_map.get(&0), Some(&vec![(1, 42), (2, 0)]));
    }
}
