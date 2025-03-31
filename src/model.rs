use ferrobus_core::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

#[gen_stub_pyclass]
#[pyclass(name = "TransitModel")]
pub struct PyTransitModel {
    pub model: TransitModel,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyTransitModel {
    pub fn stop_count(&self) -> usize {
        self.model.stop_count()
    }

    pub fn route_count(&self) -> usize {
        self.model.route_count()
    }

    pub fn feeds_info(&self) -> String {
        self.model.feeds_info()
    }

    fn __repr__(&self) -> String {
        format!(
            "TransitModel with {} stops, {} routes and {} trips",
            self.model.stop_count(),
            self.model.route_count(),
            self.model.transit_data.stop_times.len()
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

#[pyfunction(name = "create_transit_model")]
#[pyo3(signature = (osm_path, gtfs_dirs, day_of_week, max_transfer_time = 1800))]
#[gen_stub_pyfunction]
pub fn py_create_transit_model(
    py: Python<'_>,
    osm_path: &str,
    gtfs_dirs: Vec<String>,
    day_of_week: &str,
    max_transfer_time: u32,
) -> PyResult<PyTransitModel> {
    // Allow Python threads during all blocking operations
    py.allow_threads(|| {
        let osm_pathbuf = std::path::PathBuf::from(osm_path);
        let gtfs_pathbufs = gtfs_dirs
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();

        let config = TransitModelConfig {
            osm_path: osm_pathbuf,
            gtfs_dirs: gtfs_pathbufs,
            day_of_week: day_of_week.to_string(),
            max_transfer_time,
        };

        // Create transit model
        let model = ferrobus_core::create_transit_model(&config).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to create transit model: {e}"
            ))
        })?;

        Ok(PyTransitModel { model })
    })
}
