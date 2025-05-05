.. ferrobus documentation master file, created by
   sphinx-quickstart on Sat Apr  5 21:56:30 2025.
   You can adapt this file completely to your liking, but it should at least
   contain the root `toctree` directive.

Ferrobus |version|
==================

High-performance multimodal routing library for geospatial analysis workflows. Built with a Rust core and providing a straightforward Python interface.

Unlike alternatives such as R5 or OpenTripPlanner, Ferrobus doesn't require Java and installs without external dependencies.

Core routing functionality is based on RAPTOR (Round-based Public Transit Optimized Router) algorithm developed by Microsoft Research. For details, see `Microsoft's research paper <https://www.microsoft.com/en-us/research/wp-content/uploads/2012/01/raptor_alenex.pdf>`_.


Functionality
-------------

- **Multimodal Routing**: Find optimal paths combining walking and public transit
- **Detailed Journey Information**: Get complete trip details including transit legs, walking segments, and transfers
- **Isochrone Generation**: Create travel-time polygons to visualize accessibility
- **Travel Time Matrices**: Compute travel times between multiple origin-destination pairs
- **Batch Processing**: Process multiple routes or isochrones efficiently with parallel execution
- **Time-Range Routing**: Find journeys across a range of departure times
- **Pareto-Optimal Routes**: Discover multiple optimal routes with different trade-offs

.. toctree::
   :maxdepth: 2
   :caption: Contents:

   getting_started
   ferrobus
   demo

License
-------

This package is open source and licensed under the MIT OR Apache-2.0 license.
OpenStreetMap's open data license requires that derivative works provide proper attribution.
For more details, see the `OpenStreetMap copyright page <https://www.openstreetmap.org/copyright/>`_.
