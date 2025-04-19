# ruff: noqa: B017

import ferrobus
import pytest


def test_model_creation_valid(osm_path, gtfs_dirs):
    """Test creating a transit model with valid OSM and GTFS data."""
    model = ferrobus.create_transit_model(
        osm_path=osm_path,
        gtfs_dirs=gtfs_dirs,
        date=None,
        max_transfer_time=1200,
    )
    assert model is not None
    assert model.stop_count() == 194
    assert model.route_count() == 18
    assert model.__str__() == "TransitModel with 194 stops, 18 routes and 34860 trips"


def test_model_creation_invalid_osm(gtfs_dirs):
    """Test creating a transit model with an invalid OSM path."""
    with pytest.raises(Exception):
        ferrobus.create_transit_model(
            osm_path="invalid_path.pbf",
            gtfs_dirs=gtfs_dirs,
            date=None,
            max_transfer_time=1200,
        )


def test_model_creation_invalid_gtfs(osm_path):
    """Test creating a transit model with an invalid GTFS directory."""
    with pytest.raises(Exception):
        ferrobus.create_transit_model(
            osm_path=osm_path,
            gtfs_dirs=["/invalid/gtfs/dir"],
            date=None,
            max_transfer_time=1200,
        )
