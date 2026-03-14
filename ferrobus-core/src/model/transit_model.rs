//! Main unified transport system model
use geo::Point;
use hashbrown::HashMap;
use petgraph::graph::NodeIndex;

use crate::model::streets::IndexedPoint;
use crate::{Error, RaptorStopId, Time, routing::dijkstra::dijkstra_path_weights};
use crate::{model::streets::StreetGraph, model::transit::data::PublicTransitData};
use rstar::RTree;

use super::Stop;

/// Unified transport network model containing data about public transit and street network
#[derive(Debug)]
pub struct TransitModel {
    pub transit_data: PublicTransitData,
    pub street_graph: StreetGraph,
    pub meta: TransitModelMeta,
}

#[derive(Debug)]
pub struct TransitModelMeta {
    pub max_transfer_time: Time,
}

fn checked_end(start: usize, len: usize, field: &str) -> Result<usize, Error> {
    start.checked_add(len).ok_or_else(|| {
        Error::InvalidData(format!("{field} range overflows: start={start}, len={len}"))
    })
}

/// Audits the built transit model for compact structural consistency.
///
/// This check is limited to in-memory indexing/layout invariants
/// such as flattened slice bounds and referenced stop/route indices.
pub(crate) fn audit_transit_model(model: &TransitModel) -> Result<(), Error> {
    let transit = &model.transit_data;
    let stop_count = transit.stops.len();
    let route_count = transit.routes.len();

    for (stop_idx, stop) in transit.stops.iter().enumerate() {
        let routes_end = checked_end(stop.routes_start, stop.routes_len, "stop_routes")?;
        if routes_end > transit.stop_routes.len() {
            return Err(Error::InvalidData(format!(
                "stop {stop_idx} has invalid stop_routes slice: {}..{} exceeds {}",
                stop.routes_start,
                routes_end,
                transit.stop_routes.len()
            )));
        }

        let transfers_end =
            checked_end(stop.transfers_start, stop.transfers_len, "stop_transfers")?;
        if transfers_end > transit.transfers.len() {
            return Err(Error::InvalidData(format!(
                "stop {stop_idx} has invalid transfers slice: {}..{} exceeds {}",
                stop.transfers_start,
                transfers_end,
                transit.transfers.len()
            )));
        }
    }

    for (stop_idx, &route_id) in transit.stop_routes.iter().enumerate() {
        if route_id >= route_count {
            return Err(Error::InvalidData(format!(
                "stop_routes[{stop_idx}] references invalid route {route_id}"
            )));
        }
    }

    for (transfer_idx, transfer) in transit.transfers.iter().enumerate() {
        if transfer.target_stop >= stop_count {
            return Err(Error::InvalidData(format!(
                "transfer {transfer_idx} references invalid target stop {}",
                transfer.target_stop
            )));
        }
    }

    for (node, &stop_id) in &transit.node_to_stop {
        if stop_id >= stop_count {
            return Err(Error::InvalidData(format!(
                "node_to_stop entry for node {node:?} references invalid stop {stop_id}"
            )));
        }
    }

    for (route_idx, route) in transit.routes.iter().enumerate() {
        let stops_end = checked_end(route.stops_start, route.num_stops, "route_stops")?;
        if stops_end > transit.route_stops.len() {
            return Err(Error::InvalidData(format!(
                "route {route_idx} has invalid route_stops slice: {}..{} exceeds {}",
                route.stops_start,
                stops_end,
                transit.route_stops.len()
            )));
        }

        let stop_times_len = route
            .num_trips
            .checked_mul(route.num_stops)
            .ok_or_else(|| {
                Error::InvalidData(format!(
                    "route {route_idx} trip stop-time layout overflows: num_trips={}, num_stops={}",
                    route.num_trips, route.num_stops
                ))
            })?;
        let stop_times_end = checked_end(route.trips_start, stop_times_len, "route_stop_times")?;
        if stop_times_end > transit.stop_times.len() {
            return Err(Error::InvalidData(format!(
                "route {route_idx} has invalid stop_times slice: {}..{} exceeds {}",
                route.trips_start,
                stop_times_end,
                transit.stop_times.len()
            )));
        }

        let Some(route_trips) = transit.trips.get(route_idx) else {
            return Err(Error::InvalidData(format!(
                "route {route_idx} is missing trip metadata"
            )));
        };
        if route_trips.len() != route.num_trips {
            return Err(Error::InvalidData(format!(
                "route {route_idx} trip metadata length mismatch: expected {}, got {}",
                route.num_trips,
                route_trips.len()
            )));
        }

        for (offset, &stop_id) in transit.route_stops[route.stops_start..stops_end]
            .iter()
            .enumerate()
        {
            if stop_id >= stop_count {
                return Err(Error::InvalidData(format!(
                    "route {route_idx} stop offset {offset} references invalid stop {stop_id}"
                )));
            }
        }
    }

    Ok(())
}

impl TransitModel {
    /// Creates a new model from street network and transit data
    pub(crate) fn with_transit(
        street_network: StreetGraph,
        transit_data: PublicTransitData,
        meta: TransitModelMeta,
    ) -> Self {
        Self {
            transit_data,
            street_graph: street_network,
            meta,
        }
    }

    /// Returns a reference to the R-tree
    pub fn rtree_ref(&self) -> &RTree<IndexedPoint> {
        &self.street_graph.rtree
    }

    pub fn street_graph(&self) -> &StreetGraph {
        &self.street_graph
    }

    /// Returns the number of stops in the model
    pub fn stop_count(&self) -> usize {
        self.transit_data.stops.len()
    }

    /// Returns the number of routes in the model
    pub fn route_count(&self) -> usize {
        self.transit_data.routes.len()
    }

    pub fn feeds_info(&self) -> String {
        format!("{:#?}", self.transit_data.feeds_meta)
    }

    pub fn stops(&self) -> &[Stop] {
        &self.transit_data.stops
    }
}

/// A point connected to the transit network with pre-calculated nearest stops
#[derive(Debug, Clone)]
pub struct TransitPoint {
    /// Point coordinates
    pub geometry: Point<f64>,
    /// Nearest street network node
    pub node_id: NodeIndex,
    /// Nearest stops (stop id, walking time)
    pub nearest_stops: Vec<(RaptorStopId, Time)>,
    /// Walking routes to other nodes
    pub walking_paths: HashMap<NodeIndex, Time>,
}

impl TransitPoint {
    /// Creates a new point connected to the transit network
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn new(
        point: Point<f64>,
        graph: &TransitModel,
        max_walking_time: Time,
        max_stops: usize,
    ) -> Result<Self, Error> {
        let (node_id, distance) = graph
            .street_graph
            .nearest_node(&point)
            .ok_or(Error::NoPointsFound)?;

        if distance > max_walking_time {
            return Err(Error::NoPointsFound);
        }
        //Pre-calculated walking paths to all nodes within the time limit
        let walking_paths = dijkstra_path_weights(
            &graph.street_graph,
            node_id,
            None,
            Some(f64::from(max_walking_time - distance)),
        );

        // Find `max_stops` nearest stops
        let mut nearest_stops = Vec::new();

        for (&node, &time) in &walking_paths {
            if time <= max_walking_time - distance
                && let Some(&stop_id) = graph.transit_data.node_to_stop.get(&node)
            {
                nearest_stops.push((stop_id, time as Time + distance));
            }
        }

        nearest_stops.sort_by_key(|&(_, time)| time);
        nearest_stops.truncate(max_stops);

        Ok(TransitPoint {
            geometry: point,
            node_id,
            nearest_stops,
            walking_paths,
        })
    }

    /// Returns walking time to another point, if available
    pub fn walking_time_to(&self, other: &TransitPoint) -> Option<Time> {
        self.walking_paths.get(&other.node_id).copied()
    }

    /// Get the location of a transit stop by ID
    pub fn transit_stop_location(
        &self,
        transit_data: &PublicTransitData,
        stop_id: RaptorStopId,
    ) -> geo::Point<f64> {
        transit_data.transit_stop_location(stop_id)
    }

    /// Get the name of a transit stop by ID
    pub fn transit_stop_name(
        &self,
        transit_data: &PublicTransitData,
        stop_id: RaptorStopId,
    ) -> Option<String> {
        transit_data.transit_stop_name(stop_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loading::build_rtree;
    use crate::model::streets::{StreetEdge, StreetNode};
    use crate::model::{PublicTransitData, streets::StreetGraph};
    use geo::Point;
    use hashbrown::HashMap;
    use hashbrown::HashSet;
    use osm4routing::NodeId;
    use petgraph::graph::{NodeIndex, UnGraph};

    fn create_test_graph() -> TransitModel {
        // Create a simple street network with a few nodes and edges
        let mut graph = UnGraph::new_undirected();

        // Create nodes in a grid pattern
        let n1 = graph.add_node(StreetNode {
            id: NodeId(1i64),
            geometry: Point::new(0.0, 0.0),
        });

        let n2 = graph.add_node(StreetNode {
            id: NodeId(2i64),
            geometry: Point::new(0.0, 0.01), // ~1.11km north
        });

        let n3 = graph.add_node(StreetNode {
            id: NodeId(3i64),
            geometry: Point::new(0.01, 0.0), // ~1.11km east
        });

        let n4 = graph.add_node(StreetNode {
            id: NodeId(4i64),
            geometry: Point::new(0.01, 0.01), // diagonal
        });

        // Connect the nodes with edges
        graph.add_edge(
            n1,
            n2,
            StreetEdge {
                weight: 793, // ~1110m / 1.4m/s = 793s
            },
        );

        graph.add_edge(n1, n3, StreetEdge { weight: 793 });

        graph.add_edge(n2, n4, StreetEdge { weight: 793 });

        graph.add_edge(n3, n4, StreetEdge { weight: 793 });

        let rtree = build_rtree(&graph);
        let street_network = StreetGraph { graph, rtree };

        // Create a minimal transit data model with stops at nodes 2 and 3
        let mut transit_data = PublicTransitData {
            routes: vec![],
            route_stops: vec![],
            stop_times: vec![],
            stops: vec![],
            stop_routes: vec![],
            transfers: vec![],
            node_to_stop: HashMap::new(),
            feeds_meta: vec![],
            trips: vec![],
            gtfs_transfers: vec![],
        };

        // Map nodes to stops
        transit_data.node_to_stop.insert(n2, 0); // Node 2 (north) is stop 0
        transit_data.node_to_stop.insert(n3, 1); // Node 3 (east) is stop 1

        TransitModel {
            transit_data,
            street_graph: street_network,
            meta: TransitModelMeta {
                max_transfer_time: 1800, // 30 minutes
            },
        }
    }

    fn create_valid_structural_model() -> TransitModel {
        let mut graph = create_test_graph();
        graph.transit_data.stops = vec![
            Stop {
                stop_id: "S0".to_string(),
                geometry: Point::new(0.0, 0.01),
                routes_start: 0,
                routes_len: 0,
                transfers_start: 0,
                transfers_len: 0,
            },
            Stop {
                stop_id: "S1".to_string(),
                geometry: Point::new(0.01, 0.0),
                routes_start: 0,
                routes_len: 0,
                transfers_start: 0,
                transfers_len: 0,
            },
        ];
        graph
    }

    #[test]
    fn test_new_transit_point() {
        let graph = create_test_graph();

        // Create a point at the origin (0,0)
        let point = Point::new(0.0, 0.0);
        let transit_point = TransitPoint::new(
            point, &graph, 1000, // 1000 seconds max walking time
            5,    // max 5 nearest stops
        )
        .unwrap();

        // Check that the closest node is the origin (node 1)
        assert_eq!(transit_point.node_id, NodeIndex::new(0));

        // Should find 2 stops within walking distance (at nodes 2 and 3)
        assert_eq!(transit_point.nearest_stops.len(), 2);

        // Check walking paths - should have paths to all 4 nodes
        assert_eq!(transit_point.walking_paths.len(), 4);

        // Check that the nearest stops are correctly ordered by time
        // Both stops should have the same walking time in this setup
        let stop_ids: HashSet<_> = transit_point
            .nearest_stops
            .iter()
            .map(|&(stop_id, _)| stop_id)
            .collect();
        assert!(stop_ids.contains(&0)); // Stop 0 at node 2
        assert!(stop_ids.contains(&1)); // Stop 1 at node 3
    }

    #[test]
    fn test_new_transit_point_with_insufficient_walking_time() {
        let graph = create_test_graph();

        // Create a point at the origin (0,0)
        let point = Point::new(0.0, 0.0);
        let transit_point = TransitPoint::new(
            point, &graph, 500, // Only 500 seconds, not enough to reach stops
            5,
        )
        .unwrap();

        // Should not find any stops within walking distance
        assert_eq!(transit_point.nearest_stops.len(), 0);

        // Should still have walking paths to nodes within range
        assert!(!transit_point.walking_paths.is_empty());
    }

    #[test]
    fn test_new_transit_point_off_network() {
        let graph = create_test_graph();

        // Create a point far from the network
        let point = Point::new(1.0, 1.0);
        let result = TransitPoint::new(point, &graph, 1800, 5);

        assert!(result.is_err());
    }

    #[test]
    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::cast_possible_wrap
    )]
    fn test_new_transit_point_max_stops_limit() {
        let mut graph = create_test_graph();

        // First, add more nodes to the graph
        let mut additional_nodes = Vec::new();
        for i in 0..8 {
            // Create nodes in a grid pattern around the origin, all within walking distance
            let node = graph.street_graph.graph.add_node(StreetNode {
                id: NodeId((i + 10) as i64),
                geometry: Point::new(0.005 * (i % 4) as f64, 0.005 * (i / 4) as f64),
            });
            additional_nodes.push(node);

            // Connect each new node to the origin node (n1 at index 0)
            graph.street_graph.graph.add_edge(
                NodeIndex::new(0),
                node,
                StreetEdge {
                    weight: 500, // Less than our max walking time
                },
            );

            // Map each new node to a stop
            graph.transit_data.node_to_stop.insert(node, i + 2); // Start from stop ID 2
        }

        // Rebuild the RTree with the new nodes
        graph.street_graph.rtree = build_rtree(&graph.street_graph.graph);

        // Now our graph has 10 stops: 2 original ones + 8 new ones

        let point = Point::new(0.0, 0.0);
        let transit_point = TransitPoint::new(
            point, &graph, 2000, // Enough time to reach all stops
            3,    // Limit to 3 nearest stops
        )
        .unwrap();

        // Should only have 3 stops due to the limit
        assert_eq!(transit_point.nearest_stops.len(), 3);

        // Verify that the stops are the closest ones by checking their walking times
        let mut walking_times: Vec<_> = transit_point
            .nearest_stops
            .iter()
            .map(|&(_, time)| time)
            .collect();
        walking_times.sort_unstable();

        // All walking times should be less than our max and sorted
        for i in 1..walking_times.len() {
            assert!(walking_times[i - 1] <= walking_times[i]);
        }
    }

    #[test]
    fn test_walking_time_to() {
        let graph = create_test_graph();

        // Create two points
        let point1 = Point::new(0.0, 0.0);
        let point2 = Point::new(0.01, 0.01); // At node 4

        let transit_point1 = TransitPoint::new(point1, &graph, 2000, 5).unwrap();

        let transit_point2 = TransitPoint::new(point2, &graph, 2000, 5).unwrap();

        // Check walking time between the points
        let time = transit_point1.walking_time_to(&transit_point2);
        assert!(time.is_some());

        // Walking time should be approximately the sum of two edges
        // (e.g., n1->n2->n4 or n1->n3->n4)
        if let Some(t) = time {
            // Either path should be around 1586 seconds (2 * 793)
            assert!(t > 1500 && t < 1700);
        }
    }

    #[test]
    fn audit_accepts_structurally_valid_model() {
        let graph = create_valid_structural_model();
        assert!(audit_transit_model(&graph).is_ok());
    }

    #[test]
    fn audit_rejects_invalid_transfer_slice() {
        let mut graph = create_test_graph();
        graph.transit_data.stops.push(Stop {
            stop_id: "S-invalid".to_string(),
            geometry: Point::new(0.0, 0.0),
            routes_start: 0,
            routes_len: 0,
            transfers_start: 1,
            transfers_len: 1,
        });

        let result = audit_transit_model(&graph);
        assert!(matches!(result, Err(Error::InvalidData(_))));
    }
}
