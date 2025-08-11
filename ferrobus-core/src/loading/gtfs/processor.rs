use chrono::{Datelike, Weekday};
use geo::Point;
use hashbrown::{HashMap, HashSet};

use super::{
    de::deserialize_gtfs_file,
    raw_types::{FeedCalendarDates, FeedInfo, FeedService, FeedStop, FeedStopTime, FeedTrip},
};
use crate::{
    Error, RaptorStopId, RouteId,
    model::{PublicTransitData, Route, Stop, StopTime},
};
use crate::{loading::config::TransitModelConfig, model::FeedMeta};

/// Create public transit data model from GTFS files
///
/// # Panics
///
/// If a `stop_sequence` cannot be parsed as a u32
pub fn transit_model_from_gtfs(config: &TransitModelConfig) -> Result<PublicTransitData, Error> {
    let (stops, mut trips, mut stop_times, services, feed_info_vec, calendar_dates) =
        load_raw_feed(config)?;

    let feeds_meta = feed_info_vec
        .into_iter()
        .map(|info| FeedMeta { feed_info: info })
        .collect::<Vec<_>>();

    filter_trips_by_service_day(
        config,
        &services,
        &mut trips,
        &mut stop_times,
        &calendar_dates,
    );

    // Map from trip_id to vec of stop times
    let mut trip_stop_times: HashMap<String, Vec<FeedStopTime>> = HashMap::new();
    for stop_time in stop_times {
        trip_stop_times
            .entry(stop_time.trip_id.clone())
            .or_default()
            .push(stop_time);
    }

    trip_stop_times
        .values_mut()
        .for_each(|v| v.sort_by_key(|s| s.stop_sequence));

    // Process trips
    let (stop_times, route_stops, routes_vec) =
        process_trip_stop_times(&stops, &trips, &trip_stop_times);
    drop(trip_stop_times);

    // Key raptor transit data model vectors
    let mut stop_routes: Vec<RouteId> = Vec::new();
    let mut stops_vec = create_stops_vector(stops);

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
    calendar_dates: &[FeedCalendarDates],
) {
    // Create set of service_id for the selected day of the week
    // if date is not provided, return without filtering trips
    let Some(date) = config.date else { return };

    // process regular services
    let mut active_services: HashSet<&str> = services
        .iter()
        .filter(|service| match date.weekday() {
            Weekday::Mon => service.monday == "1",
            Weekday::Tue => service.tuesday == "1",
            Weekday::Wed => service.wednesday == "1",
            Weekday::Thu => service.thursday == "1",
            Weekday::Fri => service.friday == "1",
            Weekday::Sat => service.saturday == "1",
            Weekday::Sun => service.sunday == "1",
        })
        .map(|s| s.service_id.as_str())
        .collect();

    // Filter calendar_dates exceptions
    for cd in calendar_dates.iter().filter(|cd| cd.date == Some(date)) {
        match cd.exception_type {
            1 => {
                active_services.insert(cd.service_id.as_str());
            }
            2 => {
                active_services.remove(cd.service_id.as_str());
            }
            _ => {}
        }
    }

    // Filter trips and respective stop_times by day of the week
    trips.retain(|trip| active_services.contains(trip.service_id.as_str()));
    let active_trips: HashSet<&str> = trips.iter().map(|trip| trip.trip_id.as_str()).collect();
    stop_times.retain(|st| active_trips.contains(st.trip_id.as_str()));
}

fn process_trip_stop_times<'a>(
    stops: &[FeedStop],
    trips: &[FeedTrip],
    trip_stop_times: &'a HashMap<String, Vec<FeedStopTime>>, // trip id to stop times
) -> (Vec<StopTime>, Vec<usize>, Vec<Route>) {
    let stop_id_map: HashMap<&str, usize> = stops
        .iter()
        .enumerate()
        .map(|(i, s)| (s.stop_id.as_str(), i))
        .collect();
    let trip_id_map: HashMap<&str, &str> = trips
        .iter()
        .map(|t| (t.trip_id.as_str(), t.route_id.as_str()))
        .collect();

    // Group trips by route id
    let mut routes_map: HashMap<&str, Vec<&'a [FeedStopTime]>> = HashMap::new();
    trip_stop_times.iter().for_each(|(trip_id, sts)| {
        if let Some(&route_id) = trip_id_map.get(trip_id.as_str()) {
            routes_map.entry(route_id).or_default().push(sts.as_slice());
        }
    });

    // flattened array with all arrivals/departures for every trip
    let total_stop_times: usize = trip_stop_times.values().map(std::vec::Vec::len).sum();
    let mut stop_times_vec = Vec::with_capacity(total_stop_times);
    // stop indices per route pattern
    let mut route_stops = Vec::new();
    // metadata (route_id, number of trips, number of stops, and offsets into the other two vectors)
    let mut routes_vec = Vec::new();

    // Process each route group.
    for (route_id, trips) in routes_map {
        // Not all trips have the same number of stops, but Raptor requires a fixed number of stops per route.
        // So route will be added in few variations, each with a different number of stops.
        let mut groups_by_length: HashMap<usize, Vec<&'a [FeedStopTime]>> = HashMap::new();
        for ts in trips {
            groups_by_length.entry(ts.len()).or_default().push(ts);
        }

        for (num_stops, mut group) in groups_by_length {
            // Sort trips by departure time at the first stop.
            group.sort_by_key(|ts| &ts[0].departure_time);

            // Use the first trip as representative for the stop order.
            let representative = group[0];
            let stops_start = route_stops.len();
            // Build the route's stop sequence.
            for st in representative {
                if let Some(&idx) = stop_id_map.get(st.stop_id.as_str()) {
                    route_stops.push(idx);
                }
            }

            // Record the starting index for this subgroup's trips.
            let trips_start = stop_times_vec.len();

            group.iter().flat_map(|trip| trip.iter()).for_each(|st| {
                let arrival = if st.stop_sequence == 0 {
                    st.departure_time
                } else {
                    st.arrival_time
                };
                stop_times_vec.push(StopTime {
                    arrival,
                    departure: st.departure_time,
                });
            });

            routes_vec.push(Route {
                num_trips: group.len(),
                num_stops,
                stops_start,
                trips_start,
                route_id: route_id.to_string(),
            });
        }
    }
    (stop_times_vec, route_stops, routes_vec)
}

fn create_stops_vector(stops: Vec<FeedStop>) -> Vec<Stop> {
    stops
        .into_iter()
        .map(|s| Stop {
            stop_id: s.stop_id,
            geometry: Point::new(s.stop_lon, s.stop_lat),
            routes_start: 0,
            routes_len: 0,
            transfers_start: 0,
            transfers_len: 0,
        })
        .collect()
}

type RawGTFSmodel = (
    Vec<FeedStop>,
    Vec<FeedTrip>,
    Vec<FeedStopTime>,
    Vec<FeedService>,
    Vec<FeedInfo>,
    Vec<FeedCalendarDates>,
);

fn load_raw_feed(config: &TransitModelConfig) -> Result<RawGTFSmodel, Error> {
    let mut stops: Vec<FeedStop> = Vec::new();
    let mut trips: Vec<FeedTrip> = Vec::new();
    let mut stop_times: Vec<FeedStopTime> = Vec::new();
    let mut services: Vec<FeedService> = Vec::new();
    let mut feed_info_vec: Vec<FeedInfo> = Vec::new();
    let mut calendar_dates: Vec<FeedCalendarDates> = Vec::new();
    for dir in &config.gtfs_dirs {
        stops.extend(deserialize_gtfs_file(&dir.join("stops.txt"))?);
        trips.extend(deserialize_gtfs_file(&dir.join("trips.txt"))?);
        stop_times.extend(deserialize_gtfs_file(&dir.join("stop_times.txt"))?);
        services.extend(deserialize_gtfs_file(&dir.join("calendar.txt"))?);

        // This file is optional, so we can safely ignore errors
        feed_info_vec.extend(deserialize_gtfs_file(&dir.join("feed_info.txt")).unwrap_or_default());
        calendar_dates
            .extend(deserialize_gtfs_file(&dir.join("calendar_dates.txt")).unwrap_or_default());
    }
    stops.shrink_to_fit();
    trips.shrink_to_fit();
    stop_times.shrink_to_fit();
    services.shrink_to_fit();
    Ok((
        stops,
        trips,
        stop_times,
        services,
        feed_info_vec,
        calendar_dates,
    ))
}
