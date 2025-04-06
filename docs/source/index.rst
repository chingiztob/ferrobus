.. ferrobus documentation master file, created by
   sphinx-quickstart on Sat Apr  5 21:56:30 2025.
   You can adapt this file completely to your liking, but it should at least
   contain the root `toctree` directive.

Ferrobus documentation
=======================


Introduction
------------

Ferrobus is a Python library providing efficient multimodal transit routing capabilities for geospatial analysis workflows. Built with a Rust core, it delivers strong performance while maintaining a straightforward Python interface.

Unlike alternatives such as R5 or OpenTripPlanner, Ferrobus doesn't require Java and installs without external dependencies, making it easier to integrate into existing workflows and use in tightly controlled environments.

Core routing functionality is based on RAPTOR (Round-based Public Transit Optimized Router) algorithm developed by Microsoft Research. For details, see `Microsoft's research paper <https://www.microsoft.com/en-us/research/wp-content/uploads/2012/01/raptor_alenex.pdf>`_.


Functionality
-------------

- **Routing**: Find optimal paths combining walking and public transit.
- **Isochrone generation**: Create travel-time polygons using a h3-based spatial index.
- **Travel time matrices**: Compute travel times between multiple points.
- **Batch processing**: Process multiple routes or isochrones efficiently with native pure-rust multithreading.
- **Time-range routing**: Find journeys across a range of departure times.

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