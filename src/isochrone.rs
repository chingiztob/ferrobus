//! # Isochrone calculations for transit networks
//!
//! This module provides highly optimized functions for calculating isochrones
//! (service-area polygons) in transit networks. Traditional isochrone generation
//! is computationally expensive due to complex geometric operations. This module
//! solves this problem using a hexagonal grid system (H3 cells) that can be
//! rapidly queried and processed.
//!
//! ## Workflow
//!
//! 1. Create an isochrone index for your area of interest
//! 2. Use this index to rapidly calculate isochrones from any point
//!
//! ```python
//! # Example Python usage
//! index = ferrobus.create_isochrone_index(model, area_wkt, 8)
//! isochrone = ferrobus.calculate_isochrone(model, point, departure_time,
//!                                          max_transfers, cutoff, index)
//! ```

use ferrobus_core::prelude::*;
use ferrobus_macros::stubgen;
use geo::{Coord, LineString, Polygon};
use pyo3::prelude::*;
use wkt::{ToWkt, TryFromWkt};

use geojson::{Feature, FeatureCollection};
use serde_json::map::Map;

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

/// A spatial index structure for highly efficient isochrone calculations in transit networks.
///
/// Traditional isochrone generation involves computationally expensive geometric operations
/// such as buffering network edges and performing unary unions on complex polygons.
/// This approach often becomes prohibitively slow for interactive applications or
/// batch processing multiple isochrones.
///
/// `IsochroneIndex` solves this problem by pre-processing the transit network into a
/// hexagonal grid system (H3 cells) that can be rapidly queried and merged during
/// isochrone generation. This provides orders-of-magnitude performance improvements
/// by avoiding expensive geometric operations at query time.
///
/// Rather than working with precise network geometry during isochrone calculations,
/// this index:
///
/// 1. Discretizes the geographic area into hexagonal cells
/// 2. Pre-computes network connectivity at cell boundaries
/// 3. Enables fast cell-based isochrone expansion
/// 4. Dissolves contiguous cells during final isochrone generation
///
/// This approach trades some precision (based on cell resolution) for dramatic
/// performance improvements, making it practical for interactive applications.
///
/// Example
/// -------
///
/// .. code-block:: python
///
///     index = create_isochrone_index(model, area_wkt, 8, 1200);
///     isochrone = calculate_isochrone(py, model, point, departure, 3, 1800, index);
///
///     # The resulting isochrone is a WKT string representing the polygon
///     print(isochrone)
///     >>> MULTIPOLYGON ((...))
///
/// References
/// ----------
///
/// For more information about the H3 hexagonal grid system used in this module,
/// see the `H3 documentation <https://h3geo.org/>`_.
#[stubgen]
#[pyclass(name = "IsochroneIndex")]
pub struct PyIsochroneIndex {
    inner: IsochroneIndex,
}

#[stubgen]
#[pymethods]
impl PyIsochroneIndex {
    /// Get the number of cells in the isochrone index
    ///
    /// Returns the total number of hexagonal cells stored in the isochrone index.
    ///
    /// Returns
    /// -------
    /// int
    ///     The number of cells in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the isochrone index is empty
    ///
    /// Determines whether the isochrone index contains any cells.
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if the index is empty, False otherwise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the resolution of the isochrone index
    ///
    /// Returns the resolution of the `H3` hexagonal grid used in the isochrone index.
    /// Higher resolutions correspond to smaller hexagonal cells.
    ///
    /// Returns
    /// -------
    /// int
    ///     The resolution of the hexagonal grid (0-15).
    #[must_use]
    pub fn resolution(&self) -> u8 {
        self.inner.resolution()
    }
}

/// Create a spatial index for isochrone calculations
///
/// Parameters
/// ----------
/// `transit_data` : `TransitModel`
///     The transportation model containing transit network information.
/// area : str
///     Geographic area over which to build the isochrone, as a WKT string.
/// `cell_resolution` : int
///     Resolution of hexagonal grid cells (0-255). Higher values create
///     finer-grained but larger indexes.
/// `max_walking_time` : int, default=1200
///     Maximum time in seconds for walking connections.
///
/// Returns
/// -------
/// `IsochroneIndex`
///     Pre-computed spatial index structure for rapid isochrone calculations.
///
/// Raises
/// ------
/// `ValueError`
///     If the area WKT string cannot be parsed.
/// `RuntimeError`
///     If the isochrone index creation fails for other reasons.
///
/// Notes
/// -----
/// Creating this index may be compute-intensive but allows for extremely fast
/// subsequent isochrone calculations, making it ideal for interactive applications
/// or batch processing multiple isochrones from different starting points.
#[stubgen]
#[pyfunction]
#[pyo3(signature = (transit_data, area, cell_resolution, max_walking_time=1200))]
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

/// Calculate an isochrone from a single starting point
///
/// Computes an accessibility isochrone (travel-time polygon) using the provided
/// spatial index for rapid calculation.
///
/// Parameters
/// ----------
/// `transit_data` : `TransitModel`
///     The transit model to use for routing.
/// start : `TransitPoint`
///     Starting location for the isochrone.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int
///     Maximum number of transfers allowed in route planning.
/// cutoff : int
///     Maximum travel time in seconds to include in the isochrone.
/// index : `IsochroneIndex`
///     Pre-computed isochrone spatial index for the area.
///
/// Returns
/// -------
/// str
///     WKT representation of the resulting polygon isochrone.
///
/// Raises
/// ------
/// `RuntimeError`
///     If the isochrone calculation fails.
#[pyfunction]
#[stubgen]
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
        let isochrone = ferrobus_core::algo::calculate_isochrone(
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

/// Calculate isochrones from multiple starting points in batch mode
///
/// This is an optimized bulk version of `calculate_isochrone` that processes
/// multiple starting points in parallel, which is significantly faster than
/// repeated individual calculations.
///
/// Parameters
/// ----------
/// `transit_data` : `TransitModel`
///     The transit model to use for routing.
/// starts : list[`TransitPoint`]
///     List of starting locations for isochrone calculations.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int
///     Maximum number of transfers allowed in route planning.
/// cutoff : int
///     Maximum travel time in seconds to include in the isochrones.
/// index : `IsochroneIndex`
///     Pre-computed isochrone spatial index for the area.
///
/// Returns
/// -------
/// list[str]
///     List of WKT representations of the resulting isochrones,
///     matching the order of the input starting points.
///
/// Raises
/// ------
/// `RuntimeError`
///     If the batch isochrone calculation fails.
#[pyfunction]
#[stubgen]
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
        let isochrones = ferrobus_core::algo::bulk_isochrones(
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

/// Calculate percentage-based accessibility across multiple departure times
///
/// Computes how frequently each cell in the area is accessible across a range
/// of departure times, producing a heat map of transit reliability.
///
/// Parameters
/// ----------
/// `transit_data` : `TransitModel`
///     The transit model to use for routing.
/// start : `TransitPoint`
///     Starting location for the isochrone.
/// `departure_range` : tuple(int, int)
///     Range of departure times to sample (`start_time`, `end_time`) in seconds.
/// `sample_interval` : int
///     Time interval between samples in seconds.
/// `max_transfers` : int
///     Maximum number of transfers allowed in route planning.
/// cutoff : int
///     Maximum travel time in seconds to include in the isochrone.
/// index : `IsochroneIndex`
///     Pre-computed isochrone spatial index for the area.
///
/// Returns
/// -------
/// str
///     `GeoJSON` `FeatureCollection` string containing polygons for each grid cell
///     with properties indicating the percentage of sampled times the cell
///     was accessible.
///
/// Raises
/// ------
/// `RuntimeError`
///     If isochrone calculation fails.
///
/// Notes
/// -----
/// This function is useful for analyzing transit reliability and service
/// frequency across different times of day.
#[pyfunction]
#[stubgen]
#[allow(clippy::too_many_arguments)]
pub fn calculate_percent_access_isochrone(
    py: Python<'_>,
    transit_data: &PyTransitModel,
    start: &PyTransitPoint,
    departure_range: (Time, Time),
    sample_interval: Time,
    max_transfers: usize,
    cutoff: Time,
    index: &PyIsochroneIndex,
) -> PyResult<String> {
    py.allow_threads(|| {
        let percent_access = ferrobus_core::algo::calculate_percent_access_isochrone(
            &transit_data.model,
            &start.inner,
            departure_range,
            sample_interval,
            max_transfers,
            cutoff,
            &index.inner,
        )
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to calculate isochrone: {e}"
            ))
        })?;

        let mut features = vec![];

        // Create a MultiPolygon for each cell and assign the percentage access
        for (cell, percentage) in &percent_access {
            let mut properties = Map::new();
            let boundary: LineString = cell
                .boundary()
                .iter()
                .map(|lat_lng| Coord {
                    x: lat_lng.lng(),
                    y: lat_lng.lat(),
                })
                .collect();

            let polygon = Polygon::new(boundary, vec![]);
            properties.insert("percent_access".to_owned(), (*percentage).into());

            features.push(Feature {
                geometry: Some(geojson::Geometry::from(&polygon)),
                properties: Some(properties),
                id: None,
                bbox: None,
                foreign_members: None,
            });
        }

        let collection = geojson::GeoJson::FeatureCollection(FeatureCollection {
            features,
            bbox: None,
            foreign_members: None,
        })
        .to_string();

        Ok(collection)
    })
}
