# ruff: noqa: B017

import datetime

import ferrobus
import pytest


def test_model_creation_valid(osm_path, gtfs_dirs):
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


def test_model_creation_with_calendar(osm_path, gtfs_dirs):
    model = ferrobus.create_transit_model(
        osm_path=osm_path,
        gtfs_dirs=gtfs_dirs,
        date=datetime.date(2024, 5, 1),
        max_transfer_time=1200,
    )
    assert model is not None
    assert model.stop_count() == 194
    assert model.route_count() == 17
    assert model.__str__() == "TransitModel with 194 stops, 17 routes and 12235 trips"


def test_model_creation_invalid_osm(gtfs_dirs):
    with pytest.raises(Exception):
        ferrobus.create_transit_model(
            osm_path="invalid_path.pbf",
            gtfs_dirs=gtfs_dirs,
            date=None,
            max_transfer_time=1200,
        )


def test_model_creation_invalid_gtfs(osm_path):
    with pytest.raises(Exception):
        ferrobus.create_transit_model(
            osm_path=osm_path,
            gtfs_dirs=["/invalid/gtfs/dir"],
            date=None,
            max_transfer_time=1200,
        )
