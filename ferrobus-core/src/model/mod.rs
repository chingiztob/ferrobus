//! Data model for public transportation routing
//!
//! Contains types and structures for representing a transit network.

// Re-export of main modules
mod streets;
mod transit;
mod transit_model;

pub(crate) use streets::{IndexedPoint, StreetEdge, StreetNode};

// Re-export of the main model structure
pub use transit_model::{TransitModel, TransitModelMeta, TransitPoint};

// Re-export of basic types for convenience
pub use streets::StreetGraph;
pub use transit::data::PublicTransitData;
pub use transit::types::{FeedMeta, RaptorStopId, Route, RouteId, Stop, StopTime, Time, Transfer};
