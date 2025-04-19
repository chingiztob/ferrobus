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
    # Adjust date and max_transfer_time as needed
    print("osm_path", osm_path)
    print("gtfs_dirs", gtfs_dirs)
    return ferrobus.create_transit_model(
        osm_path=osm_path,
        gtfs_dirs=gtfs_dirs,
        date=None,
        max_transfer_time=600,
    )
