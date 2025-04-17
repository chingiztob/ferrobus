//! Processing raw GTFS data for transit model construction

mod de;
mod processor;
mod raw_types;

pub(super) use processor::transit_model_from_gtfs;
pub use raw_types::FeedInfo;
