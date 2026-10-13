//! Explanation-first projection of a chronological inference trace.
//!
//! The complete trace contains failed searches, cache hits, and repeated
//! attempts. Drawing all of them makes an unreadable hairball. This module
//! follows *only* causal evidence for the selected query, retaining the
//! original event IDs so nothing is fabricated or disconnected from the engine.

use std::collections::{HashMap, HashSet};

use crate::report::{ExecutionReport, QueryExecution, QueryOutcome};
use crate::trace::{
    ConstraintOutcome, EventId, Literal, LogicValue, ProofOrigin, TraceEventKind,
};

pub type DisplayNodeId = u64;
const QUERY_ID: DisplayNodeId = u64::MAX;
const NOTE_ID: DisplayNodeId = u64::MAX - 1;
const MAX_DISPLAY_NODES: usize = 280;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind
{
    Answer,
    Fact,
    Rule,
    Clause,
    Derived,
    Notice,
}

impl NodeKind
{
    pub fn name(self) -> &'static str
    {
        match self
        {
            Self::Answer => "ANSWER",
            Self::Fact => "INITIAL FACT",
            Self::Rule => "RULE APPLIED",
            Self::Clause => "OR CONSTRAINT",
            Self::Derived => "DEDUCTION",
            Self::Notice => "NOT ESTABLISHED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExplanationNode
{
    pub id: DisplayNodeId,
    pub kind: NodeKind,
    pub title: String,
    pub subtitle: String,
    pub event_id: Option<EventId>,
    pub rule_id: Option<usize>,
    pub literal: Option<Literal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkKind
{
    Premise,
    Activates,
    Derives,
    RulesOut,
    Forces,
    Supports,
    Answer,
    Unresolved,
}

impl LinkKind
{
    pub fn label(self) -> &'static str
    {
        match self
        {
            Self::Premise => "satisfies premise",
            Self::Activates => "activates",
            Self::Derives => "derives",
            Self::RulesOut => "rules out an alternative",
            Self::Forces => "forces",
            Self::Supports => "supports",
            Self::Answer => "establishes the answer",
            Self::Unresolved => "remains ambiguous",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExplanationLink
{
    pub from: DisplayNodeId,
    pub to: DisplayNodeId,
    pub kind: LinkKind,
}

#[derive(Debug, Default)]
pub struct ExplanationGraph
{
    pub nodes: Vec<ExplanationNode>,
    pub links: Vec<ExplanationLink>,
    pub root: DisplayNodeId,
    pub limited: bool,
}

impl ExplanationGraph
{
    pub fn node(&self, id: DisplayNodeId) -> Option<&ExplanationNode>
    {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn new(report: &ExecutionReport, query: &QueryExecution) -> Self
    {
        let mut builder = Builder {
            report,
            query,
            output: Self {
                root: QUERY_ID,
                ..Self::default()
            },
            existing: HashSet::new(),
            links: HashSet::new(),
            visited: HashSet::new(),
        };

        builder.add_node(ExplanationNode {
            id: QUERY_ID,
            kind: NodeKind::Answer,
            title: format!("{} = {}", query.symbol, outcome_name(&query.outcome)),
            subtitle: match &query.outcome
            {
                QueryOutcome::Value(LogicValue::Contradiction) =>
                    "Positive and negative proofs both exist".into(),
                QueryOutcome::Value(LogicValue::Undetermined) =>
                    "An active disjunction remains unresolved".into(),
                QueryOutcome::Value(LogicValue::False) =>
                    "Explicit negation, or default false if unproven".into(),
                QueryOutcome::Value(LogicValue::True) =>
                    "Supported by recorded evidence".into(),
                QueryOutcome::Error { .. } =>
                    "The engine did not finish this query".into(),
            },
            event_id: None,
            rule_id: None,
            literal: None,
        });

        let final_event = query.trace.events.iter().rev().find(|event|
        {
            matches!(&event.kind,
                TraceEventKind::QueryFinished { .. } | TraceEventKind::QueryFailed { .. })
        });

        if let Some(event) = final_event
        {
            for &cause in &event.causes
            {
                for source in builder.resolve(cause)
                {
                    builder.link(source, QUERY_ID, LinkKind::Answer);
                }
            }
        }

        let has_answer_evidence = builder.output.links.iter()
            .any(|link| link.to == QUERY_ID && link.kind == LinkKind::Answer);

        if !has_answer_evidence
        {
            builder.no_proof_context();
        }

        builder.output.nodes.sort_by_key(|node| node.id);
        builder.output.links.sort_by_key(|link|
        {
            (link.from, link.to, link.kind as u8)
        });
        builder.output
    }
}

pub fn literal_text(literal: Literal) -> String
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

pub fn outcome_name(outcome: &QueryOutcome) -> &'static str
{
    match outcome
    {
        QueryOutcome::Value(LogicValue::True) => "TRUE",
        QueryOutcome::Value(LogicValue::False) => "FALSE",
        QueryOutcome::Value(LogicValue::Undetermined) => "UNDETERMINED",
        QueryOutcome::Value(LogicValue::Contradiction) => "CONTRADICTION",
        QueryOutcome::Error { .. } => "ERROR",
    }
}

struct Builder<'a>
{
    report: &'a ExecutionReport,
    query: &'a QueryExecution,
    output: ExplanationGraph,
    existing: HashSet<DisplayNodeId>,
    links: HashSet<(DisplayNodeId, DisplayNodeId, LinkKind)>,
    visited: HashSet<EventId>,
}

impl Builder<'_>
{
    fn add_node(&mut self, node: ExplanationNode)
    {
        if self.existing.contains(&node.id)
        {
            return;
        }
        if self.output.nodes.len() >= MAX_DISPLAY_NODES
        {
            self.output.limited = true;
            return;
        }

        self.existing.insert(node.id);
        self.output.nodes.push(node);
    }

    fn link(&mut self, from: DisplayNodeId, to: DisplayNodeId, kind: LinkKind)
    {
        if from == to || !self.existing.contains(&from) || !self.existing.contains(&to)
        {
            return;
        }
        if self.links.insert((from, to, kind))
        {
            self.output.links.push(ExplanationLink { from, to, kind });
        }
    }

    fn normalized_rule(&self, rule_id: usize) -> (String, String)
    {
        if let Some(rule) = self.report.rewrite.normalized_rules.iter()
            .find(|rule| rule.id == rule_id)
        {
            let line = self.report.rewrite.original_rules.iter()
                .find(|original| original.id == rule.original_rule_id)
                .map(|original| format!("Input line {}", original.source_line))
                .unwrap_or_else(|| "Normalized conclusion".into());
            (rule.expression.clone(), line)
        }
        else
        {
            (format!("Rule #{rule_id}"), "Normalized rule".into())
        }
    }

    /// Returns the nearest visible predecessors for a trace event. Uninteresting
    /// intermediate events (premise evaluation, AST operations, cached reuse)
    /// are traversed but not rendered as standalone cards.
    fn resolve(&mut self, event_id: EventId) -> Vec<DisplayNodeId>
    {
        if self.visited.contains(&event_id)
        {
            return if self.existing.contains(&event_id)
            {
                vec![event_id]
            }
            else
            {
                // Intermediate nodes may be shared. Their ancestors are
                // recalculated to preserve all causal edges.
                self.resolve_ancestors(event_id)
            };
        }
        self.visited.insert(event_id);

        let Some(event) = self.query.trace.events.get(event_id as usize) else
        {
            return Vec::new();
        };
        let kind = event.kind.clone();
        let causes = event.causes.clone();

        match kind
        {
            TraceEventKind::LiteralProven { literal, origin } =>
            {
                let (kind, subtitle, rule_id) = match origin
                {
                    ProofOrigin::InitialFact =>
                        (NodeKind::Fact, "Enabled initial fact".into(), None),
                    ProofOrigin::Rule { rule_id } =>
                        (NodeKind::Derived, format!("Directly inferred by rule #{rule_id}"), Some(rule_id)),
                    ProofOrigin::UnitPropagation { constraint_id } =>
                        (NodeKind::Derived, format!("Forced by OR constraint #{constraint_id}"), Some(constraint_id)),
                };

                self.add_node(ExplanationNode {
                    id: event_id,
                    kind,
                    title: format!("{} established", literal_text(literal)),
                    subtitle,
                    event_id: Some(event_id),
                    rule_id,
                    literal: Some(literal),
                });

                let ancestors = causes.iter().flat_map(|cause| self.resolve(*cause))
                    .collect::<Vec<_>>();
                if let ProofOrigin::UnitPropagation { .. } = origin
                {
                    // Other proven literals eliminate alternative choices.
                    // Show their support entering the clause rather than
                    // drawing a misleading proof directly into the forced fact.
                    let clause = ancestors.iter().find(|id| self.output.node(**id)
                        .is_some_and(|node| node.kind == NodeKind::Clause)).copied();
                    if let Some(clause) = clause
                    {
                        self.link(clause, event_id, LinkKind::Forces);
                        for predecessor in ancestors
                        {
                            if predecessor != clause
                            {
                                self.link(predecessor, clause, LinkKind::RulesOut);
                            }
                        }
                    }
                    else
                    {
                        for predecessor in ancestors
                        {
                            self.link(predecessor, event_id, LinkKind::Supports);
                        }
                    }
                }
                else
                {
                    for predecessor in ancestors
                    {
                        self.link(predecessor, event_id, LinkKind::Derives);
                    }
                }

                vec![event_id]
            }

            TraceEventKind::RuleApplied { rule_id } =>
            {
                let (title, subtitle) = self.normalized_rule(rule_id);
                self.add_node(ExplanationNode {
                    id: event_id,
                    kind: NodeKind::Rule,
                    title,
                    subtitle,
                    event_id: Some(event_id),
                    rule_id: Some(rule_id),
                    literal: None,
                });

                let ancestors = causes.iter().flat_map(|cause| self.resolve(*cause))
                    .collect::<Vec<_>>();
                for predecessor in ancestors
                {
                    self.link(predecessor, event_id, LinkKind::Premise);
                }
                vec![event_id]
            }

            TraceEventKind::ConstraintActivated { constraint_id, rule_id, literals } =>
            {
                let title = literals.iter().map(|literal| literal_text(*literal))
                    .collect::<Vec<_>>().join(" | ");
                self.add_node(ExplanationNode {
                    id: event_id,
                    kind: NodeKind::Clause,
                    title,
                    subtitle: format!("At least one must hold · from rule #{rule_id}"),
                    event_id: Some(event_id),
                    rule_id: Some(constraint_id),
                    literal: None,
                });

                let ancestors = causes.iter().flat_map(|cause| self.resolve(*cause))
                    .collect::<Vec<_>>();
                for predecessor in ancestors
                {
                    self.link(predecessor, event_id, LinkKind::Activates);
                }
                vec![event_id]
            }

            TraceEventKind::ProofReused { original_event, .. } =>
                self.resolve(original_event),

            // All other events are evidence carriers but not graphical steps.
            // They remain available through the inspector's trace details.
            _ =>
            {
                let mut sources = Vec::new();
                for cause in causes
                {
                    sources.extend(self.resolve(cause));
                }
                sources.sort_unstable();
                sources.dedup();
                sources
            }
        }
    }

    fn resolve_ancestors(&mut self, id: EventId) -> Vec<DisplayNodeId>
    {
        let Some(event) = self.query.trace.events.get(id as usize) else
        {
            return Vec::new();
        };
        let mut ancestors = event.causes.clone();
        if let TraceEventKind::ProofReused { original_event, .. } = &event.kind
        {
            ancestors.push(*original_event);
        }
        let mut results = Vec::new();
        for cause in ancestors
        {
            if cause < id
            {
                results.extend(self.resolve(cause));
            }
        }
        results.sort_unstable();
        results.dedup();
        results
    }

    fn no_proof_context(&mut self)
    {
        match &self.query.outcome
        {
            QueryOutcome::Value(LogicValue::Undetermined) =>
            {
                // Unresolved clauses can explain ambiguity, but *do not*
                // prove the queried literal. Use a distinct dashed link.
                let clauses = self.query.trace.events.iter()
                    .filter_map(|event|
                    {
                        let TraceEventKind::ConstraintActivated {
                            literals, constraint_id, ..
                        } = &event.kind else { return None; };
                        if !literals.iter().any(|literal| literal.symbol == self.query.symbol)
                        {
                            return None;
                        }
                        let unresolved = self.query.trace.events.iter().any(|other|
                        {
                            matches!(&other.kind,
                                TraceEventKind::ConstraintEvaluated {
                                    constraint_id: idx,
                                    result: ConstraintOutcome::Unresolved { .. },
                                } if idx == constraint_id)
                        });
                        unresolved.then_some(event.id)
                    })
                    .collect::<Vec<_>>();

                if clauses.is_empty()
                {
                    self.add_note(
                        "Insufficient evidence",
                        "Neither side could be established; one or more conditions remain unresolved.",
                        LinkKind::Unresolved,
                    );
                }
                else
                {
                    for id in clauses.into_iter().take(8)
                    {
                        for node in self.resolve(id)
                        {
                            self.link(node, QUERY_ID, LinkKind::Unresolved);
                        }
                    }
                }
            }
            QueryOutcome::Value(LogicValue::False) =>
            {
                self.add_note(
                    "No proof established",
                    "No explicit !query proof was recorded. This engine returns False by default when no unresolved constraint applies. This is not a proof of negation.",
                    LinkKind::Unresolved,
                );
            }
            QueryOutcome::Error { message } =>
            {
                self.add_note(
                    "Evaluation error",
                    message,
                    LinkKind::Unresolved,
                );
            }
            _ =>
            {
                self.add_note(
                    "No recorded proof",
                    "The query result has no traceable final evidence.",
                    LinkKind::Unresolved,
                );
            }
        }
    }

    fn add_note(&mut self, title: &str, subtitle: &str, kind: LinkKind)
    {
        self.add_node(ExplanationNode {
            id: NOTE_ID,
            kind: NodeKind::Notice,
            title: title.to_owned(),
            subtitle: subtitle.to_owned(),
            event_id: None,
            rule_id: None,
            literal: None,
        });
        self.link(NOTE_ID, QUERY_ID, kind);
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::engine::{run_source, SolveMode};

    fn explanation(source: &str) -> ExplanationGraph
    {
        let run = run_source(source, SolveMode::Sequential).unwrap();
        ExplanationGraph::new(&run.execution, &run.execution.queries[0])
    }

    #[test]
    fn proof_shows_premise_rule_conclusion_and_answer()
    {
        let graph = explanation("A=>B\n=A\n?B");
        assert!(graph.nodes.iter().any(|n| n.kind == NodeKind::Fact && n.title.contains('A')));
        assert!(graph.nodes.iter().any(|n| n.kind == NodeKind::Rule && n.title.contains("=>")));
        assert!(graph.nodes.iter().any(|n| n.kind == NodeKind::Derived && n.title.contains('B')));
        assert!(graph.links.iter().any(|e| e.kind == LinkKind::Premise));
        assert!(graph.links.iter().any(|e| e.kind == LinkKind::Derives));
        assert!(graph.links.iter().any(|e| e.kind == LinkKind::Answer));
        assert!(graph.nodes.len() < 8, "Do not render exploratory searches as evidence");
    }

    #[test]
    fn contradiction_has_both_polarities()
    {
        let graph = explanation("A=>B\nC=>!B\n=AC\n?B");
        assert!(graph.nodes.iter().any(|n|
            n.literal == Some(Literal { symbol:'B', positive:true })));
        assert!(graph.nodes.iter().any(|n|
            n.literal == Some(Literal { symbol:'B', positive:false })));
        assert_eq!(graph.links.iter().filter(|link| link.to == QUERY_ID &&
            link.kind == LinkKind::Answer).count(), 2);
    }

    #[test]
    fn absence_of_proof_is_labeled_not_invented()
    {
        let graph = explanation("A=>B\n=\n?B");
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.iter().any(|node| node.kind == NodeKind::Notice));
        assert!(!graph.links.iter().any(|link| link.kind == LinkKind::Answer));
    }

    #[test]
    fn unresolved_clause_is_explanatory_not_a_proof()
    {
        let graph = explanation("A=>B|C\n=A\n?B");
        assert!(graph.nodes.iter().any(|n| n.kind == NodeKind::Clause));
        assert!(graph.links.iter().any(|e| e.kind == LinkKind::Unresolved));
        assert!(!graph.links.iter().any(|e| e.kind == LinkKind::Answer));
    }

    #[test]
    fn forced_clause_has_refutation_and_derivation()
    {
        let graph = explanation("A=>B|C\nD=>!C\n=AD\n?B");
        assert!(graph.nodes.iter().any(|node| node.kind == NodeKind::Clause));
        assert!(graph.links.iter().any(|edge| edge.kind == LinkKind::Forces));
        assert!(graph.links.iter().any(|edge| edge.kind == LinkKind::RulesOut));
    }

    #[test]
    fn explanation_nodes_do_not_duplicate_visits()
    {
        let graph = explanation(include_str!("../../../test.txt"));
        let mut ids = HashSet::new();
        assert!(graph.nodes.iter().all(|node| ids.insert(node.id)));
        assert!(graph.links.iter().all(|link| ids.contains(&link.from) && ids.contains(&link.to)));
    }
}
