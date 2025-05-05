# Ferrobus: Multimodal Transit Routing Library

[![CodSpeed Badge](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/chingiztob/ferrobus)

> [!NOTE]
> This project is still in its early stages, and the API is subject to changes. Please check the [documentation](https://ferrobus.readthedocs.io/) for the latest updates.

High-performance multimodal routing library for geospatial analysis workflows. Built with a Rust core and providing a straightforward Python interface. It aims to be orders of magnitude faster than existing tools.

Unlike alternatives such as R5 or OpenTripPlanner, Ferrobus doesn't require Java and installs without external dependencies.

Core routing functionality is based on the RAPTOR (Round-based Public Transit Optimized Router) algorithm developed by Microsoft Research. For details, see [Microsoft's research paper](https://www.microsoft.com/en-us/research/wp-content/uploads/2012/01/raptor_alenex.pdf).

## Features

- **Multimodal Routing**: Find optimal paths combining walking and public transit
- **Isochrones**: Fast and uncertainty-aware
- **Travel Time Matrices**: Compute travel times between multiple origin-destination pairs
- **Batch Processing**: Efficient native multithreading
- **Time-Range Routing**: Find journeys across a range of departure times with rRAPTOR
- **Detailed Journey Information**: Get complete trip details *(in progress ...)*
- **Pareto-Optimal Routes**: Discover multiple optimal routes with different trade-offs *(in progress ...)*

## Installation

To install Ferrobus, run:

```bash
pip install ferrobus
```

Pre-built wheels are available for the following platforms:

- **Windows**: x86 and x86_64
- **macOS**: x86_64 and arm64
- **Linux**: x86_64 and arm64 (including musl-based systems like Alpine Linux and manylinux2014-compliant systems)

Supported Python versions are **CPython 3.8 and later**, including **PyPy >3.8**.
If a pre-built binary is not available, the package will be built from source, requiring Rust to be installed. You can install Rust using [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Quick Start

```python
import ferrobus
import time

# Create a transit model from OpenStreetMap and GTFS data
model = ferrobus.create_transit_model(
    osm_path="path/to/city.osm.pbf", # OSM pbf file, preferably filtered by region and tag
    gtfs_dirs=["path/to/gtfs_data", "path/to/another_gtfs"], # feeds, operating in the same region
    date=datetime.date.today() # date to use when filtering GTFS data / None for all dates
)

# Initializes and pre-calculates transit points for efficient geographic operations.
#
# Transit points represent specific geographic locations and should be used as inputs for all route calculations and related
# operations. By creating and reusing these pre-initialized points, the system avoids redundant computations of geographic
# positions, resulting in significantly improved performance for routing and spatial queries.
origin = ferrobus.create_transit_point(52.52, 13.40, model)
destination = ferrobus.create_transit_point(52.53, 13.42, model)_

# Find route (departure at noon)
departure_time = 12 * 3600  # 12:00 noon in seconds since midnight
start_time = time.perf_counter()
route = ferrobus.find_route(
    start_point=origin,
    end_point=destination,
    departure_time=departure_time,
    max_transfers=3  # Allow up to 3 transfers
)
end_time = time.perf_counter()

# Display route information
print(f"Route found in {end_time - start_time:.3f} seconds")
print(f"Travel time: {route['travel_time_seconds'] / 60:.1f} minutes")
print(f"Transit time: {route['transit_time_seconds'] / 60:.1f} minutes")
print(f"Walking time: {route['walking_time_seconds'] / 60:.1f} minutes")
print(f"Number of transfers: {route['transfers']}")
```

## Advanced Features

### Detailed Journey Visualization

```python
# Get detailed journey information with all legs (walking, transit)
journey = ferrobus.detailed_journey(
    transit_model=model,
    start_point=origin,
    end_point=destination,
    departure_time=departure_time,
    max_transfers=3
)
```

### Travel Time Matrix

```python
# Calculate travel times between multiple points
points = [origin, destination, point3, point4]
matrix = ferrobus.travel_time_matrix(
    transit_model=model,
    points=points,
    departure_time=departure_time,
    max_transfers=3
)
```

### Isochrones

```python
# Create an isochrone index for a specific area
index = ferrobus.create_isochrone_index(model, area_wkt, 8)

# Calculate isochrone (areas reachable within 30 minutes)
isochrone = ferrobus.calculate_isochrone(
    transit_model=model,
    origin=origin,
    departure_time=departure_time,
    max_transfers=2,
    max_travel_time=1800,  # 30 minutes in seconds
    isochrone_index=index
)
```

## Documentation

For more detailed information, see the [full rendered documentation](https://ferrobus.readthedocs.io/):

## Benchmarks

Ferrobus is designed for high performance and low memory footprint. Benchmarks are continuously run using [CodSpeed](https://codspeed.io/chingiztob/ferrobus) to avoid regressions. Typical routing queries (including multimodal and batch operations) complete in milliseconds on modern hardware.

### Comparative Benchmarks

*to be added ...*

## License

This package is open source and licensed under the MIT OR Apache-2.0 license. OpenStreetMap's open data license requires that derivative works provide proper attribution. For more details, see the [OpenStreetMap copyright page](https://www.openstreetmap.org/copyright/).
