//! Read-only inspector for a selected reasoning graph node.

use eframe::egui;

use crate::report::{
    ExecutionReport, GraphNode, GraphNodeKind, NodeId, QueryExecution, QueryOutcome,
};
use crate::trace::{Literal, LogicValue, TraceEventKind};

fn literal_text(value: &Literal) -> String
{
    if value.positive { value.symbol.to_string() }
    else { format!("!{}", value.symbol) }
}

fn outcome_text(value: &QueryOutcome) -> String
{
    match value
    {
        QueryOutcome::Value(LogicValue::True) => "True".to_owned(),
        QueryOutcome::Value(LogicValue::False) => "False".to_owned(),
        QueryOutcome::Value(LogicValue::Undetermined) => "Undetermined".to_owned(),
        QueryOutcome::Value(LogicValue::Contradiction) => "Contradiction".to_owned(),
        QueryOutcome::Error { message } => format!("Error: {message}"),
    }
}

fn node_category(kind: &GraphNodeKind) -> &'static str
{
    match kind
    {
        GraphNodeKind::InitialFact { .. } => "Initial fact",
        GraphNodeKind::Deduction { .. } => "Deduction",
        GraphNodeKind::Query { .. } => "Query result",
        GraphNodeKind::SearchGoal { .. } => "Search goal",
        GraphNodeKind::Rule { .. } => "Normalized rule",
        GraphNodeKind::Constraint { .. } => "Constraint",
        GraphNodeKind::SearchAttempt { .. } => "Search attempt",
        GraphNodeKind::Cycle { .. } => "Cycle",
        GraphNodeKind::Event { .. } => "Event",
        GraphNodeKind::Transformation { .. } => "Transformation",
        GraphNodeKind::RuleVersion { .. } => "Rule version",
    }
}

fn show_node_metadata(
    ui: &mut egui::Ui,
    node: &GraphNode,
    report: &ExecutionReport,
    query: &QueryExecution,
)
{
    match &node.kind
    {
        GraphNodeKind::Query { symbol, outcome } =>
        {
            ui.label(format!("Query symbol: {symbol}"));
            ui.label(format!("Result: {}", outcome_text(outcome)));
        }

        GraphNodeKind::InitialFact { symbol } =>
        {
            ui.label(format!("{symbol} is an explicitly enabled initial fact."));
            ui.weak("Initial facts come from the active GUI checkboxes.");
        }

        GraphNodeKind::Rule { rule_id } =>
        {
            if let Some(rule) = report.rewrite.normalized_rules
                .iter().find(|rule| rule.id == *rule_id)
            {
                ui.label("Normalized expression:");
                ui.monospace(&rule.expression);
                if let Some(original) = report.rewrite.original_rules.iter()
                    .find(|r| r.id == rule.original_rule_id)
                {
                    ui.add_space(4.0);
                    ui.label(format!("Original input line {}", original.source_line));
                    ui.monospace(&original.expression);
                }
            }
            else
            {
                ui.label(format!("Rule identifier: {rule_id}"));
            }
        }

        GraphNodeKind::Constraint { constraint_id, literals } =>
        {
            ui.label(format!("Constraint index: {constraint_id}"));
            let expression = literals.iter().map(literal_text)
                .collect::<Vec<_>>().join(" | ");
            ui.monospace(expression);
            ui.weak("A disjunctive conclusion activated by a rule.");
        }

        GraphNodeKind::Deduction { literal, proof_event } =>
        {
            ui.label(format!("Established literal: {}", literal_text(literal)));
            ui.label(format!("Proof event: {proof_event}"));
            if let Some(event) = query.trace.events.iter()
                .find(|event| event.id == *proof_event)
            {
                if let TraceEventKind::LiteralProven { origin, .. } = &event.kind
                {
                    ui.label(format!("Origin: {origin:?}"));
                    if !event.causes.is_empty()
                    {
                        ui.label(format!("Causal event IDs: {:?}", event.causes));
                    }
                }
            }
        }

        GraphNodeKind::SearchGoal { goal } =>
        {
            ui.label(format!("Attempting to prove: {}", literal_text(goal)));
            ui.weak("All recursive attempts for this literal share this display node.");
        }

        GraphNodeKind::SearchAttempt { attempt_id, goal, outcome } =>
        {
            ui.label(format!("Attempt #{attempt_id}"));
            ui.monospace(literal_text(goal));
            ui.label(format!("Outcome: {outcome:?}"));
        }

        GraphNodeKind::Cycle { attempt_id, previous_attempt } =>
        {
            ui.label(format!(
                "Cycle between attempts {attempt_id} and {previous_attempt}"
            ));
        }

        GraphNodeKind::Event { event_id } =>
        {
            ui.label(format!("Trace event ID: {event_id}"));
        }

        GraphNodeKind::Transformation { transformation_id } =>
        {
            ui.label(format!("Transformation ID: {transformation_id}"));
        }

        GraphNodeKind::RuleVersion { version_id } =>
        {
            ui.label(format!("Rule version ID: {version_id}"));
        }
    }
}

/// The caller owns the selection; clicking a neighboring node here uses the
/// same node IDs and the same immutable, generated graph as canvas clicks.
pub fn show(
    ui: &mut egui::Ui,
    report: &ExecutionReport,
    query: &QueryExecution,
    selection: &mut Option<NodeId>,
)
{
    ui.heading("Node details");
    ui.separator();

    let Some(id) = *selection else
    {
        ui.weak("Click a graph node to inspect its rule, fact, proof, or constraint.");
        return;
    };

    let Some(node) = query.graph.nodes.iter().find(|node| node.id == id) else
    {
        *selection = None;
        ui.weak("The selected node is not part of this query.");
        return;
    };

    egui::ScrollArea::vertical()
        .id_salt("graph_node_details_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui|
        {
            ui.strong(&node.label);
            ui.label(node_category(&node.kind));
            ui.small(format!("Node ID: {}", node.id));
            ui.add_space(7.0);

            show_node_metadata(ui, node, report, query);

            ui.add_space(12.0);
            ui.strong("Connections");
            ui.separator();

            let related_edges = query.graph.edges.iter()
                .filter(|edge| edge.from == id || edge.to == id)
                .collect::<Vec<_>>();

            if related_edges.is_empty()
            {
                ui.weak("No connections recorded.");
            }
            else
            {
                for edge in related_edges.iter().take(60)
                {
                    let outbound = edge.from == id;
                    let other = if outbound { edge.to } else { edge.from };
                    let other_label = query.graph.nodes.iter()
                        .find(|candidate| candidate.id == other)
                        .map(|node| node.label.as_str())
                        .unwrap_or("Unknown");
                    let arrow = if outbound { "→" } else { "←" };

                    ui.horizontal_wrapped(|ui|
                    {
                        ui.small(format!("{arrow} {:?}", edge.kind));
                        if ui.small_button(other_label).clicked()
                        {
                            *selection = Some(other);
                        }
                    });
                }
                if related_edges.len() > 60
                {
                    ui.small(format!(
                        "Showing 60 of {} connections", related_edges.len()
                    ));
                }
            }

            ui.add_space(12.0);
            ui.strong(format!(
                "Related trace events ({})",
                node.related_events.len()
            ));
            ui.separator();

            for id in node.related_events.iter().take(16)
            {
                if let Some(event) = query.trace.events.iter()
                    .find(|event| event.id == *id)
                {
                    ui.small(format!("#{}: {:?}", event.id, event.kind));
                }
            }
            if node.related_events.len() > 16
            {
                ui.small(format!(
                    "Showing 16 of {} events", node.related_events.len()
                ));
            }
        });
}
