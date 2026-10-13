//! A width-constrained, scrollable explanation panel.
//!
//! Never display raw Debug dumps in a horizontal row. All user-visible text
//! is wrapped inside the allocated side panel so long rules and errors cannot
//! extend beyond the application window.

use eframe::egui;

use crate::report::{ExecutionReport, QueryExecution, QueryOutcome};
use crate::trace::{
    ConstraintOutcome, LogicValue, ProofOrigin, TraceEvent, TraceEventKind,
};

use super::model::{
    DisplayNodeId, ExplanationGraph, ExplanationNode, LinkKind, NodeKind,
    literal_text, outcome_name, rule_display_expression,
};

fn wrapped(ui: &mut egui::Ui, text: impl Into<String>)
{
    ui.add(egui::Label::new(text.into()).wrap());
}

fn code(ui: &mut egui::Ui, text: impl Into<String>)
{
    ui.add(egui::Label::new(
        egui::RichText::new(text.into()).monospace()
    ).wrap());
}

fn section(ui: &mut egui::Ui, heading: &str)
{
    ui.add_space(12.0);
    ui.strong(heading);
    ui.separator();
}

fn outcome_description(outcome: &QueryOutcome) -> &'static str
{
    match outcome
    {
        QueryOutcome::Value(LogicValue::True) =>
            "There is recorded support for the positive literal.",
        QueryOutcome::Value(LogicValue::False) =>
            "False may be explicitly proven by a negative literal, or returned by the engine's default-false policy when no proof was established. Check the explanation for which case applies.",
        QueryOutcome::Value(LogicValue::Undetermined) =>
            "One or more active disjunctions remain unresolved, so the query cannot be reduced to a definite value.",
        QueryOutcome::Value(LogicValue::Contradiction) =>
            "Both a positive and a negative proof were found. This is a logical inconsistency, not a parser error.",
        QueryOutcome::Error { .. } =>
            "Inference failed. This error is not a logical False or a contradiction.",
    }
}

/// One proof uses one signed literal; the query may independently contain
/// evidence for its opposite. Never imply the contradiction cancels this step.
fn show_literal_status(
    ui: &mut egui::Ui,
    node: &ExplanationNode,
    query: &QueryExecution,
)
{
    let Some(literal) = node.literal else { return; };

    section(ui, "Evidence used here");
    ui.strong(format!(
        "{} ({} proof)",
        literal_text(literal),
        if literal.positive { "positive" } else { "negative" },
    ));

    match node.observed_status
    {
        Some(LogicValue::Contradiction) =>
        {
            ui.colored_label(
                egui::Color32::from_rgb(245, 183, 81),
                format!("CONTRADICTION: {} and !{} both proven", literal.symbol, literal.symbol),
            );
            wrapped(ui, format!(
                "This step still uses the {} proof of {}. The opposing proof \
is recorded separately; it does not erase the evidence used by this rule.",
                if literal.positive { "positive" } else { "negative" },
                literal_text(literal),
            ));

            section(ui, "Opposing evidence (not a prerequisite)");
            let mut count = 0;
            for event in &query.trace.events
            {
                let TraceEventKind::LiteralProven {
                    literal: other,
                    origin,
                } = &event.kind else { continue; };

                if other.symbol != literal.symbol || other.positive == literal.positive
                {
                    continue;
                }
                wrapped(ui, format!(
                    "Event #{}: {} proven ({origin:?})",
                    event.id, literal_text(*other),
                ));
                count += 1;
                if count == 3 { break; }
            }
        }
        Some(LogicValue::True) =>
        {
            ui.small(format!("{} has recorded positive support in this query.", literal.symbol));
        }
        Some(LogicValue::False) =>
        {
            ui.small(format!("!{} has recorded negative support in this query.", literal.symbol));
        }
        _ =>
        {
            ui.small("No matching proof was recorded for this query.");
        }
    }
}

fn node_details(
    ui: &mut egui::Ui,
    node: &ExplanationNode,
    report: &ExecutionReport,
    query: &QueryExecution,
)
{
    if matches!(node.kind, NodeKind::Fact | NodeKind::Derived)
    {
        show_literal_status(ui, node, query);
    }

    match node.kind
    {
        NodeKind::Answer =>
        {
            ui.label(format!("Query: {}", query.symbol));
            ui.strong(format!("Result: {}", outcome_name(&query.outcome)));
            wrapped(ui, outcome_description(&query.outcome));

            if let QueryOutcome::Error { message } = &query.outcome
            {
                section(ui, "Engine error");
                wrapped(ui, message.clone());
            }

            let positive = query.trace.events.iter().any(|event|
            {
                matches!(&event.kind, TraceEventKind::LiteralProven { literal, .. }
                    if literal.symbol == query.symbol && literal.positive)
            });
            let negative = query.trace.events.iter().any(|event|
            {
                matches!(&event.kind, TraceEventKind::LiteralProven { literal, .. }
                    if literal.symbol == query.symbol && !literal.positive)
            });

            section(ui, "Evidence summary");
            ui.label(format!("Positive proof: {}", if positive { "Recorded" } else { "Not recorded" }));
            ui.label(format!("Negative proof: {}", if negative { "Recorded" } else { "Not recorded" }));
            ui.label(format!("Trace events: {}", query.trace.events.len()));
            ui.label(format!("Recursive attempts: {}", query.trace.attempts));
        }

        NodeKind::Rule =>
        {
            let Some(rule_id) = node.rule_id else { return; };
            ui.label(format!("Normalized rule #{rule_id}"));

            if let Some(rule) = report.rewrite.normalized_rules.iter()
                .find(|rule| rule.id == rule_id)
            {
                section(ui, "Normalized expression");
                code(ui, rule_display_expression(&rule.expression).to_owned());

                if let Some(original) = report.rewrite.original_rules.iter()
                    .find(|original| original.id == rule.original_rule_id)
                {
                    section(ui, "Original source");
                    ui.label(format!("Line {}", original.source_line));
                    code(ui, original.expression.clone());
                }

                section(ui, "Transformation lineage");
                ui.label(format!(
                    "Original rule #{} · normalized version #{}",
                    rule.original_rule_id, rule.version_id,
                ));
                let transformations = report.rewrite.transformations.iter()
                    .filter(|t| t.outputs.contains(&rule.version_id))
                    .collect::<Vec<_>>();
                for transform in transformations
                {
                    wrapped(ui, format!("Produced by: {:?}", transform.kind));
                }
            }

            if let Some(event_id) = node.event_id
            {
                // Multiple attempts may evaluate the same normalized rule.
                // Inspect the premise immediately preceding *this* application,
                // not the first evaluation recorded for the rule.
                let premise = query.trace.events.iter().rev().find(|event|
                {
                    matches!(&event.kind, TraceEventKind::PremiseEvaluated {
                        rule_id: index, ..
                    } if *index == rule_id && event.id < event_id)
                });

                if let Some(premise) = premise
                {
                    if let TraceEventKind::PremiseEvaluated { result, .. } = &premise.kind
                    {
                        section(ui, "Premise evaluation");
                        ui.label(format!("Most recent recorded evaluation: {result:?}"));
                        if *result == LogicValue::Contradiction
                        {
                            wrapped(ui,
                                "This premise has both positive and negative support. \
The rule can be applied because positive support exists; only that supporting \
witness is connected to the rule in the explanation graph.");
                        }
                        else
                        {
                            ui.small("Only rules that were applied appear in this proof view.");
                        }
                    }
                }
            }
        }

        NodeKind::Fact =>
        {
            if let Some(literal) = node.literal
            {
                wrapped(ui, format!(
                    "{} is an enabled initial fact, supplied by the file's fact declaration or the GUI's runtime overrides.",
                    literal_text(literal)
                ));
                ui.small("This is a premise of the knowledge base, not a derived conclusion.");
            }
        }

        NodeKind::Derived =>
        {
            if let Some(literal) = node.literal
            {
                wrapped(ui, format!(
                    "The engine explicitly established {}.",
                    literal_text(literal),
                ));
            }
            if let Some(event_id) = node.event_id
            {
                ui.label(format!("Proof event #{event_id}"));
                if let Some(event) = query.trace.events.get(event_id as usize)
                {
                    match &event.kind
                    {
                        TraceEventKind::LiteralProven {
                            origin: ProofOrigin::InitialFact, ..
                        } =>
                            wrapped(ui, "Source: initial fact."),
                        TraceEventKind::LiteralProven {
                            origin: ProofOrigin::Rule { rule_id }, ..
                        } =>
                            wrapped(ui, format!(
                                "Direct conclusion of normalized rule #{rule_id}."
                            )),
                        TraceEventKind::LiteralProven {
                            origin: ProofOrigin::UnitPropagation { constraint_id }, ..
                        } =>
                            wrapped(ui, format!(
                                "Forced by OR constraint #{constraint_id}: the alternative literals were ruled out."
                            )),
                        _ => {}
                    }
                    if !event.causes.is_empty()
                    {
                        section(ui, "Recorded causal event IDs");
                        code(ui, event.causes.iter().map(u64::to_string)
                            .collect::<Vec<_>>().join(", "));
                    }
                }
            }
        }

        NodeKind::Clause =>
        {
            wrapped(ui, "A disjunctive conclusion: at least one displayed literal must hold.");
            section(ui, "Clause");
            code(ui, node.title.clone());
            if let Some(event_id) = node.event_id
            {
                if let Some(event) = query.trace.events.get(event_id as usize)
                {
                    if let TraceEventKind::ConstraintActivated {
                        constraint_id, rule_id, ..
                    } = &event.kind
                    {
                        ui.label(format!("Constraint #{constraint_id}"));
                        ui.label(format!("Activated by normalized rule #{rule_id}"));
                    }
                }
            }

            if let Some(rule_id) = node.rule_id
            {
                if let Some(last_result) = query.trace.events.iter().rev().find_map(|event|
                {
                    match &event.kind
                    {
                        TraceEventKind::ConstraintEvaluated { constraint_id, result }
                            if *constraint_id == rule_id => Some(result),
                        _ => None,
                    }
                })
                {
                    section(ui, "Last recorded evaluation");
                    match last_result
                    {
                        ConstraintOutcome::Satisfied =>
                            wrapped(ui, "Satisfied: at least one literal is established."),
                        ConstraintOutcome::Unresolved { literals } =>
                        {
                            wrapped(ui, "Unresolved: these alternatives remain possible:");
                            for literal in literals
                            {
                                code(ui, literal_text(*literal));
                            }
                        }
                        ConstraintOutcome::Forced { literal } =>
                            wrapped(ui, format!(
                                "Forced: {} is the remaining possibility.",
                                literal_text(*literal)
                            )),
                        ConstraintOutcome::Contradiction =>
                            wrapped(ui, "Contradiction: no literal can satisfy this clause."),
                    }
                }
            }
        }

        NodeKind::Notice =>
        {
            wrapped(ui, node.subtitle.clone());
            ui.add_space(6.0);
            ui.small("This explanation is informational; it does not represent proven evidence.");
        }
    }
}

fn event_description(event: &TraceEvent) -> String
{
    match &event.kind
    {
        TraceEventKind::LiteralProven { literal, origin } =>
            format!("Proved {} ({origin:?})", literal_text(*literal)),
        TraceEventKind::RuleApplied { rule_id } =>
            format!("Applied normalized rule #{rule_id}"),
        TraceEventKind::ConstraintActivated { constraint_id, .. } =>
            format!("Activated OR constraint #{constraint_id}"),
        TraceEventKind::PremiseEvaluated { rule_id, result } =>
            format!("Rule #{rule_id} premise evaluated: {result:?}"),
        TraceEventKind::ExpressionEvaluated { expression, result, .. } =>
            format!("Expression {expression}: {result:?}"),
        TraceEventKind::ConstraintEvaluated { constraint_id, result } =>
            format!("Constraint #{constraint_id}: {result:?}"),
        TraceEventKind::QueryFinished { result } =>
            format!("Query finished: {result:?}"),
        TraceEventKind::QueryFailed { message } =>
            format!("Query error: {message}"),
        TraceEventKind::ProofReused { literal, .. } =>
            format!("Reused proof of {}", literal_text(*literal)),
        TraceEventKind::CycleDetected { goal, .. } =>
            format!("Cycle detected for {}", literal_text(*goal)),
        TraceEventKind::SearchStarted { goal, .. } =>
            format!("Search started for {}", literal_text(*goal)),
        TraceEventKind::SearchFinished { outcome, .. } =>
            format!("Search completed: {outcome:?}"),
        TraceEventKind::RuleConsidered { rule_id } =>
            format!("Considered rule #{rule_id}"),
    }
}

fn connection_section(
    ui: &mut egui::Ui,
    graph: &ExplanationGraph,
    selected: DisplayNodeId,
    selection: &mut Option<DisplayNodeId>,
)
{
    let parents = graph.links.iter().filter(|edge| edge.to == selected)
        .collect::<Vec<_>>();
    let children = graph.links.iter().filter(|edge| edge.from == selected)
        .collect::<Vec<_>>();

    section(ui, "Why does this node follow?");
    if parents.is_empty()
    {
        ui.weak("No prerequisites recorded for this node.");
    }
    for link in parents
    {
        link_button(ui, graph, link.from, link.kind, "←", selection);
    }

    section(ui, "What does this node support?");
    if children.is_empty()
    {
        ui.weak("This node does not support another visible step.");
    }
    for link in children
    {
        link_button(ui, graph, link.to, link.kind, "→", selection);
    }
}

fn link_button(
    ui: &mut egui::Ui,
    graph: &ExplanationGraph,
    neighbor: DisplayNodeId,
    kind: LinkKind,
    arrow: &str,
    selection: &mut Option<DisplayNodeId>,
)
{
    if let Some(node) = graph.node(neighbor)
    {
        ui.small(format!("{arrow} {}", kind.label()));
        let width = ui.available_width().max(60.0);
        // Button text is intentionally shortened: a long unbroken rule
        // must not widen the inspector or overpaint its neighboring widgets.
        let capacity = ((width / 7.5) as usize).clamp(8, 48);
        let display = if node.title.chars().count() > capacity
        {
            format!(
                "{}…",
                node.title.chars().take(capacity.saturating_sub(1))
                    .collect::<String>()
            )
        }
        else
        {
            node.title.clone()
        };
        let button = ui.add_sized(
            [width, 27.0],
            egui::Button::new(display),
        );
        if button.clicked()
        {
            *selection = Some(neighbor);
        }
        button.on_hover_text(&node.title);
        ui.add_space(5.0);
    }
}

/// A bounded inspector; the containing SidePanel sets its size. Every label
/// is explicitly wrapped, and the whole body gets a vertical scrollbar.
pub fn show(
    ui: &mut egui::Ui,
    report: &ExecutionReport,
    query: &QueryExecution,
    graph: &ExplanationGraph,
    selection: &mut Option<DisplayNodeId>,
)
{
    ui.heading("Explanation");
    ui.separator();

    let Some(selected) = *selection else
    {
        wrapped(ui, "Select a proof step to read its justification.");
        return;
    };
    let Some(node) = graph.node(selected) else
    {
        *selection = Some(graph.root);
        return;
    };

    let inner_width = ui.available_width().max(40.0);

    egui::ScrollArea::vertical()
        .id_salt("proof_inspector_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui|
        {
            // ScrollArea measures unwrapped contents: constrain it explicitly
            // before adding any long rule expression or path.
            ui.set_max_width(inner_width);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

            ui.small(node.kind.name());
            wrapped(ui, node.title.clone());
            ui.add_space(4.0);
            wrapped(ui, node.subtitle.clone());
            ui.small(format!("Graph node #{}", node.id));

            section(ui, "Details");
            node_details(ui, node, report, query);

            connection_section(ui, graph, selected, selection);

            section(ui, "Trace provenance");
            if let Some(event_id) = node.event_id
            {
                if let Some(event) = query.trace.events.get(event_id as usize)
                {
                    ui.small(format!("Event #{}", event.id));
                    wrapped(ui, event_description(event));
                    if !event.causes.is_empty()
                    {
                        ui.small("Direct recorded causes");
                        code(ui, event.causes.iter().map(u64::to_string)
                            .collect::<Vec<_>>().join(", "));
                    }
                }
            }
            else
            {
                ui.small("This is a summary node, not a single trace event.");
            }
        });
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn long_expression_is_preserved_for_inspection()
    {
        let expression = "A + B + C + D + E + F + G => H | I | J".to_owned();
        let node = ExplanationNode {
            id: 1,
            kind: NodeKind::Rule,
            title: expression.clone(),
            subtitle: "Line 4".into(),
            event_id: None,
            rule_id: Some(2),
            literal: None,
            observed_status: None,
        };
        assert_eq!(node.title, expression);
        assert_eq!(node.kind.name(), "RULE APPLIED");
    }
}
