use serde::Deserialize;

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
    pub arrival_time: String,
    pub departure_time: String,
    pub stop_id: String,
    pub stop_sequence: String,
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
    pub stop_lat: String,
    pub stop_lon: String,
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
    pub exception_type: String,
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

fn deserialize_gtfs_date<'de, D>(deserializer: D) -> Result<Option<chrono::NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let date_str = String::deserialize(deserializer)?;
    if date_str.is_empty() {
        Ok(None)
    } else {
        chrono::NaiveDate::parse_from_str(&date_str, "%Y%m%d")
            .map(Some)
            .map_err(serde::de::Error::custom)
    }
}
