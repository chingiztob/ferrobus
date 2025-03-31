use geo::Point;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::model::PyTransitModel;
use ferrobus_core::prelude::*;

/// Python wrapper for TransitPoint
#[gen_stub_pyclass]
#[pyclass(name = "TransitPoint")]
#[derive(Clone)]
pub struct PyTransitPoint {
    pub inner: TransitPoint,
}

#[pymethods]
#[gen_stub_pymethods]
impl PyTransitPoint {
    #[new]
    #[pyo3(signature = (lat, lon, transit_model, max_walking_time=1200, max_nearest_stops=10))]
    pub fn new(
        lat: f64,
        lon: f64,
        transit_model: &PyTransitModel,
        max_walking_time: Time,
        max_nearest_stops: usize,
    ) -> PyResult<Self> {
        let point = Point::new(lon, lat);

        let transit_point = TransitPoint::new(
            point,
            &transit_model.model,
            max_walking_time,
            max_nearest_stops,
        )
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Failed to create transit point: {e}"
            ))
        })?;

        Ok(PyTransitPoint {
            inner: transit_point,
        })
    }

    /// Get the coordinates of this transit point
    #[getter]
    fn coordinates(&self) -> (f64, f64) {
        (self.inner.geometry.y(), self.inner.geometry.x()) // Return as (lat, lon)
    }

    fn __repr__(&self) -> String {
        format!(
            "TransitPoint(lat={}, lon={})",
            self.inner.geometry.y(),
            self.inner.geometry.x()
        )
    }

    fn nearest_stops(&self) -> Vec<usize> {
        self.inner.nearest_stops.iter().map(|stop| stop.0).collect()
    }
}

#[pyfunction]
#[gen_stub_pyfunction]
#[pyo3(signature = (lat, lon, transit_model, max_walking_time=1200, max_nearest_stops=10))]
pub fn create_transit_point(
    lat: f64,
    lon: f64,
    transit_model: &PyTransitModel,
    max_walking_time: Time,
    max_nearest_stops: usize,
) -> PyResult<PyTransitPoint> {
    PyTransitPoint::new(lat, lon, transit_model, max_walking_time, max_nearest_stops)
}

/// Convert an Option<MultiModalResult> to a Python dictionary or None
pub(crate) fn optional_result_to_py(py: Python<'_>, result: Option<&MultiModalResult>) -> PyObject {
    match result {
        Some(result) => {
            let dict = PyDict::new(py);

            dict.set_item("travel_time_seconds", result.travel_time)
                .unwrap();
            dict.set_item("walking_time_seconds", result.walking_time)
                .unwrap();

            if let Some(transit_time) = result.transit_time {
                dict.set_item("transit_time_seconds", transit_time).unwrap();
                dict.set_item("transfers", result.transfers).unwrap();
                dict.set_item("used_transit", true).unwrap();
            } else {
                dict.set_item("used_transit", false).unwrap();
                dict.set_item("transfers", 0).unwrap();
            }

            dict.into()
        }
        None => py.None(),
    }
}

#[pyfunction]
#[gen_stub_pyfunction]
#[pyo3(signature = (transit_model, start_point, end_point, departure_time, max_transfers=3))]
pub fn find_route(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    start_point: &PyTransitPoint,
    end_point: &PyTransitPoint,
    departure_time: Time,
    max_transfers: usize,
) -> PyResult<PyObject> {
    let result = multimodal_routing(
        &transit_model.model,
        &start_point.inner,
        &end_point.inner,
        departure_time,
        max_transfers,
    )
    .map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Route calculation failed: {e}"))
    })?;

    Ok(optional_result_to_py(py, result.as_ref()))
}

#[pyfunction]
#[gen_stub_pyfunction]
#[pyo3(signature = (transit_model, start_point, end_points, departure_time, max_transfers=3))]
pub fn find_routes_one_to_many(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    start_point: &PyTransitPoint,
    end_points: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
) -> PyResult<Vec<PyObject>> {
    let end_points = end_points.into_iter().map(|p| p.inner).collect::<Vec<_>>();

    // Perform the routing
    let results = py
        .allow_threads(|| {
            multimodal_routing_one_to_many(
                &transit_model.model,
                &start_point.inner,
                &end_points,
                departure_time,
                max_transfers,
            )
        })
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "One-to-many routing failed: {e}"
            ))
        })?;

    // Convert results to Python objects
    let py_results = results
        .iter()
        .map(|res| optional_result_to_py(py, res.as_ref()))
        .collect();

    Ok(py_results)
}
