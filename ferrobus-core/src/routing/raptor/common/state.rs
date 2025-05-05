use fixedbitset::FixedBitSet;
use thiserror::Error;

use crate::{PublicTransitData, Time};

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

/// Result of the RAPTOR algorithm.
#[derive(Debug)]
pub enum RaptorResult {
    SingleTarget {
        arrival_time: Option<Time>,
        transfers_used: usize,
    },
    AllTargets(Vec<Time>),
}

/// Common validation and setup for RAPTOR algorithms
pub fn validate_raptor_inputs(
    data: &PublicTransitData,
    source: usize,
    target: Option<usize>,
    departure_time: Time,
) -> Result<(), RaptorError> {
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
                return Ok(true);
            }
        }
        Ok(false) // No improvement
    }
}
