use fixedbitset::FixedBitSet;
use std::collections::VecDeque;

use crate::model::transit::types::Transfer;
use crate::routing::raptor::common::{
    RaptorError, RaptorResult, RaptorState, find_earliest_trip, find_earliest_trip_at_stop,
    get_target_bound, validate_raptor_inputs,
};
use crate::{PublicTransitData, RaptorStopId, Time};

#[allow(clippy::too_many_lines)]
pub fn raptor(
    data: &PublicTransitData,
    source: RaptorStopId,
    target: Option<RaptorStopId>,
    departure_time: Time,
    max_transfers: usize,
) -> Result<RaptorResult, RaptorError> {
    // Validate inputs using the common function
    validate_raptor_inputs(data, source, target, departure_time)?;

    let num_stops = data.stops.len();
    let max_rounds = max_transfers + 1;
    let mut state = RaptorState::new(num_stops, max_rounds);

    // Initialize round 0.
    // At the source, both the arrival time and the boarding time are the departure_time.
    state.update(0, source, departure_time, departure_time)?;
    state.marked_stops[0].set(source, true);

    // Process foot-path transfers from the source.
    let transfers = data.get_stop_transfers(source)?;
    for &Transfer {
        target_stop,
        duration,
        ..
    } in transfers
    {
        let new_time = departure_time.saturating_add(duration);
        // For foot-paths we assume no waiting time (arrival equals boarding).
        if state.update(0, target_stop, new_time, new_time)? {
            state.marked_stops[0].set(target_stop, true);
        }
    }

    // Main rounds.
    for round in 1..max_rounds {
        let prev_round = round - 1;

        let mut queue = create_route_queue(data, &state.marked_stops[prev_round])?;
        state.marked_stops[prev_round].clear();

        // When a target is given, use its best known arrival time for pruning.
        let target_bound = get_target_bound(&state, target);

        while let Some((route_id, start_pos)) = queue.pop_front() {
            let stops = data.get_route_stops(route_id)?;

            // Use shared function to find earliest trip
            if let Some((trip_idx, current_board_pos)) = find_earliest_trip_at_stop(
                data,
                route_id,
                stops,
                &state.board_times[prev_round],
                start_pos,
            ) {
                let mut trip_idx = trip_idx;
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

        // If a target is given, check if we can prune the search.
        if let Some(target_stop) = target {
            let arrival_time = state.arrival_times[round][target_stop];
            let target_bound = state.best_arrival[target_stop];

            // If the arrival time in this round is worse than our best known time,
            // there's no point continuing
            if arrival_time != Time::MAX && arrival_time > target_bound {
                return Ok(RaptorResult::SingleTarget {
                    arrival_time: Some(target_bound),
                    transfers_used: prev_round,
                });
            }
        }

        // If no stops were marked in this round, we can stop.
        if state.marked_stops[round].is_clear() {
            break;
        }
    }

    // Report final result.
    if let Some(target_stop) = target {
        let best_time = Some(state.best_arrival[target_stop]).filter(|&t| t != Time::MAX);
        Ok(RaptorResult::SingleTarget {
            arrival_time: best_time,
            transfers_used: max_transfers,
        })
    } else {
        Ok(RaptorResult::AllTargets(state.best_arrival))
    }
}

pub(crate) fn process_foot_paths(
    data: &PublicTransitData,
    target: Option<usize>,
    num_stops: usize,
    state: &mut RaptorState,
    round: usize,
) -> Result<FixedBitSet, RaptorError> {
    let current_marks: Vec<RaptorStopId> = state.marked_stops[round].ones().collect();
    let mut new_marks = FixedBitSet::with_capacity(num_stops);
    let target_bound = if let Some(target_stop) = target {
        state.best_arrival[target_stop]
    } else {
        Time::MAX
    };
    for stop in current_marks {
        let current_board = state.board_times[round][stop];
        let transfers = data.get_stop_transfers(stop)?;
        for &Transfer {
            target_stop,
            duration,
            ..
        } in transfers
        {
            let new_time = current_board.saturating_add(duration);
            if new_time >= state.board_times[round][target_stop] || new_time >= target_bound {
                continue;
            }
            // For transfers, assume arrival equals boarding.
            if state.update(round, target_stop, new_time, new_time)? {
                new_marks.set(target_stop, true);
            }
        }
    }
    Ok(new_marks)
}

pub(crate) fn create_route_queue(
    data: &PublicTransitData,
    marked_stops: &FixedBitSet,
) -> Result<VecDeque<(usize, usize)>, RaptorError> {
    let mut queue = VecDeque::new();

    for route_id in 0..data.routes.len() {
        let stops = data.get_route_stops(route_id)?;
        if let Some(pos) = stops.iter().position(|&stop| marked_stops.contains(stop)) {
            queue.push_back((route_id, pos));
        }
    }

    Ok(queue)
}
