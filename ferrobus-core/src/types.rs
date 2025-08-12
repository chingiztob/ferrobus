//! Central type definitions for the ferrobus-core crate

use petgraph::graph::{EdgeIndex, NodeIndex};

// === Core Transit Types ===

/// Unique identifier for a RAPTOR stop
///
/// This represents an index into the stops array in the transit model.
pub type RaptorStopId = usize;

/// Unique identifier for a route
///
/// This represents an index into the routes array in the transit model.
pub type RouteId = usize;

/// Unique identifier for a trip
///
/// This represents an index into the trips array within a route.
pub type TripId = usize;

/// Time representation in seconds since midnight
///
/// All transit schedules use this format, with 0 representing midnight
/// and values continuing past 86400 for trips that extend past midnight.
pub type Time = u32;

/// Duration in seconds
///
/// Used for transfer times, walking times, and other duration measurements.
pub type Duration = u32;

// === Street Network Types ===

/// Node identifier in the street network graph
///
/// This is a reference to a node in the petgraph-based street network.
pub type StreetNodeId = NodeIndex;

/// Edge identifier in the street network graph
///
/// This is a reference to an edge in the petgraph-based street network.
pub type StreetEdgeId = EdgeIndex;

// === Index Types ===

/// Index into arrays of stops
pub type StopIndex = usize;

/// Index into arrays of routes
pub type RouteIndex = usize;

/// Number of stops in a route or maximum stops for algorithms
pub type StopCount = usize;

/// Number of routes passing through a stop
pub type RouteCount = usize;

/// Round number in RAPTOR algorithm iterations
pub type Round = usize;
