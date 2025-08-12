use fixedbitset::FixedBitSet;
use std::collections::VecDeque;

use crate::routing::raptor::common::{RaptorError, RaptorState};
use crate::{PublicTransitData, RouteId, Time};

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
        let mid = usize::midpoint(low, high);
        let trip_start = trips_offset + mid * num_stops;

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

pub(crate) fn process_foot_paths(
    data: &PublicTransitData,
    target: Option<usize>,
    num_stops: usize,
    state: &mut RaptorState,
    round: usize,
) -> Result<FixedBitSet, RaptorError> {
    let mut current_marks = Vec::with_capacity(state.marked_stops.count_ones(..));
    for stop in state.marked_stops.ones() {
        current_marks.push(stop);
    }

    let mut new_marks = FixedBitSet::with_capacity(num_stops);

    let target_bound = if let Some(ts) = target {
        state.best_arrival[ts]
    } else {
        Time::MAX
    };

    for stop in current_marks {
        let current_board = state.curr_board_times[stop];
        let transfers = data.get_stop_transfers(stop)?;
        for tr in transfers {
            let target_stop = tr.target_stop;
            let new_time = current_board.saturating_add(tr.duration);

            let prev = state.curr_board_times[target_stop];
            if new_time >= prev || new_time >= target_bound {
                continue;
            }

            // Note: still using the current round number from the caller
            if state.update(round, target_stop, new_time, new_time)? {
                new_marks.set(target_stop, true);
            }
        }
    }

    Ok(new_marks)
}

pub(crate) fn create_route_queue(
    data: &PublicTransitData,
    marked_stops: &FixedBitSet,
) -> Result<VecDeque<(usize, usize)>, RaptorError> {
    let mut queue = VecDeque::new();

    for route_id in 0..data.routes.len() {
        let stops = data.get_route_stops(route_id)?;
        if let Some(pos) = stops.iter().position(|&stop| marked_stops.contains(stop)) {
            queue.push_back((route_id, pos));
        }
    }

    Ok(queue)
}
