use axum::{Json, extract::State, http::StatusCode};
use rayon::prelude::*;

use crate::{
    error::{ApiError, map_core_error},
    state::AppState,
};

use super::{
    convert::point_from_input,
    exec::run_blocking,
    models::{DEFAULT_MAX_TRANSFERS, MatrixRequest, StatisticsRequest},
};

pub(crate) async fn matrix(
    State(state): State<AppState>,
    Json(req): Json<MatrixRequest>,
) -> Result<Json<Vec<Vec<Option<u32>>>>, ApiError> {
    let model = state.model.clone();
    let response = run_blocking(move || {
        let points = req
            .points
            .iter()
            .map(|p| point_from_input(&model, p))
            .collect::<Result<Vec<_>, _>>()?;

        let matrix = points
            .par_iter()
            .map(|start| {
                ferrobus_core::multimodal_routing_one_to_many(
                    &model,
                    start,
                    &points,
                    req.departure_time,
                    req.max_transfers.unwrap_or(DEFAULT_MAX_TRANSFERS),
                )
                .map_err(map_core_error)
                .map(|row| {
                    row.into_iter()
                        .map(|item| item.map(|r| r.travel_time))
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(matrix)
    })
    .await?;

    Ok(Json(response))
}

pub(crate) async fn statistics(
    State(state): State<AppState>,
    Json(req): Json<StatisticsRequest>,
) -> Result<Json<Vec<Option<f64>>>, ApiError> {
    let threshold = req.threshold.unwrap_or(0.75);
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_THRESHOLD",
            "threshold must be a finite number in [0.0, 1.0]",
        ));
    }

    let stat = req.stat.unwrap_or_else(|| "mean".to_string());
    if stat != "mean" && stat != "median" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_STAT",
            r#"stat must be "mean" or "median""#,
        ));
    }

    let model = state.model.clone();
    let response = run_blocking(move || {
        let points = req
            .points
            .iter()
            .map(|p| point_from_input(&model, p))
            .collect::<Result<Vec<_>, _>>()?;
        let target_count = points.len();
        if target_count == 0 {
            return Ok(Vec::new());
        }

        let results = points
            .par_iter()
            .map(|start| {
                let routing_result = ferrobus_core::multimodal_routing_one_to_many(
                    &model,
                    start,
                    &points,
                    req.departure_time,
                    req.max_transfers.unwrap_or(DEFAULT_MAX_TRANSFERS),
                )
                .map_err(map_core_error)?;

                let mut reached_times = Vec::with_capacity(routing_result.len());
                for destination in routing_result.into_iter().flatten() {
                    if let Some(cutoff) = req.filter_cutoff
                        && u64::from(destination.travel_time) > cutoff
                    {
                        continue;
                    }
                    reached_times.push(u64::from(destination.travel_time));
                }

                let reached_count = reached_times.len();
                if reached_count == 0 || (reached_count as f64 / target_count as f64) < threshold {
                    return Ok(None);
                }

                if stat == "mean" {
                    let sum: u64 = reached_times.iter().sum();
                    Ok(Some(sum as f64 / reached_count as f64))
                } else {
                    let mid = reached_count / 2;
                    let (lower, hi, _) = reached_times.select_nth_unstable(mid);
                    if reached_count % 2 == 1 {
                        Ok(Some(*hi as f64))
                    } else {
                        let lo = lower.iter().max().copied().ok_or_else(|| {
                            ApiError::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "INTERNAL_ERROR",
                                "Median computation failed",
                            )
                        })?;
                        Ok(Some(f64::midpoint(lo as f64, *hi as f64)))
                    }
                }
            })
            .collect::<Result<Vec<_>, ApiError>>()?;

        Ok(results)
    })
    .await?;

    Ok(Json(response))
}
