use geo::{Coord, LineString, line_string};
use geojson::{Feature, FeatureCollection, Geometry, Value as GeoJsonValue};
use serde_json::json;

use crate::{
    Error, PublicTransitData, RaptorStopId, TransitModel,
    routing::{dijkstra::dijkstra_paths, raptor::JourneyLeg},
    types::{RouteId, Time},
};

use super::DetailedJourney;

impl DetailedJourney {
    /// Converts the complete journey to a `GeoJSON` `FeatureCollection`.
    pub fn to_geojson(&self, transit_model: &TransitModel) -> Result<FeatureCollection, Error> {
        let mut features = Vec::new();

        if let Some(access) = &self.access_leg {
            features.push(access.to_feature("access_walk")?);
        }

        if let Some(transit) = &self.transit_journey {
            for (idx, leg) in transit.legs.iter().enumerate() {
                let feature = match leg {
                    JourneyLeg::Transit {
                        route_id,
                        trip_id,
                        from_stop,
                        departure_time,
                        to_stop,
                        arrival_time,
                    } => create_transit_feature(
                        &transit_model.transit_data,
                        idx,
                        *route_id,
                        trip_id,
                        *from_stop,
                        *to_stop,
                        *departure_time,
                        *arrival_time,
                    )?,
                    JourneyLeg::Transfer {
                        from_stop,
                        to_stop,
                        departure_time,
                        arrival_time,
                        duration,
                    } => create_transfer_feature(
                        transit_model,
                        idx,
                        *from_stop,
                        *to_stop,
                        *departure_time,
                        *arrival_time,
                        *duration,
                    )?,
                    JourneyLeg::Waiting { at_stop, duration } => {
                        create_waiting_feature(&transit_model.transit_data, *at_stop, *duration)?
                    }
                };
                features.push(feature);
            }
        }

        if let Some(egress) = &self.egress_leg {
            features.push(egress.to_feature("egress_walk")?);
        }

        Ok(FeatureCollection {
            features,
            bbox: None,
            foreign_members: None,
        })
    }

    pub fn to_geojson_string(&self, transit_model: &TransitModel) -> Result<String, Error> {
        serde_json::to_string(&self.to_geojson(transit_model)?)
            .map_err(|e| Error::GeoJsonError(e.to_string()))
    }
}

#[allow(clippy::too_many_arguments)]
fn create_transit_feature(
    transit_data: &PublicTransitData,
    leg_idx: usize,
    route_id: RouteId,
    trip_id: &str,
    from_stop: RaptorStopId,
    to_stop: RaptorStopId,
    departure_time: Time,
    arrival_time: Time,
) -> Result<Feature, Error> {
    let from_loc = transit_data.transit_stop_location(from_stop);
    let to_loc = transit_data.transit_stop_location(to_stop);

    let mut coords: Vec<Coord<f64>> = vec![from_loc.into()];

    // Attempt to fill in intermediate stops for better visualization
    if let Ok(route_stops) = transit_data.get_route_stops(route_id)
        && let (Some(start_idx), Some(end_idx)) = (
            route_stops.iter().position(|&s| s == from_stop),
            route_stops.iter().position(|&s| s == to_stop),
        )
    {
        let range: Vec<_> = if start_idx < end_idx {
            (start_idx + 1..end_idx).collect()
        } else {
            (end_idx + 1..start_idx).rev().collect()
        };
        for idx in range {
            let stop_loc = transit_data.transit_stop_location(route_stops[idx]);
            coords.push(stop_loc.into());
        }
    }
    coords.push(to_loc.into());

    let geometry = Geometry::new(GeoJsonValue::from(&LineString::new(coords)));

    let value = json!({
        "type": "Feature",
        "geometry": geometry,
        "properties": {
            "leg_type": "transit",
            "leg_index": leg_idx,
            "route_id": transit_data.routes[route_id].route_id,
            "trip_id": trip_id,
            "from_name": transit_data.transit_stop_name(from_stop).unwrap_or_default(),
            "to_name": transit_data.transit_stop_name(to_stop).unwrap_or_default(),
            "departure_time": departure_time,
            "arrival_time": arrival_time,
            "duration": arrival_time.saturating_sub(departure_time),
        }
    });

    Feature::from_json_value(value).map_err(|e| Error::GeoJsonError(e.to_string()))
}

fn create_transfer_feature(
    transit_model: &TransitModel,
    leg_idx: usize,
    from_stop: RaptorStopId,
    to_stop: RaptorStopId,
    departure_time: Time,
    arrival_time: Time,
    duration: u32,
) -> Result<Feature, Error> {
    let transit_data = &transit_model.transit_data;

    let geometry = calculate_transfer_geometry(transit_model, from_stop, to_stop);

    let value = json!({
        "type": "Feature",
        "geometry": geometry,
        "properties": {
            "leg_type": "transfer",
            "leg_index": leg_idx,
            "from_name": transit_data.transit_stop_name(from_stop).unwrap_or_default(),
            "to_name": transit_data.transit_stop_name(to_stop).unwrap_or_default(),
            "departure_time": departure_time,
            "arrival_time": arrival_time,
            "duration": duration,
        }
    });

    Feature::from_json_value(value).map_err(|e| Error::GeoJsonError(e.to_string()))
}

fn calculate_transfer_geometry(
    transit_model: &TransitModel,
    from_stop: RaptorStopId,
    to_stop: RaptorStopId,
) -> Geometry {
    let transit_data = &transit_model.transit_data;
    let source_stop = &transit_data.stops[from_stop];
    let target_stop = &transit_data.stops[to_stop];
    let rtree = transit_model.rtree_ref();

    let source_node = rtree
        .nearest_neighbor(&source_stop.geometry)
        .map(|n| n.data);
    let target_node = rtree
        .nearest_neighbor(&target_stop.geometry)
        .map(|n| n.data);

    if let (Some(source_street_node), Some(target_street_node)) = (source_node, target_node) {
        let mut paths = dijkstra_paths(
            &transit_model.street_graph,
            source_street_node,
            Some(target_street_node),
            Some(f64::from(transit_model.meta.max_transfer_time)),
        );

        if let Some(transfer) = paths.remove(&target_street_node)
            && transfer.nodes().len() > 1
        {
            let mut nodes = transfer.into_nodes();

            // Snap the first and last nodes to the exact stop locations for visual continuity
            let source_loc = transit_data.transit_stop_location(from_stop);
            let target_loc = transit_data.transit_stop_location(to_stop);

            if let Some(first) = nodes.first_mut() {
                first.x = source_loc.x();
                first.y = source_loc.y();
            }
            if let Some(last) = nodes.last_mut() {
                last.x = target_loc.x();
                last.y = target_loc.y();
            }

            if nodes.iter().all(|n| n.x.is_finite() && n.y.is_finite()) {
                return Geometry::new(GeoJsonValue::from(&LineString::new(nodes)));
            }
        }
    }

    // Fallback if no path found or nodes invalid
    create_direct_line_geometry(transit_data, from_stop, to_stop)
}

fn create_waiting_feature(
    transit_data: &PublicTransitData,
    at_stop: RaptorStopId,
    duration: Time,
) -> Result<Feature, Error> {
    let geom = transit_data.transit_stop_location(at_stop);
    let geometry = Geometry::new(GeoJsonValue::from(&geom));

    let value = json!({
        "type": "Feature",
        "geometry": geometry,
        "properties": {
            "leg_type": "waiting",
            "duration": duration,
            "stop_name": transit_data.transit_stop_name(at_stop).unwrap_or_default(),
        }
    });

    Feature::from_json_value(value).map_err(|e| Error::GeoJsonError(e.to_string()))
}

fn create_direct_line_geometry(
    transit_data: &PublicTransitData,
    from_stop: RaptorStopId,
    to_stop: RaptorStopId,
) -> Geometry {
    let from_loc = transit_data.transit_stop_location(from_stop);
    let to_loc = transit_data.transit_stop_location(to_stop);
    let direct_line = line_string![
        (x: from_loc.x(), y: from_loc.y()),
        (x: to_loc.x(), y: to_loc.y())
    ];
    Geometry::new(GeoJsonValue::from(&direct_line))
}
