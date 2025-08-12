//! Basic types for public transit data model

use geo::Point;

use crate::loading::FeedInfo;

// Re-export the common types for convenience
pub use crate::types::{RaptorStopId, RouteId, Time};

/// Arrival/departure time at a stop
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopTime {
    /// Arrival time in seconds from the beginning of the day
    pub arrival: Time,
    /// Departure time in seconds from the beginning of the day
    pub departure: Time,
}

/// Public transport route
#[derive(Debug, Clone)]
pub struct Route {
    /// Number of trips on the route
    pub num_trips: usize,
    /// Number of stops on the route
    pub num_stops: usize,
    /// Index of the start of the stop list in the general array
    pub stops_start: usize,
    /// Index of the start of the trip list in the general array
    pub trips_start: usize,
    /// Route ID
    pub route_id: String,
}

/// Public transport stop
#[derive(Debug, Clone)]
pub struct Stop {
    /// Unique stop identifier
    pub stop_id: String,
    /// Geographic coordinates of the stop
    pub geometry: Point<f64>,
    /// Index of the start of the route list in the general array
    pub(crate) routes_start: usize,
    ///) Number of routes through the stop
    pub(crate) routes_len: usize,
    /// Index of the start of the transfer list in the general array
    pub(crate) transfers_start: usize,
    /// Number of available transfers
    pub(crate) transfers_len: usize,
}

#[derive(Debug, Clone)]
pub struct FeedMeta {
    pub feed_info: FeedInfo,
}

#[derive(Debug, Clone)]
pub struct Transfer {
    pub target_stop: RaptorStopId,
    pub duration: Time,
}
