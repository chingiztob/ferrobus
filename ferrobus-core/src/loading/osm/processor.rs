use hashbrown::HashMap;
#[cfg(test)]
use hashbrown::HashSet;
use log::info;
use osm4routing::FootAccessibility;
use petgraph::graph::{NodeIndex, UnGraph};
use rstar::RTree;
use rustworkx_core::connectivity::connected_components;
use std::path::Path;

use crate::{
    Error, Time, WALKING_SPEED,
    model::{IndexedPoint, StreetEdge, StreetGraph, StreetNode},
};

fn rebuild_largest_component_graph(
    graph: &UnGraph<StreetNode, StreetEdge>,
    largest_component: &[NodeIndex],
) -> UnGraph<StreetNode, StreetEdge> {
    let mut new_graph = UnGraph::<StreetNode, StreetEdge>::new_undirected();
    let mut new_node_indices = HashMap::new();

    for node_index in largest_component {
        let node = graph[*node_index].clone();
        let new_node_index = new_graph.add_node(node);
        new_node_indices.insert(*node_index, new_node_index);
    }

    for node_index in largest_component {
        for neighbor in graph.neighbors(*node_index) {
            if node_index.index() >= neighbor.index() {
                continue;
            }

            let edge = graph
                .find_edge(*node_index, neighbor)
                .expect("edge must exist for neighbor");
            let edge_type = graph[edge].clone();

            new_graph.add_edge(
                new_node_indices[node_index],
                new_node_indices[&neighbor],
                edge_type,
            );
        }
    }

    new_graph
}

/// Create the street network graph based on an OSM .pbf file
pub(crate) fn create_street_graph(filename: impl AsRef<Path>) -> Result<StreetGraph, Error> {
    info!("Reading OSM data from: {}", filename.as_ref().display());

    let mut graph = UnGraph::<StreetNode, StreetEdge>::new_undirected();
    // Store OSM node IDs and their corresponding graph node indices
    let (nodes, edges) = osm4routing::Reader::new()
        .read(filename)
        .map_err(|e| Error::InvalidData(format!("Error reading OSM data: {e}")))?;

    // filter only pedestrian allowed ways and edges with Unknown pedestrian accessibility
    let edges = edges
        .into_iter()
        .filter(|edge| {
            matches!(
                edge.properties.foot,
                FootAccessibility::Allowed | FootAccessibility::Unknown
            )
        })
        .collect::<Vec<_>>();

    let mut node_indices = HashMap::new();

    for node in nodes {
        node_indices.entry(node.id).or_insert_with(|| {
            let node_obj = StreetNode {
                id: node.id,
                geometry: node.coord.into(),
            };

            graph.add_node(node_obj)
        });
    }

    for edge in edges {
        let source_index = *node_indices
            .get(&edge.source)
            .ok_or_else(|| Error::InvalidData(format!("Missing source node: {:?}", edge.source)))?;
        let target_index = *node_indices
            .get(&edge.target)
            .ok_or_else(|| Error::InvalidData(format!("Missing target node: {:?}", edge.target)))?;

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let weight = (edge.length() / WALKING_SPEED) as Time;

        let edge_obj = StreetEdge { weight };

        graph.add_edge(source_index, target_index, edge_obj);
    }

    // Keep only the largest connected component to avoid isolated parts of the graph
    // affecting routing
    #[allow(clippy::redundant_closure_for_method_calls)]
    let largest_component = connected_components(&graph)
        .into_iter()
        .max_by_key(|c| c.len())
        .ok_or(Error::InvalidData(
            "No connected components found".to_string(),
        ))?;
    let largest_component: Vec<NodeIndex> = largest_component.into_iter().collect();

    // Create a new graph for the largest connected component
    let new_graph = rebuild_largest_component_graph(&graph, &largest_component);
    drop(graph);

    info!("Building R-Tree spatial index");
    let rtree = build_rtree(&new_graph);

    let street_network = StreetGraph {
        graph: new_graph,
        rtree,
    };

    Ok(street_network)
}

/// R*-tree spatial index for quick nearest neighbor queries
pub(crate) fn build_rtree(graph: &UnGraph<StreetNode, StreetEdge>) -> RTree<IndexedPoint> {
    let mut points = Vec::with_capacity(graph.node_count());
    for (idx, node) in graph.node_weights().enumerate() {
        let idx = NodeIndex::new(idx);
        points.push(IndexedPoint::new(node.geometry, idx));
    }
    RTree::bulk_load(points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Point;
    use osm4routing::NodeId;

    #[test]
    fn rebuild_largest_component_does_not_duplicate_undirected_edges() {
        let mut graph = UnGraph::<StreetNode, StreetEdge>::new_undirected();
        let n0 = graph.add_node(StreetNode {
            id: NodeId(1),
            geometry: Point::new(0.0, 0.0),
        });
        let n1 = graph.add_node(StreetNode {
            id: NodeId(2),
            geometry: Point::new(1.0, 0.0),
        });
        let n2 = graph.add_node(StreetNode {
            id: NodeId(3),
            geometry: Point::new(2.0, 0.0),
        });

        graph.add_edge(n0, n1, StreetEdge { weight: 10 });
        graph.add_edge(n1, n2, StreetEdge { weight: 20 });

        let rebuilt = rebuild_largest_component_graph(&graph, &[n0, n1, n2]);
        assert_eq!(rebuilt.edge_count(), 2);

        let unique_pairs: HashSet<(usize, usize)> = rebuilt
            .edge_indices()
            .map(|edge_idx| {
                let (a, b) = rebuilt
                    .edge_endpoints(edge_idx)
                    .expect("edge endpoints must exist");
                let a = a.index();
                let b = b.index();
                if a < b { (a, b) } else { (b, a) }
            })
            .collect();

        assert_eq!(unique_pairs.len(), rebuilt.edge_count());
    }
}
