use axum::{Json, extract::State, http::StatusCode};
use ferrobus_core::{
    RangeRoutingResult, multimodal_routing, multimodal_routing_one_to_many,
    pareto_range_multimodal_routing, range_multimodal_routing,
    routing::itinerary::traced_multimodal_routing,
};

use crate::{
    error::{ApiError, map_core_error},
    state::AppState,
};

use super::{
    convert::{point_from_input, route_response_from_core},
    exec::run_blocking,
    models::{
        DEFAULT_MAX_TRANSFERS, DetailedJourneyRequest, RangeRouteRequest, RouteRequest,
        RouteResponse, RoutesOneToManyRequest,
    },
};

pub(crate) async fn route(
    State(state): State<AppState>,
    Json(req): Json<RouteRequest>,
) -> Result<Json<Option<RouteResponse>>, ApiError> {
    let model = state.model.clone();
    let response = run_blocking(move || {
        let start = point_from_input(&model, &req.start_point)?;
        let end = point_from_input(&model, &req.end_point)?;
        let result = multimodal_routing(
            &model,
            &start,
            &end,
            req.departure_time,
            req.max_transfers.unwrap_or(DEFAULT_MAX_TRANSFERS),
        )
        .map_err(map_core_error)?;
        Ok(result.as_ref().map(route_response_from_core))
    })
    .await?;

    Ok(Json(response))
}

pub(crate) async fn routes_one_to_many(
    State(state): State<AppState>,
    Json(req): Json<RoutesOneToManyRequest>,
) -> Result<Json<Vec<Option<RouteResponse>>>, ApiError> {
    let model = state.model.clone();
    let response = run_blocking(move || {
        let start = point_from_input(&model, &req.start_point)?;
        let end_points = req
            .end_points
            .iter()
            .map(|p| point_from_input(&model, p))
            .collect::<Result<Vec<_>, _>>()?;

        let result = multimodal_routing_one_to_many(
            &model,
            &start,
            &end_points,
            req.departure_time,
            req.max_transfers.unwrap_or(DEFAULT_MAX_TRANSFERS),
        )
        .map_err(map_core_error)?;

        Ok(result
            .iter()
            .map(|item| item.as_ref().map(route_response_from_core))
            .collect())
    })
    .await?;

    Ok(Json(response))
}

pub(crate) async fn detailed_journey(
    State(state): State<AppState>,
    Json(req): Json<DetailedJourneyRequest>,
) -> Result<Json<Option<String>>, ApiError> {
    let model = state.model.clone();
    let response = run_blocking(move || {
        let start = point_from_input(&model, &req.start_point)?;
        let end = point_from_input(&model, &req.end_point)?;
        let journey = traced_multimodal_routing(
            &model,
            &start,
            &end,
            req.departure_time,
            req.max_transfers.unwrap_or(DEFAULT_MAX_TRANSFERS),
        )
        .map_err(map_core_error)?;

        journey
            .map(|j| j.to_geojson_string(&model).map_err(map_core_error))
            .transpose()
    })
    .await?;

    Ok(Json(response))
}

pub(crate) async fn range_route(
    State(state): State<AppState>,
    Json(req): Json<RangeRouteRequest>,
) -> Result<Json<RangeRoutingResult>, ApiError> {
    let [start_dep, end_dep] = req.departure_range;
    if start_dep > end_dep {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_DEPARTURE_RANGE",
            "departure_range[0] must be <= departure_range[1]",
        ));
    }

    let model = state.model.clone();
    let response = run_blocking(move || {
        let start = point_from_input(&model, &req.start_point)?;
        let end = point_from_input(&model, &req.end_point)?;
        let result = range_multimodal_routing(
            &model,
            &start,
            &end,
            (start_dep, end_dep),
            req.max_transfers.unwrap_or(DEFAULT_MAX_TRANSFERS),
        )
        .map_err(map_core_error)?;
        Ok(result)
    })
    .await?;

    Ok(Json(response))
}

pub(crate) async fn pareto_range_route(
    State(state): State<AppState>,
    Json(req): Json<RangeRouteRequest>,
) -> Result<Json<RangeRoutingResult>, ApiError> {
    let [start_dep, end_dep] = req.departure_range;
    if start_dep > end_dep {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_DEPARTURE_RANGE",
            "departure_range[0] must be <= departure_range[1]",
        ));
    }

    let model = state.model.clone();
    let response = run_blocking(move || {
        let start = point_from_input(&model, &req.start_point)?;
        let end = point_from_input(&model, &req.end_point)?;
        let result = pareto_range_multimodal_routing(
            &model,
            &start,
            &end,
            (start_dep, end_dep),
            req.max_transfers.unwrap_or(DEFAULT_MAX_TRANSFERS),
        )
        .map_err(map_core_error)?;
        Ok(result)
    })
    .await?;

    Ok(Json(response))
}
