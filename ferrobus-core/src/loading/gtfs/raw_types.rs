use serde::Deserialize;

use super::de::{deserialize_gtfs_date, deserialize_gtfs_time};

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FeedCalendar {
    pub service_id: String,
    pub monday: String,
    pub tuesday: String,
    pub wednesday: String,
    pub thursday: String,
    pub friday: String,
    pub saturday: String,
    pub sunday: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FeedTrip {
    pub route_id: String,
    pub service_id: String,
    pub trip_id: String,
    pub trip_headsign: String,
    pub trip_short_name: String,
    pub direction_id: String,
    pub block_id: String,
    pub shape_id: String,
    pub wheelchair_accessible: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FeedRoute {
    pub route_id: String,
    pub agency_id: String,
    pub route_short_name: String,
    pub route_long_name: String,
    pub route_desc: String,
    pub route_type: String,
    pub route_url: String,
    pub route_color: String,
    pub route_text_color: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct FeedStopTime {
    pub trip_id: String,
    #[serde(deserialize_with = "deserialize_gtfs_time")]
    pub arrival_time: u32,
    #[serde(deserialize_with = "deserialize_gtfs_time")]
    pub departure_time: u32,
    pub stop_id: String,
    pub stop_sequence: u32,
    /* pub stop_headsign: String,
    pub pickup_type: String,
    pub drop_off_type: String,
    pub shape_dist_traveled: String, */
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FeedStop {
    pub stop_id: String,
    pub stop_code: String,
    pub stop_name: String,
    pub stop_desc: String,
    pub stop_lat: f64,
    pub stop_lon: f64,
    pub zone_id: String,
    pub stop_url: String,
    pub location_type: String,
    pub parent_station: String,
    pub stop_timezone: String,
    pub wheelchair_boarding: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FeedTripEntity {
    pub route_id: String,
    pub service_id: String,
    pub trip_id: String,
    pub trip_headsign: String,
    pub trip_short_name: String,
    pub direction_id: String,
    pub block_id: String,
    pub shape_id: String,
    pub wheelchair_accessible: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FeedService {
    pub service_id: String,
    pub monday: String,
    pub tuesday: String,
    pub wednesday: String,
    pub thursday: String,
    pub friday: String,
    pub saturday: String,
    pub sunday: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FeedCalendarDates {
    pub service_id: String,
    #[serde(deserialize_with = "deserialize_gtfs_date")]
    pub date: Option<chrono::NaiveDate>,
    pub exception_type: u8,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
#[allow(clippy::struct_field_names)]
pub struct FeedInfo {
    pub feed_publisher_name: String,
    pub feed_publisher_url: String,
    pub feed_lang: String,
    #[serde(deserialize_with = "deserialize_gtfs_date")]
    pub feed_start_date: Option<chrono::NaiveDate>,
    #[serde(deserialize_with = "deserialize_gtfs_date")]
    pub feed_end_date: Option<chrono::NaiveDate>,
    pub feed_version: String,
}
