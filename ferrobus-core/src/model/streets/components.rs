//! Street network components - nodes, edges, and transit points

use geo::Point;
pub use osm4routing::NodeId;

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
}

impl StreetEdge {
    pub fn walking_time(&self) -> Time {
        self.weight
    }
}
