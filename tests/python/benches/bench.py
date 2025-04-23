import datetime

import ferrobus
import pytest


@pytest.mark.benchmark
def test_model_creation_performance(osm_path, gtfs_dirs):
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


def test_find_routes_one_to_many(model, benchmark):
    start = ferrobus.create_transit_point(56.256657, 93.533561, model)
    ends = [
        ferrobus.create_transit_point(56.242574, 93.499159, model),
        ferrobus.create_transit_point(56.231878, 93.552460, model),
    ]

    @benchmark
    def bench():
        results = ferrobus.find_routes_one_to_many(
            model, start, ends, departure_time=43200, max_transfers=2
        )
        assert isinstance(results, list)
        assert len(results) == len(ends)
        for res in results:
            assert res is None or isinstance(res, dict)

        assert results[0]["travel_time_seconds"] == 1524
        assert results[1]["travel_time_seconds"] == 735


def test_calculate_isochrone_performance(model, isochrone_index, benchmark):
    lat, lon = 56.25788847445582, 93.53960625054688
    point = ferrobus.create_transit_point(lat, lon, model)

    @benchmark
    def bench():
        isochrone = ferrobus.calculate_isochrone(
            model,
            point,
            departure_time=43200,
            max_transfers=3,
            cutoff=1200,
            index=isochrone_index,
        )

        assert isinstance(isochrone, str)
        assert isochrone[0:18] == "MULTIPOLYGON(((93."
