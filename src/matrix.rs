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
    let full_vec = py.allow_threads(|| {
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
