//! Обработка GTFS данных для транспортной модели

mod parser;
mod processor;
mod raw_types;

pub use parser::deserialize_gtfs_file;
pub use processor::transit_model_from_gtfs;
pub use raw_types::{FeedInfo, FeedRoute, FeedStop, FeedStopTime, FeedTrip};
