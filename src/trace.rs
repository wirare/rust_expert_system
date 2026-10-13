use serde::{Deserialize, Serialize};

pub type EventId = u64;
pub type AttemptId = u64;
pub type RuleId = usize;
pub type ConstraintId = usize;

/// Maximum retained events per query. The inference engine keeps running after
/// this limit; only tracing stops. This bounds CLI and GUI report memory.
pub const MAX_TRACE_EVENTS_PER_QUERY: usize = 100_000;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchOutcome
{
    Proven,
    NotProven,
    BlockedByCycle,
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationTrace
{
    pub query: char,
    pub events: Vec<TraceEvent>,
    #[serde(default)]
    pub attempts: u64,
    /// True if recording stopped at the per-query memory budget. The solver
    /// result remains complete, but no proof graph may be trusted afterward.
    #[serde(default)]
    pub truncated: bool,
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

    ExpressionEvaluated
    {
        expression: String,
        result: LogicValue,
        operand_events: Vec<EventId>,
    },

    QueryFinished
    {
        result: LogicValue,
    },

    QueryFailed
    {
        message: String,
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
        literals: Vec<Literal>,
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

impl EvaluationTrace
{
    pub fn new(query: char) -> Self
    {
        Self { query, events: Vec::new(), attempts: 0, truncated: false }
    }

    pub fn record(
        &mut self,
        attempt_id: Option<AttemptId>,
        kind: TraceEventKind,
        causes: Vec<EventId>,
    ) -> Option<EventId>
    {
        if self.events.len() >= MAX_TRACE_EVENTS_PER_QUERY
        {
            self.truncated = true;
            return None;
        }
        let id = self.events.len() as EventId;
        self.events.push(TraceEvent { id, attempt_id, kind, causes });
        Some(id)
    }

    pub fn is_full(&self) -> bool
    {
        self.truncated || self.events.len() >= MAX_TRACE_EVENTS_PER_QUERY
    }
}

#[cfg(test)]
mod bounds_tests
{
    use super::*;

    #[test]
    fn bounded_trace_drops_excess_events_without_growing()
    {
        let mut trace = EvaluationTrace::new('V');
        for index in 0..MAX_TRACE_EVENTS_PER_QUERY
        {
            assert_eq!(
                trace.record(None, TraceEventKind::RuleConsidered { rule_id: index }, vec![]),
                Some(index as u64),
            );
        }

        assert!(!trace.truncated);
        assert_eq!(trace.events.len(), MAX_TRACE_EVENTS_PER_QUERY);
        assert!(trace.record(None, TraceEventKind::RuleConsidered {
            rule_id: MAX_TRACE_EVENTS_PER_QUERY,
        }, vec![]).is_none());
        assert!(trace.truncated);

        for _ in 0..100_000
        {
            assert!(trace.record(None, TraceEventKind::RuleConsidered {
                rule_id: 0,
            }, vec![]).is_none());
        }
        assert_eq!(trace.events.len(), MAX_TRACE_EVENTS_PER_QUERY);

        let serialized = serde_json::to_string(&trace).unwrap();
        let recovered: EvaluationTrace = serde_json::from_str(&serialized).unwrap();
        assert_eq!(recovered.events.len(), MAX_TRACE_EVENTS_PER_QUERY);
        assert!(recovered.truncated);
    }
}
