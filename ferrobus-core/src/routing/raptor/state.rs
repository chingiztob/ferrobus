use fixedbitset::FixedBitSet;
use thiserror::Error;

use crate::{PublicTransitData, RouteId, Time};

#[derive(Debug)]
pub(crate) struct RaptorState {
    // For each round and stop, we now store both the journey’s arrival time
    // and the effective boarding time (usually the trip’s departure time).
    pub(crate) arrival_times: Vec<Vec<Time>>,
    pub(crate) board_times: Vec<Vec<Time>>,
    pub(crate) marked_stops: Vec<FixedBitSet>,
    // For reporting the final journey arrival time.
    pub(crate) best_arrival: Vec<Time>,
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
}

impl RaptorState {
    pub(crate) fn new(num_stops: usize, max_rounds: usize) -> Self {
        RaptorState {
            arrival_times: vec![vec![Time::MAX; num_stops]; max_rounds],
            board_times: vec![vec![Time::MAX; num_stops]; max_rounds],
            marked_stops: (0..max_rounds)
                .map(|_| FixedBitSet::with_capacity(num_stops))
                .collect(),
            best_arrival: vec![Time::MAX; num_stops],
        }
    }

    pub(crate) fn update(
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
pub(crate) fn find_earliest_trip(
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

/// Result of the RAPTOR algorithm.
#[derive(Debug)]
pub enum RaptorResult {
    SingleTarget {
        arrival_time: Option<Time>,
        transfers_used: usize,
    },
    AllTargets(Vec<Time>),
}
