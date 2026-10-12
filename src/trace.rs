use serde::{Deserialize, Serialize};

pub type EventId = u64;
pub type AttemptId = u64;
pub type RuleId = usize;
pub type ConstraintId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Literal
{
    pub symbol: char,
    pub positive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicValue
{
    True,
    False,
    Undetermined,
    Contradiction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchOutcome
{
    Proven,
    NotProven,
    BlockedByCycle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationTrace
{
    pub query: char,
    pub events: Vec<TraceEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent
{
    pub id: EventId,
    pub attempt_id: Option<AttemptId>,
    pub kind: TraceEventKind,
    pub causes: Vec<EventId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceEventKind
{
    SearchStarted
    {
        attempt_id: AttemptId,
        parent_attempt: Option<AttemptId>,
        goal: Literal,
    },

    SearchFinished
    {
        attempt_id: AttemptId,
        outcome: SearchOutcome,
    },

    RuleConsidered
    {
        rule_id: RuleId,
    },

    PremiseEvaluated
    {
        rule_id: RuleId,
        result: LogicValue,
    },

    RuleApplied
    {
        rule_id: RuleId,
    },

    ConstraintActivated
    {
        constraint_id: ConstraintId,
        rule_id: RuleId,
    },

    ConstraintEvaluated
    {
        constraint_id: ConstraintId,
        result: ConstraintOutcome,
    },

    LiteralProven
    {
        literal: Literal,
        origin: ProofOrigin,
    },

    ProofReused
    {
        literal: Literal,
        original_event: EventId,
    },

    CycleDetected
    {
        goal: Literal,
        previous_attempt: AttemptId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintOutcome
{
    Satisfied,

    Unresolved
    {
        literals: Vec<Literal>,
    },

    Forced
    {
        literal: Literal,
    },

    Contradiction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofOrigin
{
    InitialFact,

    Rule
    {
        rule_id: RuleId,
    },

    UnitPropagation
    {
        constraint_id: ConstraintId,
    },
}
