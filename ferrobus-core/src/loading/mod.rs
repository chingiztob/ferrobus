//! This module is responsible for loading data from various sources (GTFS, OSM)
//! and building an multimodal routing model.

mod builder;
mod config;
mod gtfs;
mod osm;
mod transfers;

#[cfg(test)]
pub(crate) use osm::build_rtree;

pub use builder::create_transit_model;
pub use config::TransitModelConfig;
pub use gtfs::FeedInfo;
