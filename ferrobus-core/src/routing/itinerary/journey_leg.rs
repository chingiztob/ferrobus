use crate::{Error, Time};
use geo::{Point, line_string};
use geojson::{Feature, Geometry, JsonObject, JsonValue};

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

    /// Convert the walking leg to a GeoJSON Feature.
    pub fn to_feature(&self, leg_type: &str) -> Result<Feature, Error> {
        let coordinates = line_string![
            (x: self.from_location.x(), y: self.from_location.y()),
            (x: self.to_location.x(), y: self.to_location.y()),
        ];

        let geometry = Geometry::new((&coordinates).into());

        let mut properties = JsonObject::new();
        properties.insert("leg_type".to_string(), JsonValue::from(leg_type));
        properties.insert(
            "from_name".to_string(),
            JsonValue::from(self.from_name.clone()),
        );
        properties.insert("to_name".to_string(), JsonValue::from(self.to_name.clone()));
        properties.insert(
            "departure_time".to_string(),
            JsonValue::from(self.departure_time),
        );
        properties.insert(
            "arrival_time".to_string(),
            JsonValue::from(self.arrival_time),
        );
        properties.insert("duration".to_string(), JsonValue::from(self.duration));

        Ok(Feature {
            bbox: None,
            geometry: Some(geometry),
            id: None,
            properties: Some(properties),
            foreign_members: None,
        })
    }
}
