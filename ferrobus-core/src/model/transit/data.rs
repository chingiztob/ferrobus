//! Public transit data structure and methods to work with it

use super::types::{FeedMeta, RaptorStopId, Route, RouteId, Stop, StopTime, Time};
use crate::routing::raptor::RaptorError;
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
    pub transfers: Vec<(RaptorStopId, Time)>,
    /// Mapping road network nodes to stops
    pub node_to_stop: HashMap<NodeIndex, RaptorStopId>,
    /// Metadata for feeds
    pub feeds_meta: Vec<FeedMeta>,
}

impl PublicTransitData {
    /// Returns all departure times from the given source stop within the specified time range.
    pub(crate) fn get_source_departures(
        &self,
        source: RaptorStopId,
        min_departure: Time,
        max_departure: Time,
    ) -> Result<Vec<Time>, RaptorError> {
        // Validate the source stop
        self.validate_stop(source)?;

        let mut departures = Vec::new();

        // Get all routes through this stop
        let routes = self.routes_for_stop(source);

        for &route_id in routes {
            // Get stops for the route to find the index of the source stop in the route
            let route_stops = self.get_route_stops(route_id)?;

            // Find the index of the source stop in the route
            if let Some(stop_idx) = route_stops.iter().position(|&stop| stop == source) {
                let route = &self.routes[route_id];

                // For each trip on this route
                for trip_idx in 0..route.num_trips {
                    // Get the trip's stop times
                    let trip = self.get_trip(route_id, trip_idx)?;

                    // Get the departure time at the source stop
                    let departure_time = trip[stop_idx].departure;

                    // If the departure time is within the specified range, add it
                    if departure_time >= min_departure && departure_time <= max_departure {
                        departures.push(departure_time);
                    }
                }
            }
        }

        // Sort and remove duplicates
        departures.sort_unstable();
        departures.dedup();

        Ok(departures)
    }

    /// check if such stop exists
    pub(crate) fn validate_stop(&self, stop: RaptorStopId) -> Result<(), RaptorError> {
        if stop >= self.stops.len() {
            Err(RaptorError::InvalidStop)
        } else {
            Ok(())
        }
    }

    /// Stops for specific route
    pub(crate) fn get_route_stops(
        &self,
        route_id: RouteId,
    ) -> Result<&[RaptorStopId], RaptorError> {
        self.routes
            .get(route_id)
            .ok_or(RaptorError::InvalidRoute)
            .and_then(|route| {
                let end = route.stops_start + route.num_stops;
                if end > self.route_stops.len() {
                    Err(RaptorError::InvalidRoute)
                } else {
                    Ok(&self.route_stops[route.stops_start..end])
                }
            })
    }

    /// `StopTime` slice for specific route and trip
    pub(crate) fn get_trip(
        &self,
        route_id: RouteId,
        trip_idx: usize,
    ) -> Result<&[StopTime], RaptorError> {
        let route = self.routes.get(route_id).ok_or(RaptorError::InvalidRoute)?;

        if trip_idx >= route.num_trips {
            return Err(RaptorError::InvalidTrip);
        }

        let start = route.trips_start + trip_idx * route.num_stops;
        let end = start + route.num_stops;

        if end > self.stop_times.len() {
            Err(RaptorError::InvalidRoute)
        } else {
            Ok(&self.stop_times[start..end])
        }
    }

    /// Returns transfers from the specified stop
    pub(crate) fn get_stop_transfers(
        &self,
        stop_id: RaptorStopId,
    ) -> Result<&[(RaptorStopId, Time)], RaptorError> {
        self.validate_stop(stop_id)?;
        let stop = &self.stops[stop_id];
        let end = stop.transfers_start + stop.transfers_len;
        if end > self.transfers.len() {
            Err(RaptorError::InvalidStop)
        } else {
            Ok(&self.transfers[stop.transfers_start..end])
        }
    }

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
