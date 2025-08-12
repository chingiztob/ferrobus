use ferrobus_core::prelude::*;

use ferrobus_macros::stubgen;
use pyo3::prelude::*;

/// `TransitModel`
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
#[stubgen]
#[pyclass(name = "TransitModel", frozen)]
pub struct PyTransitModel {
    pub(crate) model: TransitModel,
}

#[stubgen]
#[pymethods]
impl PyTransitModel {
    /// Get total stop count of all feeds in the model
    #[must_use]
    pub fn stop_count(&self) -> usize {
        self.model.stop_count()
    }

    /// Get total route count of all feeds in the model
    #[must_use]
    pub fn route_count(&self) -> usize {
        self.model.route_count()
    }

    /// Get information about the GTFS feeds in the transit model
    ///
    /// Returns a summary of the GTFS feeds included in the transit model, such as
    /// feed names, versions, and other metadata. The output is formatted as a JSON
    /// string, similar to the GTFS `feed_info.txt`.
    ///
    /// Returns
    /// -------
    /// str
    ///     A JSON string containing information about the GTFS feeds.
    ///
    /// Example
    /// -------
    /// .. code-block:: python
    ///
    ///     info = model.feeds_info()
    ///     print(json.loads(info))
    ///     # Example output:
    ///     # [
    ///     #     {
    ///     #         "feed_publisher_name": "City Transit",
    ///     #         "feed_publisher_url": "http://citytransit.example.com",
    ///     #         "feed_lang": "en",
    ///     #         "feed_start_date": "2025-01-01",
    ///     #         "feed_end_date": "2025-12-31",
    ///     #         "feed_version": "2025.04"
    ///     #     }
    ///     # ]
    #[must_use]
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
///
/// - Processing OpenStreetMap data to create the street network
/// - Loading GTFS transit schedules into RAPTOR model
/// - Connecting transit stops to the street network
/// - Creating transfer connections between nearby stops
///
/// The resulting model enables multimodal routing, isochrone generation,
/// and travel time matrix calculations.
///
/// Parameters
/// ----------
/// `osm_path` : str
///     Path to OpenStreetMap PBF file containing street network data
/// `gtfs_dirs` : list[str]
///     List of paths to directories containing GTFS data
/// date : datetime.date, optional
///     Filter transit schedules to services running on this date.
///     If None, includes all services.
/// `max_transfer_time` : int, default=1800
///     Maximum walking time in seconds allowed for transfers between stops
///
/// Returns
/// -------
/// `TransitModel`
///     An integrated model for multimodal routing operations
///
/// Raises
/// ------
/// `RuntimeError`
///     If the model creation fails due to data errors
///
/// Notes
/// -----
/// The function releases the GIL during processing to allow other Python threads to continue execution.
#[stubgen]
#[pyfunction(name = "create_transit_model")]
#[pyo3(signature = (osm_path, gtfs_dirs, date, max_transfer_time = 1200))]
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
