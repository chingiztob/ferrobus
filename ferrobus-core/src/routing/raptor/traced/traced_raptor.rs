use fixedbitset::FixedBitSet;
use std::collections::VecDeque;

use super::state::{TraceRecord, TracedRaptorState};
use crate::PublicTransitData;
use crate::model::Transfer;
use crate::routing::raptor::common::{
    RaptorError, fill_route_queue, find_earliest_trip, find_earliest_trip_at_stop,
    validate_raptor_inputs,
};
use crate::types::{Duration, RaptorStopId, RouteId, Time};

/// Represents a single leg of an itinerary.
#[derive(Debug, Clone)]
pub enum JourneyLeg {
    /// A transit trip segment.
    Transit {
        route_id: RouteId,
        trip_id: String,
        from_stop: RaptorStopId,
        departure_time: Time,
        to_stop: RaptorStopId,
        arrival_time: Time,
    },
    /// A walking transfer between stops.
    Transfer {
        from_stop: RaptorStopId,
        departure_time: Time,
        to_stop: RaptorStopId,
        arrival_time: Time,
        duration: Duration,
    },
    /// A derived waiting period between visible legs.
    Waiting {
        at_stop: RaptorStopId,
        duration: Time,
    },
}

/// Complete journey from source to target.
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
/// Runs RAPTOR while keeping enough information to reconstruct a public journey.
pub fn traced_raptor(
    data: &PublicTransitData,
    source: RaptorStopId,
    target: Option<RaptorStopId>,
    departure_time: Time,
    max_transfers: usize,
) -> Result<TracedRaptorResult, RaptorError> {
    validate_raptor_inputs(data, source, target, departure_time)?;

    let num_stops = data.stops.len();
    let max_rounds = max_transfers + 1;
    let mut state = TracedRaptorState::new(num_stops, max_rounds);
    let mut route_seen = FixedBitSet::with_capacity(data.routes.len());
    let mut route_queue = VecDeque::new();

    initialize_source_round(data, &mut state, source, departure_time)?;

    for round in 1..max_rounds {
        scan_routes_for_round(
            data,
            target,
            &mut state,
            round,
            &mut route_seen,
            &mut route_queue,
        )?;
        process_detailed_foot_paths(data, target, &mut state, round)?;

        if let Some(target_stop) = target {
            let arrival_time = state.rounds[round].arrival_times[target_stop];
            if arrival_time != Time::MAX && arrival_time > state.best_arrival[target_stop] {
                let journey = reconstruct_journey(data, &state, source, target_stop)?;
                return Ok(TracedRaptorResult::SingleTarget(Some(journey)));
            }
        }

        if state.rounds[round].marked_stops.is_clear() {
            break;
        }
    }

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

/// Seeds round 0 with the source stop and its immediate footpaths.
fn initialize_source_round(
    data: &PublicTransitData,
    state: &mut TracedRaptorState,
    source: RaptorStopId,
    departure_time: Time,
) -> Result<(), RaptorError> {
    state.update(0, source, departure_time, departure_time, || {
        TraceRecord::Source
    })?;
    state.rounds[0].marked_stops.set(source, true);

    let transfers = data.get_stop_transfers(source)?;
    for &Transfer {
        target_stop,
        duration,
        ..
    } in transfers
    {
        let arrival_time = departure_time.saturating_add(duration);
        if state.update(0, target_stop, arrival_time, arrival_time, || {
            TraceRecord::TransferSegment {
                from_stop: source,
                departure_time,
                arrival_time,
                duration,
            }
        })? {
            state.rounds[0].marked_stops.set(target_stop, true);
        }
    }

    Ok(())
}

/// Performs the route-scanning step for one traced RAPTOR round.
fn scan_routes_for_round(
    data: &PublicTransitData,
    target: Option<RaptorStopId>,
    state: &mut TracedRaptorState,
    round: usize,
    route_seen: &mut FixedBitSet,
    route_queue: &mut VecDeque<(usize, usize)>,
) -> Result<(), RaptorError> {
    let prev_round = round - 1;
    fill_route_queue(
        data,
        &state.rounds[prev_round].marked_stops,
        route_seen,
        route_queue,
    )?;
    state.rounds[prev_round].marked_stops.clear();

    let target_bound = state.get_target_bound(target);
    while let Some((route_id, start_pos)) = route_queue.pop_front() {
        let stops = data.get_route_stops(route_id)?;
        if let Some((trip_idx, current_board_pos)) = find_earliest_trip_at_stop(
            data,
            route_id,
            stops,
            &state.rounds[prev_round].board_times,
            start_pos,
        ) {
            let mut trip_idx = trip_idx;
            let mut trip = data.get_trip(route_id, trip_idx)?;
            let mut boarding_idx = current_board_pos;

            for (trip_stop_idx, &stop) in stops.iter().enumerate().skip(current_board_pos) {
                let prev_board = state.rounds[prev_round].board_times[stop];
                if prev_board < trip[trip_stop_idx].departure
                    && let Some(new_trip_idx) =
                        find_earliest_trip(data, route_id, trip_stop_idx, prev_board)
                    && new_trip_idx != trip_idx
                {
                    trip_idx = new_trip_idx;
                    trip = data.get_trip(route_id, new_trip_idx)?;
                    boarding_idx = trip_stop_idx;
                }

                let actual_arrival = trip[trip_stop_idx].arrival;
                let effective_board = if target == Some(stop) {
                    actual_arrival
                } else {
                    trip[trip_stop_idx].departure
                };
                let from_stop = stops[boarding_idx];
                let departure_time = trip[boarding_idx].departure;

                if state.update(round, stop, actual_arrival, effective_board, || {
                    TraceRecord::TransitSegment {
                        route_id,
                        trip_id: trip_idx,
                        from_stop,
                        departure_time,
                        arrival_time: actual_arrival,
                    }
                })? {
                    state.rounds[round].marked_stops.set(stop, true);
                }

                if effective_board >= target_bound {
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Relaxes footpaths inside the current round and records them as transfer segments.
fn process_detailed_foot_paths(
    data: &PublicTransitData,
    target: Option<usize>,
    state: &mut TracedRaptorState,
    round: usize,
) -> Result<(), RaptorError> {
    let num_stops = state.rounds[round].arrival_times.len();
    let current_marks = std::mem::replace(
        &mut state.rounds[round].marked_stops,
        FixedBitSet::with_capacity(num_stops),
    );
    let target_bound = if let Some(target_stop) = target {
        state.best_arrival[target_stop]
    } else {
        Time::MAX
    };

    for stop in current_marks.ones() {
        let current_board = state.rounds[round].board_times[stop];
        let transfers = data.get_stop_transfers(stop)?;
        for &Transfer {
            target_stop,
            duration,
            ..
        } in transfers
        {
            let arrival_time = current_board.saturating_add(duration);
            if arrival_time >= state.rounds[round].board_times[target_stop]
                || arrival_time >= target_bound
            {
                continue;
            }

            if state.update(round, target_stop, arrival_time, arrival_time, || {
                TraceRecord::TransferSegment {
                    from_stop: stop,
                    departure_time: current_board,
                    arrival_time,
                    duration,
                }
            })? {
                state.rounds[round].marked_stops.set(target_stop, true);
            }
        }
    }

    state.rounds[round].marked_stops.union_with(&current_marks);
    Ok(())
}

/// Walks predecessor links back from the target and emits transit/transfer legs only.
fn backtrack_raw_legs(
    data: &PublicTransitData,
    state: &TracedRaptorState,
    source: RaptorStopId,
    target: RaptorStopId,
    start_round: usize,
) -> Result<Vec<JourneyLeg>, RaptorError> {
    let mut legs = Vec::new();
    let mut current_stop = target;
    let mut current_round = start_round;

    while current_stop != source {
        let Some(round_state) = state.rounds.get(current_round) else {
            return Err(RaptorError::InvalidJourney);
        };

        match &round_state.predecessors[current_stop] {
            TraceRecord::None | TraceRecord::Source => return Err(RaptorError::InvalidJourney),
            TraceRecord::TransitSegment {
                route_id,
                trip_id,
                from_stop,
                departure_time,
                arrival_time,
            } => {
                if round_state.arrival_times[current_stop] != *arrival_time || current_round == 0 {
                    return Err(RaptorError::InvalidJourney);
                }

                let trip_id = data
                    .get_trip_id(*route_id, *trip_id)
                    .ok_or(RaptorError::InvalidJourney)?
                    .to_string();
                legs.push(JourneyLeg::Transit {
                    route_id: *route_id,
                    trip_id,
                    from_stop: *from_stop,
                    departure_time: *departure_time,
                    to_stop: current_stop,
                    arrival_time: *arrival_time,
                });

                // Transit edges come from the previous RAPTOR round.
                current_stop = *from_stop;
                current_round -= 1;
            }
            TraceRecord::TransferSegment {
                from_stop,
                departure_time,
                arrival_time,
                duration,
            } => {
                if round_state.arrival_times[current_stop] != *arrival_time {
                    return Err(RaptorError::InvalidJourney);
                }

                legs.push(JourneyLeg::Transfer {
                    from_stop: *from_stop,
                    departure_time: *departure_time,
                    to_stop: current_stop,
                    arrival_time: *arrival_time,
                    duration: *duration,
                });

                // Transfer relaxations are intra-round, so the round stays unchanged.
                current_stop = *from_stop;
            }
        }
    }

    if !matches!(
        state
            .rounds
            .first()
            .map(|round| &round.predecessors[source]),
        Some(TraceRecord::Source)
    ) {
        return Err(RaptorError::InvalidJourney);
    }

    legs.reverse();
    Ok(legs)
}

/// Validates chronology/connectivity and inserts public waiting legs.
fn normalize_legs(raw_legs: Vec<JourneyLeg>) -> Result<Vec<JourneyLeg>, RaptorError> {
    let mut iter = raw_legs.into_iter();
    let Some(mut prev_leg) = iter.next() else {
        return Ok(Vec::new());
    };

    let mut result = Vec::new();
    for next_leg in iter {
        let (prev_to, prev_arrival) = match &prev_leg {
            JourneyLeg::Transit {
                to_stop,
                arrival_time,
                ..
            }
            | JourneyLeg::Transfer {
                to_stop,
                arrival_time,
                ..
            } => (*to_stop, *arrival_time),
            JourneyLeg::Waiting { .. } => return Err(RaptorError::InvalidJourney),
        };

        let (next_from, next_departure) = match &next_leg {
            JourneyLeg::Transit {
                from_stop,
                departure_time,
                ..
            }
            | JourneyLeg::Transfer {
                from_stop,
                departure_time,
                ..
            } => (*from_stop, *departure_time),
            JourneyLeg::Waiting { .. } => return Err(RaptorError::InvalidJourney),
        };

        if prev_to != next_from || next_departure < prev_arrival {
            return Err(RaptorError::InvalidJourney);
        }

        // Zero-duration transfers are synthetic search edges and stay hidden in public output.
        if !matches!(&prev_leg, JourneyLeg::Transfer { duration: 0, .. }) {
            result.push(prev_leg);
        }

        // Waiting is presentation-only and only shown right before boarding transit.
        if matches!(&next_leg, JourneyLeg::Transit { .. }) && next_departure > prev_arrival {
            result.push(JourneyLeg::Waiting {
                at_stop: prev_to,
                duration: next_departure - prev_arrival,
            });
        }

        prev_leg = next_leg;
    }

    if !matches!(&prev_leg, JourneyLeg::Transfer { duration: 0, .. }) {
        result.push(prev_leg);
    }

    Ok(result)
}

/// Reconstructs one final public journey from traced round state.
fn reconstruct_journey(
    data: &PublicTransitData,
    state: &TracedRaptorState,
    source: RaptorStopId,
    target: RaptorStopId,
) -> Result<Journey, RaptorError> {
    let best_round = state
        .best_round_for(target)
        .ok_or(RaptorError::InvalidJourney)?;
    let raw_legs = backtrack_raw_legs(data, state, source, target, best_round)?;
    let legs = normalize_legs(raw_legs)?;
    let transfers_count = legs
        .iter()
        .filter(|leg| matches!(leg, JourneyLeg::Transfer { .. }))
        .count();

    Ok(Journey {
        legs,
        departure_time: state.rounds[0].board_times[source],
        arrival_time: state.best_arrival[target],
        transfers_count,
    })
}

#[cfg(test)]
mod tests {
    use geo::Point;
    use hashbrown::HashMap;

    use super::{
        JourneyLeg, TracedRaptorResult, backtrack_raw_legs, reconstruct_journey, traced_raptor,
    };
    use crate::model::{FeedMeta, PublicTransitData, Route, Stop, StopTime, Transfer, Trip};
    use crate::routing::raptor::traced::state::{TraceRecord, TracedRaptorState};

    fn build_test_data_with_colocated_transfer() -> PublicTransitData {
        let stops = vec![
            Stop {
                stop_id: "S0".to_string(),
                geometry: Point::new(0.0, 0.0),
                routes_start: 0,
                routes_len: 0,
                transfers_start: 0,
                transfers_len: 1,
            },
            Stop {
                stop_id: "S1".to_string(),
                geometry: Point::new(0.0, 0.0),
                routes_start: 0,
                routes_len: 1,
                transfers_start: 1,
                transfers_len: 0,
            },
            Stop {
                stop_id: "S2".to_string(),
                geometry: Point::new(1.0, 1.0),
                routes_start: 1,
                routes_len: 1,
                transfers_start: 1,
                transfers_len: 0,
            },
        ];

        PublicTransitData {
            routes: vec![Route {
                num_trips: 1,
                num_stops: 2,
                stops_start: 0,
                trips_start: 0,
                route_id: "R0".to_string(),
            }],
            route_stops: vec![1, 2],
            stop_times: vec![
                StopTime {
                    arrival: 100,
                    departure: 100,
                },
                StopTime {
                    arrival: 200,
                    departure: 200,
                },
            ],
            stops,
            stop_routes: vec![0, 0],
            transfers: vec![Transfer {
                target_stop: 1,
                duration: 0,
            }],
            node_to_stop: HashMap::new(),
            feeds_meta: Vec::<FeedMeta>::new(),
            trips: vec![vec![Trip {
                trip_id: "T0".to_string(),
            }]],
            gtfs_transfers: vec![],
        }
    }

    fn build_manual_trace_data() -> PublicTransitData {
        PublicTransitData {
            routes: vec![
                Route {
                    num_trips: 1,
                    num_stops: 2,
                    stops_start: 0,
                    trips_start: 0,
                    route_id: "R0".to_string(),
                },
                Route {
                    num_trips: 1,
                    num_stops: 2,
                    stops_start: 2,
                    trips_start: 2,
                    route_id: "R1".to_string(),
                },
            ],
            route_stops: vec![0, 1, 1, 2],
            stop_times: vec![
                StopTime {
                    arrival: 100,
                    departure: 100,
                },
                StopTime {
                    arrival: 110,
                    departure: 110,
                },
                StopTime {
                    arrival: 120,
                    departure: 120,
                },
                StopTime {
                    arrival: 130,
                    departure: 130,
                },
            ],
            stops: vec![
                Stop {
                    stop_id: "S0".to_string(),
                    geometry: Point::new(0.0, 0.0),
                    routes_start: 0,
                    routes_len: 1,
                    transfers_start: 0,
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S1".to_string(),
                    geometry: Point::new(1.0, 0.0),
                    routes_start: 1,
                    routes_len: 2,
                    transfers_start: 0,
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S2".to_string(),
                    geometry: Point::new(2.0, 0.0),
                    routes_start: 3,
                    routes_len: 1,
                    transfers_start: 0,
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S3".to_string(),
                    geometry: Point::new(3.0, 0.0),
                    routes_start: 4,
                    routes_len: 0,
                    transfers_start: 0,
                    transfers_len: 0,
                },
            ],
            stop_routes: vec![0, 0, 1, 1],
            transfers: vec![],
            node_to_stop: HashMap::new(),
            feeds_meta: Vec::<FeedMeta>::new(),
            trips: vec![
                vec![Trip {
                    trip_id: "T0".to_string(),
                }],
                vec![Trip {
                    trip_id: "T1".to_string(),
                }],
            ],
            gtfs_transfers: vec![],
        }
    }

    fn build_round_mark_retention_data() -> PublicTransitData {
        PublicTransitData {
            routes: vec![
                Route {
                    num_trips: 1,
                    num_stops: 2,
                    stops_start: 0,
                    trips_start: 0,
                    route_id: "R0".to_string(),
                },
                Route {
                    num_trips: 1,
                    num_stops: 2,
                    stops_start: 2,
                    trips_start: 2,
                    route_id: "R1".to_string(),
                },
            ],
            route_stops: vec![0, 1, 1, 3],
            stop_times: vec![
                StopTime {
                    arrival: 100,
                    departure: 100,
                },
                StopTime {
                    arrival: 110,
                    departure: 110,
                },
                StopTime {
                    arrival: 120,
                    departure: 120,
                },
                StopTime {
                    arrival: 130,
                    departure: 130,
                },
            ],
            stops: vec![
                Stop {
                    stop_id: "S0".to_string(),
                    geometry: Point::new(0.0, 0.0),
                    routes_start: 0,
                    routes_len: 1,
                    transfers_start: 0,
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S1".to_string(),
                    geometry: Point::new(1.0, 0.0),
                    routes_start: 1,
                    routes_len: 2,
                    transfers_start: 0,
                    transfers_len: 1,
                },
                Stop {
                    stop_id: "S2".to_string(),
                    geometry: Point::new(2.0, 0.0),
                    routes_start: 3,
                    routes_len: 0,
                    transfers_start: 1,
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S3".to_string(),
                    geometry: Point::new(3.0, 0.0),
                    routes_start: 3,
                    routes_len: 1,
                    transfers_start: 1,
                    transfers_len: 0,
                },
            ],
            stop_routes: vec![0, 0, 1, 1],
            transfers: vec![Transfer {
                target_stop: 2,
                duration: 5,
            }],
            node_to_stop: HashMap::new(),
            feeds_meta: Vec::<FeedMeta>::new(),
            trips: vec![
                vec![Trip {
                    trip_id: "T0".to_string(),
                }],
                vec![Trip {
                    trip_id: "T1".to_string(),
                }],
            ],
            gtfs_transfers: vec![],
        }
    }

    fn build_waiting_state() -> TracedRaptorState {
        let mut state = TracedRaptorState::new(3, 3);
        state
            .update(0, 0, 100, 100, || TraceRecord::Source)
            .expect("source update should succeed");
        state
            .update(1, 1, 110, 110, || TraceRecord::TransitSegment {
                route_id: 0,
                trip_id: 0,
                from_stop: 0,
                departure_time: 100,
                arrival_time: 110,
            })
            .expect("first transit update should succeed");
        state
            .update(2, 2, 130, 130, || TraceRecord::TransitSegment {
                route_id: 1,
                trip_id: 0,
                from_stop: 1,
                departure_time: 120,
                arrival_time: 130,
            })
            .expect("second transit update should succeed");
        state
    }

    fn build_invariant_state() -> TracedRaptorState {
        let mut state = TracedRaptorState::new(4, 2);
        state
            .update(0, 0, 100, 100, || TraceRecord::Source)
            .expect("source update should succeed");
        state
            .update(0, 1, 105, 105, || TraceRecord::TransferSegment {
                from_stop: 0,
                departure_time: 100,
                arrival_time: 105,
                duration: 5,
            })
            .expect("first transfer update should succeed");
        state
            .update(0, 2, 108, 108, || TraceRecord::TransferSegment {
                from_stop: 1,
                departure_time: 105,
                arrival_time: 108,
                duration: 3,
            })
            .expect("second transfer update should succeed");
        state
            .update(1, 3, 130, 130, || TraceRecord::TransitSegment {
                route_id: 1,
                trip_id: 0,
                from_stop: 2,
                departure_time: 120,
                arrival_time: 130,
            })
            .expect("transit update should succeed");
        state
    }

    #[test]
    fn traced_journey_hides_zero_duration_transfer_legs() {
        let data = build_test_data_with_colocated_transfer();

        let result = traced_raptor(&data, 0, Some(2), 50, 1).expect("traced raptor should succeed");
        let TracedRaptorResult::SingleTarget(Some(journey)) = result else {
            panic!("expected a single target journey")
        };

        assert!(
            journey
                .legs
                .iter()
                .all(|leg| !matches!(leg, JourneyLeg::Transfer { duration: 0, .. })),
            "zero-duration synthetic transfer legs must be hidden in output"
        );
        assert_eq!(journey.transfers_count, 0);
        assert!(journey.legs.iter().any(|leg| matches!(
            leg,
            JourneyLeg::Transit {
                from_stop: 1,
                to_stop: 2,
                ..
            }
        )));
    }

    #[test]
    fn reconstruct_inserts_waiting_between_transit_legs() {
        let data = build_manual_trace_data();
        let state = build_waiting_state();

        let journey = reconstruct_journey(&data, &state, 0, 2).expect("journey should reconstruct");
        assert_eq!(journey.departure_time, 100);
        assert_eq!(journey.arrival_time, 130);
        assert!(matches!(journey.legs[0], JourneyLeg::Transit { .. }));
        assert!(matches!(
            journey.legs[1],
            JourneyLeg::Waiting {
                at_stop: 1,
                duration: 10
            }
        ));
        assert!(matches!(journey.legs[2], JourneyLeg::Transit { .. }));
    }

    #[test]
    fn backtracked_journeys_preserve_time_and_transfer_invariants() {
        let data = build_manual_trace_data();
        let state = build_invariant_state();

        let journey = reconstruct_journey(&data, &state, 0, 3).expect("journey should reconstruct");
        let mut current_time = journey.departure_time;
        let mut current_stop = None;
        let mut visible_transfer_count = 0;

        for (index, leg) in journey.legs.iter().enumerate() {
            match leg {
                JourneyLeg::Transfer {
                    from_stop,
                    to_stop,
                    departure_time,
                    arrival_time,
                    ..
                } => {
                    if index == 0 {
                        current_stop = Some(*from_stop);
                    }
                    assert_eq!(current_stop, Some(*from_stop));
                    assert_eq!(*departure_time, current_time);
                    assert!(*arrival_time >= *departure_time);
                    current_time = *arrival_time;
                    current_stop = Some(*to_stop);
                    visible_transfer_count += 1;
                }
                JourneyLeg::Transit {
                    from_stop,
                    to_stop,
                    departure_time,
                    arrival_time,
                    ..
                } => {
                    if index == 0 {
                        current_stop = Some(*from_stop);
                    }
                    assert_eq!(current_stop, Some(*from_stop));
                    assert_eq!(*departure_time, current_time);
                    assert!(*arrival_time >= *departure_time);
                    current_time = *arrival_time;
                    current_stop = Some(*to_stop);
                }
                JourneyLeg::Waiting { at_stop, duration } => {
                    assert_eq!(current_stop, Some(*at_stop));
                    current_time += *duration;
                }
            }
        }

        assert_eq!(current_time, journey.arrival_time);
        assert_eq!(visible_transfer_count, journey.transfers_count);

        let raw_legs =
            backtrack_raw_legs(&data, &state, 0, 3, 1).expect("raw legs should backtrack");
        assert_eq!(raw_legs.len(), 3);
        assert!(matches!(raw_legs[0], JourneyLeg::Transfer { .. }));
        assert!(matches!(raw_legs[1], JourneyLeg::Transfer { .. }));
        assert!(matches!(raw_legs[2], JourneyLeg::Transit { .. }));
    }

    #[test]
    fn round_marks_keep_route_progress_after_same_round_transfer_relaxation() {
        let data = build_round_mark_retention_data();

        let result =
            traced_raptor(&data, 0, Some(3), 100, 2).expect("traced raptor should succeed");
        let TracedRaptorResult::SingleTarget(Some(journey)) = result else {
            panic!("expected a journey to target stop 3")
        };

        assert_eq!(journey.arrival_time, 130);
        assert!(journey.legs.iter().any(|leg| matches!(
            leg,
            JourneyLeg::Transit {
                from_stop: 0,
                to_stop: 1,
                ..
            }
        )));
        assert!(journey.legs.iter().any(|leg| matches!(
            leg,
            JourneyLeg::Transit {
                from_stop: 1,
                to_stop: 3,
                ..
            }
        )));
    }
}
