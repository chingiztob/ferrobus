use chrono::{Datelike, Weekday};
use geo::Point;
use hashbrown::{HashMap, HashSet};

use super::{
    de::deserialize_gtfs_file,
    raw_types::{FeedCalendarDates, FeedInfo, FeedService, FeedStop, FeedStopTime, FeedTrip},
};
use crate::{
    Error, RaptorStopId, RouteId,
    model::{PublicTransitData, Route, Stop, StopTime, Trip},
};
use crate::{loading::config::TransitModelConfig, model::FeedMeta};

struct RawGTFSData {
    stops: Vec<FeedStop>,
    trips: Vec<FeedTrip>,
    stop_times: Vec<FeedStopTime>,
    services: Vec<FeedService>,
    feed_info: Vec<FeedInfo>,
    calendar_dates: Vec<FeedCalendarDates>,
}

struct ServiceFilter<'a> {
    date: chrono::NaiveDate,
    services: &'a [FeedService],
    calendar_dates: &'a [FeedCalendarDates],
}

impl<'a> ServiceFilter<'a> {
    fn new(
        date: chrono::NaiveDate,
        services: &'a [FeedService],
        calendar_dates: &'a [FeedCalendarDates],
    ) -> Self {
        Self {
            date,
            services,
            calendar_dates,
        }
    }

    fn get_active_services(&self) -> HashSet<&str> {
        let mut active_services = self.get_regular_services();
        self.apply_calendar_exceptions(&mut active_services);
        active_services
    }

    fn get_regular_services(&self) -> HashSet<&str> {
        self.services
            .iter()
            .filter(|service| self.is_service_active_on_weekday(service))
            .map(|s| s.service_id.as_str())
            .collect()
    }

    fn is_service_active_on_weekday(&self, service: &FeedService) -> bool {
        match self.date.weekday() {
            Weekday::Mon => service.monday == "1",
            Weekday::Tue => service.tuesday == "1",
            Weekday::Wed => service.wednesday == "1",
            Weekday::Thu => service.thursday == "1",
            Weekday::Fri => service.friday == "1",
            Weekday::Sat => service.saturday == "1",
            Weekday::Sun => service.sunday == "1",
        }
    }

    fn apply_calendar_exceptions(&self, active_services: &mut HashSet<&'a str>) {
        for cd in self
            .calendar_dates
            .iter()
            .filter(|cd| cd.date == Some(self.date))
        {
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
    }
}

/// Create public transit data model from GTFS files
pub fn transit_model_from_gtfs(config: &TransitModelConfig) -> Result<PublicTransitData, Error> {
    let raw_data = load_raw_feed(config)?;
    let filtered_data = filter_data_by_date(config, raw_data);
    let processed_data = process_transit_data(filtered_data);
    Ok(build_public_transit_data(processed_data))
}

fn load_raw_feed(config: &TransitModelConfig) -> Result<RawGTFSData, Error> {
    let mut stops = Vec::new();
    let mut trips = Vec::new();
    let mut stop_times = Vec::new();
    let mut services = Vec::new();
    let mut feed_info = Vec::new();
    let mut calendar_dates = Vec::new();

    for dir in &config.gtfs_dirs {
        stops.extend(deserialize_gtfs_file(&dir.join("stops.txt"))?);
        trips.extend(deserialize_gtfs_file(&dir.join("trips.txt"))?);
        stop_times.extend(deserialize_gtfs_file(&dir.join("stop_times.txt"))?);
        services.extend(deserialize_gtfs_file(&dir.join("calendar.txt"))?);
        feed_info.extend(deserialize_gtfs_file(&dir.join("feed_info.txt")).unwrap_or_default());
        calendar_dates
            .extend(deserialize_gtfs_file(&dir.join("calendar_dates.txt")).unwrap_or_default());
    }

    stops.shrink_to_fit();
    trips.shrink_to_fit();
    stop_times.shrink_to_fit();
    services.shrink_to_fit();

    Ok(RawGTFSData {
        stops,
        trips,
        stop_times,
        services,
        feed_info,
        calendar_dates,
    })
}

fn filter_data_by_date(config: &TransitModelConfig, mut raw_data: RawGTFSData) -> FilteredGTFSData {
    let feeds_meta = raw_data
        .feed_info
        .into_iter()
        .map(|info| FeedMeta { feed_info: info })
        .collect();

    if let Some(date) = config.date {
        let service_filter = ServiceFilter::new(date, &raw_data.services, &raw_data.calendar_dates);
        let active_services = service_filter.get_active_services();

        raw_data
            .trips
            .retain(|trip| active_services.contains(trip.service_id.as_str()));
        let active_trips: HashSet<&str> = raw_data
            .trips
            .iter()
            .map(|trip| trip.trip_id.as_str())
            .collect();
        raw_data
            .stop_times
            .retain(|st| active_trips.contains(st.trip_id.as_str()));
    }

    FilteredGTFSData {
        stops: raw_data.stops,
        trips: raw_data.trips,
        stop_times: raw_data.stop_times,
        feeds_meta,
    }
}

struct FilteredGTFSData {
    stops: Vec<FeedStop>,
    trips: Vec<FeedTrip>,
    stop_times: Vec<FeedStopTime>,
    feeds_meta: Vec<FeedMeta>,
}

fn process_transit_data(filtered_data: FilteredGTFSData) -> ProcessedTransitData {
    let trip_stop_times = group_stop_times_by_trip(filtered_data.stop_times);
    let (stop_times, route_stops, routes, trips) =
        process_trip_stop_times(&filtered_data.stops, &filtered_data.trips, &trip_stop_times);
    let stops = create_stops_vector(filtered_data.stops);

    ProcessedTransitData {
        stop_times,
        route_stops,
        routes,
        trips,
        stops,
        feeds_meta: filtered_data.feeds_meta,
    }
}

struct ProcessedTransitData {
    stop_times: Vec<StopTime>,
    route_stops: Vec<usize>,
    routes: Vec<Route>,
    trips: Vec<Vec<Trip>>,
    stops: Vec<Stop>,
    feeds_meta: Vec<FeedMeta>,
}

fn group_stop_times_by_trip(stop_times: Vec<FeedStopTime>) -> HashMap<String, Vec<FeedStopTime>> {
    let mut trip_stop_times: HashMap<String, Vec<FeedStopTime>> = HashMap::new();

    for stop_time in stop_times {
        trip_stop_times
            .entry(stop_time.trip_id.clone())
            .or_default()
            .push(stop_time);
    }

    for stop_times in trip_stop_times.values_mut() {
        stop_times.sort_by_key(|s| s.stop_sequence);
    }

    trip_stop_times
}

fn build_public_transit_data(processed_data: ProcessedTransitData) -> PublicTransitData {
    let mut stop_routes: Vec<RouteId> = Vec::new();
    let mut stops_vec = processed_data.stops;

    let mut stop_to_routes: HashMap<RaptorStopId, HashSet<RouteId>> =
        HashMap::with_capacity(stops_vec.len());

    for (route_idx, route) in processed_data.routes.iter().enumerate() {
        for stop_idx in
            &processed_data.route_stops[route.stops_start..route.stops_start + route.num_stops]
        {
            stop_to_routes
                .entry(*stop_idx)
                .or_default()
                .insert(route_idx);
        }
    }

    for (stop_idx, routes) in stop_to_routes {
        stops_vec[stop_idx].routes_start = stop_routes.len();
        stops_vec[stop_idx].routes_len = routes.len();
        stop_routes.extend(routes);
    }

    PublicTransitData {
        routes: processed_data.routes,
        route_stops: processed_data.route_stops,
        stop_times: processed_data.stop_times,
        stops: stops_vec,
        stop_routes,
        transfers: vec![],
        node_to_stop: HashMap::new(),
        feeds_meta: processed_data.feeds_meta,
        trips: processed_data.trips,
    }
}

fn build_route_stops(
    representative: &[FeedStopTime],
    stop_id_map: &HashMap<&str, usize>,
    route_stops: &mut Vec<usize>,
) -> usize {
    let stops_start = route_stops.len();
    for st in representative {
        if let Some(&idx) = stop_id_map.get(st.stop_id.as_str()) {
            route_stops.push(idx);
        }
    }
    stops_start
}

fn build_route_trips(
    group: &[&[FeedStopTime]],
    trip_data_map: &HashMap<&str, &FeedTrip>,
) -> Vec<Trip> {
    group
        .iter()
        .filter_map(|trip_stop_times| {
            let trip_id = &trip_stop_times[0].trip_id;
            trip_data_map.get(trip_id.as_str()).map(|trip_data| Trip {
                trip_id: trip_data.trip_id.clone(),
            })
        })
        .collect()
}

fn build_stop_times(group: &[&[FeedStopTime]], stop_times_vec: &mut Vec<StopTime>) {
    for trip in group {
        for st in *trip {
            let arrival = if st.stop_sequence == 0 {
                st.departure_time
            } else {
                st.arrival_time
            };
            stop_times_vec.push(StopTime {
                arrival,
                departure: st.departure_time,
            });
        }
    }
}

fn group_trips_by_route<'a>(
    trips: &'a [FeedTrip],
    trip_stop_times: &'a HashMap<String, Vec<FeedStopTime>>,
) -> HashMap<&'a str, Vec<&'a [FeedStopTime]>> {
    let trip_id_map: HashMap<&str, &str> = trips
        .iter()
        .map(|t| (t.trip_id.as_str(), t.route_id.as_str()))
        .collect();

    let mut routes_map: HashMap<&str, Vec<&'a [FeedStopTime]>> = HashMap::new();
    for (trip_id, sts) in trip_stop_times {
        if let Some(&route_id) = trip_id_map.get(trip_id.as_str()) {
            routes_map.entry(route_id).or_default().push(sts.as_slice());
        }
    }
    routes_map
}

struct RouteBuilder<'a> {
    stop_times_vec: &'a mut Vec<StopTime>,
    route_stops: &'a mut Vec<usize>,
    routes_vec: &'a mut Vec<Route>,
    trips_vec: &'a mut Vec<Vec<Trip>>,
}

fn process_route_variants<'a>(
    route_id: &str,
    trips_data: Vec<&'a [FeedStopTime]>,
    stop_id_map: &HashMap<&str, usize>,
    trip_data_map: &HashMap<&str, &FeedTrip>,
    builder: &mut RouteBuilder,
) {
    let mut groups_by_length: HashMap<usize, Vec<&'a [FeedStopTime]>> = HashMap::new();
    for ts in trips_data {
        groups_by_length.entry(ts.len()).or_default().push(ts);
    }

    for (num_stops, mut group) in groups_by_length {
        group.sort_by_key(|ts| &ts[0].departure_time);

        let representative = group[0];
        let stops_start = build_route_stops(representative, stop_id_map, builder.route_stops);
        let trips_start = builder.stop_times_vec.len();

        let route_trips = build_route_trips(&group, trip_data_map);
        build_stop_times(&group, builder.stop_times_vec);

        builder.routes_vec.push(Route {
            num_trips: group.len(),
            num_stops,
            stops_start,
            trips_start,
            route_id: route_id.to_string(),
        });

        builder.trips_vec.push(route_trips);
    }
}

fn process_trip_stop_times<'a>(
    stops: &'a [FeedStop],
    trips: &'a [FeedTrip],
    trip_stop_times: &'a HashMap<String, Vec<FeedStopTime>>,
) -> (Vec<StopTime>, Vec<usize>, Vec<Route>, Vec<Vec<Trip>>) {
    let stop_id_map: HashMap<&str, usize> = stops
        .iter()
        .enumerate()
        .map(|(i, s)| (s.stop_id.as_str(), i))
        .collect();

    let trip_data_map: HashMap<&str, &FeedTrip> =
        trips.iter().map(|t| (t.trip_id.as_str(), t)).collect();

    let routes_map = group_trips_by_route(trips, trip_stop_times);

    let total_stop_times: usize = trip_stop_times.values().map(Vec::len).sum();
    let mut stop_times_vec = Vec::with_capacity(total_stop_times);
    let mut route_stops = Vec::new();
    let mut routes_vec = Vec::new();
    let mut trips_vec = Vec::new();

    for (route_id, trips_data) in routes_map {
        let mut builder = RouteBuilder {
            stop_times_vec: &mut stop_times_vec,
            route_stops: &mut route_stops,
            routes_vec: &mut routes_vec,
            trips_vec: &mut trips_vec,
        };

        process_route_variants(
            route_id,
            trips_data,
            &stop_id_map,
            &trip_data_map,
            &mut builder,
        );
    }

    (stop_times_vec, route_stops, routes_vec, trips_vec)
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
