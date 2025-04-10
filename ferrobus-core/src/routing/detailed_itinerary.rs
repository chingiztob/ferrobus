use geo::{LineString, Point, line_string};
use geojson::{Feature, FeatureCollection, Geometry};
use serde_json::json;

use crate::{
    Error, MAX_CANDIDATE_STOPS, PublicTransitData, RaptorStopId, Time, TransitModel,
    model::TransitPoint,
    routing::{
        multimodal_routing::TransitCandidate,
        raptor::{Journey, JourneyLeg, TracedRaptorResult, traced_raptor},
    },
};

/// Represents a walking leg outside the transit network.
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
    /// Create a new walking leg.
    pub fn new(
        from_location: Point<f64>,
        to_location: Point<f64>,
        from_name: String,
        to_name: String,
        departure_time: Time,
        duration: Time,
    ) -> Self {
        Self {
            from_location,
            to_location,
            from_name,
            to_name,
            departure_time,
            arrival_time: departure_time + duration,
            duration,
        }
    }

    /// Convert the walking leg to a `GeoJSON` Feature using the `json!` macro.
    ///
    /// # Panics
    /// This function will panic if `Feature::from_json_value` fails to parse the JSON value.
    pub fn to_feature(&self, leg_type: &str) -> Feature {
        let coordinates = line_string![
            (x: self.from_location.x(), y: self.from_location.y()),
            (x: self.to_location.x(), y: self.to_location.y()),
        ];
        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&coordinates).into()),
            "properties": {
                "leg_type": leg_type,
                "from_name": self.from_name,
                "to_name": self.to_name,
                "departure_time": self.departure_time,
                "arrival_time": self.arrival_time,
                "duration": self.duration,
            }
        });
        Feature::from_json_value(value).unwrap()
    }
}

/// Represents a complete journey with first/last mile connections.
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
    /// Creates a walking-only journey.
    pub fn walking_only(
        start: &TransitPoint,
        end: &TransitPoint,
        departure_time: Time,
        walking_time: Time,
    ) -> Self {
        let walk_leg = WalkingLeg::new(
            start.geometry,
            end.geometry,
            String::new(),
            String::new(),
            departure_time,
            walking_time,
        );
        Self {
            access_leg: Some(walk_leg),
            transit_journey: None,
            egress_leg: None,
            total_time: walking_time,
            walking_time,
            transit_time: None,
            transfers: 0,
            departure_time,
            arrival_time: departure_time + walking_time,
        }
    }

    /// Creates a multimodal journey with transit.
    #[allow(clippy::too_many_arguments)]
    pub fn with_transit(
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

        let access_stop_info = &transit_data.stops[access_stop];
        let egress_stop_info = &transit_data.stops[egress_stop];

        let access_leg = WalkingLeg::new(
            start.geometry,
            access_stop_info.geometry,
            String::new(),
            access_stop_info.stop_id.clone(),
            departure_time,
            access_time,
        );
        let egress_leg = WalkingLeg::new(
            egress_stop_info.geometry,
            end.geometry,
            egress_stop_info.stop_id.clone(),
            String::new(),
            transit_arrival,
            egress_time,
        );

        let transfer_count = transit_journey.transfers_count;

        Self {
            access_leg: Some(access_leg),
            transit_journey: Some(transit_journey),
            egress_leg: Some(egress_leg),
            total_time: final_arrival - departure_time,
            walking_time: access_time + egress_time,
            transit_time: Some(transit_arrival - transit_departure),
            transfers: transfer_count,
            departure_time,
            arrival_time: final_arrival,
        }
    }

    /// Converts the complete journey to a `GeoJSON` `FeatureCollection`.
    pub fn to_geojson(&self, transit_data: &PublicTransitData) -> FeatureCollection {
        let mut features = Vec::new();

        if let Some(access) = &self.access_leg {
            features.push(access.to_feature("access_walk"));
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
                    } => Self::transit_leg_feature(
                        transit_data,
                        *route_id,
                        *trip_id,
                        *from_stop,
                        *to_stop,
                        *departure_time,
                        *arrival_time,
                        idx,
                    ),
                    JourneyLeg::Transfer {
                        from_stop,
                        departure_time,
                        to_stop,
                        arrival_time,
                        duration,
                    } => Self::transfer_leg_feature(
                        transit_data,
                        *from_stop,
                        *to_stop,
                        *departure_time,
                        *arrival_time,
                        *duration,
                        idx,
                    ),
                    JourneyLeg::Waiting { at_stop, duration } => {
                        Self::waiting_leg_feature(transit_data, *at_stop, *duration)
                    }
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
    pub fn to_geojson_string(&self, transit_data: &PublicTransitData) -> String {
        serde_json::to_string(&self.to_geojson(transit_data)).unwrap_or_default()
    }

    /// Converts a transit leg to a `GeoJSON` Feature.
    #[allow(clippy::too_many_arguments)]
    fn transit_leg_feature(
        transit_data: &PublicTransitData,
        route_id: usize,
        trip_id: usize,
        from_stop: RaptorStopId,
        to_stop: RaptorStopId,
        departure_time: Time,
        arrival_time: Time,
        leg_idx: usize,
    ) -> Feature {
        let from_loc = transit_data.transit_stop_location(from_stop);
        let to_loc = transit_data.transit_stop_location(to_stop);
        let from_name = transit_data
            .transit_stop_name(from_stop)
            .unwrap_or_default();
        let to_name = transit_data.transit_stop_name(to_stop).unwrap_or_default();

        let mut coords = vec![(from_loc.x(), from_loc.y())];
        if let Ok(route_stops) = transit_data.get_route_stops(route_id) {
            if let (Some(start_idx), Some(end_idx)) = (
                route_stops.iter().position(|&s| s == from_stop),
                route_stops.iter().position(|&s| s == to_stop),
            ) {
                let range: Vec<_> = if start_idx < end_idx {
                    (start_idx + 1..end_idx).collect()
                } else {
                    (end_idx + 1..start_idx).rev().collect()
                };
                for idx in range {
                    let stop_loc = transit_data.transit_stop_location(route_stops[idx]);
                    coords.push((stop_loc.x(), stop_loc.y()));
                }
            }
        }
        coords.push((to_loc.x(), to_loc.y()));
        let line: LineString<_> = coords.into();

        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&line).into()),
            "properties": {
                "leg_type": "transit",
                "leg_index": leg_idx,
                "route_id": transit_data.routes[route_id].route_id,
                "trip_id": trip_id,
                "from_name": from_name,
                "to_name": to_name,
                "departure_time": departure_time,
                "arrival_time": arrival_time,
                "duration": arrival_time - departure_time,
            }
        });
        Feature::from_json_value(value).unwrap()
    }

    /// Converts a transfer leg to a `GeoJSON` Feature.
    fn transfer_leg_feature(
        transit_data: &PublicTransitData,
        from_stop: RaptorStopId,
        to_stop: RaptorStopId,
        departure_time: Time,
        arrival_time: Time,
        duration: Time,
        leg_idx: usize,
    ) -> Feature {
        let from_name = transit_data
            .transit_stop_name(from_stop)
            .unwrap_or_default();
        let to_name = transit_data.transit_stop_name(to_stop).unwrap_or_default();

        let transfers_start = &transit_data.stops[from_stop].transfers_start;
        let transfers_end = transit_data.stops[from_stop].transfers_len + transfers_start;
        let geometry = &transit_data.transfers[*transfers_start..transfers_end]
            .iter()
            .find(|t| t.target_stop == to_stop)
            .unwrap()
            .geometry;

        let geometry = Geometry::new(geometry.as_ref().into());

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
        Feature::from_json_value(value).unwrap()
    }

    /// Converts a waiting leg to a `GeoJSON` Feature.
    fn waiting_leg_feature(
        transit_data: &PublicTransitData,
        at_stop: RaptorStopId,
        duration: Time,
    ) -> Feature {
        let geom = transit_data.transit_stop_location(at_stop);
        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&geom).into()),
            "properties": {
                "leg_type": "waiting",
                "duration": duration,
            }
        });
        Feature::from_json_value(value).unwrap()
    }
}

/// Traced multimodal routing from one point to another.
#[allow(clippy::missing_panics_doc)]
pub fn traced_multimodal_routing(
    transit_model: &TransitModel,
    start: &TransitPoint,
    end: &TransitPoint,
    departure_time: Time,
    max_transfers: usize,
) -> Result<Option<DetailedJourney>, Error> {
    let transit_data = &transit_model.transit_data;
    let direct_walking = start.walking_time_to(end);
    let mut best_candidate: Option<(TransitCandidate, Journey, RaptorStopId, RaptorStopId)> = None;

    for &(access_stop, access_time) in start.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
        for &(egress_stop, egress_time) in end.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
            if let Some(walk_time) = direct_walking {
                if access_time + egress_time >= walk_time {
                    continue;
                }
            }
            if let Some((best, _, _, _)) = best_candidate.as_ref() {
                if access_time + egress_time >= best.total_time {
                    continue;
                }
            }
            if let Ok(TracedRaptorResult::SingleTarget(Some(journey))) = traced_raptor(
                transit_data,
                access_stop,
                Some(egress_stop),
                departure_time + access_time,
                max_transfers,
            ) {
                let transit_time = journey.arrival_time - (departure_time + access_time);
                let total_time = access_time + transit_time + egress_time;
                let candidate = TransitCandidate {
                    total_time,
                    transit_time,
                    transfers_used: journey.transfers_count,
                };
                if best_candidate
                    .as_ref()
                    .is_none_or(|(best, _, _, _)| candidate.total_time < best.total_time)
                {
                    best_candidate = Some((candidate, journey, access_stop, egress_stop));
                }
            }
        }
    }

    if let Some(walk_time) = direct_walking {
        if best_candidate.is_none() || walk_time <= best_candidate.as_ref().unwrap().0.total_time {
            return Ok(Some(DetailedJourney::walking_only(
                start,
                end,
                departure_time,
                walk_time,
            )));
        }
    }
    if let Some((_, journey, access_stop, egress_stop)) = best_candidate {
        let access_time = start
            .nearest_stops
            .iter()
            .find(|(s, _)| *s == access_stop)
            .map_or(0, |(_, t)| *t);
        let egress_time = end
            .nearest_stops
            .iter()
            .find(|(s, _)| *s == egress_stop)
            .map_or(0, |(_, t)| *t);
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
    }
    if let Some(walk_time) = direct_walking {
        return Ok(Some(DetailedJourney::walking_only(
            start,
            end,
            departure_time,
            walk_time,
        )));
    }
    Ok(None)
}
