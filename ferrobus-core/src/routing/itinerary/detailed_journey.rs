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
        start: &TransitPoint,
        end: &TransitPoint,
        departure_time: Time,
        walking_time: Time,
    ) -> Self {
        let walk_leg = WalkingLeg::new(
            start.geometry,
            end.geometry,
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
}
