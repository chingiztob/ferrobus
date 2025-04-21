# Ferrobus: Multimodal Transit Routing Library

High-performance multimodal routing library for geospatial analysis workflows. Built with a Rust core and providing a straightforward Python interface.

Unlike alternatives such as R5 or OpenTripPlanner, Ferrobus doesn't require Java and installs without external dependencies, making it easier to integrate into existing workflows and use in tightly controlled environments.

Core routing functionality is based on the RAPTOR (Round-based Public Transit Optimized Router) algorithm developed by Microsoft Research. For details, see [Microsoft's research paper](https://www.microsoft.com/en-us/research/wp-content/uploads/2012/01/raptor_alenex.pdf).

## Features

- **Multimodal Routing**: Find optimal paths combining walking and public transit
- **Detailed Journey Information**: Get complete trip details including transit legs, walking segments, and transfers
- **Isochrone Generation**: Create travel-time polygons to visualize accessibility
- **Travel Time Matrices**: Compute travel times between multiple origin-destination pairs
- **Batch Processing**: Process multiple routes or isochrones efficiently with parallel execution
- **Time-Range Routing**: Find journeys across a range of departure times
- **Pareto-Optimal Routes**: Discover multiple optimal routes with different trade-offs

## Installation

Ferrobus is easily installable from PyPI and supports Windows, Linux, and macOS. Pre-built wheels are available for the following platforms:

- **Windows**: x86 and x86_64
- **macOS**: x86_64 and arm64
- **Linux**: x86_64 and arm64 (including musl-based systems like Alpine Linux and manylinux2014-compliant systems)

Supported Python versions are **CPython 3.8 and later**, including **PyPy >3.8**.

To install Ferrobus, run:

```bash
pip install ferrobus
```

This will download and install the pre-built binaries for your platform. If a pre-built binary is not available, the package will be built from source, requiring Rust to be installed. You can install Rust using [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

For more details, see the [installation guide](https://ferrobus.readthedocs.io/en/latest/installation.html).

## Quick Start

```python
import ferrobus
import time

# Create a transit model from OpenStreetMap and GTFS data
model = ferrobus.create_transit_model("streets.osm.pbf", ["gtfs_data"], None)

# Create origin and destination points
origin = ferrobus.create_transit_point(52.52, 13.40, model)
destination = ferrobus.create_transit_point(52.53, 13.42, model)

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

## License

This package is open source and licensed under the MIT OR Apache-2.0 license. OpenStreetMap's open data license requires that derivative works provide proper attribution. For more details, see the [OpenStreetMap copyright page](https://www.openstreetmap.org/copyright/).
