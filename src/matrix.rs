use ferrobus_core::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyfunction;
use rayon::prelude::*;

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

#[gen_stub_pyfunction]
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
