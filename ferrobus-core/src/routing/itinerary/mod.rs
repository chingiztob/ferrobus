mod detailed_journey;
mod journey_leg;
mod to_geojson;

pub use detailed_journey::DetailedJourney;
pub use journey_leg::WalkingLeg;

use crate::{
    Error, MAX_CANDIDATE_STOPS, RaptorStopId, Time, TransitModel,
    model::TransitPoint,
    routing::raptor::{Journey, TracedRaptorResult, traced_raptor},
};

/// Traced multimodal routing from one point to another.
pub fn traced_multimodal_routing(
    transit_model: &TransitModel,
    start_point: &TransitPoint,
    end_point: &TransitPoint,
    departure_time: Time,
    max_transfers: usize,
) -> Result<Option<DetailedJourney>, Error> {
    let transit_data = &transit_model.transit_data;
    let direct_walking = start_point.walking_time_to(end_point);
    let access_candidates =
        &start_point.nearest_stops[..start_point.nearest_stops.len().min(MAX_CANDIDATE_STOPS)];
    let egress_candidates =
        &end_point.nearest_stops[..end_point.nearest_stops.len().min(MAX_CANDIDATE_STOPS)];
    let min_egress_time = egress_candidates.first().map_or(Time::MAX, |&(_, t)| t);

    debug_assert!(access_candidates.windows(2).all(|w| w[0].1 <= w[1].1));
    debug_assert!(egress_candidates.windows(2).all(|w| w[0].1 <= w[1].1));

    let mut best_total_time: Option<Time> = None;
    let mut best_journey: Option<Journey> = None;
    let mut best_access_stop: Option<RaptorStopId> = None;
    let mut best_egress_stop: Option<RaptorStopId> = None;
    let mut best_access_time = 0;
    let mut best_egress_time = 0;

    for &(access_stop, access_time) in access_candidates {
        let bound = best_total_time
            .unwrap_or(Time::MAX)
            .min(direct_walking.unwrap_or(Time::MAX));
        if access_time.saturating_add(min_egress_time) >= bound {
            break;
        }

        for &(egress_stop, egress_time) in egress_candidates {
            let bound = best_total_time
                .unwrap_or(Time::MAX)
                .min(direct_walking.unwrap_or(Time::MAX));
            if access_time.saturating_add(egress_time) >= bound {
                break;
            }

            // Phase 2: run traced RAPTOR for the pair.
            let Ok(TracedRaptorResult::SingleTarget(Some(journey))) = traced_raptor(
                transit_data,
                access_stop,
                Some(egress_stop),
                departure_time + access_time,
                max_transfers,
            ) else {
                continue;
            };

            // Phase 3: evaluate and store best candidate.
            let transit_time = journey.arrival_time.saturating_sub(journey.departure_time);
            let candidate_total_time = access_time
                .saturating_add(transit_time)
                .saturating_add(egress_time);
            if best_total_time.is_none_or(|best| candidate_total_time < best) {
                best_total_time = Some(candidate_total_time);
                best_journey = Some(journey);
                best_access_stop = Some(access_stop);
                best_egress_stop = Some(egress_stop);
                best_access_time = access_time;
                best_egress_time = egress_time;
            }
        }
    }

    if let Some(walk_time) = direct_walking
        && best_total_time.is_none_or(|best| walk_time <= best)
    {
        return Ok(Some(DetailedJourney::walking_only(
            start_point,
            end_point,
            departure_time,
            walk_time,
        )));
    }

    if let (Some(journey), Some(access_stop), Some(egress_stop)) =
        (best_journey, best_access_stop, best_egress_stop)
    {
        return Ok(Some(DetailedJourney::with_transit(
            start_point,
            end_point,
            transit_data,
            access_stop,
            egress_stop,
            best_access_time,
            best_egress_time,
            journey,
            departure_time,
        )));
    }

    if let Some(walk_time) = direct_walking {
        return Ok(Some(DetailedJourney::walking_only(
            start_point,
            end_point,
            departure_time,
            walk_time,
        )));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use geo::Point;
    use hashbrown::HashMap;
    use osm4routing::NodeId;
    use petgraph::graph::{NodeIndex, UnGraph};

    use super::traced_multimodal_routing;
    use crate::loading::build_rtree;
    use crate::model::{
        FeedMeta, PublicTransitData, Route, Stop, StopTime, StreetEdge, StreetGraph, StreetNode,
        TransitModel, TransitModelMeta, TransitPoint, Trip,
    };

    fn build_connected_model() -> TransitModel {
        let mut graph = UnGraph::new_undirected();
        let n0 = graph.add_node(StreetNode {
            id: NodeId(1),
            geometry: Point::new(0.0, 0.0),
        });
        let n1 = graph.add_node(StreetNode {
            id: NodeId(2),
            geometry: Point::new(1.0, 0.0),
        });
        graph.add_edge(n0, n1, StreetEdge { weight: 20 });

        let street_graph = StreetGraph {
            rtree: build_rtree(&graph),
            graph,
        };

        let transit_data = PublicTransitData {
            routes: vec![Route {
                num_trips: 1,
                num_stops: 2,
                stops_start: 0,
                trips_start: 0,
                route_id: "R0".to_string(),
            }],
            route_stops: vec![0, 1],
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
                    routes_len: 1,
                    transfers_start: 0,
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S1".to_string(),
                    geometry: Point::new(1.0, 0.0),
                    routes_start: 1,
                    routes_len: 1,
                    transfers_start: 0,
                    transfers_len: 0,
                },
            ],
            stop_routes: vec![0, 0],
            transfers: vec![],
            node_to_stop: HashMap::from([(n0, 0usize), (n1, 1usize)]),
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

    fn build_pruning_model() -> TransitModel {
        let mut graph = UnGraph::new_undirected();
        let n0 = graph.add_node(StreetNode {
            id: NodeId(11),
            geometry: Point::new(0.0, 0.0),
        });
        let n1 = graph.add_node(StreetNode {
            id: NodeId(12),
            geometry: Point::new(1.0, 0.0),
        });
        let street_graph = StreetGraph {
            rtree: build_rtree(&graph),
            graph,
        };

        let transit_data = PublicTransitData {
            routes: vec![Route {
                num_trips: 1,
                num_stops: 2,
                stops_start: 0,
                trips_start: 0,
                route_id: "R0".to_string(),
            }],
            route_stops: vec![0, 1],
            stop_times: vec![
                StopTime {
                    arrival: 100,
                    departure: 100,
                },
                StopTime {
                    arrival: 120,
                    departure: 120,
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
                    routes_len: 1,
                    transfers_start: 0,
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S2".to_string(),
                    geometry: Point::new(2.0, 0.0),
                    routes_start: 2,
                    routes_len: 0,
                    transfers_start: 0,
                    transfers_len: 0,
                },
                Stop {
                    stop_id: "S3".to_string(),
                    geometry: Point::new(3.0, 0.0),
                    routes_start: 2,
                    routes_len: 0,
                    transfers_start: 0,
                    transfers_len: 0,
                },
            ],
            stop_routes: vec![0, 0],
            transfers: vec![],
            node_to_stop: HashMap::from([(n0, 0usize), (n1, 1usize)]),
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

    #[test]
    fn traced_multimodal_prefers_direct_walking_when_faster() {
        let model = build_connected_model();
        let start = TransitPoint {
            geometry: Point::new(0.0, 0.0),
            node_id: NodeIndex::new(0),
            nearest_stops: vec![(0, 0)],
            walking_paths: HashMap::from([(NodeIndex::new(0), 0), (NodeIndex::new(1), 20)]),
        };
        let end = TransitPoint {
            geometry: Point::new(1.0, 0.0),
            node_id: NodeIndex::new(1),
            nearest_stops: vec![(1, 0)],
            walking_paths: HashMap::from([(NodeIndex::new(1), 0)]),
        };

        let journey = traced_multimodal_routing(&model, &start, &end, 100, 1)
            .expect("routing should succeed")
            .expect("journey should exist");

        assert!(journey.transit_journey.is_none());
        assert_eq!(journey.walking_time, 20);
        assert_eq!(journey.total_time, 20);
        assert_eq!(journey.arrival_time, 120);
    }

    #[test]
    fn traced_multimodal_pruning_keeps_best_sorted_candidate() {
        let model = build_pruning_model();
        let start = TransitPoint {
            geometry: Point::new(0.0, 0.0),
            node_id: NodeIndex::new(0),
            nearest_stops: vec![(0, 0), (2, 30)],
            walking_paths: HashMap::new(),
        };
        let end = TransitPoint {
            geometry: Point::new(1.0, 0.0),
            node_id: NodeIndex::new(1),
            nearest_stops: vec![(1, 0), (3, 5)],
            walking_paths: HashMap::new(),
        };

        let journey = traced_multimodal_routing(&model, &start, &end, 100, 1)
            .expect("routing should succeed")
            .expect("journey should exist");

        assert!(journey.transit_journey.is_some());
        assert_eq!(journey.total_time, 20);
        assert_eq!(journey.arrival_time, 120);
        assert_eq!(
            journey.access_leg.as_ref().map(|leg| leg.to_name.as_str()),
            Some("S0")
        );
        assert_eq!(
            journey
                .egress_leg
                .as_ref()
                .map(|leg| leg.from_name.as_str()),
            Some("S1")
        );
    }
}
