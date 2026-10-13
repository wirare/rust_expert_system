//! Stable, dependency-oriented layout for the *focused* proof explanation.
//!
//! petgraph condenses recursion safely, then lays the remaining dependency
//! graph out from supporting facts (left) toward the requested answer (right).

use std::collections::{BTreeSet, HashMap};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use super::model::{DisplayNodeId, ExplanationGraph, NodeKind};

pub const NODE_WIDTH: f32 = 310.0;
pub const NODE_HEIGHT: f32 = 130.0;
pub const COLUMN_GAP: f32 = 160.0;
pub const ROW_GAP: f32 = 48.0;
pub const PADDING: f32 = 102.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement
{
    pub id: DisplayNodeId,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Default)]
pub struct GraphLayout
{
    pub nodes: Vec<Placement>,
    pub width: f32,
    pub height: f32,
}

impl GraphLayout
{
    pub fn new(explanation: &ExplanationGraph) -> Self
    {
        if explanation.nodes.is_empty()
        {
            return Self::default();
        }

        let mut graph = DiGraph::<DisplayNodeId, ()>::new();
        let mut indices = HashMap::<DisplayNodeId, NodeIndex>::new();
        for node in &explanation.nodes
        {
            indices.insert(node.id, graph.add_node(node.id));
        }
        for link in &explanation.links
        {
            if let (Some(&a), Some(&b)) =
                (indices.get(&link.from), indices.get(&link.to))
            {
                if a != b
                {
                    graph.add_edge(a, b, ());
                }
            }
        }

        // Robust to accidental cycles, even if a malformed or partially
        // captured trace contains circular dependencies.
        let components = kosaraju_scc(&graph);
        let mut owner = vec![0_usize; graph.node_count()];
        for (component, members) in components.iter().enumerate()
        {
            for member in members
            {
                owner[member.index()] = component;
            }
        }

        let mut successors = vec![BTreeSet::new(); components.len()];
        let mut indegree = vec![0_usize; components.len()];
        for edge in graph.edge_references()
        {
            let source = owner[edge.source().index()];
            let target = owner[edge.target().index()];
            if source != target && successors[source].insert(target)
            {
                indegree[target] += 1;
            }
        }

        let mut queue = indegree.iter().enumerate()
            .filter_map(|(i, degree)| (*degree == 0).then_some(i))
            .collect::<BTreeSet<usize>>();
        let mut ranks = vec![0_usize; components.len()];

        while let Some(&component) = queue.iter().next()
        {
            queue.remove(&component);
            for &next in &successors[component]
            {
                ranks[next] = ranks[next].max(ranks[component] + 1);
                indegree[next] -= 1;
                if indegree[next] == 0
                {
                    queue.insert(next);
                }
            }
        }

        let mut layer_of = HashMap::<DisplayNodeId, usize>::new();
        for node in &explanation.nodes
        {
            layer_of.insert(node.id, ranks[owner[indices[&node.id].index()]]);
        }

        let max_layer = layer_of.values().copied().max().unwrap_or(0);
        // The answer is always a terminal node in the visible explanation.
        layer_of.insert(explanation.root, max_layer);

        let total_columns = layer_of.values().copied().max().unwrap_or(0) + 1;
        let mut columns = vec![Vec::<DisplayNodeId>::new(); total_columns];
        for node in &explanation.nodes
        {
            columns[layer_of[&node.id]].push(node.id);
        }

        let mut order = HashMap::<DisplayNodeId, usize>::new();
        for layer in &mut columns
        {
            layer.sort_unstable();
        }

        // Barycentric sweep: keep facts feeding the same rule or clause near
        // one another, reducing crossing compared with alphabetic sorting.
        for (depth, layer) in columns.iter_mut().enumerate()
        {
            let scores = layer.iter().map(|&id|
            {
                let predecessors = explanation.links.iter()
                    .filter(|link| link.to == id && layer_of[&link.from] < depth)
                    .filter_map(|link| order.get(&link.from).copied())
                    .collect::<Vec<_>>();
                let mean = if predecessors.is_empty()
                {
                    f32::MAX
                }
                else
                {
                    predecessors.iter().map(|x| *x as f32).sum::<f32>()
                        / predecessors.len() as f32
                };
                (id, mean)
            }).collect::<HashMap<_, _>>();

            layer.sort_by(|left, right|
            {
                scores[left].total_cmp(&scores[right])
                    .then_with(||
                    {
                        let left_kind = explanation.node(*left).map(|n| n.kind);
                        let right_kind = explanation.node(*right).map(|n| n.kind);
                        fn priority(kind: Option<NodeKind>) -> u8
                        {
                            match kind
                            {
                                Some(NodeKind::Fact) => 0,
                                Some(NodeKind::Rule) => 1,
                                Some(NodeKind::Clause) => 2,
                                Some(NodeKind::Derived) => 3,
                                Some(NodeKind::Notice) => 4,
                                Some(NodeKind::Answer) => 5,
                                None => 6,
                            }
                        }
                        priority(left_kind).cmp(&priority(right_kind))
                    })
                    .then_with(|| left.cmp(right))
            });

            for (row, &id) in layer.iter().enumerate()
            {
                order.insert(id, row);
            }
        }

        let rows = columns.iter().map(Vec::len).max().unwrap_or(1);
        let total_height = PADDING * 2.0
            + rows as f32 * NODE_HEIGHT
            + rows.saturating_sub(1) as f32 * ROW_GAP;

        let mut positions = Vec::with_capacity(explanation.nodes.len());
        for (column, layer) in columns.iter().enumerate()
        {
            let column_height = layer.len() as f32 * NODE_HEIGHT
                + layer.len().saturating_sub(1) as f32 * ROW_GAP;
            let offset = ((total_height - column_height) * 0.5).max(PADDING * 0.5);

            for (row, &id) in layer.iter().enumerate()
            {
                positions.push(Placement {
                    id,
                    x: PADDING + column as f32 * (NODE_WIDTH + COLUMN_GAP),
                    y: offset + row as f32 * (NODE_HEIGHT + ROW_GAP),
                });
            }
        }

        Self {
            nodes: positions,
            width: PADDING * 2.0
                + total_columns as f32 * NODE_WIDTH
                + total_columns.saturating_sub(1) as f32 * COLUMN_GAP,
            height: total_height,
        }
    }

    pub fn location(&self, id: DisplayNodeId) -> Option<&Placement>
    {
        self.nodes.iter().find(|node| node.id == id)
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::engine::{run_source, SolveMode};
    use super::super::model::ExplanationGraph;

    #[test]
    fn dependencies_flow_left_to_right()
    {
        let run = run_source("A=>B\n=A\n?B", SolveMode::Sequential).unwrap();
        let explanation = ExplanationGraph::new(
            &run.execution, &run.execution.queries[0],
        );
        let layout = GraphLayout::new(&explanation);
        assert_eq!(layout.nodes.len(), explanation.nodes.len());

        for link in &explanation.links
        {
            let left = layout.location(link.from).unwrap();
            let right = layout.location(link.to).unwrap();
            assert!(left.x <= right.x,
                "Evidence must precede its conclusion: {:?} -> {:?}",
                left, right);
        }

        let again = GraphLayout::new(&explanation);
        assert_eq!(layout.nodes, again.nodes);
    }

    #[test]
    fn layout_reserves_a_clear_routing_gutter_above_every_card()
    {
        let run = run_source(
            "A=>B|C\nD=>!C\n=AD\n?B", SolveMode::Sequential,
        ).unwrap();
        let explanation = ExplanationGraph::new(
            &run.execution, &run.execution.queries[0],
        );
        let layout = GraphLayout::new(&explanation);
        assert!(layout.nodes.iter().all(|position| position.y >= PADDING));
        assert!(layout.width >= NODE_WIDTH + PADDING * 2.0);
    }

    #[test]
    fn contradiction_has_two_paths_reaching_the_same_answer()
    {
        let run = run_source(
            "A=>B\nC=>!B\n=AC\n?B", SolveMode::Sequential,
        ).unwrap();
        let explanation = ExplanationGraph::new(
            &run.execution, &run.execution.queries[0],
        );
        let layout = GraphLayout::new(&explanation);
        let answer = layout.location(explanation.root).unwrap();

        assert!(explanation.links.iter().filter(|e| e.to == explanation.root)
            .all(|e| layout.location(e.from).unwrap().x < answer.x));
        assert!(layout.height >= NODE_HEIGHT);
    }
}
