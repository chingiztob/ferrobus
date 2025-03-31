use fixedbitset::FixedBitSet;
use hashbrown::HashMap;
use log::warn;
use std::collections::VecDeque;

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
    // (You need to implement this helper if it does not already exist.)
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
        for &(t_stop, duration) in transfers {
            if t_stop >= num_stops {
                warn!("Invalid transfer target {t_stop}");
                continue;
            }
            let new_time = dep_time.saturating_add(duration);
            state.update(0, t_stop, new_time, new_time)?;
            state.marked_stops[0].set(t_stop, true);
        }

        // For rounds 1..max_rounds, first carry over improvements from the previous round.
        for round in 1..max_rounds {
            // Carry-over step: if the previous round has a better boarding time, propagate it.
            for stop in 0..num_stops {
                if state.board_times[round - 1][stop] < state.board_times[round][stop] {
                    state.arrival_times[round][stop] = state.arrival_times[round - 1][stop];
                    state.board_times[round][stop] = state.board_times[round - 1][stop];
                    state.marked_stops[round].set(stop, true);
                }
            }
            if state.marked_stops[round - 1].count_ones(..num_stops) == 0 {
                break;
            }

            // Build a set of stops marked in the previous round.
            let mut is_marked = vec![false; num_stops];
            for stop in state.marked_stops[round - 1].ones() {
                is_marked[stop] = true;
            }
            // Build a queue of routes that serve any of these stops.
            let mut route_queue: HashMap<RaptorStopId, usize> =
                HashMap::with_capacity(data.routes.len() / 4);
            for (route_id, _route) in data.routes.iter().enumerate() {
                let stops = data.get_route_stops(route_id)?;
                let mut best_pos: Option<usize> = None;
                for (i, &stop) in stops.iter().enumerate() {
                    if is_marked[stop] {
                        best_pos = Some(i);
                        break;
                    }
                }
                if let Some(pos) = best_pos {
                    route_queue
                        .entry(route_id)
                        .and_modify(|existing_pos| {
                            if pos < *existing_pos {
                                *existing_pos = pos;
                            }
                        })
                        .or_insert(pos);
                }
            }
            state.marked_stops[round - 1].clear();

            // Process each route in the queue.
            let mut queue: VecDeque<(RaptorStopId, usize)> =
                VecDeque::with_capacity(route_queue.len());
            queue.extend(route_queue.into_iter());
            while let Some((route_id, start_pos)) = queue.pop_front() {
                let stops = data.get_route_stops(route_id)?;
                let mut current_trip_opt = None;
                let mut current_board_pos = 0;
                // Find the earliest trip on this route that is catchable.
                for (idx, &stop) in stops.iter().enumerate().skip(start_pos) {
                    let earliest_board = state.board_times[round - 1][stop];
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
                    // Use target_bound for pruning (if a target is specified).
                    let target_bound = if let Some(target_stop) = target {
                        state.best_arrival[target_stop]
                    } else {
                        Time::MAX
                    };
                    // Process the stops along the route using a while-loop so that updates
                    // to current_board_pos are used.
                    let mut idx = current_board_pos;
                    while idx < stops.len() {
                        let stop = stops[idx];
                        let prev_board = state.board_times[round - 1][stop];
                        if prev_board < trip[idx].departure {
                            if let Some(new_trip_idx) =
                                find_earliest_trip(data, route_id, idx, prev_board)
                            {
                                if new_trip_idx != trip_idx {
                                    trip_idx = new_trip_idx;
                                    trip = data.get_trip(route_id, new_trip_idx)?;
                                    //current_board_pos = idx;
                                }
                            }
                        }
                        let actual_arrival = trip[idx].arrival;
                        let effective_board = if let Some(target_stop) = target {
                            if stop == target_stop {
                                actual_arrival
                            } else {
                                trip[idx].departure
                            }
                        } else {
                            trip[idx].departure
                        };
                        if state.update(round, stop, actual_arrival, effective_board)? {
                            state.marked_stops[round].set(stop, true);
                        }
                        if effective_board >= target_bound {
                            break;
                        }
                        idx += 1;
                    }
                }
            }
            // Process foot-path transfers for this round.
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
                for &(t_stop, duration) in transfers {
                    if t_stop >= num_stops {
                        warn!("Invalid transfer target {t_stop}");
                        continue;
                    }
                    let new_time = current_board.saturating_add(duration);
                    if new_time >= state.board_times[round][t_stop] || new_time >= target_bound {
                        continue;
                    }
                    if state.update(round, t_stop, new_time, new_time)? {
                        new_marks.set(t_stop, true);
                    }
                }
            }
            state.marked_stops[round].union_with(&new_marks);
        }

        // After processing rounds for this departure, record the result for the target.
        // We choose the best (earliest arrival) among all rounds.
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
