use ferrobus_core::prelude::*;
use ferrobus_macros::stubgen;
use geo::{Distance, Haversine};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

#[derive(Debug)]
struct OriginRelativeEfficiencyLevels {
    delta_a_local_sec: Option<f64>,
    delta_a_regional_sec: Option<f64>,
    delta_a_global_sec: Option<f64>,
}

fn build_allowed_groups(own_group: i64, neighbors: &HashMap<i64, Vec<i64>>) -> HashSet<i64> {
    let mut allowed = HashSet::new();
    allowed.insert(own_group);

    if let Some(neighbor_groups) = neighbors.get(&own_group) {
        allowed.extend(neighbor_groups.iter().copied());
    }

    allowed
}

fn validate_ref_speed_kmh(speed_kmh: f64, name: &str) -> PyResult<()> {
    if speed_kmh.is_finite() && speed_kmh > 0.0 {
        Ok(())
    } else {
        Err(pyo3::exceptions::PyValueError::new_err(format!(
            "{name} must be a finite positive number"
        )))
    }
}

#[inline]
fn kmh_to_mps(speed_kmh: f64) -> f64 {
    speed_kmh / 3.6
}

#[inline]
fn transit_point_distance_meters(origin: &TransitPoint, destination: &TransitPoint) -> f64 {
    Haversine.distance(origin.geometry, destination.geometry)
}

fn median_f64(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }

    let len = values.len();
    let upper_mid = len / 2;
    let upper_value = {
        let (_, upper, _) = values.select_nth_unstable_by(upper_mid, f64::total_cmp);
        *upper
    };

    if len % 2 == 1 {
        Some(upper_value)
    } else {
        let lower = values[..upper_mid]
            .iter()
            .copied()
            .max_by(f64::total_cmp)
            .expect("non-empty lower half for even-length slice");
        Some((lower + upper_value) * 0.5)
    }
}

#[allow(clippy::too_many_arguments)]
fn aggregate_relative_efficiency_levels_from_travel_times(
    travel_times: &[Option<Time>],
    points: &[TransitPoint],
    origin_idx: usize,
    lau_idx: &[i64],
    nuts3_idx: &[i64],
    allowed_lau: &HashSet<i64>,
    allowed_nuts3: &HashSet<i64>,
    local_ref_speed_mps: f64,
    regional_ref_speed_mps: f64,
    global_ref_speed_mps: f64,
) -> OriginRelativeEfficiencyLevels {
    debug_assert_eq!(travel_times.len(), points.len());
    debug_assert_eq!(travel_times.len(), lau_idx.len());
    debug_assert_eq!(travel_times.len(), nuts3_idx.len());

    let origin = &points[origin_idx];
    let mut delta_local = Vec::new();
    let mut delta_regional = Vec::new();
    let mut delta_global = Vec::new();

    for (destination_idx, travel_time) in travel_times.iter().enumerate() {
        let Some(travel_time) = travel_time else {
            continue;
        };

        let distance_m = transit_point_distance_meters(origin, &points[destination_idx]);
        let factual_sec = f64::from(*travel_time);

        delta_global.push(factual_sec - distance_m / global_ref_speed_mps);

        if allowed_lau.contains(&lau_idx[destination_idx]) {
            delta_local.push(factual_sec - distance_m / local_ref_speed_mps);
        }

        if allowed_nuts3.contains(&nuts3_idx[destination_idx]) {
            delta_regional.push(factual_sec - distance_m / regional_ref_speed_mps);
        }
    }

    OriginRelativeEfficiencyLevels {
        delta_a_local_sec: median_f64(&mut delta_local),
        delta_a_regional_sec: median_f64(&mut delta_regional),
        delta_a_global_sec: median_f64(&mut delta_global),
    }
}

/// Computes per-origin relative travel-time efficiency (ΔA) for three levels:
/// local (own LAU + neighboring LAU), regional (own NUTS3 + neighboring NUTS3),
/// and global (all points).
#[allow(clippy::too_many_arguments)]
#[stubgen]
#[pyfunction]
pub fn travel_time_relative_efficiency_levels(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    points: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
    lau_idx: Vec<i64>,
    nuts3_idx: Vec<i64>,
    lau_neighbors: HashMap<i64, Vec<i64>>,
    nuts3_neighbors: HashMap<i64, Vec<i64>>,
    local_ref_speed_kmh: f64,
    regional_ref_speed_kmh: f64,
    global_ref_speed_kmh: f64,
) -> PyResult<Py<PyAny>> {
    let point_count = points.len();
    if point_count != lau_idx.len() || point_count != nuts3_idx.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "points, lau_idx and nuts3_idx must have the same length",
        ));
    }

    validate_ref_speed_kmh(local_ref_speed_kmh, "local_ref_speed_kmh")?;
    validate_ref_speed_kmh(regional_ref_speed_kmh, "regional_ref_speed_kmh")?;
    validate_ref_speed_kmh(global_ref_speed_kmh, "global_ref_speed_kmh")?;

    let local_ref_speed_mps = kmh_to_mps(local_ref_speed_kmh);
    let regional_ref_speed_mps = kmh_to_mps(regional_ref_speed_kmh);
    let global_ref_speed_mps = kmh_to_mps(global_ref_speed_kmh);
    let points: Vec<_> = points.into_iter().map(|p| p.inner).collect();

    let per_origin = py
        .detach(|| {
            points
                .par_iter()
                .enumerate()
                .map(|(origin_idx, start_point)| {
                    let routing_result = multimodal_routing_one_to_many(
                        &transit_model.model,
                        start_point,
                        &points,
                        departure_time,
                        max_transfers,
                    )
                    .map_err(|e| format!("Routing failed for point {start_point:?}, error: {e}"))?;

                    let travel_times: Vec<Option<Time>> = routing_result
                        .into_iter()
                        .map(|result| result.map(|route| route.travel_time))
                        .collect();

                    let allowed_lau = build_allowed_groups(lau_idx[origin_idx], &lau_neighbors);
                    let allowed_nuts3 =
                        build_allowed_groups(nuts3_idx[origin_idx], &nuts3_neighbors);

                    Ok(aggregate_relative_efficiency_levels_from_travel_times(
                        &travel_times,
                        &points,
                        origin_idx,
                        &lau_idx,
                        &nuts3_idx,
                        &allowed_lau,
                        &allowed_nuts3,
                        local_ref_speed_mps,
                        regional_ref_speed_mps,
                        global_ref_speed_mps,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;

    let mut delta_a_local_sec = Vec::with_capacity(point_count);
    let mut delta_a_regional_sec = Vec::with_capacity(point_count);
    let mut delta_a_global_sec = Vec::with_capacity(point_count);

    for origin_metrics in per_origin {
        delta_a_local_sec.push(origin_metrics.delta_a_local_sec);
        delta_a_regional_sec.push(origin_metrics.delta_a_regional_sec);
        delta_a_global_sec.push(origin_metrics.delta_a_global_sec);
    }

    let result = PyDict::new(py);
    result.set_item("delta_a_local_sec", delta_a_local_sec)?;
    result.set_item("delta_a_regional_sec", delta_a_regional_sec)?;
    result.set_item("delta_a_global_sec", delta_a_global_sec)?;
    Ok(result.into())
}

#[cfg(test)]
mod tests {
    use super::median_f64;

    #[test]
    fn median_f64_returns_none_for_empty() {
        let mut values = vec![];
        assert_eq!(median_f64(&mut values), None);
    }

    #[test]
    fn median_f64_works_for_even_and_odd_lengths() {
        let mut odd = vec![7.0, 1.0, 5.0];
        assert_eq!(median_f64(&mut odd), Some(5.0));

        let mut even = vec![9.0, 1.0, 5.0, 3.0];
        assert_eq!(median_f64(&mut even), Some(4.0));
    }
}
