//! Street network components - nodes, edges, and transit points

use geo::{LineString, Point};
use osm4routing::NodeId;

use crate::Time;

/// Street graph node
#[derive(Debug, Clone)]
pub struct StreetNode {
    /// OSM ID of the node
    pub id: NodeId,
    /// Node coordinates
    pub geometry: Point<f64>,
}

/// Street graph edge (street segment)
#[derive(Debug, Clone)]
pub struct StreetEdge {
    /// Pedestrian crossing time in seconds
    pub weight: Time,
    /// Optional geometry for visualization
    pub geometry: LineString<f64>,
}

impl StreetEdge {
    pub fn walking_time(&self) -> Time {
        self.weight
    }
}
