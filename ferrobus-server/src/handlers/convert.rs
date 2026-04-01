use ferrobus_core::{MultiModalResult, TransitModel, TransitPoint};
use geo::Point;

use crate::error::{ApiError, map_core_error};

use super::models::{
    DEFAULT_MAX_NEAREST_STOPS, DEFAULT_MAX_WALKING_TIME, PointInput, RouteResponse,
};

pub(crate) fn point_from_input(
    model: &TransitModel,
    point: &PointInput,
) -> Result<TransitPoint, ApiError> {
    TransitPoint::new(
        Point::new(point.lon, point.lat),
        model,
        point.max_walking_time.unwrap_or(DEFAULT_MAX_WALKING_TIME),
        point.max_nearest_stops.unwrap_or(DEFAULT_MAX_NEAREST_STOPS),
    )
    .map_err(map_core_error)
}

pub(crate) fn route_response_from_core(result: &MultiModalResult) -> RouteResponse {
    RouteResponse {
        travel_time_seconds: result.travel_time,
        walking_time_seconds: result.walking_time,
        transit_time_seconds: result.transit_time,
        transfers: result.transfers,
        used_transit: result.transit_time.is_some(),
    }
}
