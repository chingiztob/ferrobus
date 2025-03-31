//! Data model for public transportation routing
//!
//! Contains types and structures for representing a transit network.

// Re-export of main modules
pub mod streets;
pub mod transit;
pub mod transit_model;

// Re-export of the main model structure
pub use transit_model::{TransitModel, TransitPoint};

// Re-export of basic types for convenience
pub use streets::network::StreetGraph;
pub use transit::data::PublicTransitData;
pub use transit::types::{RaptorStopId, Route, RouteId, Stop, StopTime, Time};
