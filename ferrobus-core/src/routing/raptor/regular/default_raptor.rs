use crate::model::Transfer;
use crate::routing::raptor::common::{
    RaptorError, RaptorResult, RaptorState, create_route_queue, find_earliest_trip,
    find_earliest_trip_at_stop, get_target_bound, process_foot_paths, validate_raptor_inputs,
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

            // find earlist possible "hop" on the route
            if let Some((mut trip_idx, current_board_pos)) = find_earliest_trip_at_stop(
                data,
                route_id,
                stops,
                &state.board_times[prev_round],
                start_pos,
            ) {
                // Attempt to improve downstream stops arrival times with the current trip
                let mut trip = data.get_trip(route_id, trip_idx)?;

                // Propagate the trip downstream.
                for (trip_stop_idx, &stop) in stops.iter().enumerate().skip(current_board_pos) {
                    // Check if we can "upgrade" the trip at this stop.
                    // This is possible, if one of the stops can be reached earlier
                    // with a different chain of transfers.
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
                            actual_arrival // For target, report arrival.
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
        // Find the actual round where the best arrival was achieved
        let transfers_used = (0..=max_transfers)
            .find(|&round| {
                state.arrival_times[round][target_stop] == state.best_arrival[target_stop]
            })
            .unwrap_or(max_transfers);

        Ok(RaptorResult::SingleTarget {
            arrival_time: best_time,
            transfers_used,
        })
    } else {
        Ok(RaptorResult::AllTargets(state.best_arrival))
    }
}
