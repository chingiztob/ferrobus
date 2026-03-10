use ferrobus_core::prelude::*;
use ferrobus_macros::stubgen;
use pyo3::prelude::*;
use rayon::prelude::*;

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

/// Computes a matrix of travel times between
/// all points in the input set in parallel.
///
/// Parameters
/// ----------
/// `transit_model` : `TransitModel`
///     The transit model to use for routing.
/// points : list[`TransitPoint`]
///     List of points between which to calculate travel times.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int
///     Maximum number of transfers allowed in route planning.
///
/// Returns
/// -------
/// list[list[Optional[int]]]
///     A 2D matrix where each cell [i][j] contains the travel time in seconds
///     from point i to point j, or None if the point is unreachable.
#[stubgen]
#[pyfunction]
pub fn travel_time_matrix(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    points: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
) -> PyResult<Vec<Vec<Option<u32>>>> {
    // Perform the routing
    let points: Vec<_> = points.into_iter().map(|p| p.inner).collect();
    let full_vec = py.detach(|| {
        points
            .par_iter()
            .map(|start_point| {
                match multimodal_routing_one_to_many(
                    &transit_model.model,
                    start_point,
                    &points,
                    departure_time,
                    max_transfers,
                ) {
                    Ok(result) => result,
                    Err(e) => {
                        println!("Routing failed for point {start_point:?}, error: {e}");
                        vec![None; points.len()]
                    }
                }
            })
            .map(|vector| {
                vector
                    .into_iter()
                    .map(|result| result.map(|dict| dict.travel_time))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });

    Ok(full_vec)
}

/// Computes travel time statistics from each point to all targets in parallel.
///
/// For each origin point, returns the travel time statistic to reachable targets
/// only if at least the specified percentage of all targets are reachable;
/// otherwise returns None.
///
/// Parameters
/// ----------
/// `transit_model` : `TransitModel`
///     The transit model to use for routing.
/// points : list[`TransitPoint`]
///     List of points used as both origins and targets.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int
///     Maximum number of transfers allowed in route planning.
/// `threshold` : float, default = 0.75
///     Percentage of target points which must be reached to allow statistics.
/// `stat` : str, default = "mean"
///     Statistic computed from reachable travel times:
///
///     • `"mean"`   — arithmetic mean travel time
///     • `"median"` — median travel time
/// `filter_cutoff` : Optional[int], default = None
///     If provided, excludes destinations with travel time strictly greater than
///     this cutoff (in seconds) from the statistic computation.
///
/// Returns
/// -------
/// list[Optional[float]]
///     A list where each element corresponds to an origin point and contains
///     the computed travel time statistic in seconds, or None if fewer than the
///     specified percentage of targets are reachable from that origin.
#[allow(clippy::too_many_arguments)]
#[stubgen]
#[pyfunction]
#[pyo3(signature = (transit_model, points, departure_time, max_transfers, threshold=0.75, stat="mean", filter_cutoff=None))]
pub fn travel_time_statistics(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    points: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
    threshold: f64,
    stat: &str, // "mean" or "median"
    filter_cutoff: Option<u64>,
) -> PyResult<Vec<Option<f64>>> {
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "threshold must be a finite number in [0.0, 1.0]",
        ));
    }

    if stat != "mean" && stat != "median" {
        return Err(pyo3::exceptions::PyValueError::new_err(
            r#"stat must be "mean" or "median""#,
        ));
    }

    let points: Vec<_> = points.into_iter().map(|p| p.inner).collect();
    let target_count = points.len();

    if target_count == 0 {
        return Ok(vec![]);
    }

    let results = py.detach(|| {
        points
            .par_iter()
            .map(|start_point| {
                let routing_result = multimodal_routing_one_to_many(
                    &transit_model.model,
                    start_point,
                    &points,
                    departure_time,
                    max_transfers,
                )
                .map_err(|e| format!("Routing failed for point {start_point:?}, error: {e}"))?;

                let mut reached_times: Vec<u64> = Vec::with_capacity(routing_result.len());
                for destination in routing_result.into_iter().flatten() {
                    if let Some(cutoff) = filter_cutoff
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
                        let Some(lo) = lower.iter().max().copied() else {
                            return Err(format!(
                                "Median computation failed for point {start_point:?}: empty lower partition"
                            ));
                        };
                        Ok(Some(f64::midpoint(lo as f64, *hi as f64)))
                    }
                }
            })
            .collect::<Result<Vec<_>, String>>()
    });

    results.map_err(pyo3::exceptions::PyRuntimeError::new_err)
}
