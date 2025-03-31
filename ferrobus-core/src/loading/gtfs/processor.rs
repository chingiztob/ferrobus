use geo::Point;
use hashbrown::{HashMap, HashSet};
use log::warn;

use super::{
    parser::{deserialize_gtfs_file, parse_time},
    raw_types::{FeedInfo, FeedRoute, FeedService, FeedStop, FeedStopTime, FeedTrip},
};
use crate::{
    Error,
    model::{PublicTransitData, RaptorStopId, Route, RouteId, Stop, StopTime},
};
use crate::{loading::config::TransitModelConfig, model::transit::types::FeedMeta};

/// Create public transit data model from GTFS files
///
/// # Panics
///
/// If a `stop_sequence` cannot be parsed as a u32
pub fn transit_model_from_gtfs(config: &TransitModelConfig) -> Result<PublicTransitData, Error> {
    let (stops, mut trips, mut stop_times, services, feed_info_vec) = load_raw_feed(config)?;

    let feeds_meta = feed_info_vec
        .into_iter()
        .map(|info| FeedMeta { feed_info: info })
        .collect::<Vec<_>>();

    filter_trips_by_service_day(config, &services, &mut trips, &mut stop_times);

    // Create maps for fast lookup during conversion
    let stop_id_map: HashMap<String, RaptorStopId> = stops
        .iter()
        .enumerate()
        .map(|(idx, stop)| (stop.stop_id.clone(), idx))
        .collect();

    let trip_id_map: HashMap<&str, usize> = trips
        .iter()
        .enumerate()
        .map(|(idx, trip)| (trip.trip_id.as_str(), idx))
        .collect();

    // Map from trip_id to vec of stop times
    let mut trip_stop_times: HashMap<String, Vec<FeedStopTime>> = HashMap::new();
    for stop_time in stop_times {
        trip_stop_times
            .entry(stop_time.trip_id.clone())
            .or_default()
            .push(stop_time);
    }

    for stop_list in trip_stop_times.values_mut() {
        stop_list.sort_by_key(|s| {
            s.stop_sequence.parse::<u32>().unwrap_or_else(|e| {
                panic!(
                    "Failed to parse stop_sequence {} with Error: {}",
                    s.stop_sequence, e
                );
            })
        });
    }

    // Key raptor transit data model vectors

    let mut stop_routes: Vec<RouteId> = Vec::new();

    // convert Raw GTFS data to Raptor data
    let mut stops_vec = create_stops_vector(stops);
    // Process trips
    let (stop_times, route_stops, routes_vec) =
        process_trip_stop_times(&stop_id_map, &trip_id_map, &trip_stop_times);
    drop(trip_stop_times);

    // Index of routes for each stop
    let mut stop_to_routes: HashMap<RaptorStopId, HashSet<RouteId>> =
        HashMap::with_capacity(stops_vec.len());
    for (route_idx, route) in routes_vec.iter().enumerate() {
        for stop_idx in &route_stops[route.stops_start..route.stops_start + route.num_stops] {
            stop_to_routes
                .entry(*stop_idx)
                .or_default()
                .insert(route_idx);
        }
    }

    // Route index for stops
    for (stop_idx, routes) in stop_to_routes {
        stops_vec[stop_idx].routes_start = stop_routes.len();
        stops_vec[stop_idx].routes_len = routes.len();
        stop_routes.extend(routes);
    }

    Ok(PublicTransitData {
        routes: routes_vec,
        route_stops,
        stop_times,
        stops: stops_vec,
        stop_routes,
        transfers: vec![],            // Will be filled in `calculate_transfers`
        node_to_stop: HashMap::new(), // Empty node to stop mapping initially
        feeds_meta,
    })
}

fn filter_trips_by_service_day(
    config: &TransitModelConfig,
    services: &[FeedService],
    trips: &mut Vec<FeedTrip>,
    stop_times: &mut Vec<FeedStopTime>,
) {
    // Create set of service_id for the selected day of the week
    let active_services: HashSet<&str> = services
        .iter()
        .filter_map(|service| {
            let is_active = match config.day_of_week.as_str() {
                "monday" => service.monday == "1",
                "tuesday" => service.tuesday == "1",
                "wednesday" => service.wednesday == "1",
                "thursday" => service.thursday == "1",
                "friday" => service.friday == "1",
                "saturday" => service.saturday == "1",
                "sunday" => service.sunday == "1",
                _ => false,
            };
            if is_active {
                Some(service.service_id.as_str())
            } else {
                None
            }
        })
        .collect();

    // Filter trips and respective stop_times by day of the week
    trips.retain(|trip| active_services.contains(trip.service_id.as_str()));
    let active_trips = trips
        .iter()
        .map(|trip| trip.trip_id.as_str())
        .collect::<HashSet<&str>>();
    stop_times.retain(|stop_time| active_trips.contains(stop_time.trip_id.as_str()));
}

fn process_trip_stop_times(
    stop_id_map: &HashMap<String, usize>,
    trip_id_map: &HashMap<&str, usize>,
    trip_stop_times: &HashMap<String, Vec<FeedStopTime>>,
) -> (Vec<StopTime>, Vec<usize>, Vec<Route>) {
    let mut stop_times_vec = Vec::new();
    let mut route_stops = Vec::new();
    let mut routes_vec = Vec::new();

    for (trip_id, stop_list) in trip_stop_times {
        let stops_start = route_stops.len();
        let trips_start = stop_times_vec.len();
        let num_stops = stop_list.len();

        for stop_time in stop_list {
            if let Some(&stop_idx) = stop_id_map.get(&stop_time.stop_id) {
                route_stops.push(stop_idx);
                stop_times_vec.push(StopTime {
                    arrival: parse_time(&stop_time.arrival_time),
                    departure: parse_time(&stop_time.departure_time),
                });
            }
        }

        if let Some(&_route_idx) = trip_id_map.get(trip_id.as_str()) {
            routes_vec.push(Route {
                num_trips: 1,
                num_stops,
                stops_start,
                trips_start,
            });
        }
    }

    (stop_times_vec, route_stops, routes_vec)
}

fn create_stops_vector(stops: Vec<FeedStop>) -> Vec<Stop> {
    let stops_vec: Vec<Stop> = stops
        .into_iter()
        .map(|feed_stop| {
            let geometry = Point::new(
                feed_stop.stop_lon.parse::<f64>().unwrap_or_else(|e| {
                    warn!("Invalid stop_lon '{}': {}", feed_stop.stop_lon, e);
                    0.0
                }),
                feed_stop.stop_lat.parse::<f64>().unwrap_or_else(|e| {
                    warn!("Invalid stop_lat '{}': {}", feed_stop.stop_lat, e);
                    0.0
                }),
            );

            Stop {
                stop_id: feed_stop.stop_id,
                geometry,
                routes_start: 0,
                routes_len: 0,
                transfers_start: 0,
                transfers_len: 0,
            }
        })
        .collect();
    stops_vec
}

type RawGTFSmodel = (
    Vec<FeedStop>,
    Vec<FeedTrip>,
    Vec<FeedStopTime>,
    Vec<FeedService>,
    Vec<FeedInfo>,
);

fn load_raw_feed(config: &TransitModelConfig) -> Result<RawGTFSmodel, Error> {
    let mut stops: Vec<FeedStop> = Vec::new();
    let mut routes: Vec<FeedRoute> = Vec::new();
    let mut trips: Vec<FeedTrip> = Vec::new();
    let mut stop_times: Vec<FeedStopTime> = Vec::new();
    let mut services: Vec<FeedService> = Vec::new();
    let mut feed_info_vec: Vec<FeedInfo> = Vec::new();
    for dir in &config.gtfs_dirs {
        stops.extend(deserialize_gtfs_file(&dir.join("stops.txt"))?);
        routes.extend(deserialize_gtfs_file(&dir.join("routes.txt"))?);
        trips.extend(deserialize_gtfs_file(&dir.join("trips.txt"))?);
        stop_times.extend(deserialize_gtfs_file(&dir.join("stop_times.txt"))?);
        services.extend(deserialize_gtfs_file(&dir.join("calendar.txt"))?);
        feed_info_vec.extend(deserialize_gtfs_file(&dir.join("feed_info.txt"))?);
    }
    stops.shrink_to_fit();
    routes.shrink_to_fit();
    trips.shrink_to_fit();
    stop_times.shrink_to_fit();
    services.shrink_to_fit();
    Ok((stops, trips, stop_times, services, feed_info_vec))
}
