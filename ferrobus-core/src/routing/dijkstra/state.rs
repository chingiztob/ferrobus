use std::cmp::Ordering;

use petgraph::graph::NodeIndex;

#[derive(Copy, Clone, Eq, PartialEq)]
pub(super) struct State {
    pub(super) cost: u32,
    pub(super) node: NodeIndex,
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap by cost
        other.cost.cmp(&self.cost)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
