use log::warn;

use crate::model::Transfer;
use crate::routing::raptor::common::{
    RaptorError, RaptorState, create_route_queue, find_earliest_trip, find_earliest_trip_at_stop,
    get_target_bound, process_foot_paths, validate_raptor_inputs,
};
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

impl RaptorState {
    // Special method for rRAPTOR to carry over improvements from previous round
    fn carry_over_improvements(&mut self, stop: usize) -> bool {
        if self.prev_board_times[stop] < self.curr_board_times[stop] {
            self.curr_arrival_times[stop] = self.prev_arrival_times[stop];
            self.curr_board_times[stop] = self.prev_board_times[stop];
            return true;
        }
        false
    }

    // Reset the state for a new departure time while preserving best arrivals
    fn reset_for_new_departure(&mut self) {
        self.prev_arrival_times.fill(Time::MAX);
        self.prev_board_times.fill(Time::MAX);
        self.curr_arrival_times.fill(Time::MAX);
        self.curr_board_times.fill(Time::MAX);
        self.marked_stops.clear();
    }
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
    validate_raptor_inputs(data, source, target, departure_range.1)?;

    // For the range, we assume departure_range = (min_departure, max_departure)
    // and that max_departure is within allowed limits.
    let num_stops = data.stops.len();
    let max_rounds = max_transfers + 1;

    // Retrieve all departure times from the source within the given range.
    let mut departures =
        data.get_source_departures(source, departure_range.0, departure_range.1)?;
    // Process departures from latest to earliest.
    departures.sort_by(|a, b| b.cmp(a));

    let mut state = RaptorState::new(num_stops, max_rounds);
    let mut journeys = Vec::with_capacity(departures.len());

    // For each departure time, update state and run RAPTOR rounds.
    for &dep_time in &departures {
        // Reset the state for a new departure time (keeping best arrivals)
        state.reset_for_new_departure();

        // Inject the new departure at the source for round 0.
        state.update(0, source, dep_time, dep_time)?;
        state.marked_stops.set(source, true);

        // Process foot-path transfers from the source.
        let transfers = data.get_stop_transfers(source)?;
        for &Transfer {
            target_stop,
            duration,
            ..
        } in transfers
        {
            if target_stop >= num_stops {
                warn!("Invalid transfer target {target_stop}");
                continue;
            }
            let new_time = dep_time.saturating_add(duration);
            // For foot-paths we assume no waiting time (arrival equals boarding).
            if state.update(0, target_stop, new_time, new_time)? {
                state.marked_stops.set(target_stop, true);
            }
        }

        // Main rounds.
        for round in 1..max_rounds {
            // Advance to next round - swap current and previous
            state.advance_round();

            // Create queue from original marked stops (from previous operations)
            let mut queue = create_route_queue(data, &state.marked_stops)?;

            // Clear marked stops first
            state.marked_stops.clear();

            // Directly repopulate marked_stops with stops that have improvements carried over
            for stop in 0..num_stops {
                if state.carry_over_improvements(stop) {
                    state.marked_stops.set(stop, true);
                }
            }

            // If no stops were marked from carry-over AND no stops were in the queue, we can break early
            if queue.is_empty() && state.marked_stops.is_clear() {
                break;
            }

            let target_bound = get_target_bound(&state, target);

            while let Some((route_id, start_pos)) = queue.pop_front() {
                let stops = data.get_route_stops(route_id)?;

                if let Some((mut trip_idx, current_board_pos)) = find_earliest_trip_at_stop(
                    data,
                    route_id,
                    stops,
                    &state.prev_board_times,
                    start_pos,
                ) {
                    let mut trip = data.get_trip(route_id, trip_idx)?;

                    for (trip_stop_idx, &stop) in stops.iter().enumerate().skip(current_board_pos) {
                        // Check if we can "upgrade" the trip at this stop.
                        let prev_board = state.prev_board_times[stop];
                        if prev_board < trip[trip_stop_idx].departure
                            && let Some(new_trip_idx) =
                                find_earliest_trip(data, route_id, trip_stop_idx, prev_board)
                            && new_trip_idx != trip_idx
                        {
                            trip_idx = new_trip_idx;
                            trip = data.get_trip(route_id, new_trip_idx)?;
                        }
                        // Separate the times: the actual arrival (when the bus reaches the stop)
                        // and the boarding time (when the bus departs from the stop).
                        let actual_arrival = trip[trip_stop_idx].arrival;
                        // For further connections, use the departure time.
                        let effective_board = if let Some(target_stop) = target {
                            if stop == target_stop {
                                actual_arrival
                            } else {
                                trip[trip_stop_idx].departure
                            }
                        } else {
                            trip[trip_stop_idx].departure
                        };

                        // Only update if this effective boarding time is an improvement.
                        if state.update(round, stop, actual_arrival, effective_board)? {
                            state.marked_stops.set(stop, true);
                        }
                        // Prune if we've already exceeded the target bound.
                        if effective_board >= target_bound {
                            break;
                        }
                    }
                }
            }

            let new_marks = process_foot_paths(data, target, num_stops, &mut state, round)?;
            state.marked_stops.union_with(&new_marks);

            // Check if we should continue with this round
            if let Some(target_stop) = target {
                let arrival_time = state.curr_arrival_times[target_stop];
                let target_bound = state.best_arrival[target_stop];

                // If the arrival time in this round is worse than our best known time,
                // there's no point continuing
                if arrival_time != Time::MAX && arrival_time > target_bound {
                    break;
                }
            }

            // If no stops were marked in this round, we can stop.
            if state.marked_stops.is_clear() {
                break;
            }
        }

        // After processing rounds for this departure, record the result for the target.
        if let Some(target_stop) = target {
            let best_arr = state.best_arrival[target_stop];
            let best_round = if best_arr == Time::MAX {
                0
            } else {
                state.best_transfer_count[target_stop]
            };

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
    }

    Ok(journeys)
}
