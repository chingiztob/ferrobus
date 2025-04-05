# Ferrobus: Multimodal transit routing library

Ferrobus is a Python library providing efficient multimodal transit routing capabilities for geospatial analysis workflows. Built with a Rust core, it delivers strong performance while maintaining a straightforward Python interface.

Unlike alternatives such as R5 or OpenTripPlanner, Ferrobus doesn't require Java and installs without external dependencies, making it easier to integrate into existing workflows and use in tightly controlled environments.

Core routing functionality is based on RAPTOR (Round-based Public Transit Optimized Router) algotihm developed
by Microsoft Research. For details, see [Microsoft's research paper](https://www.microsoft.com/en-us/research/wp-content/uploads/2012/01/raptor_alenex.pdf).

## Functionality

- **Routing**: Find optimal paths combining walking and public transit based on the RAPTOR
- **Isochrone generation**: Create travel-time polygons using a hexagonal grid system
- **Travel time matrices**: Compute travel times between multiple points
- **Batch processing**: Process multiple routes or isochrones efficiently with parallel processing
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
