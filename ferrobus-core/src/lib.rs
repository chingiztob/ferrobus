use petgraph::graph::NodeIndex;

use thiserror::Error;

pub mod algo;
pub mod error;
pub mod loading;
pub mod model;
pub mod prelude;
pub mod routing;

/// Maximum number of candidate stops to check
/// when performing a multimodal routing query
/// with raptor algorithm
pub const MAX_CANDIDATE_STOPS: usize = 1;
/// Pedestrian speed in meters per second (1.4 m/s ~ 5 km/h)
pub const WALKING_SPEED: f64 = 1.4;

pub use error::Error;
pub use loading::{TransitModelConfig, create_transit_model};
pub use model::{PublicTransitData, TransitModel, TransitPoint};
pub use routing::multimodal_routing::{
    MultiModalResult, multimodal_routing, multimodal_routing_one_to_many,
};

// Core types for the street network
pub type StreetNodeId = NodeIndex;

// Core types for transit routing
pub type RaptorStopId = usize;
pub type RouteId = usize;
pub type Time = u32;
