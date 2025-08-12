use hashbrown::HashMap;
use log::info;
use osm4routing::FootAccessibility;
use petgraph::graph::UnGraph;
use rustworkx_core::connectivity::connected_components;
use std::path::Path;

use crate::{
    Error, Time, WALKING_SPEED,
    model::{IndexedPoint, StreetEdge, StreetGraph, StreetNode},
};

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
    let largest_component = connected_components(&graph)
        .into_iter()
        .max_by_key(hashbrown::HashSet::len)
        .ok_or(Error::InvalidData(
            "No connected components found".to_string(),
        ))?;

    // Create a new graph for the largest connected component
    let mut new_graph = UnGraph::<StreetNode, StreetEdge>::new_undirected();
    let mut new_node_indices = HashMap::new();

    for node_index in &largest_component {
        let node = graph[*node_index].clone();
        let new_node_index = new_graph.add_node(node);
        new_node_indices.insert(node_index, new_node_index); // Keep track of indices
    }

    for node_index in &largest_component {
        for neighbor in graph.neighbors(*node_index) {
            let edge = graph.find_edge(*node_index, neighbor).unwrap();
            let edge_type = graph[edge].clone();

            new_graph.add_edge(
                new_node_indices[&node_index],
                new_node_indices[&neighbor],
                edge_type,
            );
        }
    }
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
pub(crate) fn build_rtree(graph: &UnGraph<StreetNode, StreetEdge>) -> rstar::RTree<IndexedPoint> {
    let mut points = Vec::with_capacity(graph.node_count());
    for (idx, node) in graph.node_weights().enumerate() {
        let idx = petgraph::graph::NodeIndex::new(idx);
        points.push(IndexedPoint::new(node.geometry, idx));
    }
    rstar::RTree::bulk_load(points)
}
