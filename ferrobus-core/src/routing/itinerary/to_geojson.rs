use geo::{Coord, LineString, line_string};
use geojson::{Feature, FeatureCollection, Geometry};
use serde_json::json;

use crate::{
    Error, PublicTransitData, RaptorStopId, TransitModel,
    routing::{dijkstra::dijkstra_paths, raptor::JourneyLeg},
};

use super::DetailedJourney;

impl DetailedJourney {
    /// Converts the complete journey to a `GeoJSON` `FeatureCollection`.
    pub fn to_geojson(&self, transit_model: &TransitModel) -> FeatureCollection {
        let transit_data = &transit_model.transit_data;
        let mut features = Vec::new();

        if let Some(access) = &self.access_leg {
            features.push(access.to_feature("access_walk"));
        }

        if let Some(transit) = &self.transit_journey {
            for (idx, leg) in transit.legs.iter().enumerate() {
                let feature = match leg {
                    JourneyLeg::Transit { .. } => transit_leg_feature(transit_data, leg, idx),
                    JourneyLeg::Transfer { .. } => transfer_leg_feature(transit_model, leg, idx),
                    JourneyLeg::Waiting { .. } => waiting_leg_feature(transit_data, leg),
                };
                features.push(feature);
            }
        }

        if let Some(egress) = &self.egress_leg {
            features.push(egress.to_feature("egress_walk"));
        }

        FeatureCollection {
            features,
            bbox: None,
            foreign_members: None,
        }
    }

    /// Converts the journey to a `GeoJSON` string.
    pub fn to_geojson_string(&self, transit_model: &TransitModel) -> String {
        serde_json::to_string(&self.to_geojson(transit_model)).unwrap_or_default()
    }
}

fn transit_leg_feature(
    transit_data: &PublicTransitData,
    leg: &JourneyLeg,
    leg_idx: usize,
) -> Feature {
    if let JourneyLeg::Transit {
        route_id,
        trip_id,
        from_stop,
        departure_time,
        to_stop,
        arrival_time,
    } = leg
    {
        let from_loc = transit_data.transit_stop_location(*from_stop);
        let to_loc = transit_data.transit_stop_location(*to_stop);

        let from_name = transit_data
            .transit_stop_name(*from_stop)
            .unwrap_or_default();
        let to_name = transit_data.transit_stop_name(*to_stop).unwrap_or_default();

        let mut coords: Vec<Coord<f64>> = vec![from_loc.into()];
        if let Ok(route_stops) = transit_data.get_route_stops(*route_id)
            && let (Some(start_idx), Some(end_idx)) = (
                route_stops.iter().position(|&s| s == *from_stop),
                route_stops.iter().position(|&s| s == *to_stop),
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
        let line: LineString<_> = coords.into();

        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&line).into()),
            "properties": {
                "leg_type": "transit",
                "leg_index": leg_idx,
                "route_id": transit_data.routes[*route_id].route_id,
                "trip_id": trip_id,
                "from_name": from_name,
                "to_name": to_name,
                "departure_time": departure_time,
                "arrival_time": arrival_time,
                "duration": arrival_time - departure_time,
            }
        });

        Feature::from_json_value(value).expect("Failed to create feature from valid JSON")
    } else {
        panic!("Invalid leg type passed to transit_leg_feature");
    }
}

fn transfer_leg_feature(transit_model: &TransitModel, leg: &JourneyLeg, leg_idx: usize) -> Feature {
    if let JourneyLeg::Transfer {
        from_stop,
        departure_time,
        to_stop,
        arrival_time,
        duration,
    } = leg
    {
        let transit_data = &transit_model.transit_data;
        let from_name = transit_data
            .transit_stop_name(*from_stop)
            .unwrap_or_default();
        let to_name = transit_data.transit_stop_name(*to_stop).unwrap_or_default();
        let source_stop = &transit_data.stops[*from_stop];
        let target_stop = &transit_data.stops[*to_stop];

        let rtree = transit_model.rtree_ref();

        let source_street_node = rtree
            .nearest_neighbor(&source_stop.geometry)
            .ok_or(Error::NoPointsFound)
            .unwrap()
            .data;

        let target_street_node = rtree
            .nearest_neighbor(&target_stop.geometry)
            .ok_or(Error::NoPointsFound)
            .unwrap()
            .data;

        let mut paths = dijkstra_paths(
            &transit_model.street_graph,
            source_street_node,
            Some(target_street_node),
            Some(f64::from(transit_model.meta.max_transfer_time)),
        );

        let transfer_geometry = paths.remove(&target_street_node);
        let source_stop_geom = transit_data.transit_stop_location(*from_stop);
        let target_stop_geom = transit_data.transit_stop_location(*to_stop);

        let geometry = match transfer_geometry {
            Some(transfer) if transfer.nodes().len() > 1 => {
                let mut nodes = transfer.into_nodes();
                if let Some(first_node) = nodes.first_mut() {
                    first_node.x = source_stop_geom.x();
                    first_node.y = source_stop_geom.y();
                }

                if let Some(last_node) = nodes.last_mut() {
                    last_node.x = target_stop_geom.x();
                    last_node.y = target_stop_geom.y();
                }

                if nodes
                    .iter()
                    .all(|node| node.x.is_finite() && node.y.is_finite())
                {
                    let linestring = LineString::new(nodes);
                    Geometry::new((&linestring).into())
                } else {
                    create_direct_line_geometry(transit_data, *from_stop, *to_stop)
                }
            }
            _ => create_direct_line_geometry(transit_data, *from_stop, *to_stop),
        };

        let value = json!({
            "type": "Feature",
            "geometry": geometry,
            "properties": {
                "leg_type": "transfer",
                "leg_index": leg_idx,
                "from_name": from_name,
                "to_name": to_name,
                "departure_time": departure_time,
                "arrival_time": arrival_time,
                "duration": duration,
            }
        });

        Feature::from_json_value(value).expect("Failed to create feature from valid JSON")
    } else {
        panic!("Invalid leg type passed to transfer_leg_feature");
    }
}

/// Helper function to create a direct line geometry between stops
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
    Geometry::new((&direct_line).into())
}

fn waiting_leg_feature(transit_data: &PublicTransitData, leg: &JourneyLeg) -> Feature {
    if let JourneyLeg::Waiting { at_stop, duration } = leg {
        let geom = transit_data.transit_stop_location(*at_stop);
        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&geom).into()),
            "properties": {
                "leg_type": "waiting",
                "duration": duration,
            }
        });

        Feature::from_json_value(value).expect("Failed to create feature from valid JSON")
    } else {
        panic!("Invalid leg type passed to waiting_leg_feature");
    }
}
