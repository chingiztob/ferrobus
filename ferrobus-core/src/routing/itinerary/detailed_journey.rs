use crate::{
    PublicTransitData, RaptorStopId, Time,
    model::TransitPoint,
    routing::{itinerary::WalkingLeg, raptor::Journey},
};

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
        start_point: &TransitPoint,
        end_point: &TransitPoint,
        departure_time: Time,
        walking_time: Time,
    ) -> Self {
        let arrival_time = departure_time + walking_time;

        let walk_leg = WalkingLeg::new(
            start_point.geometry,
            end_point.geometry,
            // Empty strings, because there are no transit stops in a walking-only journey
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
            arrival_time,
        }
    }

    /// Creates a multimodal journey with transit.
    #[allow(clippy::too_many_arguments)]
    pub fn with_transit(
        start_point: &TransitPoint,
        end_point: &TransitPoint,
        transit_data: &PublicTransitData,
        access_stop: RaptorStopId,
        egress_stop: RaptorStopId,
        access_time: Time,
        egress_time: Time,
        transit_journey: Journey,
        departure_time: Time,
    ) -> Self {
        let transit_departure = transit_journey.departure_time;
        let transit_arrival = transit_journey.arrival_time;
        let transit_time = transit_arrival - transit_departure;
        let walking_time = access_time + egress_time;
        let total_time = walking_time + transit_time;
        let transfers = transit_journey.transfers_count;
        let arrival_time = departure_time + total_time;

        let access_stop_info = &transit_data.stops[access_stop];
        let egress_stop_info = &transit_data.stops[egress_stop];

        let access_leg = WalkingLeg::new(
            start_point.geometry,
            access_stop_info.geometry,
            String::new(),
            access_stop_info.stop_id.clone(),
            departure_time,
            access_time,
        );
        let egress_leg = WalkingLeg::new(
            egress_stop_info.geometry,
            end_point.geometry,
            egress_stop_info.stop_id.clone(),
            String::new(),
            transit_arrival,
            egress_time,
        );

        Self {
            access_leg: Some(access_leg),
            transit_journey: Some(transit_journey),
            egress_leg: Some(egress_leg),
            total_time,
            walking_time,
            transit_time: Some(transit_time),
            transfers,
            departure_time,
            arrival_time,
        }
    }
}
