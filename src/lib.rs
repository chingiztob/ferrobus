use pyo3::prelude::*;

use isochrone::{
    PyIsochroneIndex, calculate_bulk_isochrones, calculate_isochrone,
    calculate_percent_access_isochrone, create_isochrone_index,
};
use matrix::travel_time_matrix;
use model::{PyTransitModel, py_create_transit_model};
use range_routing::{
    PyRangeRoutingResult, py_pareto_range_multimodal_routing, py_range_multimodal_routing,
};
use routing::{
    PyTransitPoint, create_transit_point, detailed_journey, find_route, find_routes_one_to_many,
};

pub mod isochrone;
pub mod matrix;
pub mod model;
pub mod range_routing;
pub mod routing;

/// Ferrobus: Multimodal transit routing library
///
/// Ferrobus is a fast Python library that provides multimodal
/// transit routing capabilities, designed for geospatial analysis workflows.
///
/// Ferrobus is fully implemented in Rust, which makes it extremely fast.
/// This also allows for zero-dependency installation, as it does not require any external
/// libraries or packages. Unlike R5 or OpenTripPlanner, Ferrobus does not require Java.
///
/// Functionality
/// =============
///
/// - **Routing**: Find optimal paths combining walking and public transit
/// - **Isochrone generation**: Create travel-time polygons using a hexagonal grid system
/// - **Travel time matrices**: Compute travel times between multiple points
/// - **Batch processing**: Process multiple routes or isochrones efficiently
/// - **Time-range routing**: Find journeys across a range of departure times
///
/// Example
/// =======
///
/// .. code-block:: python
///
///    # Create a transit model
///    model = ferrobus.create_transit_model("streets.osm.pbf", ["gtfs_data"], None)
///
///    # Create transit points
///    origin = ferrobus.create_transit_point(52.52, 13.40, model)
///    destination = ferrobus.create_transit_point(52.53, 13.42, model)
///
///    # Find route
///    departure_time = 8 * 3600  # 8:00 AM in seconds since midnight
///    route = ferrobus.find_route(model, origin, destination, departure_time)
///
///    # Generate an isochrone
///    index = ferrobus.create_isochrone_index(model, area_wkt, 8)
///    isochrone = ferrobus.calculate_isochrone(model, origin, departure_time,
///                                             2, 1800, index)
///
///    # Calculate a travel time matrix
///    points = [origin, destination, point3, point4]
///    matrix = ferrobus.travel_time_matrix(model, points, departure_time, 3)
///
/// Implementation details
/// =======================
///
/// The library uses several techniques to enable efficient routing:
/// - Parallel processing for batch operations
/// - Spatial data structures for network representation
/// - Pre-computation of access paths to transit stops
/// - RAPTOR algorithm for transit routing
#[pymodule]
fn ferrobus(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    m.add_class::<PyTransitModel>()?;
    m.add_class::<PyTransitPoint>()?;
    m.add_function(wrap_pyfunction!(py_create_transit_model, m)?)?;

    m.add_function(wrap_pyfunction!(find_route, m)?)?;
    m.add_function(wrap_pyfunction!(find_routes_one_to_many, m)?)?;
    m.add_function(wrap_pyfunction!(create_transit_point, m)?)?;
    m.add_function(wrap_pyfunction!(detailed_journey, m)?)?;

    m.add_function(wrap_pyfunction!(travel_time_matrix, m)?)?;

    m.add_class::<PyIsochroneIndex>()?;
    m.add_function(wrap_pyfunction!(create_isochrone_index, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_isochrone, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_bulk_isochrones, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_percent_access_isochrone, m)?)?;

    m.add_class::<PyRangeRoutingResult>()?;
    m.add_function(wrap_pyfunction!(py_range_multimodal_routing, m)?)?;
    m.add_function(wrap_pyfunction!(py_pareto_range_multimodal_routing, m)?)?;
    Ok(())
}

#[cfg(feature = "stubgen")]
pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
