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
    RemoveTautology,
    DeduplicateLiterals,
}
