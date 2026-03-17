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
        let transit_leg_count = self.transit_journey.as_ref().map_or(0, |j| j.legs.len());
        let mut features = Vec::with_capacity(
            usize::from(self.access_leg.is_some())
                + transit_leg_count
                + usize::from(self.egress_leg.is_some()),
        );

        // Stage 1: access walk.
        if let Some(access) = &self.access_leg {
            features.push(access.to_feature("access_walk")?);
        }

        // Stage 2: transit journey legs.
        if let Some(transit) = &self.transit_journey {
            for (leg_idx, leg) in transit.legs.iter().enumerate() {
                features.push(feature_for_leg(transit_model, leg_idx, leg)?);
            }
        }

        // Stage 3: egress walk.
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

fn feature_for_leg(
    transit_model: &TransitModel,
    leg_idx: usize,
    leg: &JourneyLeg,
) -> Result<Feature, Error> {
    match leg {
        JourneyLeg::Transit {
            route_id,
            trip_id,
            from_stop,
            departure_time,
            to_stop,
            arrival_time,
        } => create_transit_feature(
            &transit_model.transit_data,
            leg_idx,
            *route_id,
            trip_id,
            *from_stop,
            *to_stop,
            *departure_time,
            *arrival_time,
        ),
        JourneyLeg::Transfer {
            from_stop,
            to_stop,
            departure_time,
            arrival_time,
            duration,
        } => create_transfer_feature(
            transit_model,
            leg_idx,
            *from_stop,
            *to_stop,
            *departure_time,
            *arrival_time,
            *duration,
        ),
        JourneyLeg::Waiting { at_stop, duration } => {
            create_waiting_feature(&transit_model.transit_data, *at_stop, *duration)
        }
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
    let geometry = transit_geometry(transit_data, route_id, from_stop, to_stop);

    feature_from_json(json!({
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
    }))
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
    let geometry = transfer_geometry_with_fallback(transit_model, from_stop, to_stop);

    feature_from_json(json!({
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
    }))
}

fn create_waiting_feature(
    transit_data: &PublicTransitData,
    at_stop: RaptorStopId,
    duration: Time,
) -> Result<Feature, Error> {
    let geom = transit_data.transit_stop_location(at_stop);
    let geometry = Geometry::new(GeoJsonValue::from(&geom));

    feature_from_json(json!({
        "type": "Feature",
        "geometry": geometry,
        "properties": {
            "leg_type": "waiting",
            "duration": duration,
            "stop_name": transit_data.transit_stop_name(at_stop).unwrap_or_default(),
        }
    }))
}

#[allow(clippy::needless_range_loop)]
fn transit_geometry(
    transit_data: &PublicTransitData,
    route_id: RouteId,
    from_stop: RaptorStopId,
    to_stop: RaptorStopId,
) -> Geometry {
    let from_loc = transit_data.transit_stop_location(from_stop);
    let to_loc = transit_data.transit_stop_location(to_stop);
    let mut coords: Vec<Coord<f64>> = vec![from_loc.into()];

    // Include intermediate route stops when available for clearer visualization.
    if let Ok(route_stops) = transit_data.get_route_stops(route_id)
        && let (Some(start_idx), Some(end_idx)) = (
            route_stops.iter().position(|&s| s == from_stop),
            route_stops.iter().position(|&s| s == to_stop),
        )
    {
        if start_idx < end_idx {
            for idx in start_idx + 1..end_idx {
                coords.push(transit_data.transit_stop_location(route_stops[idx]).into());
            }
        } else if start_idx > end_idx {
            for idx in ((end_idx + 1)..start_idx).rev() {
                coords.push(transit_data.transit_stop_location(route_stops[idx]).into());
            }
        }
    }

    coords.push(to_loc.into());
    Geometry::new(GeoJsonValue::from(&LineString::new(coords)))
}

fn transfer_geometry_with_fallback(
    transit_model: &TransitModel,
    from_stop: RaptorStopId,
    to_stop: RaptorStopId,
) -> Geometry {
    let transit_data = &transit_model.transit_data;
    let source_stop = &transit_data.stops[from_stop];
    let target_stop = &transit_data.stops[to_stop];
    let source_loc = transit_data.transit_stop_location(from_stop);
    let target_loc = transit_data.transit_stop_location(to_stop);

    let fallback_geometry = || {
        let direct_line = line_string![
            (x: source_loc.x(), y: source_loc.y()),
            (x: target_loc.x(), y: target_loc.y())
        ];
        Geometry::new(GeoJsonValue::from(&direct_line))
    };

    let source_node = transit_model
        .rtree_ref()
        .nearest_neighbor(&source_stop.geometry)
        .map(|n| n.data);
    let target_node = transit_model
        .rtree_ref()
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

            // Snap path ends to stop coordinates for visual continuity.
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

    fallback_geometry()
}

fn feature_from_json(value: serde_json::Value) -> Result<Feature, Error> {
    Feature::from_json_value(value).map_err(|e| Error::GeoJsonError(e.to_string()))
}
