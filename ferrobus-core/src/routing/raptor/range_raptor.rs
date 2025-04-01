use log::warn;

use super::regular::{create_route_queue, process_foot_paths};
use super::state::{RaptorError, RaptorState, find_earliest_trip};
use crate::{PublicTransitData, RaptorStopId, Time};

#[derive(Debug)]
/// Result for a range query journey.
pub struct RaptorRangeJourney {
    /// The departure time from the source.
    pub departure_time: Time,
    /// The arrival time at the target (if a journey was found).
    pub arrival_time: Option<Time>,
    /// The number of transfers used in the journey.
    pub transfers_used: usize,
}

/// rRAPTOR: Range Query Version of RAPTOR.
///
/// Instead of a single departure time, a time range (`min_dep`, `max_dep`)
/// is given. The algorithm first extracts all departure times at the source
/// within the range, orders them from latest to earliest, and then runs RAPTOR
/// for each departure time while reusing previously computed labels. The output
/// is a vector of journeys (one per departure time) for the target stop.
#[allow(clippy::too_many_lines)]
pub fn rraptor(
    data: &PublicTransitData,
    source: RaptorStopId,
    target: Option<RaptorStopId>,
    departure_range: (Time, Time),
    max_transfers: usize,
) -> Result<Vec<RaptorRangeJourney>, RaptorError> {
    // Validate source and target.
    data.validate_stop(source)?;
    if let Some(t) = target {
        data.validate_stop(t)?;
    }
    // For the range, we assume departure_range = (min_departure, max_departure)
    // and that max_departure is within allowed limits.
    if departure_range.1 > 86400 * 2 {
        return Err(RaptorError::InvalidTime);
    }
    let num_stops = data.stops.len();
    let max_rounds = max_transfers + 1;

    // Retrieve all departure times from the source within the given range.
    let mut departures =
        data.get_source_departures(source, departure_range.0, departure_range.1)?;
    // Process departures from latest to earliest.
    departures.sort_by(|a, b| b.cmp(a));

    // Initialize the RAPTOR state.
    let mut state = RaptorState::new(num_stops, max_rounds);
    let mut journeys = Vec::with_capacity(departures.len());

    // For each departure time, update state and run RAPTOR rounds.
    for &dep_time in &departures {
        // Inject the new departure at the source for round 0.
        state.update(0, source, dep_time, dep_time)?;
        state.marked_stops[0].set(source, true);

        // Process foot-path transfers from the source.
        let transfers = data.get_stop_transfers(source)?;
        for &(target_stop, duration) in transfers {
            if target_stop >= num_stops {
                warn!("Invalid transfer target {target_stop}");
                continue;
            }
            let new_time = dep_time.saturating_add(duration);
            // For foot-paths we assume no waiting time (arrival equals boarding).
            if state.update(0, target_stop, new_time, new_time)? {
                state.marked_stops[0].set(target_stop, true);
            }
        }

        // For rounds 1..max_rounds, first carry over improvements from the previous round.
        for round in 1..max_rounds {
            let prev_round = round - 1;
            // Carry-over step: if the previous round has a better boarding time, propagate it.
            for stop in 0..num_stops {
                if state.board_times[prev_round][stop] < state.board_times[round][stop] {
                    state.arrival_times[round][stop] = state.arrival_times[prev_round][stop];
                    state.board_times[round][stop] = state.board_times[prev_round][stop];
                    state.marked_stops[round].set(stop, true);
                }
            }

            // If no stops were marked in the previous round, we can break early
            if state.marked_stops[prev_round].is_clear() {
                break;
            }

            let mut queue = create_route_queue(data, &state.marked_stops[prev_round])?;
            state.marked_stops[prev_round].clear();

            // When a target is given, use its best known arrival time for pruning.
            let target_bound = if let Some(target_stop) = target {
                state.best_arrival[target_stop]
            } else {
                Time::MAX
            };

            while let Some((route_id, start_pos)) = queue.pop_front() {
                let stops = data.get_route_stops(route_id)?;
                let mut current_trip_opt = None;
                let mut current_board_pos = 0;

                // Find the earliest trip on this route that is catchable.
                for (idx, &stop) in stops.iter().enumerate().skip(start_pos) {
                    let earliest_board = state.board_times[prev_round][stop];
                    if earliest_board == Time::MAX {
                        continue;
                    }
                    if let Some(trip_idx) = find_earliest_trip(data, route_id, idx, earliest_board)
                    {
                        current_trip_opt = Some(trip_idx);
                        current_board_pos = idx;
                        break;
                    }
                }

                if let Some(mut trip_idx) = current_trip_opt {
                    let mut trip = data.get_trip(route_id, trip_idx)?;

                    for (trip_stop_idx, &stop) in stops.iter().enumerate().skip(current_board_pos) {
                        // Check if we can "upgrade" the trip at this stop.
                        let prev_board = state.board_times[prev_round][stop];
                        if prev_board < trip[trip_stop_idx].departure {
                            if let Some(new_trip_idx) =
                                find_earliest_trip(data, route_id, trip_stop_idx, prev_board)
                            {
                                if new_trip_idx != trip_idx {
                                    trip_idx = new_trip_idx;
                                    trip = data.get_trip(route_id, new_trip_idx)?;
                                    //current_board_pos = trip_stop_idx;
                                }
                            }
                        }
                        // Separate the times: the actual arrival (when the bus reaches the stop)
                        // and the boarding time (when the bus departs from the stop).
                        let actual_arrival = trip[trip_stop_idx].arrival;
                        // For further connections, use the departure time.
                        let effective_board = if let Some(target_stop) = target {
                            if stop == target_stop {
                                actual_arrival // For target, we report arrival.
                            } else {
                                trip[trip_stop_idx].departure
                            }
                        } else {
                            trip[trip_stop_idx].departure
                        };

                        // Only update if this effective boarding time is an improvement.
                        if state.update(round, stop, actual_arrival, effective_board)? {
                            state.marked_stops[round].set(stop, true);
                        }
                        // Prune if we've already exceeded the target bound.
                        if effective_board >= target_bound {
                            break;
                        }
                    }
                }
            }

            let new_marks = process_foot_paths(data, target, num_stops, &mut state, round)?;
            state.marked_stops[round].union_with(&new_marks);

            // Check if we should continue with this round
            if let Some(target_stop) = target {
                let arrival_time = state.arrival_times[round][target_stop];
                let target_bound = state.best_arrival[target_stop];

                // If the arrival time in this round is worse than our best known time,
                // there's no point continuing
                if arrival_time != Time::MAX && arrival_time > target_bound {
                    break;
                }
            }

            // If no stops were marked in this round, we can stop.
            if state.marked_stops[round].is_clear() {
                break;
            }
        }

        // After processing rounds for this departure, record the result for the target.
        let mut best_arr = Time::MAX;
        let mut best_round = 0;
        if let Some(target_stop) = target {
            for round in 0..max_rounds {
                let t = state.arrival_times[round][target_stop];
                if t != Time::MAX && t < best_arr {
                    best_arr = t;
                    best_round = round;
                }
            }
        }

        let journey = RaptorRangeJourney {
            departure_time: dep_time,
            arrival_time: if best_arr == Time::MAX {
                None
            } else {
                Some(best_arr)
            },
            transfers_used: best_round,
        };
        journeys.push(journey);
    }

    Ok(journeys)
}
