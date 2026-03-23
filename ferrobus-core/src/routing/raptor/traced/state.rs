use fixedbitset::FixedBitSet;

use crate::routing::raptor::common::RaptorError;
use crate::types::{Duration, RaptorStopId, RouteId, Time, TripId};

/// Records the concrete segment used to reach a stop in a round.
#[derive(Debug, Clone)]
pub(crate) enum TraceRecord {
    None,
    Source,
    TransitSegment {
        route_id: RouteId,
        trip_id: TripId,
        from_stop: RaptorStopId,
        departure_time: Time,
        arrival_time: Time,
    },
    TransferSegment {
        from_stop: RaptorStopId,
        departure_time: Time,
        arrival_time: Time,
        duration: Duration,
    },
}

pub(crate) struct TracedRoundState {
    pub arrival_times: Vec<Time>,
    pub board_times: Vec<Time>,
    pub predecessors: Vec<TraceRecord>,
    pub marked_stops: FixedBitSet,
}

impl TracedRoundState {
    fn new(num_stops: usize) -> Self {
        Self {
            arrival_times: vec![Time::MAX; num_stops],
            board_times: vec![Time::MAX; num_stops],
            predecessors: vec![TraceRecord::None; num_stops],
            marked_stops: FixedBitSet::with_capacity(num_stops),
        }
    }
}

pub struct TracedRaptorState {
    pub rounds: Vec<TracedRoundState>,
    pub best_arrival: Vec<Time>,
}

impl TracedRaptorState {
    /// Allocates per-round traced state for one RAPTOR search.
    pub fn new(num_stops: usize, num_rounds: usize) -> Self {
        Self {
            rounds: (0..num_rounds)
                .map(|_| TracedRoundState::new(num_stops))
                .collect(),
            best_arrival: vec![Time::MAX; num_stops],
        }
    }

    /// Applies a candidate update for one stop in one round.
    /// `predecessor` is lazy because most candidates lose, so we avoid building
    /// a `TraceRecord` unless the arrival actually improves the stop.
    pub fn update<F>(
        &mut self,
        round: usize,
        stop: RaptorStopId,
        arrival: Time,
        board: Time,
        predecessor: F,
    ) -> Result<bool, RaptorError>
    where
        F: FnOnce() -> TraceRecord,
    {
        let Some(round_state) = self.rounds.get_mut(round) else {
            return Err(RaptorError::InvalidJourney);
        };

        if stop >= round_state.arrival_times.len() {
            return Err(RaptorError::InvalidStop);
        }

        let mut updated = false;

        if arrival < round_state.arrival_times[stop] {
            round_state.arrival_times[stop] = arrival;
            round_state.predecessors[stop] = predecessor();
            updated = true;
        }

        if board < round_state.board_times[stop] {
            round_state.board_times[stop] = board;
        }

        if arrival < self.best_arrival[stop] {
            self.best_arrival[stop] = arrival;
        }

        Ok(updated)
    }

    /// Returns the best known target arrival used to prune route scans.
    pub fn get_target_bound(&self, target: Option<usize>) -> Time {
        if let Some(target_stop) = target {
            self.best_arrival[target_stop]
        } else {
            Time::MAX
        }
    }

    /// Finds the first round that produced the final best arrival for `target`.
    pub fn best_round_for(&self, target: RaptorStopId) -> Option<usize> {
        let best_arrival = *self.best_arrival.get(target)?;
        if best_arrival == Time::MAX {
            return None;
        }

        self.rounds
            .iter()
            .position(|round| round.arrival_times[target] == best_arrival)
    }
}
