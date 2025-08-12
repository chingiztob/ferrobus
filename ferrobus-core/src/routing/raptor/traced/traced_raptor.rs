use fixedbitset::FixedBitSet;
use itertools::Itertools;

use super::state::{Predecessor, TracedRaptorState};
use crate::PublicTransitData;
use crate::model::Transfer;
use crate::routing::raptor::common::create_route_queue;
use crate::routing::raptor::common::{RaptorError, find_earliest_trip};
use crate::types::{Duration, RaptorStopId, RouteId, Time, TripId};

/// Represents a single leg of an itinerary
#[derive(Debug, Clone)]
pub enum JourneyLeg {
    /// A transit trip segment
    Transit {
        route_id: RouteId,
        trip_id: TripId,
        from_stop: RaptorStopId,
        departure_time: Time,
        to_stop: RaptorStopId,
        arrival_time: Time,
    },
    /// A walking transfer between stops
    Transfer {
        from_stop: RaptorStopId,
        departure_time: Time,
        to_stop: RaptorStopId,
        arrival_time: Time,
        duration: Duration,
    },
    Waiting {
        at_stop: RaptorStopId,
        duration: Time,
    },
}

/// Complete journey from source to target
#[derive(Debug, Clone)]
pub struct Journey {
    pub legs: Vec<JourneyLeg>,
    pub departure_time: Time,
    pub arrival_time: Time,
    pub transfers_count: usize,
}

#[allow(unused)]
pub enum TracedRaptorResult {
    SingleTarget(Option<Journey>),
    AllTargets(Vec<Option<Journey>>),
}

#[allow(clippy::too_many_lines)]
pub fn traced_raptor(
    data: &PublicTransitData,
    source: RaptorStopId,
    target: Option<RaptorStopId>,
    departure_time: Time,
    max_transfers: usize,
) -> Result<TracedRaptorResult, RaptorError> {
    crate::routing::raptor::common::validate_raptor_inputs(data, source, target, departure_time)?;

    let num_stops = data.stops.len();
    let max_rounds = max_transfers + 1;
    let mut state = TracedRaptorState::new(num_stops, max_rounds);

    // Initialize round 0
    state.update(
        0,
        source,
        departure_time,
        departure_time,
        Predecessor::Source,
    )?;
    state.marked_stops[0].set(source, true);

    // Process foot-path transfers from the source
    let transfers = data.get_stop_transfers(source)?;
    for &Transfer {
        target_stop,
        duration,
        ..
    } in transfers
    {
        let new_time = departure_time.saturating_add(duration);
        if state.update(
            0,
            target_stop,
            new_time,
            new_time,
            Predecessor::Transfer {
                from_stop: source,
                departure_time,
                duration,
            },
        )? {
            state.marked_stops[0].set(target_stop, true);
        }
    }

    // Main rounds
    for round in 1..max_rounds {
        let prev_round = round - 1;

        let mut queue = create_route_queue(data, &state.marked_stops[prev_round])?;
        state.marked_stops[prev_round].clear();

        let target_bound = state.get_target_bound(target);

        while let Some((route_id, start_pos)) = queue.pop_front() {
            let stops = data.get_route_stops(route_id)?;

            if let Some((trip_idx, current_board_pos)) =
                crate::routing::raptor::common::find_earliest_trip_at_stop(
                    data,
                    route_id,
                    stops,
                    &state.board_times[prev_round],
                    start_pos,
                )
            {
                let mut trip_idx = trip_idx;
                let mut trip = data.get_trip(route_id, trip_idx)?;
                let mut boarding_idx = current_board_pos;

                for (trip_stop_idx, &stop) in stops.iter().enumerate().skip(current_board_pos) {
                    // Check if we can "upgrade" to an earlier trip
                    let prev_board = state.board_times[prev_round][stop];
                    if prev_board < trip[trip_stop_idx].departure
                        && let Some(new_trip_idx) =
                            find_earliest_trip(data, route_id, trip_stop_idx, prev_board)
                        && new_trip_idx != trip_idx
                    {
                        trip_idx = new_trip_idx;
                        trip = data.get_trip(route_id, new_trip_idx)?;
                        // Re-board at the current stop on the new trip
                        boarding_idx = trip_stop_idx;
                    }

                    let actual_arrival = trip[trip_stop_idx].arrival;
                    let effective_board = if let Some(target_stop) = target {
                        if stop == target_stop {
                            actual_arrival
                        } else {
                            trip[trip_stop_idx].departure
                        }
                    } else {
                        trip[trip_stop_idx].departure
                    };

                    if state.update(
                        round,
                        stop,
                        actual_arrival,
                        effective_board,
                        Predecessor::Transit {
                            route_id,
                            trip_id: trip_idx,
                            from_stop: stops[boarding_idx],
                            from_idx: boarding_idx,
                            to_idx: trip_stop_idx,
                        },
                    )? {
                        state.marked_stops[round].set(stop, true);
                    }

                    if effective_board >= target_bound {
                        break;
                    }
                }
            }
        }

        // Process footpaths for this round
        let new_marks = process_detailed_foot_paths(data, target, num_stops, &mut state, round)?;
        state.marked_stops[round].union_with(&new_marks);

        // Check if we can terminate early
        if let Some(target_stop) = target {
            let arrival_time = state.arrival_times[round][target_stop];
            if arrival_time != Time::MAX && arrival_time > state.best_arrival[target_stop] {
                let journey = reconstruct_journey(data, &state, source, target_stop)?;
                return Ok(TracedRaptorResult::SingleTarget(Some(journey)));
            }
        }

        // If no stops were marked in this round, we can stop
        if state.marked_stops[round].is_clear() {
            break;
        }
    }

    // Reconstruct journeys
    if let Some(target_stop) = target {
        let journey = if state.best_arrival[target_stop] == Time::MAX {
            None
        } else {
            Some(reconstruct_journey(data, &state, source, target_stop)?)
        };
        Ok(TracedRaptorResult::SingleTarget(journey))
    } else {
        let mut journeys = vec![None; num_stops];

        #[allow(clippy::needless_range_loop)]
        for stop in 0..num_stops {
            if state.best_arrival[stop] != Time::MAX {
                journeys[stop] = Some(reconstruct_journey(data, &state, source, stop)?);
            }
        }
        Ok(TracedRaptorResult::AllTargets(journeys))
    }
}

fn process_detailed_foot_paths(
    data: &PublicTransitData,
    target: Option<usize>,
    num_stops: usize,
    state: &mut TracedRaptorState,
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

            // For transfers, track the source stop and duration
            if state.update(
                round,
                target_stop,
                new_time,
                new_time,
                Predecessor::Transfer {
                    from_stop: stop,
                    departure_time: current_board,
                    duration,
                },
            )? {
                new_marks.set(target_stop, true);
            }
        }
    }

    Ok(new_marks)
}

fn reconstruct_journey(
    data: &PublicTransitData,
    state: &TracedRaptorState,
    source: RaptorStopId,
    target: RaptorStopId,
) -> Result<Journey, RaptorError> {
    let mut legs = Vec::new();
    let mut current_stop = target;
    let mut current_round = 0;

    // Find which round has the best arrival time for target
    for round in 0..state.arrival_times.len() {
        if state.arrival_times[round][target] == state.best_arrival[target] {
            current_round = round;
            break;
        }
    }

    let arrival_time = state.best_arrival[target];

    // Backtrack from target to source
    while current_stop != source {
        match &state.predecessors[current_round][current_stop] {
            Predecessor::None => {
                return Err(RaptorError::InvalidJourney);
            }
            Predecessor::Source => {
                // We've reached the source
                break;
            }
            Predecessor::Transit {
                route_id,
                trip_id,
                from_stop,
                from_idx,
                to_idx,
            } => {
                let trip = data.get_trip(*route_id, *trip_id)?;
                legs.push(JourneyLeg::Transit {
                    route_id: *route_id,
                    trip_id: *trip_id,
                    from_stop: *from_stop,
                    departure_time: trip[*from_idx].departure,
                    to_stop: current_stop,
                    arrival_time: trip[*to_idx].arrival,
                });

                current_stop = *from_stop;
                current_round -= 1;
            }
            Predecessor::Transfer {
                from_stop,
                departure_time,
                duration,
            } => {
                legs.push(JourneyLeg::Transfer {
                    from_stop: *from_stop,
                    departure_time: *departure_time,
                    to_stop: current_stop,
                    arrival_time: departure_time.saturating_add(*duration),
                    duration: *duration,
                });

                // Move to previous stop, same round (transfers are handled in same round)
                current_stop = *from_stop;
            }
        }
    }

    // Legs are in reverse order (target to source), so reverse them
    legs.reverse();

    // Add "waiting" points to result
    let mut walking_legs = Vec::new();

    // Iterate over journeys with window, if next departure is `transit` , then calculate delay
    // between this departure and out arrival on that stop
    for (idx, (prev_leg, next_leg)) in legs.iter().tuple_windows().enumerate() {
        if let (
            JourneyLeg::Transit { arrival_time, .. } | JourneyLeg::Transfer { arrival_time, .. },
            JourneyLeg::Transit {
                from_stop,
                departure_time,
                ..
            },
        ) = (prev_leg, next_leg)
        {
            walking_legs.push((
                idx,
                JourneyLeg::Waiting {
                    at_stop: *from_stop,
                    duration: (*departure_time - *arrival_time),
                },
            ));
        }
    }
    // Shift accounts for elements shifting on each insert, +1 alligns waits to correct position (cringe)
    for (shift, (idx, leg)) in walking_legs.into_iter().enumerate() {
        legs.insert(idx + shift + 1, leg);
    }

    let transfers_count = legs
        .iter()
        .filter(|leg| matches!(leg, JourneyLeg::Transfer { .. }))
        .count();

    Ok(Journey {
        legs,
        departure_time: state.board_times[0][source],
        arrival_time,
        transfers_count,
    })
}
