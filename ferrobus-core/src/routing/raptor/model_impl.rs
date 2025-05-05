//! Methods for `PublicTransitData`, requiered for Raptor

use crate::PublicTransitData;

use super::RaptorError;
use crate::model::{RaptorStopId, RouteId, StopTime, Time, Transfer};

impl PublicTransitData {
    /// `StopTime` slice for specific route and trip
    pub(crate) fn get_trip(
        &self,
        route_id: RouteId,
        trip_idx: usize,
    ) -> Result<&[StopTime], RaptorError> {
        let route = self.routes.get(route_id).ok_or(RaptorError::InvalidRoute)?;

        if trip_idx >= route.num_trips {
            return Err(RaptorError::InvalidTrip);
        }

        let start = route.trips_start + trip_idx * route.num_stops;
        let end = start + route.num_stops;

        if end > self.stop_times.len() {
            Err(RaptorError::InvalidRoute)
        } else {
            Ok(&self.stop_times[start..end])
        }
    }

    /// Returns transfers from the specified stop
    pub(crate) fn get_stop_transfers(
        &self,
        stop_id: RaptorStopId,
    ) -> Result<&[Transfer], RaptorError> {
        self.validate_stop(stop_id)?;
        let stop = &self.stops[stop_id];
        let end = stop.transfers_start + stop.transfers_len;
        if end > self.transfers.len() {
            Err(RaptorError::InvalidStop)
        } else {
            Ok(&self.transfers[stop.transfers_start..end])
        }
    }

    /// Returns all departure times from the given source stop within the specified time range.
    pub(crate) fn get_source_departures(
        &self,
        source: RaptorStopId,
        min_departure: Time,
        max_departure: Time,
    ) -> Result<Vec<Time>, RaptorError> {
        self.validate_stop(source)?;

        let mut departures = Vec::new();

        let routes = self.routes_for_stop(source);

        for &route_id in routes {
            let route_stops = self.get_route_stops(route_id)?;

            // Find the index of the source stop in the route
            if let Some(stop_idx) = route_stops.iter().position(|&stop| stop == source) {
                let route = &self.routes[route_id];

                // For each trip on this route
                for trip_idx in 0..route.num_trips {
                    // Get the trip's stop times
                    let trip = self.get_trip(route_id, trip_idx)?;

                    // Get the departure time at the source stop
                    let departure_time = trip[stop_idx].departure;

                    // If the departure time is within the specified range, add it
                    if departure_time >= min_departure && departure_time <= max_departure {
                        departures.push(departure_time);
                    }
                }
            }
        }

        // Sort and remove duplicates
        departures.sort_unstable();
        departures.dedup();

        Ok(departures)
    }

    /// check if such stop exists
    pub(crate) fn validate_stop(&self, stop: RaptorStopId) -> Result<(), RaptorError> {
        if stop >= self.stops.len() {
            Err(RaptorError::InvalidStop)
        } else {
            Ok(())
        }
    }

    /// Stops for specific route
    pub(crate) fn get_route_stops(
        &self,
        route_id: RouteId,
    ) -> Result<&[RaptorStopId], RaptorError> {
        self.routes
            .get(route_id)
            .ok_or(RaptorError::InvalidRoute)
            .and_then(|route| {
                let end = route.stops_start + route.num_stops;
                if end > self.route_stops.len() {
                    Err(RaptorError::InvalidRoute)
                } else {
                    Ok(&self.route_stops[route.stops_start..end])
                }
            })
    }
}
