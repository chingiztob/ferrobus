API Documentation
=================

This module provides algorithms for multimodal transit routing, isochrone generation, and travel-time matrix calculations. It includes functions to:

- **Find optimal routes combining walking and public transit** (:func:`find_route`).
- **Compute routes from a single origin to multiple destinations** (:func:`find_routes_one_to_many`).
- **Generate isochrones** to visualize travel-time polygons (:func:`calculate_isochrone`).
- **Calculate travel-time matrices** for multiple points (:func:`travel_time_matrix`).
- **Perform time-range routing** to find journeys across a range of departure times (:func:`py_range_multimodal_routing`).

The module also defines several classes, including:

- :class:`TransitPoint`: A Python wrapper for geographic points connected to the transit network, used as origins or destinations in routing operations.
- :class:`TransitModel`: A unified model integrating street networks and public transit schedules for multimodal routing.
- :class:`RangeRoutingResult`: A result object for time-range multimodal routing.

Examples
--------

.. code-block:: python

   from ferrobus import create_transit_model, find_route, create_transit_point

   # Create a transit model
   model = create_transit_model(
       osm_path="path/to/roads.osm.pbf",
       gtfs_dirs=["path/to/gtfs"],
       max_transfer_time=1800,
       date=None  # Use current date
   )

   # Create transit points
   origin = create_transit_point(lat=59.85, lon=30.22, transit_model=model)
   destination = create_transit_point(lat=59.97, lon=30.50, transit_model=model)

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

API Reference
-------------

Routing Functions
^^^^^^^^^^^^^^^^^

.. autosummary::
    :toctree: generated
    :nosignatures:

    ferrobus.find_route
    ferrobus.find_routes_one_to_many
    ferrobus.pareto_range_multimodal_routing
    ferrobus.range_multimodal_routing
    ferrobus.detailed_journey

Isochrone and Matrix Functions
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autosummary::
    :toctree: generated
    :nosignatures:

    ferrobus.calculate_isochrone
    ferrobus.calculate_bulk_isochrones
    ferrobus.calculate_percent_access_isochrone
    ferrobus.travel_time_matrix

Utility Functions
^^^^^^^^^^^^^^^^^

.. autosummary::
    :toctree: generated
    :nosignatures:

    ferrobus.create_transit_model
    ferrobus.create_transit_point
    ferrobus.create_isochrone_index

Classes
-------

.. autoclass:: ferrobus.TransitModel
    :members:
    :undoc-members:

.. autoclass:: ferrobus.TransitPoint
    :members:
    :undoc-members:

.. autoclass:: ferrobus.RangeRoutingResult
    :members:
    :undoc-members:

.. autoclass:: ferrobus.IsochroneIndex
    :members:
    :undoc-members:
