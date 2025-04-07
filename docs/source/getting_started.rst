Getting Started
===============

Ferrobus is easily installable from PyPI and supports Windows, Linux, and macOS.

Pre-built binaries are available for **Windows (x86_64, x86), macOS (x86_64 and arm64), and Linux (x86_64, arm64)**.
Wheels for Linux are available for musl-based systems (e.g., Alpine Linux) and manylinux2014-compliant systems.

Supported Python versions are **CPython 3.8 and later, including free-threaded 3.13t and PyPy >3.8**

Installation
------------

To install Ferrobus, run:

.. code-block:: bash

   pip install ferrobus

This will install pre-built binaries for all major platforms from PyPI.

Quick Usage
-----------

Here's a simple example to get you started:

.. code-block:: python

   import ferrobus
   import datetime

   # Create a transit model from GTFS and OSM data
   model = ferrobus.create_transit_model(
       osm_path="path/to/city.osm.pbf", # OSM pbf file, preferably filtered by region and tag
       gtfs_dirs=["path/to/gtfs_data", "path/to/another_gtfs"], # feeds, operating in the same region
       date=datetime.date.today() # date to use when filtering GTFS data / None for all dates
   )

   # Create origin and destination points
   origin = ferrobus.create_transit_point(
       latitude=59.85,
       longitude=30.22,
       transit_model=model
   )

   destination = ferrobus.create_transit_point(
       latitude=59.97,
       longitude=30.50,
       transit_model=model
   )

   # Find the optimal route at noon (12:00)
   route = ferrobus.find_route(
       transit_model=model,
       start_point=origin,
       end_point=destination,
       departure_time=12*3600,  # 12:00 noon in seconds since midnight
       max_transfers=3
   )

   # Print the results
   print(f"Travel time: {route['travel_time_seconds'] / 60:.1f} minutes")
   print(f"Number of transfers: {route['num_transfers']}")

For detailed examples, see the :doc:`demo` notebook and the :doc:`ferrobus` API documentation.
