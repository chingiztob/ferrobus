use fixedbitset::FixedBitSet;
use thiserror::Error;

use crate::{PublicTransitData, RouteId, Time};

#[derive(Debug)]
pub struct RaptorState {
    // For each round and stop, we now store both the journey’s arrival time
    // and the effective boarding time (usually the trip’s departure time).
    pub arrival_times: Vec<Vec<Time>>,
    pub board_times: Vec<Vec<Time>>,
    pub marked_stops: Vec<FixedBitSet>,
    // For reporting the final journey arrival time.
    pub best_arrival: Vec<Time>,
}

#[derive(Error, Debug, PartialEq)]
pub enum RaptorError {
    #[error("Invalid stop ID")]
    InvalidStop,
    #[error("Invalid route ID")]
    InvalidRoute,
    #[error("Invalid trip index")]
    InvalidTrip,
    #[error("Invalid time value")]
    InvalidTime,
    #[error("Maximum transfers exceeded")]
    MaxTransfersExceeded,
    #[error("Invalid jorney")]
    InvalidJourney,
}

/// Common validation and setup for RAPTOR algorithms
pub fn validate_raptor_inputs(
    data: &PublicTransitData,
    source: usize,
    target: Option<usize>,
    departure_time: Time,
) -> Result<(), RaptorError> {
    // Validate inputs
    data.validate_stop(source)?;
    if let Some(target_stop) = target {
        data.validate_stop(target_stop)?;
    }
    if departure_time > 86400 * 2 {
        return Err(RaptorError::InvalidTime);
    }

    Ok(())
}

/// Get the target pruning bound for early termination
pub fn get_target_bound(state: &RaptorState, target: Option<usize>) -> Time {
    if let Some(target_stop) = target {
        state.best_arrival[target_stop]
    } else {
        Time::MAX
    }
}

impl RaptorState {
    pub fn new(num_stops: usize, max_rounds: usize) -> Self {
        RaptorState {
            arrival_times: vec![vec![Time::MAX; num_stops]; max_rounds],
            board_times: vec![vec![Time::MAX; num_stops]; max_rounds],
            marked_stops: (0..max_rounds)
                .map(|_| FixedBitSet::with_capacity(num_stops))
                .collect(),
            best_arrival: vec![Time::MAX; num_stops],
        }
    }

    pub fn update(
        &mut self,
        round: usize,
        stop: usize,
        arrival: Time,
        board: Time,
    ) -> Result<bool, RaptorError> {
        if round >= self.arrival_times.len() || stop >= self.arrival_times[0].len() {
            return Err(RaptorError::MaxTransfersExceeded);
        }
        // Only update if the new arrival time is better than what we've seen in this round
        if arrival < self.arrival_times[round][stop] {
            self.arrival_times[round][stop] = arrival;
            self.board_times[round][stop] = board;

            // Update best_arrival if this is better than any previous round
            if arrival < self.best_arrival[stop] {
                self.best_arrival[stop] = arrival;
                return Ok(true); // Return true ONLY if we made a true improvement
            }
        }
        Ok(false) // No improvement
    }
}

// When searching for a trip, we now use the board_times value from the previous round.
pub fn find_earliest_trip(
    data: &PublicTransitData,
    route_id: RouteId,
    stop_idx: usize,
    earliest_board: Time,
) -> Option<usize> {
    let route = &data.routes[route_id];
    let trips_offset = route.trips_start;
    let num_stops = route.num_stops;
    let mut low = 0;
    let mut high = route.num_trips;
    let mut result = None;
    while low < high {
        let mid = (low + high) / 2;
        let trip_start = trips_offset + mid * num_stops;
        // Here, we consider the departure time for boarding.
        let departure = data.stop_times[trip_start + stop_idx].departure;
        if departure >= earliest_board {
            result = Some(mid);
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    result
}

/// Find the earliest trip at a given stop on a route
/// Returns (`trip_idx`, `board_pos`) if found, None otherwise
pub fn find_earliest_trip_at_stop(
    data: &PublicTransitData,
    route_id: usize,
    stops: &[usize],
    board_times: &[Time],
    start_pos: usize,
) -> std::option::Option<(usize, usize)> {
    let mut current_trip_opt = None;
    let mut current_board_pos = 0;

    // Find the earliest trip on this route that is catchable
    for (idx, &stop) in stops.iter().enumerate().skip(start_pos) {
        let earliest_board = board_times[stop];
        if earliest_board == Time::MAX {
            continue;
        }
        if let Some(trip_idx) = find_earliest_trip(data, route_id, idx, earliest_board) {
            current_trip_opt = Some((trip_idx, idx));
            current_board_pos = idx;
            break;
        }
    }

    current_trip_opt.map(|(idx, _)| (idx, current_board_pos))
}

/// Result of the RAPTOR algorithm.
#[derive(Debug)]
pub enum RaptorResult {
    SingleTarget {
        arrival_time: Option<Time>,
        transfers_used: usize,
    },
    AllTargets(Vec<Time>),
}
