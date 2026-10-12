//! GUI presentation only. No parsing, inference, or file-system access.

use eframe::egui;

use crate::engine::{RunReport, SolveMode};
use crate::trace::LogicValue;

use super::document::SourceDocument;
use super::facts::FactControls;
use super::runner::RunState;

/// Draw the current source file. Returns true if the user changed its contents.
pub fn source_editor(ui: &mut egui::Ui, document: &mut SourceDocument, editable: bool) -> bool
{
    ui.horizontal(|ui|
    {
        ui.heading("Rules");
        ui.separator();
        ui.strong(document.name());
        if document.modified
        {
            ui.colored_label(egui::Color32::from_rgb(221, 177, 89), "Modified");
        }
    });

    if let Some(path) = document.location()
    {
        ui.small(path.display().to_string());
    }
    else
    {
        ui.weak("No file opened");
    }
    ui.add_space(5.0);
    ui.separator();

    if document.text.is_empty() && !document.has_input()
    {
        ui.weak("Open a .txt file above, or enter rules directly.");
    }

    // The text is intentionally editable: Run uses the editor contents,
    // including unsaved changes, without modifying the opened file.
    // Explicit bounded scroll area: long rule files can be scrolled with
    // the mouse wheel or the permanently visible vertical scrollbar.
    let available_height = ui.available_height();
    let changed = egui::ScrollArea::vertical()
        .id_salt("rule_source_scroll")
        .auto_shrink([false, false])
        .max_height(available_height)
        .scroll_bar_visibility(
            egui::containers::scroll_area::ScrollBarVisibility::AlwaysVisible
        )
        .show(ui, |ui|
        {
            let editor = egui::TextEdit::multiline(&mut document.text)
                .code_editor()
                .desired_width(ui.available_width())
                .desired_rows(30);

            ui.add_enabled(editable, editor).changed()
        }).inner;

    if changed
    {
        document.mark_modified();
    }
    changed
}

/// Fact controls do not edit the source's '=...' line. Return true when the
/// effective runtime facts changed.
pub fn fact_editor(ui: &mut egui::Ui, facts: &mut FactControls, editable: bool) -> bool
{
    let mut changed = false;

    ui.horizontal_wrapped(|ui|
    {
        ui.strong("Initial facts");
        ui.weak("Applied only to the next run; does not modify the file");

        if ui.add_enabled(editable && facts.override_count() > 0,
            egui::Button::new("Reset to source")).clicked()
        {
            facts.restore_source();
            changed = true;
        }
    });

    ui.add_enabled_ui(editable, |ui|
    {
        egui::Grid::new("initial_fact_toggles")
            .num_columns(13)
            .spacing([12.0, 5.0])
            .show(ui, |ui|
            {
                for index in 0..26
                {
                    let mut active = facts.enabled(index);
                    let symbol = (b'A' + index as u8) as char;
                    if ui.checkbox(&mut active, symbol.to_string()).changed()
                    {
                        facts.set_enabled(index, active);
                        changed = true;
                    }
                    if index == 12
                    {
                        ui.end_row();
                    }
                }
            });
    });

    if facts.override_count() != 0
    {
        ui.small(format!(
            "{} override(s) active. Reset restores the source's initial facts.",
            facts.override_count()
        ));
    }

    changed
}

pub fn results(ui: &mut egui::Ui, state: &RunState)
{
    ui.heading("Results");
    ui.separator();

    match state
    {
        RunState::Idle =>
        {
            ui.weak("Select a rules file and press Run engine.");
        }

        RunState::Running =>
        {
            ui.horizontal(|ui|
            {
                ui.spinner();
                ui.label("Parsing and evaluating queries...");
            });
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }

        RunState::Failed(error) =>
        {
            ui.colored_label(
                egui::Color32::from_rgb(244, 101, 101),
                "Unable to evaluate the input:",
            );
            ui.add_space(5.0);
            ui.label(error);
            ui.add_space(8.0);
            ui.weak("Check the input, correct it in the editor, then run again.");
        }

        RunState::Completed(report) =>
        {
            show_completed(ui, report);
        }
    }
}

fn show_completed(ui: &mut egui::Ui, report: &RunReport)
{
    let mode = match report.mode
    {
        SolveMode::Sequential => "Sequential",
        SolveMode::Parallel => "Parallel",
    };

    ui.horizontal_wrapped(|ui|
    {
        ui.label(format!("Mode: {mode}"));
        ui.separator();
        ui.label(format!("Queries: {}", report.results.len()));
        ui.separator();
        ui.label(format!("Normalized rules: {}", report.normalized_rules));
    });

    ui.small(format!(
        "Parse + index: {:?}   |   Inference: {:?}   |   Total: {:?}",
        report.parse_duration, report.solve_duration, report.total_duration
    ));

    ui.add_space(10.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui|
        {
            egui::Grid::new("query_results")
                .striped(true)
                .num_columns(3)
                .spacing([18.0, 8.0])
                .show(ui, |ui|
                {
                    ui.strong("Query");
                    ui.strong("Result");
                    ui.strong("Details");
                    ui.end_row();

                    for result in &report.results
                    {
                        ui.monospace(result.symbol.to_string());

                        match &result.value
                        {
                            Ok(LogicValue::True) =>
                            {
                                ui.colored_label(egui::Color32::from_rgb(99, 209, 139), "True");
                                ui.label("");
                            }
                            Ok(LogicValue::False) =>
                            {
                                ui.colored_label(egui::Color32::from_rgb(119, 164, 239), "False");
                                ui.label("");
                            }
                            Ok(LogicValue::Undetermined) =>
                            {
                                ui.colored_label(egui::Color32::from_rgb(169, 174, 182), "Undetermined");
                                ui.label("Insufficient evidence");
                            }
                            Ok(LogicValue::Contradiction) =>
                            {
                                ui.colored_label(egui::Color32::from_rgb(243, 169, 79), "Contradiction");
                                ui.label("Positive and negative evidence");
                            }
                            Err(message) =>
                            {
                                ui.colored_label(egui::Color32::from_rgb(244, 101, 101), "Error");
                                ui.label(message);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}
