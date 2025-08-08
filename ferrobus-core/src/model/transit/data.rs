//! Public transit data structure and methods to work with it

use super::types::{FeedMeta, Route, Stop, StopTime, Transfer};
use crate::types::{RaptorStopId, RouteId};
use hashbrown::HashMap;
use petgraph::graph::NodeIndex;

/// Main public transit data structure
/// based on original microsoft paper
#[derive(Debug, Clone)]
pub struct PublicTransitData {
    /// All routes
    pub routes: Vec<Route>,
    /// Stops for each route
    pub route_stops: Vec<RaptorStopId>,
    /// Schedule for each route stop
    pub stop_times: Vec<StopTime>,
    /// All stops
    pub stops: Vec<Stop>,
    /// Routes through each stop
    pub stop_routes: Vec<RouteId>,
    /// Transfers between stops
    pub transfers: Vec<Transfer>,
    /// Mapping road network nodes to stops
    pub node_to_stop: HashMap<NodeIndex, RaptorStopId>,
    /// Metadata for feeds
    pub feeds_meta: Vec<FeedMeta>,
}

impl PublicTransitData {
    /// Returns routes through the specified stop
    pub(crate) fn routes_for_stop(&self, stop_idx: RaptorStopId) -> &[RouteId] {
        let start = self.stops[stop_idx].routes_start;
        let end = start + self.stops[stop_idx].routes_len;
        &self.stop_routes[start..end]
    }

    /// Get the location of a transit stop by ID
    /// Get the location of a transit stop by ID
    pub fn transit_stop_location(&self, stop_id: RaptorStopId) -> geo::Point<f64> {
        if stop_id < self.stops.len() {
            // Return the geometry directly as it's already a Point<f64>
            self.stops[stop_id].geometry
        } else {
            // Default coordinates if stop ID is invalid
            geo::Point::new(0.0, 0.0)
        }
    }

    /// Get the name of a transit stop by ID
    pub fn transit_stop_name(&self, stop_id: RaptorStopId) -> Option<String> {
        if stop_id < self.stops.len() {
            Some(self.stops[stop_id].stop_id.clone())
        } else {
            None
        }
    }
}
