use hashbrown::HashMap;

use crate::{
    Error, MAX_CANDIDATE_STOPS, Time, TransitModel,
    model::TransitPoint,
    routing::raptor::{RaptorResult, raptor},
};

/// Combined multimodal route result
#[derive(Debug, Clone)]
pub struct MultiModalResult {
    // Total journey time
    pub travel_time: Time,
    // Time spent on transit (None if walking only)
    pub transit_time: Option<Time>,
    // Time spent walking
    pub walking_time: Time,
    // Number of transfers used
    pub transfers: usize,
}

/// Internal struct to track transit route candidates
#[derive(Debug, Clone)]
pub(crate) struct CandidateJourney {
    pub(crate) total_time: Time,
    pub(crate) transit_time: Time,
    pub(crate) transfers_used: usize,
}

/// Checks if direct walking is better than the transit option
pub(crate) fn is_walking_better(
    walking_time: Option<Time>,
    transit_candidate: Option<&CandidateJourney>,
) -> bool {
    match (walking_time, transit_candidate) {
        (Some(walking), Some(transit)) => walking <= transit.total_time,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Creates a `MultiModalResult` for a walking-only journey
pub(crate) fn create_walking_result(walking_time: Time) -> MultiModalResult {
    MultiModalResult {
        travel_time: walking_time,
        transit_time: None,
        walking_time,
        transfers: 0,
    }
}

/// Creates a `MultiModalResult` for a transit journey
pub(crate) fn create_transit_result(candidate: &CandidateJourney) -> MultiModalResult {
    let walking_time = candidate.total_time - candidate.transit_time;

    MultiModalResult {
        travel_time: candidate.total_time,
        transit_time: Some(candidate.transit_time),
        walking_time,
        transfers: candidate.transfers_used,
    }
}

///Combined multimodal routing function
pub fn multimodal_routing(
    transit_data: &TransitModel,
    start: &TransitPoint,
    end: &TransitPoint,
    departure_time: Time,
    max_transfers: usize,
) -> Result<Option<MultiModalResult>, Error> {
    if departure_time > 86400 * 2 {
        return Err(Error::InvalidData("Invalid departure time".to_string()));
    }

    let transit_data = &transit_data.transit_data;
    let direct_walking = start.walking_time_to(end);

    let mut best_candidate: Option<CandidateJourney> = None;

    for &(access_stop, access_time) in start.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
        for &(egress_stop, egress_time) in end.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
            // Skip if walking path is faster
            if let Some(walking_time) = direct_walking
                && access_time + egress_time >= walking_time
            {
                continue;
            }

            // Skip if we already have a better candidate
            if let Some(candidate) = &best_candidate
                && access_time + egress_time >= candidate.total_time
            {
                continue;
            }

            if let Ok(result) = raptor(
                transit_data,
                access_stop,
                Some(egress_stop),
                departure_time + access_time,
                max_transfers,
            ) {
                match result {
                    RaptorResult::SingleTarget(target) => {
                        if target.is_reachable() {
                            let transit_time = target.arrival_time - (departure_time + access_time);
                            let total_time = access_time + transit_time + egress_time;
                            if target.arrival_time < departure_time + access_time {
                                return Err(Error::InvalidData(format!(
                                    "Negative transit time detected: {} - {} = {}",
                                    target.arrival_time,
                                    departure_time + access_time,
                                    transit_time
                                )));
                            }

                            let candidate = CandidateJourney {
                                total_time,
                                transit_time,
                                transfers_used: target.transfers_used,
                            };

                            // Update if this is better than our current best
                            if best_candidate
                                .as_ref()
                                .is_none_or(|best| candidate.total_time < best.total_time)
                            {
                                best_candidate = Some(candidate);
                            }
                        }
                    }
                    RaptorResult::AllTargets(_) => {
                        unreachable!("Unexpected AllTargets result");
                    }
                }
            }
        }
    }

    // If some candidate transit route was found, check if it's better than walking
    if let Some(candidate) = best_candidate
        && !is_walking_better(direct_walking, Some(&candidate))
    {
        return Ok(Some(create_transit_result(&candidate)));
    }

    // if not - return walking result
    if let Some(walking_time) = direct_walking {
        return Ok(Some(create_walking_result(walking_time)));
    }

    Ok(None)
}

/// Routing from one point to many. It exploits basic RAPTOR principles to
/// calculate transit routes to all stops from the access point, so whole calculation
/// can be done in one raptor run.
pub fn multimodal_routing_one_to_many(
    transit_data: &TransitModel,
    start: &TransitPoint,
    targets: &[TransitPoint],
    departure_time: Time,
    max_transfers: usize,
) -> Result<Vec<Option<MultiModalResult>>, Error> {
    let transit_data = &transit_data.transit_data;
    let mut results = vec![None; targets.len()];

    // Run RAPTOR to all stops for each initial access point
    let mut transit_results = HashMap::new();

    for &(access_stop, access_time) in start.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
        if let Ok(RaptorResult::AllTargets(times)) = raptor(
            transit_data,
            access_stop,
            None,
            departure_time + access_time,
            max_transfers,
        ) {
            transit_results.insert(access_stop, (access_time, times));
        }
    }

    for (end_idx, end_point) in targets.iter().enumerate() {
        let direct_walking = start.walking_time_to(end_point);
        let mut best_candidate: Option<CandidateJourney> = None;

        for (_access_stop, (access_time, transit_times)) in &transit_results {
            for &(egress_stop, egress_time) in &end_point.nearest_stops {
                // Skip if walking path is faster
                if let Some(walking_time) = direct_walking
                    && access_time + egress_time >= walking_time
                {
                    continue;
                }

                // Skip if we already have a better candidate
                if let Some(candidate) = &best_candidate
                    && access_time + egress_time >= candidate.total_time
                {
                    continue;
                }

                if transit_times[egress_stop].is_reachable() {
                    let transit_time = transit_times[egress_stop].arrival_time;
                    let transfers_used = transit_times[egress_stop].transfers_used;

                    let transit_time = transit_time - (departure_time + *access_time);
                    let total_time = *access_time + transit_time + egress_time;

                    let candidate = CandidateJourney {
                        total_time,
                        transit_time,
                        transfers_used,
                    };

                    if best_candidate
                        .as_ref()
                        .is_none_or(|best| candidate.total_time < best.total_time)
                    {
                        best_candidate = Some(candidate);
                    }
                }
            }
        }

        if let Some(candidate) = best_candidate
            && !is_walking_better(direct_walking, Some(&candidate))
        {
            results[end_idx] = Some(create_transit_result(&candidate));
            continue;
        }

        // Either walking is better or no transit option exists
        if let Some(walking_time) = direct_walking {
            results[end_idx] = Some(create_walking_result(walking_time));
        }
    }

    Ok(results)
}
