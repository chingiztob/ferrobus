use hashbrown::HashMap;

use crate::{
    Error, MAX_CANDIDATE_STOPS, Time, TransitModel,
    model::TransitPoint,
    routing::raptor::{RaptorError, RaptorResult, raptor},
};

/// Combined multimodal route result
#[derive(Debug, Clone)]
pub struct MultiModalResult {
    // Total journey time
    pub travel_time: Time,
    // Time spent on transit (None if walking only)
    pub transit_time: Option<Time>,
    // Time spent walking
    pub walking_time: Time,
    // Number of transfers used
    pub transfers: usize,
}

/// Internal struct to track transit route candidates
#[derive(Debug, Clone)]
pub(crate) struct CandidateJourney {
    pub(crate) total_time: Time,
    pub(crate) transit_time: Time,
    pub(crate) transfers_used: usize,
}

/// Checks if direct walking is better than the transit option
pub(crate) fn is_walking_better(
    walking_time: Option<Time>,
    transit_candidate: Option<&CandidateJourney>,
) -> bool {
    match (walking_time, transit_candidate) {
        (Some(walking), Some(transit)) => walking <= transit.total_time,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Creates a `MultiModalResult` for a walking-only journey
pub(crate) fn create_walking_result(walking_time: Time) -> MultiModalResult {
    MultiModalResult {
        travel_time: walking_time,
        transit_time: None,
        walking_time,
        transfers: 0,
    }
}

/// Creates a `MultiModalResult` for a transit journey
pub(crate) fn create_transit_result(candidate: &CandidateJourney) -> MultiModalResult {
    let walking_time = candidate.total_time - candidate.transit_time;

    MultiModalResult {
        travel_time: candidate.total_time,
        transit_time: Some(candidate.transit_time),
        walking_time,
        transfers: candidate.transfers_used,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn map_raptor_error(err: RaptorError) -> Error {
    Error::InvalidData(format!("RAPTOR error: {err}"))
}

///Combined multimodal routing function
pub fn multimodal_routing(
    transit_model: &TransitModel,
    start_point: &TransitPoint,
    end_point: &TransitPoint,
    departure_time: Time,
    max_transfers: usize,
) -> Result<Option<MultiModalResult>, Error> {
    if departure_time > 86400 * 2 {
        return Err(Error::InvalidData("Invalid departure time".to_string()));
    }

    let transit_data = &transit_model.transit_data;
    let direct_walking = start_point.walking_time_to(end_point);

    let mut best_candidate: Option<CandidateJourney> = None;

    for &(access_stop, access_time) in start_point.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
        for &(egress_stop, egress_time) in end_point.nearest_stops.iter().take(MAX_CANDIDATE_STOPS)
        {
            // Skip if walking path is faster
            if let Some(walking_time) = direct_walking
                && access_time + egress_time >= walking_time
            {
                continue;
            }

            // Skip if we already have a better candidate
            if let Some(candidate) = &best_candidate
                && access_time + egress_time >= candidate.total_time
            {
                continue;
            }

            if let Ok(result) = raptor(
                transit_data,
                access_stop,
                Some(egress_stop),
                departure_time + access_time,
                max_transfers,
            ) {
                match result {
                    RaptorResult::SingleTarget(target) => {
                        if target.is_reachable() {
                            let transit_time = target.arrival_time - (departure_time + access_time);
                            let total_time = access_time + transit_time + egress_time;
                            if target.arrival_time < departure_time + access_time {
                                return Err(Error::InvalidData(format!(
                                    "Negative transit time detected: {} - {} = {}",
                                    target.arrival_time,
                                    departure_time + access_time,
                                    transit_time
                                )));
                            }

                            let candidate = CandidateJourney {
                                total_time,
                                transit_time,
                                transfers_used: target.transfers_used,
                            };

                            // Update if this is better than our current best
                            if best_candidate
                                .as_ref()
                                .is_none_or(|best| candidate.total_time < best.total_time)
                            {
                                best_candidate = Some(candidate);
                            }
                        }
                    }
                    RaptorResult::AllTargets(_) => {
                        unreachable!("Unexpected AllTargets result");
                    }
                }
            }
        }
    }

    // If some candidate transit route was found, check if it's better than walking
    if let Some(candidate) = best_candidate
        && !is_walking_better(direct_walking, Some(&candidate))
    {
        return Ok(Some(create_transit_result(&candidate)));
    }

    // if not - return walking result
    if let Some(walking_time) = direct_walking {
        return Ok(Some(create_walking_result(walking_time)));
    }

    Ok(None)
}

/// Routing from one point to many. It exploits basic RAPTOR principles to
/// calculate transit routes to all stops from the access point, so whole calculation
/// can be done in one raptor run.
pub fn multimodal_routing_one_to_many(
    transit_model: &TransitModel,
    start_point: &TransitPoint,
    end_points: &[TransitPoint],
    departure_time: Time,
    max_transfers: usize,
) -> Result<Vec<Option<MultiModalResult>>, Error> {
    let transit_data = &transit_model.transit_data;
    let mut results = vec![None; end_points.len()];

    // Run RAPTOR to all stops for each initial access point
    let mut transit_results = HashMap::new();

    for &(access_stop, access_time) in start_point.nearest_stops.iter().take(MAX_CANDIDATE_STOPS) {
        match raptor(
            transit_data,
            access_stop,
            None,
            departure_time + access_time,
            max_transfers,
        )
        .map_err(map_raptor_error)?
        {
            RaptorResult::AllTargets(times) => {
                transit_results.insert(access_stop, (access_time, times));
            }
            RaptorResult::SingleTarget(_) => {
                unreachable!("Unexpected SingleTarget result");
            }
        }
    }

    for (end_idx, end_point) in end_points.iter().enumerate() {
        let direct_walking = start_point.walking_time_to(end_point);
        let mut best_candidate: Option<CandidateJourney> = None;

        for (_access_stop, (access_time, transit_times)) in &transit_results {
            for &(egress_stop, egress_time) in &end_point.nearest_stops {
                // Skip if walking path is faster
                if let Some(walking_time) = direct_walking
                    && access_time + egress_time >= walking_time
                {
                    continue;
                }

                // Skip if we already have a better candidate
                if let Some(candidate) = &best_candidate
                    && access_time + egress_time >= candidate.total_time
                {
                    continue;
                }

                if transit_times[egress_stop].is_reachable() {
                    let transit_time = transit_times[egress_stop].arrival_time;
                    let transfers_used = transit_times[egress_stop].transfers_used;

                    let transit_time = transit_time - (departure_time + *access_time);
                    let total_time = *access_time + transit_time + egress_time;

                    let candidate = CandidateJourney {
                        total_time,
                        transit_time,
                        transfers_used,
                    };

                    if best_candidate
                        .as_ref()
                        .is_none_or(|best| candidate.total_time < best.total_time)
                    {
                        best_candidate = Some(candidate);
                    }
                }
            }
        }

        if let Some(candidate) = best_candidate
            && !is_walking_better(direct_walking, Some(&candidate))
        {
            results[end_idx] = Some(create_transit_result(&candidate));
            continue;
        }

        // Either walking is better or no transit option exists
        if let Some(walking_time) = direct_walking {
            results[end_idx] = Some(create_walking_result(walking_time));
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use geo::Point;
    use hashbrown::HashMap;
    use osm4routing::NodeId;
    use petgraph::graph::{NodeIndex, UnGraph};

    use super::{multimodal_routing, multimodal_routing_one_to_many};
    use crate::model::{
        FeedMeta, PublicTransitData, Route, Stop, StopTime, StreetGraph, StreetNode, Transfer,
        TransitModel, TransitModelMeta, TransitPoint, Trip,
    };
    use crate::{Error, loading::build_rtree};

    fn build_model(with_colocated_transfer: bool) -> TransitModel {
        let mut graph = UnGraph::new_undirected();
        let n0 = graph.add_node(StreetNode {
            id: NodeId(1),
            geometry: Point::new(0.0, 0.0),
        });
        let n1 = graph.add_node(StreetNode {
            id: NodeId(2),
            geometry: Point::new(1.0, 1.0),
        });
        let street_graph = StreetGraph {
            rtree: build_rtree(&graph),
            graph,
        };

        // Stops:
        // - 0: canonical stop at n0 used by TransitPoint
        // - 1: hidden co-located stop at n0 used by route
        // - 2: destination stop at n1
        let transfers = if with_colocated_transfer {
            vec![Transfer {
                target_stop: 1,
                duration: 0,
            }]
        } else {
            vec![]
        };

        let transit_data = PublicTransitData {
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
            stops: vec![
                Stop {
                    stop_id: "S0".to_string(),
                    geometry: Point::new(0.0, 0.0),
                    routes_start: 0,
                    routes_len: 0,
                    transfers_start: 0,
                    transfers_len: usize::from(with_colocated_transfer),
                },
                Stop {
                    stop_id: "S1".to_string(),
                    geometry: Point::new(0.0, 0.0),
                    routes_start: 0,
                    routes_len: 1,
                    transfers_start: transfers.len(),
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S2".to_string(),
                    geometry: Point::new(1.0, 1.0),
                    routes_start: 1,
                    routes_len: 1,
                    transfers_start: transfers.len(),
                    transfers_len: 0,
                },
            ],
            // Route R0 serves stops 1 and 2.
            stop_routes: vec![0, 0],
            transfers,
            // Canonical one-stop-per-node mapping.
            node_to_stop: HashMap::from([(n0, 0usize), (n1, 2usize)]),
            feeds_meta: Vec::<FeedMeta>::new(),
            trips: vec![vec![Trip {
                trip_id: "T0".to_string(),
            }]],
            gtfs_transfers: vec![],
        };

        TransitModel {
            transit_data,
            street_graph,
            meta: TransitModelMeta {
                max_transfer_time: 600,
            },
        }
    }

    fn make_points(model: &TransitModel) -> (TransitPoint, TransitPoint) {
        let start = TransitPoint::new(Point::new(0.0, 0.0), model, 600, 5)
            .expect("start point should be valid");
        let end = TransitPoint::new(Point::new(1.0, 1.0), model, 600, 5)
            .expect("end point should be valid");
        (start, end)
    }

    #[test]
    fn colocated_transfer_restores_single_routing_reachability() {
        let model_without = build_model(false);
        let (start_without, end_without) = make_points(&model_without);
        let without = multimodal_routing(&model_without, &start_without, &end_without, 50, 1)
            .expect("routing should not fail");
        assert!(
            without.is_none(),
            "without co-located transfer route should be unreachable"
        );

        let model_with = build_model(true);
        let (start_with, end_with) = make_points(&model_with);
        let with = multimodal_routing(&model_with, &start_with, &end_with, 50, 1)
            .expect("routing should not fail")
            .expect("route should become reachable with co-located transfer");

        assert_eq!(with.travel_time, 150);
        assert_eq!(with.transit_time, Some(150));
        assert_eq!(with.walking_time, 0);
    }

    #[test]
    fn colocated_transfer_restores_one_to_many_reachability() {
        let model = build_model(true);
        let (start, end) = make_points(&model);

        let results = multimodal_routing_one_to_many(&model, &start, &[end], 50, 1)
            .expect("one-to-many routing should not fail");
        assert_eq!(results.len(), 1);

        let route = results[0]
            .as_ref()
            .expect("route should be reachable with co-located transfer");
        assert_eq!(route.travel_time, 150);
        assert_eq!(route.transit_time, Some(150));
        assert_eq!(route.walking_time, 0);
    }

    #[test]
    fn one_to_many_propagates_raptor_errors() {
        let model = build_model(true);
        let invalid_start = TransitPoint {
            geometry: Point::new(0.0, 0.0),
            node_id: NodeIndex::new(0),
            nearest_stops: vec![(usize::MAX, 0)],
            walking_paths: HashMap::new(),
        };
        let end = TransitPoint {
            geometry: Point::new(1.0, 1.0),
            node_id: NodeIndex::new(1),
            nearest_stops: vec![(2, 0)],
            walking_paths: HashMap::new(),
        };

        let result = multimodal_routing_one_to_many(&model, &invalid_start, &[end], 50, 1);
        assert!(matches!(result, Err(Error::InvalidData(_))));
    }
}
