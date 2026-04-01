use ferrobus_core::prelude::*;
use ferrobus_macros::stubgen;
use pyo3::prelude::*;
use rayon::prelude::*;
#[cfg(feature = "matrix-zarr")]
use std::sync::Arc;

use crate::model::PyTransitModel;
use crate::routing::PyTransitPoint;

#[cfg(feature = "matrix-zarr")]
const ZARR_UNREACHABLE_SENTINEL: u32 = u32::MAX;
#[cfg(feature = "matrix-zarr")]
const ZSTD_MIN_LEVEL: i32 = -131_072;
#[cfg(feature = "matrix-zarr")]
const ZSTD_MAX_LEVEL: i32 = 22;

/// Computes a matrix of travel times between
/// all points in the input set in parallel.
///
/// Parameters
/// ----------
/// `transit_model` : `TransitModel`
///     The transit model to use for routing.
/// points : list[`TransitPoint`]
///     List of points between which to calculate travel times.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int
///     Maximum number of transfers allowed in route planning.
///
/// Returns
/// -------
/// list[list[Optional[int]]]
///     A 2D matrix where each cell [i][j] contains the travel time in seconds
///     from point i to point j, or None if the point is unreachable.
#[stubgen]
#[pyfunction]
pub fn travel_time_matrix(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    points: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
) -> PyResult<Vec<Vec<Option<u32>>>> {
    // Perform the routing
    let points: Vec<_> = points.into_iter().map(|p| p.inner).collect();
    let full_vec = py.detach(|| {
        points
            .par_iter()
            .map(|start_point| {
                match multimodal_routing_one_to_many(
                    &transit_model.model,
                    start_point,
                    &points,
                    departure_time,
                    max_transfers,
                ) {
                    Ok(result) => result,
                    Err(e) => {
                        println!("Routing failed for point {start_point:?}, error: {e}");
                        vec![None; points.len()]
                    }
                }
            })
            .map(|vector| {
                vector
                    .into_iter()
                    .map(|result| result.map(|dict| dict.travel_time))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });

    Ok(full_vec)
}

/// Computes a travel-time matrix and stores it directly
/// to a Zarr v3 array without keeping the full matrix in RAM.
///
/// Parameters
/// ----------
/// `transit_model` : `TransitModel`
///     The transit model to use for routing.
/// points : list[`TransitPoint`]
///     List of points between which to calculate travel times.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int
///     Maximum number of transfers allowed in route planning.
/// output_path : str
///     Path to target Zarr store directory (must not already exist).
/// `chunk_rows` : int, default = 512
///     Number of origin rows calculated and written per batch.
/// `chunk_cols` : int, default = 512
///     Chunk width (destination dimension) used in Zarr metadata.
/// `compression_level` : int, default = 5
///     Zstd compression level in range [-131072, 22].
///
/// Returns
/// -------
/// str
///     The `output_path` where matrix data was written.
///
/// Notes
/// -----
/// The matrix is stored as dense `uint32` with `4294967295`
/// (`u32::MAX`) denoting unreachable destinations.
#[cfg(feature = "matrix-zarr")]
#[stubgen]
#[pyfunction]
#[pyo3(signature = (transit_model, points, departure_time, max_transfers, output_path, chunk_rows=512, chunk_cols=512, compression_level=5))]
pub fn travel_time_matrix_zarr(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    points: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
    output_path: &str,
    chunk_rows: usize,
    chunk_cols: usize,
    compression_level: i32,
) -> PyResult<String> {
    if chunk_rows == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "chunk_rows must be greater than 0",
        ));
    }
    if chunk_cols == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "chunk_cols must be greater than 0",
        ));
    }
    if !(ZSTD_MIN_LEVEL..=ZSTD_MAX_LEVEL).contains(&compression_level) {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "compression_level must be in [{ZSTD_MIN_LEVEL}, {ZSTD_MAX_LEVEL}]"
        )));
    }
    if std::path::Path::new(output_path).exists() {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "output_path already exists and will not be overwritten: {output_path}"
        )));
    }

    let output_path = output_path.to_string();
    let points: Vec<_> = points.into_iter().map(|p| p.inner).collect();

    let result = py.detach(|| -> Result<String, String> {
        let output_pathbuf = std::path::PathBuf::from(&output_path);
        if output_pathbuf.exists() {
            return Err(format!(
                "Output path already exists and will not be overwritten: {}",
                output_pathbuf.display()
            ));
        }

        let point_count = points.len();
        let shape = vec![point_count as u64, point_count as u64];
        let chunks = vec![chunk_rows as u64, chunk_cols as u64];

        let store: zarrs::storage::ReadableWritableListableStorage = Arc::new(
            zarrs::filesystem::FilesystemStore::new(&output_pathbuf)
                .map_err(|e| format!("Failed to create Zarr filesystem store: {e}"))?,
        );

        zarrs::group::GroupBuilder::new()
            .build(store.clone(), "/")
            .map_err(|e| format!("Failed to create root Zarr group: {e}"))?
            .store_metadata()
            .map_err(|e| format!("Failed to write root group metadata: {e}"))?;

        let mut attributes = serde_json::Map::new();
        attributes.insert(
            "unreachable_sentinel".to_string(),
            serde_json::Value::from(ZARR_UNREACHABLE_SENTINEL),
        );
        attributes.insert(
            "departure_time".to_string(),
            serde_json::Value::from(departure_time),
        );
        attributes.insert(
            "max_transfers".to_string(),
            serde_json::Value::from(max_transfers),
        );
        attributes.insert(
            "point_count".to_string(),
            serde_json::Value::from(point_count),
        );

        let array = zarrs::array::ArrayBuilder::new(
            shape,
            chunks,
            zarrs::array::data_type::uint32(),
            ZARR_UNREACHABLE_SENTINEL,
        )
        .bytes_to_bytes_codecs(vec![Arc::new(zarrs::array::codec::ZstdCodec::new(
            compression_level,
            false,
        ))])
        .attributes(attributes)
        .build(store, "/matrix")
        .map_err(|e| format!("Failed to build Zarr array: {e}"))?;

        array
            .store_metadata()
            .map_err(|e| format!("Failed to store Zarr array metadata: {e}"))?;

        for row_start in (0..point_count).step_by(chunk_rows) {
            let row_end = (row_start + chunk_rows).min(point_count);

            let batch_rows = points[row_start..row_end]
                .par_iter()
                .map(|start_point| {
                    match multimodal_routing_one_to_many(
                        &transit_model.model,
                        start_point,
                        &points,
                        departure_time,
                        max_transfers,
                    ) {
                        Ok(result) => result
                            .into_iter()
                            .map(|destination| {
                                destination
                                    .map(|dict| dict.travel_time)
                                    .unwrap_or(ZARR_UNREACHABLE_SENTINEL)
                            })
                            .collect::<Vec<_>>(),
                        Err(e) => {
                            println!("Routing failed for point {start_point:?}, error: {e}");
                            vec![ZARR_UNREACHABLE_SENTINEL; point_count]
                        }
                    }
                })
                .collect::<Vec<_>>();

            let mut batch_values = Vec::with_capacity((row_end - row_start) * point_count);
            for row in batch_rows {
                batch_values.extend(row);
            }

            let row_start_u64 = row_start as u64;
            let row_end_u64 = row_end as u64;
            let point_count_u64 = point_count as u64;
            array
                .store_array_subset(
                    &[row_start_u64..row_end_u64, 0..point_count_u64],
                    &batch_values,
                )
                .map_err(|e| {
                    format!("Failed to write rows [{row_start}, {row_end}) to Zarr array: {e}")
                })?;
        }

        Ok(output_path)
    });

    result.map_err(pyo3::exceptions::PyRuntimeError::new_err)
}

/// Computes travel time statistics from each point to all targets in parallel.
///
/// For each origin point, returns the travel time statistic to reachable targets
/// only if at least the specified percentage of all targets are reachable;
/// otherwise returns None.
///
/// Parameters
/// ----------
/// `transit_model` : `TransitModel`
///     The transit model to use for routing.
/// points : list[`TransitPoint`]
///     List of points used as both origins and targets.
/// `departure_time` : int
///     Time of departure in seconds since midnight.
/// `max_transfers` : int
///     Maximum number of transfers allowed in route planning.
/// `threshold` : float, default = 0.75
///     Percentage of target points which must be reached to allow statistics.
/// `stat` : str, default = "mean"
///     Statistic computed from reachable travel times:
///
///     • `"mean"`   — arithmetic mean travel time
///     • `"median"` — median travel time
/// `filter_cutoff` : Optional[int], default = None
///     If provided, excludes destinations with travel time strictly greater than
///     this cutoff (in seconds) from the statistic computation.
///
/// Returns
/// -------
/// list[Optional[float]]
///     A list where each element corresponds to an origin point and contains
///     the computed travel time statistic in seconds, or None if fewer than the
///     specified percentage of targets are reachable from that origin.
#[allow(clippy::too_many_arguments)]
#[stubgen]
#[pyfunction]
#[pyo3(signature = (transit_model, points, departure_time, max_transfers, threshold=0.75, stat="mean", filter_cutoff=None))]
pub fn travel_time_statistics(
    py: Python<'_>,
    transit_model: &PyTransitModel,
    points: Vec<PyTransitPoint>,
    departure_time: Time,
    max_transfers: usize,
    threshold: f64,
    stat: &str, // "mean" or "median"
    filter_cutoff: Option<u64>,
) -> PyResult<Vec<Option<f64>>> {
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "threshold must be a finite number in [0.0, 1.0]",
        ));
    }

    if stat != "mean" && stat != "median" {
        return Err(pyo3::exceptions::PyValueError::new_err(
            r#"stat must be "mean" or "median""#,
        ));
    }

    let points: Vec<_> = points.into_iter().map(|p| p.inner).collect();
    let target_count = points.len();

    if target_count == 0 {
        return Ok(vec![]);
    }

    let results = py.detach(|| {
        points
            .par_iter()
            .map(|start_point| {
                let routing_result = multimodal_routing_one_to_many(
                    &transit_model.model,
                    start_point,
                    &points,
                    departure_time,
                    max_transfers,
                )
                .map_err(|e| format!("Routing failed for point {start_point:?}, error: {e}"))?;

                let mut reached_times: Vec<u64> = Vec::with_capacity(routing_result.len());
                for destination in routing_result.into_iter().flatten() {
                    if let Some(cutoff) = filter_cutoff
                        && u64::from(destination.travel_time) > cutoff
                    {
                        continue;
                    }
                    reached_times.push(u64::from(destination.travel_time));
                }

                let reached_count = reached_times.len();
                if reached_count == 0 || (reached_count as f64 / target_count as f64) < threshold {
                    return Ok(None);
                }

                if stat == "mean" {
                    let sum: u64 = reached_times.iter().sum();
                    Ok(Some(sum as f64 / reached_count as f64))
                } else {
                    let mid = reached_count / 2;
                    let (lower, hi, _) = reached_times.select_nth_unstable(mid);

                    if reached_count % 2 == 1 {
                        Ok(Some(*hi as f64))
                    } else {
                        let Some(lo) = lower.iter().max().copied() else {
                            return Err(format!(
                                "Median computation failed for point {start_point:?}: empty lower partition"
                            ));
                        };
                        Ok(Some(f64::midpoint(lo as f64, *hi as f64)))
                    }
                }
            })
            .collect::<Result<Vec<_>, String>>()
    });

    results.map_err(pyo3::exceptions::PyRuntimeError::new_err)
}
