//! This module is responsible for loading data from various sources (GTFS, OSM)
//! and building an multimodal routing model.

mod builder;
mod config;
pub mod gtfs;
pub mod osm;
mod transfers;

pub use builder::create_transit_model;
pub use config::TransitModelConfig;
