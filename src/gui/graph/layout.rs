//! Deterministic, read-only layered layout built with petgraph.
//!
//! The model is intentionally independent from egui. Only causal edge types
//! participate in ranking. Navigation/search/return links are still drawn,
//! but do not collapse the whole graph into a single cyclic component.

use std::collections::{BTreeSet, HashMap};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::report::{EdgeKind, GraphNode, GraphNodeKind, NodeId, ReasoningGraph};

pub const NODE_WIDTH: f32 = 182.0;
pub const NODE_HEIGHT: f32 = 62.0;
pub const COLUMN_GAP: f32 = 106.0;
pub const ROW_GAP: f32 = 26.0;
const MARGIN: f32 = 45.0;

#[derive(Debug, Clone, Copy)]
pub struct PlacedNode
{
    pub id: NodeId,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug)]
pub struct GraphLayout
{
    pub nodes: Vec<PlacedNode>,
    pub width: f32,
    pub height: f32,
}

fn is_layout_dependency(kind: EdgeKind) -> bool
{
    matches!(
        kind,
        EdgeKind::Justification
            | EdgeKind::PremiseDependency
            | EdgeKind::ConstraintActivation
            | EdgeKind::UnitPropagation
    )
}

fn node_priority(node: &GraphNode) -> u8
{
    match node.kind
    {
        GraphNodeKind::InitialFact { .. } => 0,
        GraphNodeKind::SearchGoal { .. } => 1,
        GraphNodeKind::Rule { .. } => 2,
        GraphNodeKind::Constraint { .. } => 3,
        GraphNodeKind::Deduction { .. } => 4,
        GraphNodeKind::Query { .. } => 5,
        _ => 6,
    }
}

impl GraphLayout
{
    pub fn new(source: &ReasoningGraph) -> Self
    {
        if source.nodes.is_empty()
        {
            return Self {
                nodes: Vec::new(),
                width: 0.0,
                height: 0.0,
            };
        }

        let mut graph = DiGraph::<NodeId, ()>::new();
        let mut indices: HashMap<NodeId, NodeIndex> = HashMap::new();

        for node in &source.nodes
        {
            indices.insert(node.id, graph.add_node(node.id));
        }

        for edge in &source.edges
        {
            if !is_layout_dependency(edge.kind)
            {
                continue;
            }
            if let (Some(&start), Some(&end)) =
                (indices.get(&edge.from), indices.get(&edge.to))
            {
                if start != end
                {
                    graph.add_edge(start, end, ());
                }
            }
        }

        // Condense strongly connected components. A recursive rule system
        // can be cyclic, so a plain topological sort is not sufficient.
        let components = kosaraju_scc(&graph);
        let mut component_of = vec![0_usize; graph.node_count()];

        for (component, members) in components.iter().enumerate()
        {
            for &node in members
            {
                component_of[node.index()] = component;
            }
        }

        let mut children = vec![BTreeSet::new(); components.len()];
        let mut indegree = vec![0_usize; components.len()];

        for edge in graph.edge_references()
        {
            let source = component_of[edge.source().index()];
            let target = component_of[edge.target().index()];
            if source != target && children[source].insert(target)
            {
                indegree[target] += 1;
            }
        }

        let mut ready: BTreeSet<usize> = indegree.iter().enumerate()
            .filter_map(|(index, &degree)| (degree == 0).then_some(index))
            .collect();

        let mut layer_of_component = vec![0_usize; components.len()];

        while let Some(&current) = ready.iter().next()
        {
            ready.remove(&current);
            for &child in &children[current]
            {
                layer_of_component[child] = layer_of_component[child]
                    .max(layer_of_component[current] + 1);
                indegree[child] -= 1;
                if indegree[child] == 0
                {
                    ready.insert(child);
                }
            }
        }

        let mut layers = vec![0_usize; source.nodes.len()];
        for (index, node) in source.nodes.iter().enumerate()
        {
            let node_index = indices[&node.id];
            layers[index] = layer_of_component[component_of[node_index.index()]];
        }

        // The query is the rightmost sink, even when it is unresolved and
        // consequently has no established proof events to connect to it.
        let maximum = layers.iter().copied().max().unwrap_or(0);
        for (index, node) in source.nodes.iter().enumerate()
        {
            if matches!(node.kind, GraphNodeKind::Query { .. })
            {
                layers[index] = maximum + 1;
            }
        }

        let column_count = layers.iter().copied().max().unwrap_or(0) + 1;
        let mut columns = vec![Vec::<usize>::new(); column_count];
        for (index, &column) in layers.iter().enumerate()
        {
            columns[column].push(index);
        }

        let mut placed = Vec::with_capacity(source.nodes.len());
        let mut maximum_rows = 0_usize;

        for (column, members) in columns.iter_mut().enumerate()
        {
            // Sort independent vertices predictably. The stored proof/event
            // IDs are stable, so layout does not change between UI frames.
            members.sort_by(|&a, &b|
            {
                node_priority(&source.nodes[a])
                    .cmp(&node_priority(&source.nodes[b]))
                    .then_with(|| source.nodes[a].label.cmp(&source.nodes[b].label))
                    .then_with(|| source.nodes[a].id.cmp(&source.nodes[b].id))
            });

            maximum_rows = maximum_rows.max(members.len());
            for (row, &index) in members.iter().enumerate()
            {
                placed.push(PlacedNode {
                    id: source.nodes[index].id,
                    x: MARGIN + column as f32 * (NODE_WIDTH + COLUMN_GAP),
                    y: MARGIN + row as f32 * (NODE_HEIGHT + ROW_GAP),
                });
            }
        }

        let width = MARGIN * 2.0
            + column_count as f32 * NODE_WIDTH
            + column_count.saturating_sub(1) as f32 * COLUMN_GAP;

        let height = MARGIN * 2.0
            + maximum_rows as f32 * NODE_HEIGHT
            + maximum_rows.saturating_sub(1) as f32 * ROW_GAP;

        Self { nodes: placed, width, height }
    }

    pub fn location(&self, id: NodeId) -> Option<&PlacedNode>
    {
        self.nodes.iter().find(|node| node.id == id)
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::report::{GraphEdge, GraphNode, QueryOutcome};
    use crate::trace::LogicValue;

    fn make_node(id: u64, kind: GraphNodeKind, label: &str) -> GraphNode
    {
        GraphNode {
            id,
            kind,
            label: label.to_owned(),
            related_events: vec![],
        }
    }

    #[test]
    fn layout_is_stable_and_query_is_rightmost()
    {
        let graph = ReasoningGraph {
            nodes: vec![
                make_node(11, GraphNodeKind::InitialFact { symbol:'A' }, "A"),
                make_node(12, GraphNodeKind::Rule { rule_id:0 }, "Rule 0"),
                make_node(13, GraphNodeKind::Deduction {
                    literal: crate::trace::Literal { symbol:'B', positive:true },
                    proof_event: 1
                }, "B"),
                make_node(14, GraphNodeKind::Query {
                    symbol:'B',
                    outcome:QueryOutcome::Value(LogicValue::True),
                }, "Query B"),
            ],
            edges: vec![
                GraphEdge { from:11,to:12,kind:EdgeKind::PremiseDependency,related_event:None },
                GraphEdge { from:12,to:13,kind:EdgeKind::Justification,related_event:None },
                GraphEdge { from:13,to:14,kind:EdgeKind::Justification,related_event:None },
                GraphEdge { from:14,to:12,kind:EdgeKind::SearchDependency,related_event:None },
            ],
        };

        let layout = GraphLayout::new(&graph);
        assert_eq!(layout.nodes.len(), 4);
        assert!(layout.location(11).unwrap().x < layout.location(12).unwrap().x);
        assert!(layout.location(12).unwrap().x < layout.location(13).unwrap().x);
        assert!(layout.location(13).unwrap().x < layout.location(14).unwrap().x);

        let next = GraphLayout::new(&graph);
        assert_eq!(layout.location(13).unwrap().x, next.location(13).unwrap().x);
        assert_eq!(layout.location(13).unwrap().y, next.location(13).unwrap().y);
    }

    #[test]
    fn cyclic_graph_keeps_every_vertex()
    {
        let graph = ReasoningGraph {
            nodes: vec![
                make_node(0, GraphNodeKind::Rule {rule_id:0}, "R0"),
                make_node(1, GraphNodeKind::Rule {rule_id:1}, "R1"),
            ],
            edges: vec![
                GraphEdge {from:0,to:1,kind:EdgeKind::Justification,related_event:None},
                GraphEdge {from:1,to:0,kind:EdgeKind::Justification,related_event:None},
            ],
        };

        let layout = GraphLayout::new(&graph);
        assert_eq!(layout.nodes.len(), 2);
        assert_eq!(layout.location(0).unwrap().x, layout.location(1).unwrap().x);
        assert_ne!(layout.location(0).unwrap().y, layout.location(1).unwrap().y);
    }
}
