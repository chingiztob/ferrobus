use ferrobus_core::prelude::*;

use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

/// TransitModel
///
/// A unified transit model that integrates both the street network (OSM) and
/// public transit schedules (GTFS) for multimodal routing.
///
/// This model serves as the foundation for all routing operations, containing
/// the complete graph representation of both networks with interconnections
/// between transit stops and the street network.
///
/// Core components:
///
/// - Street network for walking/access paths
/// - Transit stops, routes and schedules
/// - Transfer connections between stops
/// - Spatial indices for efficient lookups
///
/// Example:
///
/// .. code-block:: python
///
///     model = create_transit_model("path/to/osm.pbf", ["path/to/gtfs"], None, 1800)
///     transit_point = create_transit_point(lat, lon, model, 1200, 10)
#[gen_stub_pyclass]
#[pyclass(name = "TransitModel")]
pub struct PyTransitModel {
    pub(crate) model: TransitModel,
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

/// Create a unified transit model from OSM and GTFS data
///
/// This function builds a complete multimodal transportation model by:
/// 1. Processing OpenStreetMap data to create the street network
/// 2. Loading GTFS transit schedules
/// 3. Connecting transit stops to the street network
/// 4. Creating transfer connections between nearby stops
///
/// The resulting model enables multimodal routing, isochrone generation,
/// and travel time matrix calculations.
///
/// Parameters
/// ----------
/// osm_path : str
///     Path to OpenStreetMap PBF file containing street network data
/// gtfs_dirs : list[str]
///     List of paths to directories containing GTFS data
/// date : datetime.date, optional
///     Filter transit schedules to services running on this date.
///     If None, includes all services.
/// max_transfer_time : int, default=1800
///     Maximum walking time in seconds allowed for transfers between stops
///
/// Returns
/// -------
/// TransitModel
///     An integrated model for multimodal routing operations
///
/// Raises
/// ------
/// RuntimeError
///     If the model creation fails due to data errors
///
/// Notes
/// -----
/// The function releases the GIL during processing to allow other Python threads to continue execution.
#[gen_stub_pyfunction]
#[pyfunction(name = "create_transit_model")]
#[pyo3(signature = (osm_path, gtfs_dirs, date, max_transfer_time = 1800))]
pub fn py_create_transit_model(
    py: Python<'_>,
    osm_path: &str,
    gtfs_dirs: Vec<String>,
    date: Option<chrono::NaiveDate>,
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
            date,
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
