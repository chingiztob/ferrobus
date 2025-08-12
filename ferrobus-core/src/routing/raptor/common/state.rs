use std::mem;

use fixedbitset::FixedBitSet;
use thiserror::Error;

use crate::PublicTransitData;
use crate::types::{RaptorStopId, Time};

#[derive(Debug)]
pub struct RaptorState {
    pub prev_arrival_times: Vec<Time>,
    pub prev_board_times: Vec<Time>,
    pub curr_arrival_times: Vec<Time>,
    pub curr_board_times: Vec<Time>,
    pub marked_stops: FixedBitSet,
    pub best_arrival: Vec<Time>,
    pub best_transfer_count: Vec<usize>,
}

impl RaptorState {
    pub fn new(num_stops: usize, _max_rounds: usize) -> Self {
        RaptorState {
            prev_arrival_times: vec![Time::MAX; num_stops],
            prev_board_times: vec![Time::MAX; num_stops],
            curr_arrival_times: vec![Time::MAX; num_stops],
            curr_board_times: vec![Time::MAX; num_stops],
            marked_stops: FixedBitSet::with_capacity(num_stops),
            best_arrival: vec![Time::MAX; num_stops],
            best_transfer_count: vec![0; num_stops],
        }
    }

    pub fn update(
        &mut self,
        round: usize,
        stop: RaptorStopId,
        arrival: Time,
        board: Time,
    ) -> Result<bool, RaptorError> {
        if stop >= self.curr_arrival_times.len() {
            return Err(RaptorError::MaxTransfersExceeded);
        }

        // Only update if the new arrival time is better than what we've seen in this round
        if arrival < self.curr_arrival_times[stop] {
            self.curr_arrival_times[stop] = arrival;
            self.curr_board_times[stop] = board;

            // Update best_arrival if this is better than any previous round
            if arrival < self.best_arrival[stop] {
                self.best_arrival[stop] = arrival;
                self.best_transfer_count[stop] = round;
                return Ok(true);
            }
        }
        Ok(false)
    }

    // Prepare for next round by swapping current and previous data
    pub fn advance_round(&mut self) {
        mem::swap(&mut self.prev_arrival_times, &mut self.curr_arrival_times);
        mem::swap(&mut self.prev_board_times, &mut self.curr_board_times);

        self.curr_arrival_times.fill(Time::MAX);
        self.curr_board_times.fill(Time::MAX);
    }
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

/// Per-target result. `arrival_time == Time::MAX` means unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetResult {
    pub arrival_time: Time,
    pub transfers_used: usize,
}

impl TargetResult {
    #[inline]
    #[allow(unused)]
    pub fn unreachable() -> Self {
        Self {
            arrival_time: Time::MAX,
            transfers_used: 0,
        }
    }

    #[inline]
    pub fn is_reachable(&self) -> bool {
        self.arrival_time != Time::MAX
    }
}

/// Result of the RAPTOR algorithm.
#[derive(Debug)]
pub enum RaptorResult {
    /// Single target result
    SingleTarget(TargetResult),
    /// All targets indexed by stop id
    AllTargets(Vec<TargetResult>),
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
