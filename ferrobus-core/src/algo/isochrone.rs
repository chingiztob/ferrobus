//! Calculation of isochrones with naive buffer over reached nodes
//! can be very slow for large areas. This module provides an
//! alternative approach to calculate isochrones using H3 hexagonal
//! grid cells as a index.

use geo::{MultiPolygon, Point, Polygon};
use hashbrown::HashMap;
use rayon::prelude::*;

use h3o::{
    CellIndex, LatLng, Resolution,
    geom::{ContainmentMode, SolventBuilder, TilerBuilder},
};

use crate::{Error, Time, TransitModel};
use crate::{TransitPoint, multimodal_routing_one_to_many};

/// Index for isochrone calculation covering a specific area
/// It contains a grid of hexagonal H3 cells and their respective
/// transit points.
#[derive(Debug, Clone)]
pub struct IsochroneIndex {
    pub grid: Vec<CellIndex>,
    transit_points: Vec<TransitPoint>,
    resoulution: u8,
}

impl IsochroneIndex {
    pub fn len(&self) -> usize {
        self.grid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grid.is_empty() && self.transit_points.is_empty()
    }

    pub fn resolution(&self) -> u8 {
        self.resoulution
    }
}

impl IsochroneIndex {
    pub fn new(
        transit_data: &TransitModel,
        area: &Polygon,
        cell_resolution: u8,
        max_walking_time: Time,
    ) -> Result<Self, Error> {
        let original_grid = create_hex_coverage(area.clone(), cell_resolution)?;
        let grid_centroids = get_grid_centroids(&original_grid);

        // Create transit points and track which ones are successful
        let snap_results: Vec<(usize, Result<TransitPoint, Error>)> = grid_centroids
            .par_iter()
            .enumerate()
            .map(|(i, point)| {
                (
                    i,
                    TransitPoint::new(*point, transit_data, max_walking_time, 3),
                )
            })
            .collect();

        // Filter to keep only successful transit points and corresponding grid cells
        let mut grid = Vec::new();
        let mut snapped_centroids = Vec::new();

        for (idx, result) in snap_results {
            if let Ok(transit_point) = result {
                grid.push(original_grid[idx]);
                snapped_centroids.push(transit_point);
            }
        }

        println!(
            "Snapped {} of {}",
            snapped_centroids.len(),
            original_grid.len()
        );

        Ok(Self {
            grid,
            transit_points: snapped_centroids,
            resoulution: cell_resolution,
        })
    }
}

pub fn calculate_isochrone(
    transit_data: &TransitModel,
    start: &TransitPoint,
    departure_time: Time,
    max_transfers: usize,
    cutoff: Time,
    index: &IsochroneIndex,
) -> Result<MultiPolygon, Error> {
    let reached_cells = compute_reachable_cells(
        transit_data,
        start,
        departure_time,
        max_transfers,
        cutoff,
        index,
    )?;

    let solvent = SolventBuilder::new().build();
    solvent
        .dissolve(reached_cells)
        .map_err(|e| Error::IsochroneError(e.to_string()))
}

pub fn bulk_isochrones(
    transit_data: &TransitModel,
    starts: &[&TransitPoint],
    departure_time: Time,
    max_transfers: usize,
    cutoff: Time,
    index: &IsochroneIndex,
) -> Result<Vec<MultiPolygon>, Error> {
    let result: Result<Vec<MultiPolygon>, Error> = starts
        .par_iter()
        .map(|start| {
            calculate_isochrone(
                transit_data,
                start,
                departure_time,
                max_transfers,
                cutoff,
                index,
            )
        })
        .collect();

    result
}

#[allow(clippy::cast_precision_loss)]
pub fn calculate_percent_access_isochrone(
    transit_data: &TransitModel,
    start: &TransitPoint,
    departure_range: (Time, Time),
    sample_interval: Time,
    max_transfers: usize,
    cutoff: Time,
    index: &IsochroneIndex,
) -> Result<HashMap<CellIndex, f64>, Error> {
    // Generate a list of departure times to sample
    let mut departure_times = Vec::new();
    let (start_time, end_time) = departure_range;
    let mut current_time = start_time;

    while current_time <= end_time {
        departure_times.push(current_time);
        current_time += sample_interval;
    }

    // Calculate reachable cells for each departure time
    let all_reached_cells: Result<Vec<Vec<CellIndex>>, Error> = departure_times
        .par_iter()
        .map(|&departure_time| {
            compute_reachable_cells(
                transit_data,
                start,
                departure_time,
                max_transfers,
                cutoff,
                index,
            )
        })
        .collect();

    // Count how many times each cell is reached
    let mut cell_access_count = HashMap::new();
    for reached_cells in &all_reached_cells? {
        for &cell in reached_cells {
            *cell_access_count.entry(cell).or_insert(0) += 1;
        }
    }

    // Calculate percentage access for each cell
    let total_samples = departure_times.len() as f64;
    let mut percent_access = HashMap::new();
    for (cell, count) in cell_access_count {
        let percentage = (f64::from(count) / total_samples) * 100.0;
        percent_access.insert(cell, percentage);
    }

    Ok(percent_access)
}

fn create_hex_coverage(area: Polygon, resolution: u8) -> Result<Vec<CellIndex>, Error> {
    let resolution = Resolution::try_from(resolution)
        .map_err(|e| Error::InvalidData(format!("Got invalid H3 resolution {e}")))?;

    let mut tiler = TilerBuilder::new(resolution)
        .containment_mode(ContainmentMode::Covers)
        .build();
    tiler.add(area)?;

    Ok(tiler.into_coverage().collect::<Vec<_>>())
}

fn get_grid_centroids(grid: &[CellIndex]) -> Vec<Point<f64>> {
    grid.iter()
        .map(|cell| {
            let lat_lon = LatLng::from(*cell);

            Point::new(lat_lon.lng(), lat_lon.lat())
        })
        .collect()
}

fn compute_reachable_cells(
    transit_data: &TransitModel,
    start: &TransitPoint,
    departure_time: u32,
    max_transfers: usize,
    cutoff: u32,
    index: &IsochroneIndex,
) -> Result<Vec<CellIndex>, Error> {
    let snapped_centroids = &index.transit_points;
    let grid = &index.grid;
    let routing_results = multimodal_routing_one_to_many(
        transit_data,
        start,
        snapped_centroids,
        departure_time,
        max_transfers,
    )?;
    let reached_cells: Vec<CellIndex> = routing_results
        .iter()
        .enumerate()
        .filter_map(|(index, result)| {
            result
                .as_ref()
                .filter(|r| r.travel_time < cutoff)
                .map(|_| grid[index])
        })
        .collect();
    Ok(reached_cells)
}
