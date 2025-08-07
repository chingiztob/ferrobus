use std::collections::BinaryHeap;

use hashbrown::HashMap;
use petgraph::{graph::NodeIndex, visit::EdgeRef};

use super::state::State;
use crate::model::StreetGraph;

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

    heap.push(State {
        cost: 0,
        node: start,
    });
    distances.insert(start, 0);

    while let Some(State { cost, node }) = heap.pop() {
        if let Some(target_node) = target
            && node == target_node
        {
            break;
        }

        if let Some(&best) = distances.get(&node)
            && cost > best
        {
            continue;
        }

        if let Some(max) = max_cost
            && f64::from(cost) > max
        {
            continue;
        }

        for edge in graph.edges(node) {
            let next = edge.target();
            let walking_time = edge.weight().weight;
            let next_cost = cost + walking_time;

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
