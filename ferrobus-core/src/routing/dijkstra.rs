use std::{cmp::Ordering, collections::BinaryHeap};

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
/// Returns a map of node indices to walking times in seconds
pub fn dijkstra_path_weights(
    graph: &StreetGraph,
    start: NodeIndex,
    target: Option<NodeIndex>,
    max_cost: Option<f64>,
) -> HashMap<NodeIndex, u32> {
    let mut distances: HashMap<NodeIndex, u32> = HashMap::new();
    let mut heap = BinaryHeap::new();

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
                continue;
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
                }
                hashbrown::hash_map::Entry::Occupied(mut entry) => {
                    if next_cost < *entry.get() {
                        *entry.get_mut() = next_cost;
                        heap.push(State {
                            cost: next_cost,
                            node: next,
                        });
                    }
                }
            }
        }
    }

    distances
}
