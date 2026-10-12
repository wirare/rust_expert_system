use serde::{Deserialize, Serialize};

use crate::trace::{
    AttemptId,
    ConstraintId,
    EvaluationTrace,
    EventId,
    Literal,
    LogicValue,
    RuleId,
    SearchOutcome,
};

pub type NodeId = u64;
pub type TransformationId = u64;
pub type RuleVersionId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphReport
{
    pub schema_version: u32,

    pub query: char,
    pub outcome: QueryOutcome,

    pub initial_facts: Vec<char>,

    pub original_rules: Vec<OriginalRule>,
    pub normalized_rules: Vec<NormalizedRule>,

    pub rule_versions: Vec<RuleVersion>,
    pub transformations: Vec<RuleTransformation>,

    pub trace: EvaluationTrace,
    pub graph: ReasoningGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryOutcome
{
    Value(LogicValue),

    Error
    {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningGraph
{
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode
{
    pub id: NodeId,
    pub kind: GraphNodeKind,
    pub label: String,

    pub related_events: Vec<EventId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphNodeKind
{
    Query
    {
        symbol: char,
        outcome: QueryOutcome,
    },

    SearchAttempt
    {
        attempt_id: AttemptId,
        goal: Literal,
        outcome: SearchOutcome,
    },

    InitialFact
    {
        symbol: char,
    },

    Rule
    {
        rule_id: RuleId,
    },

    Constraint
    {
        constraint_id: ConstraintId,
        literals: Vec<Literal>,
    },

    Deduction
    {
        literal: Literal,
        proof_event: EventId,
    },

    Transformation
    {
        transformation_id: TransformationId,
    },

    Cycle
    {
        attempt_id: AttemptId,
        previous_attempt: AttemptId,
    },

    Event { event_id: EventId },
    RuleVersion { version_id: RuleVersionId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge
{
    pub from: NodeId,
    pub to: NodeId,

    pub kind: EdgeKind,
    pub related_event: Option<EventId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeKind
{
    SearchDependency,
    PremiseDependency,

    Justification,
    ConstraintActivation,
    UnitPropagation,

    Transformation,

    Return,
    ProofReuse,
    CycleReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginalRule
{
    pub id: usize,
    pub source_line: usize,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedRule
{
    pub id: RuleId,
    pub original_rule_id: usize,
    pub version_id: RuleVersionId,

    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleVersion
{
    pub id: RuleVersionId,
    pub original_rule_id: usize,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTransformation
{
    pub id: TransformationId,

    pub kind: TransformationKind,

    pub inputs: Vec<RuleVersionId>,
    pub outputs: Vec<RuleVersionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformationKind
{
    SplitBiconditional,
    SplitAndConclusion,
    DistributeOr,
    ExpandXor,
    DeMorgan,
    EliminateDoubleNegation,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RewriteHistory
{
    pub original_rules: Vec<OriginalRule>,
    pub normalized_rules: Vec<NormalizedRule>,
    pub rule_versions: Vec<RuleVersion>,
    pub transformations: Vec<RuleTransformation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryExecution
{
    pub symbol: char,
    pub outcome: QueryOutcome,
    pub trace: EvaluationTrace,
    pub graph: ReasoningGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport
{
    pub schema_version: u32,
    pub initial_facts: Vec<char>,
    pub rewrite: RewriteHistory,
    pub rewrite_graph: ReasoningGraph,
    pub queries: Vec<QueryExecution>,
}

impl ReasoningGraph
{
    pub fn from_trace(trace: &EvaluationTrace) -> Self
    {
        use crate::trace::{TraceEventKind as T, ProofOrigin};
        use std::collections::{HashMap, HashSet};
        const FACT_BASE: u64 = 1_000_000_000;
        const RULE_BASE: u64 = 2_000_000_000;
        const CONSTRAINT_BASE: u64 = 3_000_000_000;
        const ATTEMPT_BASE: u64 = 4_000_000_000;
        const QUERY_BASE: u64 = 5_000_000_000;
        let mut graph = Self { nodes: Vec::new(), edges: Vec::new() };
        let mut used = HashSet::new();
        let mut links = HashSet::new();
        let mut attempts = HashMap::new();
        let mut known_rules = HashSet::new();
        let mut known_constraints = HashSet::new();
        let mut known_facts = HashSet::new();

        let mut link = |from: NodeId, to: NodeId, kind: EdgeKind, event: Option<EventId>, graph: &mut Self|
        {
            if from != to && links.insert((from, to, format!("{kind:?}")))
            {
                graph.edges.push(GraphEdge { from, to, kind, related_event: event });
            }
        };
        let query_id = QUERY_BASE;
        graph.nodes.push(GraphNode {
            id: query_id,
            kind: GraphNodeKind::Query {
                symbol: trace.query,
                outcome: QueryOutcome::Value(LogicValue::Undetermined),
            },
            label: format!("Query {}", trace.query),
            related_events: Vec::new(),
        });
        used.insert(query_id);

        for event in &trace.events
        {
            let node_id = event.id;
            if !used.insert(node_id) { continue; }
            let label = format!("{:?}", event.kind);
            let kind = match &event.kind
            {
                T::SearchStarted { attempt_id, goal, .. } =>
                {
                    attempts.insert(*attempt_id, node_id);
                    GraphNodeKind::SearchAttempt {
                        attempt_id: *attempt_id,
                        goal: *goal,
                        outcome: SearchOutcome::NotProven,
                    }
                }
                T::LiteralProven { literal, .. } =>
                    GraphNodeKind::Deduction { literal: *literal, proof_event: event.id },
                T::ConstraintActivated { constraint_id, literals, .. } =>
                    GraphNodeKind::Constraint { constraint_id: *constraint_id, literals: literals.clone() },
                T::CycleDetected { previous_attempt, .. } =>
                    GraphNodeKind::Cycle {
                        attempt_id: event.attempt_id.unwrap_or(*previous_attempt),
                        previous_attempt: *previous_attempt,
                    },
                _ => GraphNodeKind::Event { event_id: event.id },
            };
            graph.nodes.push(GraphNode { id: node_id, kind, label, related_events: vec![event.id] });
            for &cause in &event.causes
            {
                link(cause, node_id, EdgeKind::Justification, Some(event.id), &mut graph);
            }
            if let Some(attempt) = event.attempt_id
            {
                if let Some(&start) = attempts.get(&attempt)
                {
                    link(start, node_id, EdgeKind::SearchDependency, Some(event.id), &mut graph);
                }
            }
            match &event.kind
            {
                T::SearchStarted { parent_attempt, .. } =>
                {
                    if let Some(parent) = parent_attempt.and_then(|a| attempts.get(&a).copied())
                    {
                        link(parent, node_id, EdgeKind::SearchDependency, Some(event.id), &mut graph);
                    }
                    else if parent_attempt.is_none()
                    {
                        link(query_id, node_id, EdgeKind::SearchDependency, Some(event.id), &mut graph);
                    }
                }
                T::SearchFinished { attempt_id, .. } =>
                {
                    if let Some(&start) = attempts.get(attempt_id)
                    {
                        link(node_id, start, EdgeKind::Return, Some(event.id), &mut graph);
                    }
                }
                T::CycleDetected { previous_attempt, .. } =>
                {
                    if let Some(&start) = attempts.get(previous_attempt)
                    {
                        link(node_id, start, EdgeKind::CycleReference, Some(event.id), &mut graph);
                    }
                }
                T::RuleConsidered { rule_id } | T::RuleApplied { rule_id } | T::PremiseEvaluated { rule_id, .. } =>
                {
                    let rule_node = RULE_BASE + *rule_id as u64;
                    if known_rules.insert(*rule_id)
                    {
                        graph.nodes.push(GraphNode { id: rule_node, kind: GraphNodeKind::Rule { rule_id: *rule_id }, label: format!("Rule {rule_id}"), related_events: Vec::new() });
                    }
                    link(rule_node, node_id, EdgeKind::PremiseDependency, Some(event.id), &mut graph);
                }
                T::ConstraintActivated { constraint_id, rule_id, .. } =>
                {
                    let rule_node = RULE_BASE + *rule_id as u64;
                    if known_rules.insert(*rule_id)
                    {
                        graph.nodes.push(GraphNode { id: rule_node, kind: GraphNodeKind::Rule { rule_id: *rule_id }, label: format!("Rule {rule_id}"), related_events: Vec::new() });
                    }
                    link(rule_node, node_id, EdgeKind::ConstraintActivation, Some(event.id), &mut graph);
                    known_constraints.insert(*constraint_id);
                }
                T::ConstraintEvaluated { constraint_id, .. } =>
                {
                    let clause = trace.events.iter().find_map(|e| match &e.kind {
                        T::ConstraintActivated { constraint_id: idx, .. } if idx == constraint_id => Some(e.id),
                        _ => None,
                    });
                    if let Some(activated) = clause
                    {
                        link(activated, node_id, EdgeKind::ConstraintActivation, Some(event.id), &mut graph);
                    }
                    let _ = CONSTRAINT_BASE; // reserved for stable constraint entities
                }
                T::LiteralProven { literal, origin } =>
                {
                    if let ProofOrigin::InitialFact = origin
                    {
                        let fact_node = FACT_BASE + (literal.symbol as u64 - 'A' as u64);
                        if known_facts.insert(literal.symbol)
                        {
                            graph.nodes.push(GraphNode { id: fact_node, kind: GraphNodeKind::InitialFact { symbol: literal.symbol }, label: literal.symbol.to_string(), related_events: vec![event.id] });
                        }
                        link(fact_node, node_id, EdgeKind::Justification, Some(event.id), &mut graph);
                    }
                }
                T::ProofReused { original_event, .. } =>
                    link(*original_event, node_id, EdgeKind::ProofReuse, Some(event.id), &mut graph),
                T::QueryFinished { .. } | T::QueryFailed { .. } =>
                    link(node_id, query_id, EdgeKind::Return, Some(event.id), &mut graph),
                _ => {}
            }
            let _ = ATTEMPT_BASE;
        }
        graph
    }

    pub fn from_rewrite(history: &RewriteHistory) -> Self
    {
        let mut graph = Self { nodes: Vec::new(), edges: Vec::new() };
        // Version and transformation IDs occupy separate ranges.
        const TRANSFORM_BASE: u64 = 1_000_000_000;
        for version in &history.rule_versions
        {
            graph.nodes.push(GraphNode {
                id: version.id,
                kind: GraphNodeKind::RuleVersion { version_id: version.id },
                label: version.expression.clone(),
                related_events: Vec::new(),
            });
        }
        for transform in &history.transformations
        {
            let transform_node = TRANSFORM_BASE + transform.id;
            graph.nodes.push(GraphNode {
                id: transform_node,
                kind: GraphNodeKind::Transformation { transformation_id: transform.id },
                label: format!("{:?}", transform.kind),
                related_events: Vec::new(),
            });
            for input in &transform.inputs
            {
                graph.edges.push(GraphEdge { from: *input, to: transform_node, kind: EdgeKind::Transformation, related_event: None });
            }
            for output in &transform.outputs
            {
                graph.edges.push(GraphEdge { from: transform_node, to: *output, kind: EdgeKind::Transformation, related_event: None });
            }
        }
        graph
    }
}
