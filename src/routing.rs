use ferrobus_macros::stubgen;
use geo::Point;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::model::PyTransitModel;
use ferrobus_core::{prelude::*, routing::itinerary::traced_multimodal_routing};

/// A geographic location connected to the transit network with pre-calculated access paths
/// to nearby transit stops and the street network.
///
/// `TransitPoint` serves as the fundamental origin/destination entity for all routing operations.
/// Each point maintains a list of nearby transit stops with walking times, enabling efficient
/// multimodal journey planning without recomputing access paths for every query.
///
/// Example
/// -------
///
/// .. code-block:: python
///
///     # Create a transit point at specific coordinates
///     point = ferrobus.create_transit_point(
///         lat=52.5200,
///         lon=13.4050,
///         transit_model=model,
///         max_walking_time=900,  # Maximum walking time in seconds
///         max_nearest_stops=5    # Maximum number of nearby stops to consider
///     )
///
///     # Use the point for routing
///     route = ferrobus.find_route(model, stёart_point, end_point, departure_time)
///
/// The ``max_walking_time`` parameter controls how far the point can connect to the transit
/// network, while ``max_nearest_stops`` limits the number of stops considered during routing.
#[stubgen]
#[pyclass(name = "TransitPoint")]
#[derive(Clone, Debug)]
pub struct PyTransitPoint {
    pub inner: TransitPoint,
}

#[pymethods]
#[stubgen]
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

        Ok(Self {
            inner: transit_point,
        })
    }

    /// Get the coordinates of this transit point
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

/// Create a transit point at specified geographic coordinates
///
/// Creates a location entity connected to the transit network that can be used
/// as an origin or destination in routing operations.
///
/// Parameters
/// ----------
/// lat : float
///     Latitude coordinate of the point.
/// lon : float
///     Longitude coordinate of the point.
/// `transit_model` : `TransitModel`
///     The transit model to which the point should be connected.
/// `max_walking_time` : int, default=1200
///     Maximum walking time in seconds this point can connect to the network.
/// `max_nearest_stops` : int, default=10
///     Maximum number of nearby transit stops to consider for connections.
///
/// Returns
/// -------
/// TransitPoint
///     A location point connected to the transit network.
///
/// Raises
/// ------
/// `ValueError`
///     If the coordinates are invalid or unreachable in the transit network.
///
/// See Also
/// --------
/// TransitPoint : For more details about transit points.
#[stubgen]
#[pyfunction]
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

/// Find an optimal route between two points in a transit network
///
/// Calculates the fastest route between two points using a multimodal approach
/// that combines walking and public transit. The algorithm considers all possible
/// transit connections as well as direct walking paths.
///
/// Parameters
/// ----------
/// `transit_model` : `TransitModel`
///     The transit model to use for routing.
/// `start_point` : `TransitPoint`
///     Starting location for the route.
/// `end_point` : `TransitPoint`
///     Destination location for the route.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int, default=3
///     Maximum number of transfers allowed in route planning.
///
/// Returns
/// -------
/// dict or None
///     A dictionary containing route details including:
///     - `travel_time_seconds`: Total travel time
///     - `walking_time_seconds`: Total walking time
///     - `transit_time_seconds`: Time spent on transit (if used)
///     - transfers: Number of transfers made (if transit used)
///     - `used_transit`: Whether transit was used or just walking
///     Returns None if the destination is unreachable.
///
/// Notes
/// -----
/// This function uses aggressive early pruning, and scans fewer possible egress stops
/// compared to :py:func:`~ferrobus.find_routes_one_to_many`. As a result, the route found may not always be
/// the absolute fastest possible, especially in complex transit networks with multiple
/// transfer options. If you require the most accurate and optimal results, prefer using
/// :py:func:`~ferrobus.find_routes_one_to_many`, which performs a more exhaustive search for each destination.
///
/// Raises
/// ------
/// `RuntimeError`
///     If the route calculation fails.
///
/// Example
/// -------
/// .. code-block:: python
///
///     result = ferrobus.find_route(model, start_point, end_point, departure_time, max_transfers)
///     if result is not None:
///         print(result)
///         # Example output:
///         # {
///         #     "travel_time_seconds": 1800,
///         #     "walking_time_seconds": 300,
///         #     "transit_time_seconds": 1500,
///         #     "transfers": 1,
///         #     "used_transit": True
///         # }
///     else:
///         print("No route found")
#[stubgen]
#[pyfunction]
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

/// Find routes from one point to multiple destinations
///
/// Efficiently calculates routes from a single starting point to multiple
/// destination points in a single operation. This is significantly faster
/// than performing separate routing calculations for each destination.
///
/// Parameters
/// ----------
/// `transit_model` : `TransitModel`
///     The transit model to use for routing.
/// `start_point` : `TransitPoint`
///     Starting location for all routes.
/// `end_points` : list[TransitPoint]
///     List of destination points.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int, default=3
///     Maximum number of transfers allowed in route planning.
///
/// Returns
/// -------
/// list[dict or None]
///     List of routing results in the same order as the input `end_points`.
///     Each result is either a dictionary with route details or None if
///     the destination is unreachable.
///
/// Notes
/// -----
/// The one-to-many nature of this function allows us to scan multiple possible egress
/// stops for each destination, so the results may be more accurate than individual route
/// calculations using :py:func:`~ferrobus.find_route`.
///
/// Example
/// -------
/// .. code-block:: python
///
///     # Create a transit model
///     model = ferrobus.create_transit_model(
///         gtfs_dirs=["path/to/you_feed"],
///         osm_path="path/to/street_network.osm.pbf"
///     )
///
///     start_point = ferrobus.create_transit_point(
///         lat=52.5200,
///         lon=13.4050,
///         transit_model=model,
///         max_walking_time=900,
///         max_nearest_stops=5
///     )
///     end_point1 = ferrobus.create_transit_point(
///         lat=52.5300,
///         lon=13.4100,
///         transit_model=model,
///         max_walking_time=900,
///         max_nearest_stops=5
///     )
///     end_point2 = ferrobus.create_transit_point(
///         lat=52.5400,
///         lon=13.4200,
///         transit_model=model,
///         max_walking_time=900,
///         max_nearest_stops=5
///     )
///
///     # Find routes
///     results = ferrobus.find_routes_one_to_many(
///         model, start_point, [end_point1, end_point2], departure_time=3600, max_transfers = 3
///     )
///     for result in results:
///         if result is not None:
///             print(result)
///             # Example output for one destination:
///             # [{
///             #     "travel_time_seconds": 1800,
///             #     "walking_time_seconds": 300,
///             #     "transit_time_seconds": 1500,
///             #     "transfers": 1,
///             #     "used_transit": True
///             # },
///             # {
///             #     "travel_time_seconds": 2100,
///             #     "walking_time_seconds": 300,
///             #     "transit_time_seconds": 1800,
///             #     "transfers": 2,
///             #     "used_transit": True
///             # }]
///         else:
///             print("No route found")
///
/// Raises
/// ------
/// `RuntimeError`
///     If the batch routing calculation fails.
///
/// Notes
/// -----
/// This function releases the GIL during computation to allow other Python threads to run.
#[stubgen]
#[pyfunction]
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

/// Find a detailed journey between two points in a transit network
///
/// Calculates a detailed multimodal route between two points, including walking
/// and public transit segments. The result is returned as a `GeoJSON` string
/// containing the full journey details.
///
/// Parameters
/// ----------
/// `transit_model` : `TransitModel`
///     The transit model to use for routing.
/// `start_point` : `TransitPoint`
///     Starting location for the journey.
/// `end_point` : `TransitPoint`
///     Destination location for the journey.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int, default=3
///     Maximum number of transfers allowed in route planning.
///
/// Returns
/// -------
/// str
///     A `GeoJSON` string representing the detailed journey, including all route
///     segments and properties such as travel time and transfer details. Returns
///     "null" if no route is found.
///
/// Raises
/// ------
/// `RuntimeError`
///     If the journey calculation fails.
#[stubgen]
#[pyfunction]
#[pyo3(signature = (transit_model, start_point, end_point, departure_time, max_transfers=3))]
pub fn detailed_journey(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    start_point: &PyTransitPoint,
    end_point: &PyTransitPoint,
    departure_time: Time,
    max_transfers: usize,
) -> PyResult<Option<String>> {
    py.allow_threads(|| {
        let result = traced_multimodal_routing(
            &transit_model.model,
            &start_point.inner,
            &end_point.inner,
            departure_time,
            max_transfers,
        )
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Routing failed: {e}"))
        })?;

        if let Some(result) = result {
            return Ok(Some(result.to_geojson_string(&transit_model.model)));
        }
        Ok(None)
    })
}
