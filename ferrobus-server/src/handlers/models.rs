use ferrobus_core::Time;
use serde::{Deserialize, Serialize};

pub(crate) const DEFAULT_MAX_WALKING_TIME: Time = 1200;
pub(crate) const DEFAULT_MAX_NEAREST_STOPS: usize = 10;
pub(crate) const DEFAULT_MAX_TRANSFERS: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PointInput {
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub max_walking_time: Option<Time>,
    #[serde(default)]
    pub max_nearest_stops: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RouteRequest {
    pub start_point: PointInput,
    pub end_point: PointInput,
    pub departure_time: Time,
    #[serde(default)]
    pub max_transfers: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RoutesOneToManyRequest {
    pub start_point: PointInput,
    pub end_points: Vec<PointInput>,
    pub departure_time: Time,
    #[serde(default)]
    pub max_transfers: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DetailedJourneyRequest {
    pub start_point: PointInput,
    pub end_point: PointInput,
    pub departure_time: Time,
    #[serde(default)]
    pub max_transfers: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MatrixRequest {
    pub points: Vec<PointInput>,
    pub departure_time: Time,
    #[serde(default)]
    pub max_transfers: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct StatisticsRequest {
    pub points: Vec<PointInput>,
    pub departure_time: Time,
    #[serde(default)]
    pub max_transfers: Option<usize>,
    #[serde(default)]
    pub threshold: Option<f64>,
    #[serde(default)]
    pub stat: Option<String>,
    #[serde(default)]
    pub filter_cutoff: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RangeRouteRequest {
    pub start_point: PointInput,
    pub end_point: PointInput,
    pub departure_range: [Time; 2],
    #[serde(default)]
    pub max_transfers: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RouteResponse {
    pub travel_time_seconds: Time,
    pub walking_time_seconds: Time,
    pub transit_time_seconds: Option<Time>,
    pub transfers: usize,
    pub used_transit: bool,
}
