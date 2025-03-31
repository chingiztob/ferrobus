use pyo3::prelude::*;
use pyo3_stub_gen::define_stub_info_gatherer;

use isochrone::{
    PyIsochroneIndex, calculate_bulk_isochrones, calculate_isochrone, create_isochrone_index,
};
use matrix::travel_time_matrix;
use model::{PyTransitModel, py_create_transit_model};
use range_routing::{
    PyRangeRoutingResult, py_pareto_range_multimodal_routing, py_range_multimodal_routing,
};
use routing::{PyTransitPoint, create_transit_point, find_route, find_routes_one_to_many};

pub mod isochrone;
pub mod matrix;
pub mod model;
pub mod range_routing;
pub mod routing;

/// A Python module implemented in Rust.
#[pymodule]
fn ferrobus(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

    m.add_class::<PyTransitModel>()?;
    m.add_class::<PyTransitPoint>()?;
    m.add_function(wrap_pyfunction!(py_create_transit_model, m)?)?;

    m.add_function(wrap_pyfunction!(find_route, m)?)?;
    m.add_function(wrap_pyfunction!(find_routes_one_to_many, m)?)?;
    m.add_function(wrap_pyfunction!(create_transit_point, m)?)?;

    m.add_function(wrap_pyfunction!(travel_time_matrix, m)?)?;

    m.add_class::<PyIsochroneIndex>()?;
    m.add_function(wrap_pyfunction!(create_isochrone_index, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_isochrone, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_bulk_isochrones, m)?)?;

    m.add_class::<PyRangeRoutingResult>()?;
    m.add_function(wrap_pyfunction!(py_range_multimodal_routing, m)?)?;
    m.add_function(wrap_pyfunction!(py_pareto_range_multimodal_routing, m)?)?;
    Ok(())
}

define_stub_info_gatherer!(stub_info);
