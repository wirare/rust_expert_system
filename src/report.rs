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

    /// Grouped searches for one logical goal; individual attempts are in the trace.
    SearchGoal { goal: Literal },

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    /// A compact semantic projection of the execution trace.
    ///
    /// The trace retains every attempt and event. This graph intentionally
    /// groups repeated visits to the same goal/rule/constraint, and does not
    /// create a second copy of every chronological event.
    pub fn from_trace(trace: &EvaluationTrace) -> Self
    {
        use crate::trace::{ProofOrigin, TraceEvent, TraceEventKind as T};
        use std::collections::{HashMap, HashSet};

        const RULE_BASE: u64 = 2_000_000_000;
        const CONSTRAINT_BASE: u64 = 3_000_000_000;
        const GOAL_BASE: u64 = 4_000_000_000;
        const QUERY_ID: u64 = 5_000_000_000;

        fn literal_label(literal: &Literal) -> String
        {
            if literal.positive
            {
                literal.symbol.to_string()
            }
            else
            {
                format!("!{}", literal.symbol)
            }
        }

        fn goal_id(literal: Literal) -> NodeId
        {
            GOAL_BASE
                + (literal.symbol as u64 - 'A' as u64) * 2
                + u64::from(!literal.positive)
        }

        fn record_node(
            graph: &mut ReasoningGraph,
            indices: &mut HashMap<NodeId, usize>,
            node: GraphNode,
        )
        {
            if let Some(&idx) = indices.get(&node.id)
            {
                graph.nodes[idx].related_events.extend(node.related_events);
            }
            else
            {
                indices.insert(node.id, graph.nodes.len());
                graph.nodes.push(node);
            }
        }

        fn link(
            graph: &mut ReasoningGraph,
            known: &mut HashSet<(NodeId, NodeId, EdgeKind)>,
            from: NodeId,
            to: NodeId,
            kind: EdgeKind,
            event_id: EventId,
        )
        {
            if from == to && kind != EdgeKind::CycleReference
            {
                return;
            }
            if known.insert((from, to, kind))
            {
                graph.edges.push(GraphEdge {
                    from,
                    to,
                    kind,
                    related_event: Some(event_id),
                });
            }
        }

        /// Traverse only recorded evidence links; never treat a failed search
        /// or an event that happened earlier as proof for an applied rule.
        fn supporting_proofs(events: &[TraceEvent], roots: &[EventId]) -> Vec<EventId>
        {
            let mut stack = roots.to_vec();
            let mut visited = HashSet::new();
            let mut proofs = HashSet::new();
            while let Some(id) = stack.pop()
            {
                if !visited.insert(id)
                {
                    continue;
                }
                let Some(event) = events.get(id as usize) else
                {
                    continue;
                };
                match &event.kind
                {
                    T::LiteralProven { .. } =>
                    {
                        proofs.insert(id);
                    }
                    T::ProofReused { original_event, .. } =>
                    {
                        stack.push(*original_event);
                    }
                    _ =>
                    {
                        stack.extend(event.causes.iter().copied());
                    }
                }
            }
            let mut proofs: Vec<_> = proofs.into_iter().collect();
            proofs.sort_unstable();
            proofs
        }

        let mut graph = Self { nodes: Vec::new(), edges: Vec::new() };
        let mut indices: HashMap<NodeId, usize> = HashMap::new();
        let mut known_links: HashSet<(NodeId, NodeId, EdgeKind)> = HashSet::new();
        let mut attempts: HashMap<AttemptId, NodeId> = HashMap::new();
        let mut parents: HashMap<AttemptId, Option<AttemptId>> = HashMap::new();

        record_node(&mut graph, &mut indices, GraphNode {
            id: QUERY_ID,
            kind: GraphNodeKind::Query {
                symbol: trace.query,
                outcome: QueryOutcome::Value(LogicValue::Undetermined),
            },
            label: format!("Query {}", trace.query),
            related_events: Vec::new(),
        });

        for event in &trace.events
        {
            match &event.kind
            {
                T::LiteralProven { literal, origin } =>
                {
                    let kind = if matches!(origin, ProofOrigin::InitialFact)
                    {
                        GraphNodeKind::InitialFact { symbol: literal.symbol }
                    }
                    else
                    {
                        GraphNodeKind::Deduction {
                            literal: *literal,
                            proof_event: event.id,
                        }
                    };

                    record_node(&mut graph, &mut indices, GraphNode {
                        id: event.id,
                        kind,
                        label: literal_label(literal),
                        related_events: vec![event.id],
                    });

                    match origin
                    {
                        ProofOrigin::InitialFact => {}
                        ProofOrigin::Rule { rule_id } =>
                        {
                            link(
                                &mut graph, &mut known_links,
                                RULE_BASE + *rule_id as u64,
                                event.id, EdgeKind::Justification, event.id,
                            );
                        }
                        ProofOrigin::UnitPropagation { constraint_id } =>
                        {
                            let clause_id = CONSTRAINT_BASE + *constraint_id as u64;
                            link(
                                &mut graph, &mut known_links,
                                clause_id, event.id, EdgeKind::UnitPropagation, event.id,
                            );
                            // Only direct proof events are independent refutations
                            // of another literal in the clause.
                            for &cause in &event.causes
                            {
                                if trace.events.get(cause as usize).is_some_and(|e|
                                    matches!(&e.kind, T::LiteralProven { .. }))
                                {
                                    link(
                                        &mut graph, &mut known_links,
                                        cause, clause_id, EdgeKind::Justification, event.id,
                                    );
                                }
                            }
                        }
                    }
                }

                T::SearchStarted { attempt_id, parent_attempt, goal } =>
                {
                    let id = goal_id(*goal);
                    attempts.insert(*attempt_id, id);
                    parents.insert(*attempt_id, *parent_attempt);
                    record_node(&mut graph, &mut indices, GraphNode {
                        id,
                        kind: GraphNodeKind::SearchGoal { goal: *goal },
                        label: format!("Prove {}", literal_label(goal)),
                        related_events: vec![event.id],
                    });

                    let parent = parent_attempt
                        .and_then(|parent| attempts.get(&parent).copied())
                        .unwrap_or(QUERY_ID);
                    link(
                        &mut graph, &mut known_links,
                        parent, id, EdgeKind::SearchDependency, event.id,
                    );
                }

                T::SearchFinished { attempt_id, outcome: _ } =>
                {
                    if let Some(&id) = attempts.get(attempt_id)
                    {
                        if let Some(&idx) = indices.get(&id)
                        {
                            graph.nodes[idx].related_events.push(event.id);
                        }
                        for &cause in &event.causes
                        {
                            if trace.events.get(cause as usize).is_some_and(|e|
                                matches!(&e.kind, T::LiteralProven { .. }))
                            {
                                link(
                                    &mut graph, &mut known_links,
                                    cause, id, EdgeKind::Justification, event.id,
                                );
                            }
                        }
                        let parent = parents.get(attempt_id).copied().flatten()
                            .and_then(|parent| attempts.get(&parent).copied())
                            .unwrap_or(QUERY_ID);
                        link(
                            &mut graph, &mut known_links,
                            id, parent, EdgeKind::Return, event.id,
                        );
                    }
                }

                T::CycleDetected { previous_attempt, .. } =>
                {
                    let to = attempts.get(previous_attempt).copied();
                    let from = event.attempt_id.and_then(|id| attempts.get(&id).copied());
                    if let (Some(from), Some(to)) = (from, to)
                    {
                        link(
                            &mut graph, &mut known_links,
                            from, to, EdgeKind::CycleReference, event.id,
                        );
                    }
                }

                T::RuleConsidered { rule_id }
                | T::PremiseEvaluated { rule_id, .. }
                | T::RuleApplied { rule_id } =>
                {
                    let id = RULE_BASE + *rule_id as u64;
                    record_node(&mut graph, &mut indices, GraphNode {
                        id,
                        kind: GraphNodeKind::Rule { rule_id: *rule_id },
                        label: format!("Rule {}", rule_id),
                        related_events: vec![event.id],
                    });

                    if let T::RuleConsidered { .. } = &event.kind
                    {
                        if let Some(goal) = event.attempt_id
                            .and_then(|attempt| attempts.get(&attempt).copied())
                        {
                            link(
                                &mut graph, &mut known_links,
                                goal, id, EdgeKind::SearchDependency, event.id,
                            );
                        }
                    }
                    if let T::RuleApplied { .. } = &event.kind
                    {
                        for proof in supporting_proofs(&trace.events, &event.causes)
                        {
                            link(
                                &mut graph, &mut known_links,
                                proof, id, EdgeKind::PremiseDependency, event.id,
                            );
                        }
                    }
                }

                T::ConstraintActivated { constraint_id, rule_id, literals } =>
                {
                    let id = CONSTRAINT_BASE + *constraint_id as u64;
                    record_node(&mut graph, &mut indices, GraphNode {
                        id,
                        kind: GraphNodeKind::Constraint {
                            constraint_id: *constraint_id,
                            literals: literals.clone(),
                        },
                        label: literals.iter().map(literal_label).collect::<Vec<_>>().join(" | "),
                        related_events: vec![event.id],
                    });
                    link(
                        &mut graph, &mut known_links,
                        RULE_BASE + *rule_id as u64, id,
                        EdgeKind::ConstraintActivation, event.id,
                    );
                    if let Some(goal) = event.attempt_id
                        .and_then(|attempt| attempts.get(&attempt).copied())
                    {
                        link(
                            &mut graph, &mut known_links,
                            id, goal, EdgeKind::SearchDependency, event.id,
                        );
                    }
                }

                T::ConstraintEvaluated { constraint_id, .. } =>
                {
                    let id = CONSTRAINT_BASE + *constraint_id as u64;
                    if let Some(&idx) = indices.get(&id)
                    {
                        graph.nodes[idx].related_events.push(event.id);
                    }
                }

                T::ProofReused { original_event, .. } =>
                {
                    let to = event.attempt_id
                        .and_then(|attempt| attempts.get(&attempt).copied())
                        .unwrap_or(QUERY_ID);
                    link(
                        &mut graph, &mut known_links,
                        *original_event, to, EdgeKind::ProofReuse, event.id,
                    );
                }

                T::QueryFinished { result } =>
                {
                    if let Some(&idx) = indices.get(&QUERY_ID)
                    {
                        if let GraphNodeKind::Query { outcome, .. } =
                            &mut graph.nodes[idx].kind
                        {
                            *outcome = QueryOutcome::Value(*result);
                        }
                        graph.nodes[idx].related_events.push(event.id);
                    }
                    for &cause in &event.causes
                    {
                        link(
                            &mut graph, &mut known_links,
                            cause, QUERY_ID, EdgeKind::Justification, event.id,
                        );
                    }
                }

                T::QueryFailed { message } =>
                {
                    if let Some(&idx) = indices.get(&QUERY_ID)
                    {
                        if let GraphNodeKind::Query { outcome, .. } =
                            &mut graph.nodes[idx].kind
                        {
                            *outcome = QueryOutcome::Error { message: message.clone() };
                        }
                        graph.nodes[idx].related_events.push(event.id);
                    }
                }

                T::ExpressionEvaluated { .. } => {}
            }
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
