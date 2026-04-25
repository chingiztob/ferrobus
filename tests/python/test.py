import json
import math

import pytest

import ferrobus


def test_create_transit_point(model):
    lat, lon = 56.252619, 93.532134
    point = ferrobus.create_transit_point(lat, lon, model)
    assert point is not None
    assert hasattr(point, "coordinates")


def test_create_transit_point_invalid(model):
    lat, lon = 0.0, 0.0  # far from any data
    with pytest.raises(Exception):  # noqa: B017
        ferrobus.create_transit_point(lat, lon, model)


def test_calculate_isochrone(model):
    lat, lon = 56.25788847445582, 93.53960625054688
    point = ferrobus.create_transit_point(lat, lon, model)
    area_wkt = "POLYGON ((93.57274857628481 56.18357044999381, 93.57274857628481 56.30437667924404, 93.39795011002934 56.30437667924404, 93.39795011002934 56.18357044999381, 93.57274857628481 56.18357044999381))"  # noqa: E501
    index = ferrobus.create_isochrone_index(
        transit_model=model, area=area_wkt, cell_resolution=10
    )
    isochrone = ferrobus.calculate_isochrone(
        transit_model=model,
        start_point=point,
        departure_time=43200,
        max_transfers=3,
        cutoff=1200,
        index=index,
    )

    assert isinstance(isochrone, str)
    assert isochrone[0:18] == "MULTIPOLYGON(((93."


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
    assert matrix[0] == [0, 1044]
    assert matrix[1] == [1253, 0]


def test_travel_time_accessibility_levels(model):
    points = [
        ferrobus.create_transit_point(56.252619, 93.532134, model),
        ferrobus.create_transit_point(56.242574, 93.499159, model),
    ]

    result = ferrobus.travel_time_accessibility_levels(
        transit_model=model,
        points=points,
        departure_time=8 * 3600,
        max_transfers=2,
        lau_idx=[1, 2],
        nuts3_idx=[10, 10],
        lau_neighbors={},
        nuts3_neighbors={},
        cutoff_local=0,
        cutoff_regional=1200,
        cutoff_global=1100,
    )

    expected_keys = {
        "accessible_count_local",
        "accessible_count_regional",
        "accessible_count_global",
        "target_count_local",
        "target_count_regional",
        "target_count_global",
        "share_local",
        "share_regional",
        "share_global",
    }
    assert set(result.keys()) == expected_keys

    for key in expected_keys:
        assert isinstance(result[key], list)
        assert len(result[key]) == len(points)

    assert result["target_count_global"] == [2, 2]
    assert result["target_count_local"] == [1, 1]
    assert result["target_count_regional"] == [2, 2]

    assert result["accessible_count_local"] == [1, 1]
    assert result["accessible_count_regional"] == [2, 1]
    assert result["accessible_count_global"] == [2, 1]

    assert result["share_local"] == [1.0, 1.0]
    assert result["share_regional"] == [1.0, 0.5]
    assert result["share_global"] == [1.0, 0.5]

    for acc_key, tgt_key in [
        ("accessible_count_local", "target_count_local"),
        ("accessible_count_regional", "target_count_regional"),
        ("accessible_count_global", "target_count_global"),
    ]:
        share_key = f"share_{acc_key.split('_')[-1]}"
        for idx, (acc, tgt) in enumerate(
            zip(result[acc_key], result[tgt_key], strict=True)
        ):
            assert acc <= tgt
            if tgt == 0:
                assert math.isnan(result[share_key][idx])


def test_find_route(model):
    start_point = ferrobus.create_transit_point(56.256657, 93.533561, model)
    end_point = ferrobus.create_transit_point(56.242574, 93.499159, model)
    result = ferrobus.find_route(
        transit_model=model,
        start_point=start_point,
        end_point=end_point,
        departure_time=43200,
        max_transfers=2,
    )
    assert isinstance(result, dict)
    assert result["travel_time_seconds"] == 1566


def test_find_routes_one_to_many(model):
    start_point = ferrobus.create_transit_point(56.256657, 93.533561, model)
    end_points = [
        ferrobus.create_transit_point(56.242574, 93.499159, model),
        ferrobus.create_transit_point(56.231878, 93.552460, model),
    ]
    results = ferrobus.find_routes_one_to_many(
        transit_model=model,
        start_point=start_point,
        end_points=end_points,
        departure_time=43200,
        max_transfers=2,
    )
    assert isinstance(results, list)
    assert len(results) == len(end_points)
    for res in results:
        assert res is None or isinstance(res, dict)

    assert results[0]["travel_time_seconds"] == 1524
    assert results[1]["travel_time_seconds"] == 735


def test_transit_point_properties(model):
    point = ferrobus.create_transit_point(56.252619, 93.532134, model)
    coords = point.coordinates()
    assert isinstance(coords, tuple)
    assert len(coords) == 2
    assert all(isinstance(x, float) for x in coords)
    assert isinstance(point.nearest_stops(), list)

    assert isinstance(repr(point), str)


def test_range_multimodal_routing(model):
    start_point = ferrobus.create_transit_point(56.256657, 93.533561, model)
    end_point = ferrobus.create_transit_point(56.242574, 93.499159, model)
    result = ferrobus.range_multimodal_routing(
        transit_model=model,
        start_point=start_point,
        end_point=end_point,
        departure_range=(43200, 44000),
        max_transfers=2,
    )

    assert eval(result.__str__()) == {
        "journeys": [
            {
                "travel_time": 809,
                "transfers": 1,
                "walking_time": 52,
                "departure_time": 43957,
                "arrival_time": 44766,
            },
            {
                "travel_time": 1109,
                "transfers": 1,
                "walking_time": 52,
                "departure_time": 43657,
                "arrival_time": 44766,
            },
            {
                "travel_time": 1469,
                "transfers": 1,
                "walking_time": 52,
                "departure_time": 43297,
                "arrival_time": 44766,
            },
        ]
    }


def test_pareto_range_multimodal_routing(model):
    start_point = ferrobus.create_transit_point(56.256657, 93.533561, model)
    end_point = ferrobus.create_transit_point(56.242574, 93.499159, model)
    result = ferrobus.pareto_range_multimodal_routing(
        transit_model=model,
        start_point=start_point,
        end_point=end_point,
        departure_range=(43200, 44000),
        max_transfers=2,
    )

    assert eval(result.__str__()) == {
        "journeys": [
            {
                "travel_time": 809,
                "transfers": 1,
                "walking_time": 52,
                "departure_time": 43957,
                "arrival_time": 44766,
            }
        ]
    }


def test_detailed_journey(model):
    start_point = ferrobus.create_transit_point(
        56.256657,
        93.533561,
        transit_model=model,
    )
    end_point = ferrobus.create_transit_point(56.231878, 93.552460, transit_model=model)

    result = ferrobus.detailed_journey(
        transit_model=model,
        start_point=start_point,
        end_point=end_point,
        departure_time=43200,  # Время отправления (12:00)
        max_transfers=3,
    )

    assert isinstance(result, str)

    geojson = json.loads(result)
    if len(geojson["features"]) == 3:
        access_leg, transit_leg, egress_leg = geojson["features"]

        assert access_leg["properties"] == {
            "arrival_time": 43223,
            "departure_time": 43200,
            "duration": 23,
            "from_name": "",
            "leg_type": "access_walk",
            "to_name": "21",
        }

        assert transit_leg["properties"] == {
            "arrival_time": 43920,
            "departure_time": 43320,
            "duration": 600,
            "from_name": "21",
            "leg_index": 0,
            "leg_type": "transit",
            "route_id": "bus_9",
            "to_name": "74",
            "trip_id": "bus_9_dir0_11_53_winter_weekday",
        }

        assert egress_leg["properties"] == {
            "arrival_time": 43935,
            "departure_time": 43920,
            "duration": 15,
            "from_name": "74",
            "leg_type": "egress_walk",
            "to_name": "",
        }
