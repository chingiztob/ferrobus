use serde::Serialize;

use crate::{
    Error, MAX_CANDIDATE_STOPS, MultiModalResult, Time, TransitModel, model::TransitPoint,
    routing::raptor::rraptor,
};

use super::{
    multimodal_routing::{CandidateJourney, create_transit_result, create_walking_result},
    raptor::RaptorRangeJourney,
};

/// A flattened representation of a journey with all relevant details
#[derive(Debug, Clone, Hash, Serialize)]
pub struct RangeJourney {
    // Journey metrics
    pub travel_time: Time,
    pub transfers: usize,
    pub walking_time: Time,
    // Timing information
    pub departure_time: Time,
    pub arrival_time: Time,
}

impl RangeJourney {
    /// Create a new `ParetoJourney` from a `MultiModalResult` and timing information
    #[allow(clippy::needless_pass_by_value)]
    fn new(result: MultiModalResult, departure_time: Time, arrival_time: Time) -> Self {
        Self {
            travel_time: result.travel_time,
            transfers: result.transfers,
            walking_time: result.walking_time,
            departure_time,
            arrival_time,
        }
    }
}

#[derive(Debug, Clone, Hash, Serialize)]
/// Represents the result of a range-based routing calculation.
///
/// Contains a collection of Pareto-optimal journeys - each journey represents
/// a different optimal trade-off between criteria like time, cost, or number of transfers.
/// The Pareto set ensures no journey is strictly better than another in all aspects.
pub struct RangeRoutingResult {
    pub journeys: Vec<RangeJourney>,
}

impl RangeRoutingResult {
    pub fn new(journeys: Vec<RangeJourney>) -> Self {
        Self { journeys }
    }

    pub fn travel_times(&self) -> Vec<Time> {
        let mut journeys = self.journeys.clone();
        journeys.sort_by_key(|j| j.departure_time);
        journeys.iter().map(|j| j.travel_time).collect()
    }

    pub fn departure_times(&self) -> Vec<Time> {
        let mut times: Vec<Time> = self.journeys.iter().map(|j| j.departure_time).collect();
        times.sort_unstable();
        times
    }

    pub fn median_travel_time(&self) -> Time {
        let mut times = self.travel_times();
        times.sort_unstable();
        times[times.len() / 2]
    }
}

/// Checks if `journey_a` dominates `journey_b` in the Pareto sense
/// A journey dominates another if it's at least as good in all criteria
/// and strictly better in at least one criterion
fn is_pareto_dominant(journey_a: &RangeJourney, journey_b: &RangeJourney) -> bool {
    // Check if journey_a is at least as good as journey_b in all criteria
    let time_better_or_equal = journey_a.travel_time <= journey_b.travel_time;
    let transfers_better_or_equal = journey_a.transfers <= journey_b.transfers;
    let walking_better_or_equal = journey_a.walking_time <= journey_b.walking_time;

    // Check if journey_a is strictly better in at least one criterion
    let time_strictly_better = journey_a.travel_time < journey_b.travel_time;
    let transfers_strictly_better = journey_a.transfers < journey_b.transfers;
    let walking_strictly_better = journey_a.walking_time < journey_b.walking_time;

    // A dominates B if it's at least as good in all criteria and strictly better in at least one
    (time_better_or_equal && transfers_better_or_equal && walking_better_or_equal)
        && (time_strictly_better || transfers_strictly_better || walking_strictly_better)
}

/// Process a single journey from rraptor into a `ParetoJourney`
fn process_rraptor_journey(
    journey: &RaptorRangeJourney,
    access_time: Time,
    egress_time: Time,
) -> Option<RangeJourney> {
    journey.arrival_time.map(|arrival_time| {
        let transit_departure = journey.departure_time;
        let origin_departure = transit_departure.saturating_sub(access_time);
        let destination_arrival = arrival_time + egress_time;

        let transit_time = arrival_time - transit_departure;
        let total_time = destination_arrival - origin_departure;
        let transfers_used = journey.transfers_used;

        let candidate = CandidateJourney {
            total_time,
            transit_time,
            transfers_used,
        };

        // Create flattened journey with all details
        let result = create_transit_result(&candidate);
        RangeJourney::new(result, origin_departure, destination_arrival)
    })
}

/// Apply Pareto filtering to a collection of journeys
fn apply_pareto_filtering(journeys: RangeRoutingResult) -> RangeRoutingResult {
    let mut pareto_front: Vec<RangeJourney> = Vec::new();

    let mut sorted_journeys = journeys.journeys;
    sorted_journeys.sort_by_key(|j| j.departure_time);

    for journey in sorted_journeys {
        // Check if the new journey is dominated by any existing journey in the front
        let is_dominated = pareto_front.iter().any(|existing| {
            is_pareto_dominant(existing, &journey) && existing.arrival_time <= journey.arrival_time
        });

        if !is_dominated {
            // Remove journeys from the front that are dominated by the new journey
            pareto_front.retain(|existing| {
                !(is_pareto_dominant(&journey, existing)
                    && journey.arrival_time <= existing.arrival_time)
            });
            // Add the new non-dominated journey
            pareto_front.push(journey);
        }
    }

    RangeRoutingResult::new(pareto_front)
}

/// Multimodal routing with a departure time range, returning all journeys without filtering
pub fn range_multimodal_routing(
    transit_data: &TransitModel,
    start: &TransitPoint,
    end: &TransitPoint,
    departure_range: (Time, Time),
    max_transfers: usize,
) -> Result<RangeRoutingResult, Error> {
    let transit_data = &transit_data.transit_data;
    let direct_walking = start.walking_time_to(end);
    let mut all_journeys: Vec<RangeJourney> = Vec::new();

    // Add direct walking option if available
    if let Some(walking_time) = direct_walking {
        let departure = departure_range.0;
        let walking_result = create_walking_result(walking_time);
        let arrival = departure + walking_time;

        all_journeys.push(RangeJourney::new(walking_result, departure, arrival));
    }

    // Find all transit options
    for &(access_stop, access_time) in start.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
        for &(egress_stop, egress_time) in end.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
            // Skip if path is dominated by direct walking
            if let Some(walking_time) = direct_walking
                && access_time + egress_time >= walking_time
            {
                continue;
            }

            // Adjust departure range for access time
            let adjusted_range = (
                departure_range.0.saturating_add(access_time),
                departure_range.1.saturating_add(access_time),
            );

            // Run rraptor for the stop pair
            if let Ok(journeys) = rraptor(
                transit_data,
                access_stop,
                Some(egress_stop),
                adjusted_range,
                max_transfers,
            ) {
                // Process each journey returned by rraptor
                for journey in journeys {
                    if let Some(pareto_journey) =
                        process_rraptor_journey(&journey, access_time, egress_time)
                    {
                        all_journeys.push(pareto_journey);
                    }
                }
            }
        }
    }

    // Sort by arrival time as a convenience
    all_journeys.sort_by_key(|journey| journey.arrival_time);

    Ok(RangeRoutingResult::new(all_journeys))
}

/// Multimodal routing with a departure time range, returning Pareto-optimal front
pub fn pareto_range_multimodal_routing(
    transit_data: &TransitModel,
    start: &TransitPoint,
    end: &TransitPoint,
    departure_range: (Time, Time),
    max_transfers: usize,
) -> Result<RangeRoutingResult, Error> {
    // Get all journeys
    let all_journeys =
        range_multimodal_routing(transit_data, start, end, departure_range, max_transfers)?;

    // Apply Pareto filtering
    let pareto_front = apply_pareto_filtering(all_journeys);

    Ok(pareto_front)
}
