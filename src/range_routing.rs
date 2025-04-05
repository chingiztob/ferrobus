use ferrobus_core::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

#[gen_stub_pyclass]
#[pyclass(name = "RangeRoutingResult")]
pub struct PyRangeRoutingResult {
    pub inner: RangeRoutingResult,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyRangeRoutingResult {
    pub fn median_travel_time(&self) -> Time {
        self.inner.median_travel_time()
    }

    pub fn travel_times(&self) -> Vec<Time> {
        self.inner.travel_times()
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

#[gen_stub_pyfunction]
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

#[gen_stub_pyfunction]
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
