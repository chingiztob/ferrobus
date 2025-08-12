mod detailed_journey;
mod journey_leg;
mod to_geojson;

pub use detailed_journey::DetailedJourney;
pub use journey_leg::WalkingLeg;

use crate::{
    Error, MAX_CANDIDATE_STOPS, RaptorStopId, Time, TransitModel,
    model::TransitPoint,
    routing::{
        multimodal_routing::CandidateJourney,
        raptor::{Journey, TracedRaptorResult, traced_raptor},
    },
};

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
    let mut best_candidate: Option<(CandidateJourney, Journey, RaptorStopId, RaptorStopId)> = None;

    for &(access_stop, access_time) in start.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
        for &(egress_stop, egress_time) in end.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
            if let Some(walk_time) = direct_walking
                && access_time + egress_time >= walk_time
            {
                continue;
            }
            if let Some((best, _, _, _)) = best_candidate.as_ref()
                && access_time + egress_time >= best.total_time
            {
                continue;
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
                let candidate = CandidateJourney {
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

    if let Some(walk_time) = direct_walking
        && (best_candidate.is_none() || walk_time <= best_candidate.as_ref().unwrap().0.total_time)
    {
        return Ok(Some(DetailedJourney::walking_only(
            start,
            end,
            departure_time,
            walk_time,
        )));
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
