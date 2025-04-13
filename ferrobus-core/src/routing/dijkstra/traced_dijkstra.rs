use std::{cmp::Ordering, collections::BinaryHeap};

use geo::Coord;
use hashbrown::HashMap;
use petgraph::{graph::NodeIndex, visit::EdgeRef};

use crate::model::StreetGraph;

#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: u32,
    node: NodeIndex,
}

// Implement Ord for State to use in BinaryHeap
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap by cost (reversed from standard Rust BinaryHeap)
        other.cost.cmp(&self.cost)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Dijkstra's algorithm for finding shortest paths in the walking network
/// Returns a map of node indices to walking paths
pub(crate) fn dijkstra_paths(
    graph: &StreetGraph,
    start: NodeIndex,
    target: Option<NodeIndex>,
    max_cost: Option<f64>,
) -> HashMap<NodeIndex, WalkingPath> {
    // Estimate capacity based on graph size (adjust as needed)
    let estimated_nodes = graph.graph.node_count().min(1000);
    let mut distances: HashMap<NodeIndex, u32> = HashMap::with_capacity(estimated_nodes);
    let mut predecessors: HashMap<NodeIndex, NodeIndex> = HashMap::with_capacity(estimated_nodes);
    let mut heap = BinaryHeap::with_capacity(estimated_nodes / 4);

    // Start node has distance 0
    heap.push(State {
        cost: 0,
        node: start,
    });
    distances.insert(start, 0);

    while let Some(State { cost, node }) = heap.pop() {
        // Check if we've reached the target
        if let Some(target_node) = target {
            if node == target_node {
                break;
            }
        }

        // Skip if we've found a better path
        if let Some(&best) = distances.get(&node) {
            if cost > best {
                continue;
            }
        }

        // Check max cost constraint
        if let Some(max) = max_cost {
            if f64::from(cost) > max {
                break;
            }
        }

        // Examine neighbors
        for edge in graph.edges(node) {
            let next = edge.target();
            let walking_time = edge.weight().weight;
            let next_cost = cost + walking_time;

            // Add or update distance if better using Entry API
            match distances.entry(next) {
                hashbrown::hash_map::Entry::Vacant(entry) => {
                    entry.insert(next_cost);
                    heap.push(State {
                        cost: next_cost,
                        node: next,
                    });
                    predecessors.insert(next, node);
                }
                hashbrown::hash_map::Entry::Occupied(mut entry) => {
                    if next_cost < *entry.get() {
                        *entry.get_mut() = next_cost;
                        heap.push(State {
                            cost: next_cost,
                            node: next,
                        });
                        predecessors.insert(next, node);
                    }
                }
            }
        }
    }

    let mut paths = HashMap::with_capacity(distances.len());

    // Construct paths for all reached nodes
    for &target_node in distances.keys() {
        // Only create path if we can reach the target (or it's the start)
        if predecessors.contains_key(&target_node) || target_node == start {
            // Estimate path length for capacity pre-allocation
            let mut path_len = 1;
            let mut current = target_node;
            while current != start {
                if let Some(&prev) = predecessors.get(&current) {
                    path_len += 1;
                    current = prev;
                } else {
                    break;
                }
            }

            // Pre-allocate vectors with exact capacity needed
            let mut node_path = Vec::with_capacity(path_len);

            // Follow predecessors backward from target to start
            current = target_node;
            while current != start {
                node_path.push(current);
                if let Some(&prev) = predecessors.get(&current) {
                    current = prev;
                } else {
                    break;
                }
            }
            node_path.push(start);
            node_path.reverse(); // Now path is from start to target

            // Create path coords directly - we know exact capacity
            let mut path_coords = Vec::with_capacity(node_path.len() + 2);
            // Placeholder for transfer source stop, which can be set only after whole itinerary is known
            path_coords.push(Coord {
                x: f64::NAN,
                y: f64::NAN,
            });

            // Collect all nodes with their positions in one pass
            for &node_idx in &node_path {
                if let Some(node_weight) = graph.graph.node_weight(node_idx) {
                    path_coords.push(node_weight.geometry.into());
                }
            }
            // Placeholder for transfer target stop, which can be set only after whole itinerary is known
            path_coords.push(Coord {
                x: f64::NAN,
                y: f64::NAN,
            });

            // We already know total cost from distances map
            let walking_path = WalkingPath { nodes: path_coords };

            paths.insert(target_node, walking_path);
        }
    }

    paths
}

#[derive(Debug, Clone)]
pub struct WalkingPath {
    nodes: Vec<Coord<f64>>,
}

impl WalkingPath {
    pub(crate) fn into_nodes(self) -> Vec<Coord<f64>> {
        self.nodes
    }

    pub(crate) fn nodes(&self) -> &[Coord<f64>] {
        &self.nodes
    }
}
