use fixedbitset::FixedBitSet;

use crate::routing::raptor::common::RaptorError;
use crate::types::{Duration, RaptorStopId, RouteId, Time, TripId};

/// Records how we arrived at a particular stop in a particular round
#[derive(Debug, Clone)]
pub enum Predecessor {
    None,
    Source,
    Transit {
        route_id: RouteId,
        trip_id: TripId,
        from_stop: RaptorStopId,
        from_idx: usize,
        to_idx: usize,
    },
    Transfer {
        from_stop: RaptorStopId,
        departure_time: Time,
        duration: Duration,
    },
}

pub struct TracedRaptorState {
    pub arrival_times: Vec<Vec<Time>>,
    pub board_times: Vec<Vec<Time>>,
    pub best_arrival: Vec<Time>,
    pub marked_stops: Vec<FixedBitSet>,
    pub predecessors: Vec<Vec<Predecessor>>,
}

impl TracedRaptorState {
    pub fn new(num_stops: usize, max_rounds: usize) -> Self {
        let arrival_times = vec![vec![Time::MAX; num_stops]; max_rounds + 1];
        let board_times = vec![vec![Time::MAX; num_stops]; max_rounds + 1];
        let marked_stops = vec![FixedBitSet::with_capacity(num_stops); max_rounds + 1];
        let predecessors = vec![vec![Predecessor::None; num_stops]; max_rounds + 1];

        Self {
            arrival_times,
            board_times,
            best_arrival: vec![Time::MAX; num_stops],
            marked_stops,
            predecessors,
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn update(
        &mut self,
        round: usize,
        stop: RaptorStopId,
        arrival: Time,
        board: Time,
        predecessor: Predecessor,
    ) -> Result<bool, RaptorError> {
        if stop >= self.arrival_times[round].len() {
            return Err(RaptorError::InvalidStop);
        }

        let mut updated = false;

        // Update arrival time if better
        if arrival < self.arrival_times[round][stop] {
            self.arrival_times[round][stop] = arrival;
            self.predecessors[round][stop] = predecessor;
            updated = true;
        }

        // Update boarding time if better
        if board < self.board_times[round][stop] {
            self.board_times[round][stop] = board;
        }

        // Update best known arrival time
        if arrival < self.best_arrival[stop] {
            self.best_arrival[stop] = arrival;
        }

        Ok(updated)
    }

    /// Get the target bound for pruning
    pub fn get_target_bound(&self, target: Option<usize>) -> Time {
        if let Some(target_stop) = target {
            self.best_arrival[target_stop]
        } else {
            Time::MAX
        }
    }
}
