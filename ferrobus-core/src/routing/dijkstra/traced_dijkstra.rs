use std::{cmp::Ordering, collections::BinaryHeap};

use geo::{Coord, LineString};
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
pub fn dijkstra_paths(
    graph: &StreetGraph,
    start: NodeIndex,
    target: Option<NodeIndex>,
    max_cost: Option<f64>,
) -> HashMap<NodeIndex, WalkingPath> {
    let mut distances: HashMap<NodeIndex, u32> = HashMap::new();
    let mut heap = BinaryHeap::new();

    // Start node has distance 0
    heap.push(State {
        cost: 0,
        node: start,
    });
    distances.insert(start, 0);
    let mut predecessors: HashMap<NodeIndex, (NodeIndex, Segment)> = HashMap::new();

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
            let geometry = &edge.weight().geometry;
            let next_cost = cost + walking_time;

            let segment = Segment {
                weight: f64::from(walking_time),
                geometry,
            };

            // Add or update distance if better using Entry API
            match distances.entry(next) {
                hashbrown::hash_map::Entry::Vacant(entry) => {
                    entry.insert(next_cost);
                    heap.push(State {
                        cost: next_cost,
                        node: next,
                    });
                    predecessors.insert(next, (node, segment));
                }
                hashbrown::hash_map::Entry::Occupied(mut entry) => {
                    if next_cost < *entry.get() {
                        *entry.get_mut() = next_cost;
                        heap.push(State {
                            cost: next_cost,
                            node: next,
                        });
                        predecessors.insert(next, (node, segment));
                    }
                }
            }
        }
    }

    let mut paths = HashMap::with_capacity(distances.len());
    for target in distances.keys() {
        let mut current_node = target;
        let mut itinerary = WalkingPath::new();

        while let Some((prev_node, segment)) = predecessors.get(current_node) {
            itinerary.push(segment.clone());
            current_node = prev_node;
        }

        itinerary.segments.reverse();
        paths.insert(*target, itinerary);
    }

    paths
}

#[derive(Debug, Clone)]
pub struct Segment<'a> {
    pub(crate) weight: f64,
    pub(crate) geometry: &'a LineString,
}

#[derive(Debug, Clone)]
pub struct WalkingPath<'a> {
    pub segments: Vec<Segment<'a>>,
}

impl<'a> WalkingPath<'a> {
    pub(crate) fn new() -> WalkingPath<'a> {
        WalkingPath {
            segments: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, segment: Segment<'a>) {
        self.segments.push(segment);
    }

    pub fn duration(&self) -> f64 {
        self.segments.iter().map(|s| s.weight).sum()
    }

    pub(crate) fn geometry(&self) -> LineString<f64> {
        // Create an empty vector to store coordinates from each segment's geometry.
        let mut coords: Vec<Coord<f64>> = Vec::new();

        for segment in &self.segments {
            let ls = segment.geometry;

            // Extract coordinates from the segment's geometry.
            // In geo, a LineString is often a wrapper around Vec<Coordinate<T>>.
            // Here we clone the underlying coordinates for our use.
            let seg_coords = ls.0.clone();

            // If this is the first segment with geometry, simply use all of its points.
            if coords.is_empty() {
                coords = seg_coords;
            } else {
                // For subsequent segments, check if the first coordinate of the current
                // segment is the same as the last coordinate of the built-up path.
                // If so, skip the duplicate coordinate to ensure a smooth continuous LineString.
                if let Some(first_seg_coord) = seg_coords.first() {
                    if first_seg_coord == coords.last().unwrap() {
                        // Extend the coordinate vector, skipping the duplicate.
                        coords.extend_from_slice(&seg_coords[1..]);
                    } else {
                        // No shared coordinate; simply add all coordinates.
                        coords.extend(seg_coords);
                    }
                }
            }
        }

        // Construct and return the combined LineString from our coordinate vector.
        LineString(coords)
    }
}
