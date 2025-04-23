import os

import ferrobus
import pytest


@pytest.fixture(scope="session")
def test_data_dir():
    return os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "test-data"))


@pytest.fixture(scope="session")
def osm_path(test_data_dir):
    return os.path.join(test_data_dir, "roads_zhelez.pbf")


@pytest.fixture(scope="session")
def gtfs_dirs(test_data_dir):
    return [os.path.join(test_data_dir, "zhelez")]


@pytest.fixture(scope="session")
def model(osm_path, gtfs_dirs):
    print("osm_path", osm_path)
    print("gtfs_dirs", gtfs_dirs)
    return ferrobus.create_transit_model(
        osm_path=osm_path,
        gtfs_dirs=gtfs_dirs,
        date=None,
        max_transfer_time=600,
    )


@pytest.fixture(scope="session")
def isochrone_index(model):
    area_wkt = "POLYGON ((93.57274857628481 56.18357044999381, 93.57274857628481 56.30437667924404, 93.39795011002934 56.30437667924404, 93.39795011002934 56.18357044999381, 93.57274857628481 56.18357044999381))"  # noqa: E501
    index = ferrobus.create_isochrone_index(model, area_wkt, 10)

    return index
