//! GUI presentation only. No parsing, inference, or file-system access.

use eframe::egui;

use crate::engine::{RunReport, SolveMode};
use crate::trace::LogicValue;

use super::document::SourceDocument;
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
    let width = ui.available_width();
    let editor = egui::TextEdit::multiline(&mut document.text)
        .code_editor()
        .desired_width(width)
        .desired_rows(30);

    let changed = ui.add_enabled(editable, editor).changed();
    if changed
    {
        document.mark_modified();
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
