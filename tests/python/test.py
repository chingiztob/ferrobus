import ferrobus
import pytest


def test_create_transit_point(model):
    """Test creating a transit point at valid coordinates."""
    lat, lon = 56.252619, 93.532134
    point = ferrobus.create_transit_point(lat, lon, model)
    assert point is not None
    assert hasattr(point, "coordinates")


def test_create_transit_point_invalid(model):
    """Test creating a transit point far from the network (should raise)."""
    lat, lon = 0.0, 0.0  # far from any data
    with pytest.raises(Exception):  # noqa: B017
        ferrobus.create_transit_point(lat, lon, model)


def test_calculate_isochrone(model):
    lat, lon = 56.252619, 93.532134
    point = ferrobus.create_transit_point(lat, lon, model)
    area_wkt = "POLYGON ((93.5214700578047 56.2456755664415,93.5382470550049 56.2430525977962,93.5474967302674 56.2626850549929,93.5456467952149 56.2645272958359,93.5295077066535 56.2667236978452,93.5235113654488 56.261480464947,93.5226182933545 56.255775861859,93.5214700578047 56.2456755664415))"  # noqa: E501
    index = ferrobus.create_isochrone_index(model, area_wkt, 8)
    isochrone = ferrobus.calculate_isochrone(
        model, point, departure_time=8 * 3600, max_transfers=2, cutoff=1800, index=index
    )
    assert isinstance(isochrone, str)


def test_travel_time_matrix(model):
    points = [
        ferrobus.create_transit_point(56.252619, 93.532134, model),
        ferrobus.create_transit_point(56.242574, 93.499159, model),
    ]
    matrix = ferrobus.travel_time_matrix(
        model, points, departure_time=8 * 3600, max_transfers=2
    )
    assert isinstance(matrix, list)
    assert len(matrix) == len(points)


def test_find_route(model):
    start = ferrobus.create_transit_point(56.256657, 93.533561, model)
    end = ferrobus.create_transit_point(56.242574, 93.499159, model)
    result = ferrobus.find_route(
        model, start, end, departure_time=43200, max_transfers=2
    )
    assert isinstance(result, dict)
    assert result["travel_time_seconds"] == 1566


def test_find_routes_one_to_many(model):
    start = ferrobus.create_transit_point(56.256657, 93.533561, model)
    ends = [
        ferrobus.create_transit_point(56.242574, 93.499159, model),
        ferrobus.create_transit_point(56.231878, 93.552460, model),
    ]
    results = ferrobus.find_routes_one_to_many(
        model, start, ends, departure_time=43200, max_transfers=2
    )
    assert isinstance(results, list)
    assert len(results) == len(ends)
    for res in results:
        assert res is None or isinstance(res, dict)

    assert results[0]["travel_time_seconds"] == 1524
    assert results[1]["travel_time_seconds"] == 735


def test_transit_model_properties(model):
    assert isinstance(model.stop_count(), int)
    assert model.stop_count() > 0
    assert isinstance(model.route_count(), int)
    assert model.route_count() > 0
    assert isinstance(model.feeds_info(), str)
    assert "feed" in model.feeds_info().lower() or model.feeds_info() != ""

    # __str__ and __repr__ should return strings
    assert isinstance(str(model), str)
    assert isinstance(repr(model), str)


def test_transit_point_properties(model):
    point = ferrobus.create_transit_point(56.252619, 93.532134, model)
    coords = point.coordinates()
    assert isinstance(coords, tuple)
    assert len(coords) == 2
    assert all(isinstance(x, float) for x in coords)
    assert isinstance(point.nearest_stops(), list)

    # __repr__ should return a string
    assert isinstance(repr(point), str)


def test_range_multimodal_routing(model):
    start = ferrobus.create_transit_point(56.256657, 93.533561, model)
    end = ferrobus.create_transit_point(56.242574, 93.499159, model)
    result = ferrobus.range_multimodal_routing(
        model, start, end, (43200, 44000), max_transfers=2
    )
    assert hasattr(result, "median_travel_time")
    assert isinstance(result.median_travel_time(), int)
    assert isinstance(result.travel_times(), list)
    assert isinstance(result.departure_times(), list)
    assert isinstance(result.as_json(), str)


def test_pareto_range_multimodal_routing(model):
    start = ferrobus.create_transit_point(56.256657, 93.533561, model)
    end = ferrobus.create_transit_point(56.242574, 93.499159, model)
    result = ferrobus.pareto_range_multimodal_routing(
        model, start, end, (43200, 44000), max_transfers=2
    )
    assert hasattr(result, "median_travel_time")
    assert isinstance(result.median_travel_time(), int)
    assert isinstance(result.travel_times(), list)
    assert isinstance(result.departure_times(), list)
    assert isinstance(result.as_json(), str)
