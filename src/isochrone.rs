use ferrobus_core::prelude::*;
use geo::Polygon;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};
use wkt::{ToWkt, TryFromWkt};

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

#[gen_stub_pyclass]
#[pyclass(name = "IsochroneIndex")]
pub struct PyIsochroneIndex {
    inner: IsochroneIndex,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyIsochroneIndex {
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn resolution(&self) -> u8 {
        self.inner.resolution()
    }
}

#[pyfunction]
#[pyo3(signature = (transit_data, area, cell_resolution, max_walking_time=1200))]
#[gen_stub_pyfunction]
pub fn create_isochrone_index(
    transit_data: &PyTransitModel,
    area: &str,
    cell_resolution: u8,
    max_walking_time: Time,
) -> PyResult<PyIsochroneIndex> {
    let area = Polygon::try_from_wkt_str(area).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Failed to parse area WKT: {e}"))
    })?;
    let index = IsochroneIndex::new(
        &transit_data.model,
        &area,
        cell_resolution,
        max_walking_time,
    )
    .map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
            "Failed to create isochrone index: {e}"
        ))
    })?;

    Ok(PyIsochroneIndex { inner: index })
}

#[pyfunction]
#[gen_stub_pyfunction]
pub fn calculate_isochrone(
    py: Python<'_>,
    transit_data: &PyTransitModel,
    start: &PyTransitPoint,
    departure_time: Time,
    max_transfers: usize,
    cutoff: Time,
    index: &PyIsochroneIndex,
) -> PyResult<String> {
    py.allow_threads(|| {
        let isochrone = ferrobus_core::algo::isochrone::calculate_isochrone(
            &transit_data.model,
            &start.inner,
            departure_time,
            max_transfers,
            cutoff,
            &index.inner,
        )
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to calculate isochrone: {e}"
            ))
        })?;

        Ok(isochrone.to_wkt().to_string())
    })
}

#[pyfunction]
#[gen_stub_pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub fn calculate_bulk_isochrones(
    py: Python<'_>,
    transit_data: &PyTransitModel,
    starts: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
    cutoff: Time,
    index: &PyIsochroneIndex,
) -> PyResult<Vec<String>> {
    py.allow_threads(|| {
        let inners = starts.iter().map(|p| &p.inner).collect::<Vec<_>>();
        let isochrones = ferrobus_core::algo::isochrone::bulk_isochrones(
            &transit_data.model,
            inners.as_slice(),
            departure_time,
            max_transfers,
            cutoff,
            &index.inner,
        )
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to calculate isochrone: {e}"
            ))
        })?;

        let result = isochrones.iter().map(|i| i.to_wkt().to_string()).collect();

        Ok(result)
    })
}
