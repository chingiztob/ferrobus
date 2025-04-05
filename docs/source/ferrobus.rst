API Documentation
=================

This module provides algorithms for finding shortest paths in time-dependent transit graphs. It includes functions to:

- **Compute the shortest paths from a source node to all other nodes** using Dijkstra's algorithm (:func:`single_source_shortest_path`).
- **Find the shortest path weight** between a source and target node (:func:`shortest_path_weight`).
- **Retrieve the actual shortest path** between a source and target node as a sequence of node indices (:func:`shortest_path`).
- **Calculate an origin-destination (OD) matrix** for a set of points, providing the shortest path weights between all pairs of points (:func:`calculate_od_matrix`).

The module also defines a :class:`TransitPoint` class, a Python wrapper for passing coordinates with an ID to the Rust backend, facilitating seamless integration between Rust and Python components.

Examples
--------

.. code-block:: python

   from ferrobus import create_transit_model, find_route, create_transit_point

   # Create a transit model
   model = create_transit_model(
       max_transfer_time=1800,
       osm_path="path/to/roads.osm.pbf",
       gtfs_dirs=["path/to/gtfs"],
       date=None  # Use current date
   )

   # Create transit points
   origin = create_transit_point(latitude=59.85, longitude=30.22, transit_model=model)
   destination = create_transit_point(latitude=59.97, longitude=30.50, transit_model=model)

   # Find the optimal route
   route = find_route(
       transit_model=model,
       start_point=origin,
       end_point=destination,
       departure_time=43200,  # 12:00 noon in seconds
       max_transfers=3
   )

   print(f"Travel time: {route['travel_time_seconds'] / 60:.1f} minutes")
   print(f"Number of transfers: {route['num_transfers']}")

.. automodule:: ferrobus
    :members:
    :undoc-members:
    :show-inheritance:
