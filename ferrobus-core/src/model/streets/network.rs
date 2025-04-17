//! Walkable network model and related functions

use geo::{Distance, Haversine, Point};
use petgraph::graph::{EdgeReference, NodeIndex, UnGraph};
use rstar::{RTree, primitives::GeomWithData};

use super::components::{StreetEdge, StreetNode};
use crate::WALKING_SPEED;

/// Struct for storing graph node in R-tree index
pub type IndexedPoint = GeomWithData<Point<f64>, NodeIndex>;

/// Pedestrian network model based on OSM data
#[derive(Debug, Clone)]
pub struct StreetGraph {
    /// Street graph
    pub graph: UnGraph<StreetNode, StreetEdge>,
    /// Spatial index for fast nearest node search
    pub rtree: RTree<IndexedPoint>,
}

impl StreetGraph {
    pub(crate) fn edges(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = EdgeReference<'_, StreetEdge, u32>> {
        self.graph.edges(node)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) fn nearest_node(&self, point: &Point<f64>) -> Option<(NodeIndex, u32)> {
        self.rtree.nearest_neighbor(point).map(|indexed_point| {
            let distance =
                (Haversine.distance(*point, *indexed_point.geom()) / WALKING_SPEED).ceil() as u32;
            (indexed_point.data, distance)
        })
    }
}

#[cfg(test)]
mod tests {
    use osm4routing::NodeId;

    use super::*;

    #[test]
    fn test_nearest_node() {
        let mut graph = UnGraph::new_undirected();
        let a = graph.add_node(StreetNode {
            id: NodeId(0i64),
            geometry: Point::new(0.0, 0.0),
        });
        let b = graph.add_node(StreetNode {
            id: NodeId(1i64),
            geometry: Point::new(1.0, 1.0),
        });
        let c = graph.add_node(StreetNode {
            id: NodeId(2i64),
            geometry: Point::new(2.0, 2.0),
        });

        let rtree = crate::loading::build_rtree(&graph);
        let network = StreetGraph { graph, rtree };

        let (node, _) = network.nearest_node(&Point::new(0.4, 0.4)).unwrap();
        assert_eq!(node, a);

        let (node, _) = network.nearest_node(&Point::new(1.4, 1.4)).unwrap();
        assert_eq!(node, b);

        let (node, _) = network.nearest_node(&Point::new(2.5, 2.5)).unwrap();
        assert_eq!(node, c);
    }
}
