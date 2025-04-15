use geo::{Point, line_string};
use geojson::{Feature, Geometry};
use serde_json::json;

use crate::Time;

/// Represents a walking leg outside the transit network.
#[derive(Debug, Clone)]
pub struct WalkingLeg {
    pub from_location: Point<f64>,
    pub to_location: Point<f64>,
    pub from_name: String,
    pub to_name: String,
    pub departure_time: Time,
    pub arrival_time: Time,
    pub duration: Time,
}

impl WalkingLeg {
    /// Create a new walking leg.
    pub fn new(
        from_location: Point<f64>,
        to_location: Point<f64>,
        from_name: String,
        to_name: String,
        departure_time: Time,
        duration: Time,
    ) -> Self {
        Self {
            from_location,
            to_location,
            from_name,
            to_name,
            departure_time,
            arrival_time: departure_time + duration,
            duration,
        }
    }

    /// Convert the walking leg to a `GeoJSON` Feature using the `json!` macro.
    ///
    /// # Panics
    /// This function will panic if `Feature::from_json_value` fails to parse the JSON value.
    pub fn to_feature(&self, leg_type: &str) -> Feature {
        let coordinates = line_string![
            (x: self.from_location.x(), y: self.from_location.y()),
            (x: self.to_location.x(), y: self.to_location.y()),
        ];
        let value = json!({
            "type": "Feature",
            "geometry": Geometry::new((&coordinates).into()),
            "properties": {
                "leg_type": leg_type,
                "from_name": self.from_name,
                "to_name": self.to_name,
                "departure_time": self.departure_time,
                "arrival_time": self.arrival_time,
                "duration": self.duration,
            }
        });
        Feature::from_json_value(value).unwrap()
    }
}
