use geo::{LineString, Point, line_string};
use geojson::{Feature, FeatureCollection, Geometry};
use serde_json::{Map, Value as JsonValue, json};

use crate::{
    MAX_CANDIDATE_STOPS, TransitModel,
    routing::{
        multimodal_routing::{TransitCandidate, is_walking_better},
        raptor::{TracedRaptorResult, traced_raptor},
    },
};

use crate::{
    Error, PublicTransitData, RaptorStopId, Time, model::TransitPoint, routing::raptor::Journey,
};

/// Represents a walking leg outside the transit network
#[derive(Debug, Clone)]
pub struct WalkingLeg {
    pub from_location: Point<f64>,
    pub to_location: Point<f64>,
    pub from_name: String,
    pub to_name: String,
    pub departure_time: Time,
    pub arrival_time: Time,
    pub duration: Time,
}

impl WalkingLeg {
    /// Convert a walking leg to a `GeoJSON` Feature
    fn to_feature(&self, leg_type: &str) -> Feature {
        let leg = self;
        // Create a LineString for the walking path

        let coordinates = line_string![
            (x: leg.from_location.x(), y: leg.from_location.y()),
            (x: leg.to_location.x(), y: leg.to_location.y()),
        ];

        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&coordinates).into()),
            "properties": {
                "leg_type": leg_type,
                "from_name": leg.from_name,
                "to_name": leg.to_name,
                "departure_time": leg.departure_time,
                "arrival_time": leg.arrival_time,
                "duration": leg.duration,
            }
        });

        Feature::from_json_value(value).unwrap()
    }
}

/// Represents a complete journey with first/last mile connections
#[derive(Debug, Clone)]
pub struct DetailedJourney {
    pub access_leg: Option<WalkingLeg>,
    pub transit_journey: Option<Journey>,
    pub egress_leg: Option<WalkingLeg>,
    pub total_time: Time,
    pub walking_time: Time,
    pub transit_time: Option<Time>,
    pub transfers: usize,
    pub departure_time: Time,
    pub arrival_time: Time,
}

impl DetailedJourney {
    /// Creates a walking-only journey
    pub fn walking_only(
        start: &TransitPoint,
        end: &TransitPoint,
        departure_time: Time,
        walking_time: Time,
    ) -> Self {
        let arrival_time = departure_time + walking_time;

        let walking_leg = WalkingLeg {
            from_location: start.geometry,
            to_location: end.geometry,
            from_name: String::new(),
            to_name: String::new(),
            departure_time,
            arrival_time,
            duration: walking_time,
        };

        Self {
            access_leg: Some(walking_leg),
            transit_journey: None,
            egress_leg: None,
            total_time: walking_time,
            walking_time,
            transit_time: None,
            transfers: 0,
            departure_time,
            arrival_time,
        }
    }

    /// Creates a multimodal journey with transit
    #[allow(clippy::too_many_arguments)]
    fn with_transit(
        start: &TransitPoint,
        end: &TransitPoint,
        transit_data: &PublicTransitData,
        access_stop: RaptorStopId,
        egress_stop: RaptorStopId,
        access_time: Time,
        egress_time: Time,
        transit_journey: Journey,
        departure_time: Time,
    ) -> Self {
        let transit_departure = departure_time + access_time;
        let transit_arrival = transit_journey.arrival_time;
        let final_arrival = transit_arrival + egress_time;

        // Get stop locations and names from transit data
        let access_stop_location = transit_data.stops[access_stop].geometry;
        let egress_stop_location = transit_data.stops[egress_stop].geometry;
        let access_stop_name = transit_data.stops[access_stop].stop_id.clone();
        let egress_stop_name = transit_data.stops[egress_stop].stop_id.clone();

        // Create access walking leg
        let access_leg = WalkingLeg {
            from_location: start.geometry,
            to_location: access_stop_location,
            from_name: String::new(),
            to_name: access_stop_name,
            departure_time,
            arrival_time: transit_departure,
            duration: access_time,
        };

        // Create egress walking leg
        let egress_leg = WalkingLeg {
            from_location: egress_stop_location,
            to_location: end.geometry,
            from_name: egress_stop_name,
            to_name: String::new(),
            departure_time: transit_arrival,
            arrival_time: final_arrival,
            duration: egress_time,
        };

        let transit_time = transit_arrival - transit_departure;
        let walking_time = access_time + egress_time;
        let total_time = final_arrival - departure_time;
        let transfer_count = transit_journey.transfers_count;

        Self {
            access_leg: Some(access_leg),
            transit_journey: Some(transit_journey),
            egress_leg: Some(egress_leg),
            total_time,
            walking_time,
            transit_time: Some(transit_time),
            transfers: transfer_count,
            departure_time,
            arrival_time: final_arrival,
        }
    }
}

impl DetailedJourney {
    /// Convert the journey to a `GeoJSON` `FeatureCollection`
    pub fn to_geojson(&self, transit_data: &PublicTransitData) -> FeatureCollection {
        let mut features = Vec::new();

        // Process access leg (walking to transit)
        if let Some(access) = &self.access_leg {
            features.push(access.to_feature("access_walk"));
        }

        // Process transit journey
        if let Some(transit) = &self.transit_journey {
            for (idx, leg) in transit.legs.iter().enumerate() {
                match leg {
                    crate::routing::raptor::JourneyLeg::Transit {
                        route_id,
                        trip_id,
                        from_stop,
                        to_stop,
                        departure_time,
                        arrival_time,
                    } => {
                        features.push(DetailedJourney::transit_leg_to_feature(
                            transit_data,
                            *route_id,
                            *trip_id,
                            *from_stop,
                            *to_stop,
                            *departure_time,
                            *arrival_time,
                            idx,
                        ));
                    }
                    crate::routing::raptor::JourneyLeg::Transfer {
                        from_stop,
                        to_stop,
                        departure_time,
                        arrival_time,
                        duration,
                    } => {
                        features.push(DetailedJourney::transfer_leg_to_feature(
                            transit_data,
                            *from_stop,
                            *to_stop,
                            *departure_time,
                            *arrival_time,
                            *duration,
                            idx,
                        ));
                    }
                    crate::routing::raptor::JourneyLeg::Waiting { at_stop, duration } => {
                        features.push(DetailedJourney::waiting_leg_to_feature(
                            transit_data,
                            *at_stop,
                            *duration,
                        ));
                    }
                }
            }
        }

        // Process egress leg (walking from transit)
        if let Some(egress) = &self.egress_leg {
            features.push(egress.to_feature("egress_walk"));
        }

        FeatureCollection {
            features,
            bbox: None,
            foreign_members: None,
        }
    }

    /// Convert a transit leg to a `GeoJSON` Feature
    #[allow(clippy::too_many_arguments)]
    fn transit_leg_to_feature(
        transit_data: &PublicTransitData,
        route_id: usize,
        trip_id: usize,
        from_stop: RaptorStopId,
        to_stop: RaptorStopId,
        departure_time: Time,
        arrival_time: Time,
        leg_idx: usize,
    ) -> Feature {
        // Get stop coordinates
        let from_location = transit_data.transit_stop_location(from_stop);
        let to_location = transit_data.transit_stop_location(to_stop);

        // Get stop names
        let from_name = transit_data
            .transit_stop_name(from_stop)
            .unwrap_or_default();
        let to_name = transit_data.transit_stop_name(to_stop).unwrap_or_default();

        // Create intermediate points by getting all stops between from_stop and to_stop on this route
        let mut coordinates = Vec::new();
        coordinates.push((from_location.x(), from_location.y()));

        // Try to get all stops for this route
        if let Ok(route_stops) = transit_data.get_route_stops(route_id) {
            let from_idx = route_stops.iter().position(|&s| s == from_stop);
            let to_idx = route_stops.iter().position(|&s| s == to_stop);

            if let (Some(start_idx), Some(end_idx)) = (from_idx, to_idx) {
                // Determine direction (forward or backward along route)
                let range: Vec<usize> = if start_idx < end_idx {
                    (start_idx + 1..end_idx).collect()
                } else {
                    (end_idx + 1..start_idx).rev().collect()
                };

                // Add intermediate stops
                for idx in range {
                    let stop_id = route_stops[idx];
                    let stop_loc = transit_data.transit_stop_location(stop_id);
                    coordinates.push((stop_loc.x(), stop_loc.y()));
                }
            }
        }

        coordinates.push((to_location.x(), to_location.y()));
        let linestring: LineString = coordinates.into();
        let route_id = &transit_data.routes[route_id].route_id;

        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&linestring).into()),
            "properties": {
                "leg_type": "transit",
                "leg_index": leg_idx,
                "route_id": route_id,
                "trip_id": trip_id,
                "from_name": from_name,
                "to_name": to_name,
                "departure_time": departure_time,
                "arrival_time": arrival_time,
                "duration": (arrival_time - departure_time),
            }
        });

        Feature::from_json_value(value).unwrap()
    }

    /// Convert a transfer leg to a `GeoJSON` Feature
    fn transfer_leg_to_feature(
        transit_data: &PublicTransitData,
        from_stop: RaptorStopId,
        to_stop: RaptorStopId,
        departure_time: Time,
        arrival_time: Time,
        duration: Time,
        leg_idx: usize,
    ) -> Feature {
        // Get stop coordinates
        let from_location = transit_data.transit_stop_location(from_stop);
        let to_location = transit_data.transit_stop_location(to_stop);

        // Get stop names
        let from_name = transit_data
            .transit_stop_name(from_stop)
            .unwrap_or_default();
        let to_name = transit_data.transit_stop_name(to_stop).unwrap_or_default();

        // Create a LineString for the transfer
        let linestring: LineString = vec![
            (from_location.x(), from_location.y()),
            (to_location.x(), to_location.y()),
        ]
        .into();

        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&linestring).into()),
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

        Feature::from_json_value(value).unwrap()
    }

    fn waiting_leg_to_feature(
        transit_data: &PublicTransitData,
        at_stop: RaptorStopId,
        duration: Time,
    ) -> Feature {
        let geom = transit_data.transit_stop_location(at_stop);

        // Create properties for the feature
        let mut properties = Map::new();
        properties.insert("duration".to_string(), JsonValue::Number(duration.into()));
        properties.insert(
            "leg_type".to_string(),
            JsonValue::String("waiting".to_string()),
        );

        Feature {
            bbox: None,
            geometry: Some(Geometry::new((&geom).into())),
            id: None,
            properties: Some(properties),
            foreign_members: None,
        }
    }

    pub fn to_geojson_string(&self, transit_data: &PublicTransitData) -> String {
        let collection = self.to_geojson(transit_data);
        serde_json::to_string(&collection).unwrap_or_default()
    }
}

/// Traced multimodal routing from one point to another with detailed itinerary
pub fn traced_multimodal_routing(
    transit_data: &TransitModel,
    start: &TransitPoint,
    end: &TransitPoint,
    departure_time: Time,
    max_transfers: usize,
) -> Result<Option<DetailedJourney>, Error> {
    let transit_data = &transit_data.transit_data;
    let direct_walking = start.walking_time_to(end);

    let mut best_candidate: Option<(TransitCandidate, Journey, RaptorStopId, RaptorStopId)> = None;

    for &(access_stop, access_time) in start.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
        for &(egress_stop, egress_time) in end.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
            // Skip if walking path is faster
            if let Some(walking_time) = direct_walking {
                if access_time + egress_time >= walking_time {
                    continue;
                }
            }

            // Skip if we already have a better candidate
            if let Some((candidate, _, _, _)) = &best_candidate {
                if access_time + egress_time >= candidate.total_time {
                    continue;
                }
            }

            if let Ok(result) = traced_raptor(
                transit_data,
                access_stop,
                Some(egress_stop),
                departure_time + access_time,
                max_transfers,
            ) {
                match result {
                    TracedRaptorResult::SingleTarget(Some(journey)) => {
                        let transit_journey_time =
                            journey.arrival_time - (departure_time + access_time);
                        let total_time = access_time + transit_journey_time + egress_time;

                        let candidate = TransitCandidate {
                            total_time,
                            transit_time: transit_journey_time,
                            transfers_used: journey.transfers_count,
                        };

                        // Update if this is better than our current best
                        if best_candidate
                            .as_ref()
                            .is_none_or(|(best, _, _, _)| candidate.total_time < best.total_time)
                        {
                            best_candidate = Some((candidate, journey, access_stop, egress_stop));
                        }
                    }
                    TracedRaptorResult::SingleTarget(None) => {}
                    TracedRaptorResult::AllTargets(_) => {
                        unreachable!("Unexpected AllTargets result")
                    }
                }
            }
        }
    }

    // Generate the final result
    if is_walking_better(
        direct_walking,
        best_candidate.as_ref().map(|(c, _, _, _)| c),
    ) {
        // Walking is faster
        if let Some(walking_time) = direct_walking {
            return Ok(Some(DetailedJourney::walking_only(
                start,
                end,
                departure_time,
                walking_time,
            )));
        }
    } else if let Some((_, journey, access_stop, egress_stop)) = best_candidate {
        // Get access and egress times
        let access_time = start
            .nearest_stops
            .iter()
            .find(|(id, _)| *id == access_stop)
            .map_or(0, |(_, time)| *time);

        let egress_time = end
            .nearest_stops
            .iter()
            .find(|(id, _)| *id == egress_stop)
            .map_or(0, |(_, time)| *time);

        // Transit route is best
        return Ok(Some(DetailedJourney::with_transit(
            start,
            end,
            transit_data,
            access_stop,
            egress_stop,
            access_time,
            egress_time,
            journey,
            departure_time,
        )));
    } else if let Some(walking_time) = direct_walking {
        // No transit option, but we can walk
        return Ok(Some(DetailedJourney::walking_only(
            start,
            end,
            departure_time,
            walking_time,
        )));
    }

    Ok(None)
}
