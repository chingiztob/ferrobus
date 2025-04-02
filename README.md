# Ferrobus: Multimodal transit routing library

Ferrobus is a fast Python library that provides multimodal
transit routing capabilities, designed for geospatial analysis workflows.

Ferrobus is fully implemented in Rust, which makes it extremely fast.
This also allows for zero-dependency installation, and
unlike R5 or OpenTripPlanner, Ferrobus does not require Java.

## Functionality

- **Routing**: Find optimal paths combining walking and public transit
- **Isochrone generation**: Create travel-time polygons using a hexagonal grid system
- **Travel time matrices**: Compute travel times between multiple points
- **Batch processing**: Process multiple routes or isochrones efficiently
- **Time-range routing**: Find journeys across a range of departure times

```python
# Create a transit model
model = ferrobus.create_transit_model("streets.osm.pbf", ["gtfs_data"], None)

# Create transit points
origin = ferrobus.create_transit_point(52.52, 13.40, model)
destination = ferrobus.create_transit_point(52.53, 13.42, model)

# Find route
departure_time = 8 * 3600  # 8:00 AM in seconds since midnight
route = ferrobus.find_route(model, origin, destination, departure_time)

# Generate an isochrone
index = ferrobus.create_isochrone_index(model, area_wkt, 8)
isochrone = ferrobus.calculate_isochrone(model, origin, departure_time, 2, 1800, index)

# Calculate a travel time matrix
points = [origin, destination, point3, point4]
matrix = ferrobus.travel_time_matrix(model, points, departure_time, 3)
```

## Implementation details

The library uses several techniques to enable efficient routing:

- Parallel processing for batch operations
- Spatial data structures for network representation
- Pre-computation of access paths to transit stops
- RAPTOR algorithm for transit routing
