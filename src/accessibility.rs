use ferrobus_core::prelude::*;
use ferrobus_macros::stubgen;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

#[derive(Debug)]
struct OriginAccessibilityLevels {
    accessible_count_local: usize,
    accessible_count_regional: usize,
    accessible_count_global: usize,
    target_count_local: usize,
    target_count_regional: usize,
    target_count_global: usize,
    share_local: f64,
    share_regional: f64,
    share_global: f64,
}

fn build_allowed_groups(own_group: i64, neighbors: &HashMap<i64, Vec<i64>>) -> HashSet<i64> {
    let mut allowed = HashSet::new();
    allowed.insert(own_group);

    if let Some(neighbor_groups) = neighbors.get(&own_group) {
        allowed.extend(neighbor_groups.iter().copied());
    }

    allowed
}

#[inline]
fn share(accessible_count: usize, target_count: usize) -> f64 {
    if target_count == 0 {
        f64::NAN
    } else {
        accessible_count as f64 / target_count as f64
    }
}

#[allow(clippy::too_many_arguments)]
fn aggregate_levels_from_travel_times(
    travel_times: &[Option<Time>],
    lau_idx: &[i64],
    nuts3_idx: &[i64],
    allowed_lau: &HashSet<i64>,
    allowed_nuts3: &HashSet<i64>,
    cutoff_local: Time,
    cutoff_regional: Time,
    cutoff_global: Time,
) -> OriginAccessibilityLevels {
    debug_assert_eq!(travel_times.len(), lau_idx.len());
    debug_assert_eq!(travel_times.len(), nuts3_idx.len());

    let mut accessible_count_local = 0usize;
    let mut accessible_count_regional = 0usize;
    let mut accessible_count_global = 0usize;

    let mut target_count_local = 0usize;
    let mut target_count_regional = 0usize;
    let mut target_count_global = 0usize;

    for (destination_idx, travel_time) in travel_times.iter().enumerate() {
        let is_local = allowed_lau.contains(&lau_idx[destination_idx]);
        let is_regional = allowed_nuts3.contains(&nuts3_idx[destination_idx]);

        target_count_global += 1;
        if let Some(time) = travel_time
            && *time <= cutoff_global
        {
            accessible_count_global += 1;
        }

        if is_local {
            target_count_local += 1;
            if let Some(time) = travel_time
                && *time <= cutoff_local
            {
                accessible_count_local += 1;
            }
        }

        if is_regional {
            target_count_regional += 1;
            if let Some(time) = travel_time
                && *time <= cutoff_regional
            {
                accessible_count_regional += 1;
            }
        }
    }

    OriginAccessibilityLevels {
        accessible_count_local,
        accessible_count_regional,
        accessible_count_global,
        target_count_local,
        target_count_regional,
        target_count_global,
        share_local: share(accessible_count_local, target_count_local),
        share_regional: share(accessible_count_regional, target_count_regional),
        share_global: share(accessible_count_global, target_count_global),
    }
}

/// Computes per-origin accessibility metrics for three geographical levels:
/// local (own LAU + neighboring LAU), regional (own NUTS3 + neighboring NUTS3),
/// and global (all points).
///
/// The function runs exactly one one-to-many routing pass per origin and returns
/// only aggregated counters and shares, without building an OD matrix.
#[allow(clippy::too_many_arguments)]
#[stubgen]
#[pyfunction]
pub fn travel_time_accessibility_levels(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    points: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
    lau_idx: Vec<i64>,
    nuts3_idx: Vec<i64>,
    lau_neighbors: HashMap<i64, Vec<i64>>,
    nuts3_neighbors: HashMap<i64, Vec<i64>>,
    cutoff_local: Time,
    cutoff_regional: Time,
    cutoff_global: Time,
) -> PyResult<Py<PyAny>> {
    let point_count = points.len();
    if point_count != lau_idx.len() || point_count != nuts3_idx.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "points, lau_idx and nuts3_idx must have the same length",
        ));
    }

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

                    Ok(aggregate_levels_from_travel_times(
                        &travel_times,
                        &lau_idx,
                        &nuts3_idx,
                        &allowed_lau,
                        &allowed_nuts3,
                        cutoff_local,
                        cutoff_regional,
                        cutoff_global,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;

    let mut accessible_count_local = Vec::with_capacity(point_count);
    let mut accessible_count_regional = Vec::with_capacity(point_count);
    let mut accessible_count_global = Vec::with_capacity(point_count);
    let mut target_count_local = Vec::with_capacity(point_count);
    let mut target_count_regional = Vec::with_capacity(point_count);
    let mut target_count_global = Vec::with_capacity(point_count);
    let mut share_local = Vec::with_capacity(point_count);
    let mut share_regional = Vec::with_capacity(point_count);
    let mut share_global = Vec::with_capacity(point_count);

    for origin_metrics in per_origin {
        accessible_count_local.push(origin_metrics.accessible_count_local);
        accessible_count_regional.push(origin_metrics.accessible_count_regional);
        accessible_count_global.push(origin_metrics.accessible_count_global);
        target_count_local.push(origin_metrics.target_count_local);
        target_count_regional.push(origin_metrics.target_count_regional);
        target_count_global.push(origin_metrics.target_count_global);
        share_local.push(origin_metrics.share_local);
        share_regional.push(origin_metrics.share_regional);
        share_global.push(origin_metrics.share_global);
    }

    let result = PyDict::new(py);
    result.set_item("accessible_count_local", accessible_count_local)?;
    result.set_item("accessible_count_regional", accessible_count_regional)?;
    result.set_item("accessible_count_global", accessible_count_global)?;
    result.set_item("target_count_local", target_count_local)?;
    result.set_item("target_count_regional", target_count_regional)?;
    result.set_item("target_count_global", target_count_global)?;
    result.set_item("share_local", share_local)?;
    result.set_item("share_regional", share_regional)?;
    result.set_item("share_global", share_global)?;

    Ok(result.into())
}

#[cfg(test)]
mod tests {
    use super::{aggregate_levels_from_travel_times, build_allowed_groups};
    use ferrobus_core::Time;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn build_allowed_groups_includes_own_group_when_missing_in_neighbors() {
        let neighbors: HashMap<i64, Vec<i64>> = HashMap::new();
        let allowed = build_allowed_groups(101, &neighbors);

        assert_eq!(allowed.len(), 1);
        assert!(allowed.contains(&101));
    }

    #[test]
    fn aggregate_levels_counts_unreachable_and_respects_inclusive_cutoff() {
        let travel_times: Vec<Option<Time>> = vec![Some(0), Some(600), None];
        let lau_idx = vec![1, 1, 2];
        let nuts3_idx = vec![10, 11, 10];
        let allowed_lau = HashSet::from([1]);
        let allowed_nuts3 = HashSet::from([10]);

        let metrics = aggregate_levels_from_travel_times(
            &travel_times,
            &lau_idx,
            &nuts3_idx,
            &allowed_lau,
            &allowed_nuts3,
            0,
            0,
            600,
        );

        assert_eq!(metrics.target_count_local, 2);
        assert_eq!(metrics.accessible_count_local, 1);
        assert_eq!(metrics.target_count_regional, 2);
        assert_eq!(metrics.accessible_count_regional, 1);
        assert_eq!(metrics.target_count_global, 3);
        assert_eq!(metrics.accessible_count_global, 2);

        assert_eq!(metrics.share_local, 0.5);
        assert_eq!(metrics.share_regional, 0.5);
        assert_eq!(metrics.share_global, 2.0 / 3.0);
    }

    #[test]
    fn aggregate_levels_returns_nan_share_when_target_count_is_zero() {
        let travel_times: Vec<Option<Time>> = vec![];
        let lau_idx: Vec<i64> = vec![];
        let nuts3_idx: Vec<i64> = vec![];
        let allowed_lau: HashSet<i64> = HashSet::new();
        let allowed_nuts3: HashSet<i64> = HashSet::new();

        let metrics = aggregate_levels_from_travel_times(
            &travel_times,
            &lau_idx,
            &nuts3_idx,
            &allowed_lau,
            &allowed_nuts3,
            0,
            0,
            0,
        );

        assert!(metrics.share_local.is_nan());
        assert!(metrics.share_regional.is_nan());
        assert!(metrics.share_global.is_nan());
    }
}
