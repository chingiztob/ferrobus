use ferrobus_core::prelude::*;
use ferrobus_macros::stubgen;
use pyo3::prelude::*;

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

#[stubgen]
#[pyclass(name = "RangeRoutingResult")]
pub struct PyRangeRoutingResult {
    pub inner: RangeRoutingResult,
}

#[stubgen]
#[pymethods]
impl PyRangeRoutingResult {
    #[must_use]
    pub fn median_travel_time(&self) -> Time {
        self.inner.median_travel_time()
    }

    #[must_use]
    pub fn travel_times(&self) -> Vec<Time> {
        self.inner.travel_times()
    }

    #[must_use]
    pub fn departure_times(&self) -> Vec<Time> {
        self.inner.departure_times()
    }

    pub fn as_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to serialize RangeRoutingResult to JSON: {e}"
            ))
        })
    }

    fn __repr__(&self) -> PyResult<String> {
        self.as_json()
    }

    fn __str__(&self) -> PyResult<String> {
        self.as_json()
    }
}

#[stubgen]
#[pyfunction(name = "range_multimodal_routing")]
#[pyo3(signature = (transit_model, start, end, departure_range, max_transfers=3))]
pub fn py_range_multimodal_routing(
    transit_model: &PyTransitModel,
    start: &PyTransitPoint,
    end: &PyTransitPoint,
    departure_range: (Time, Time),
    max_transfers: usize,
) -> PyResult<PyRangeRoutingResult> {
    let result = ferrobus_core::prelude::range_multimodal_routing(
        &transit_model.model,
        &start.inner,
        &end.inner,
        departure_range,
        max_transfers,
    )
    .map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
            "Range multomodal routing failed: {e}"
        ))
    })?;

    Ok(PyRangeRoutingResult { inner: result })
}

#[stubgen]
#[pyfunction(name = "pareto_range_multimodal_routing")]
#[pyo3(signature = (transit_model, start, end, departure_range, max_transfers=3))]
pub fn py_pareto_range_multimodal_routing(
    transit_model: &PyTransitModel,
    start: &PyTransitPoint,
    end: &PyTransitPoint,
    departure_range: (Time, Time),
    max_transfers: usize,
) -> PyResult<PyRangeRoutingResult> {
    let result = ferrobus_core::prelude::pareto_range_multimodal_routing(
        &transit_model.model,
        &start.inner,
        &end.inner,
        departure_range,
        max_transfers,
    )
    .map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
            "Range multomodal routing failed: {e}"
        ))
    })?;

    Ok(PyRangeRoutingResult { inner: result })
}
