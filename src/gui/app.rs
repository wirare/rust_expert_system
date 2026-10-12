//! Application shell and composition root for future graph/trace views.

use eframe::egui;

use crate::engine::SolveMode;

use super::document::SourceDocument;
use super::facts::FactControls;
use super::graph::GraphViewer;
use super::runner::RunState;
use super::runner::RunController;
use super::views;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum WorkspaceView
{
    #[default]
    Rules,
    Graph,
}

#[derive(Default)]
pub struct ExpertSystemApp
{
    document: SourceDocument,
    facts: FactControls,
    runner: RunController,
    parallel: bool,
    file_error: Option<String>,
    confirm_discard_before_open: bool,
    workspace: WorkspaceView,
    graph_viewer: GraphViewer,
}

impl ExpertSystemApp
{
    pub fn new(_creation_context: &eframe::CreationContext<'_>) -> Self
    {
        Self::default()
    }

    fn request_open(&mut self)
    {
        if self.document.modified
        {
            self.confirm_discard_before_open = true;
        }
        else
        {
            self.open_document();
        }
    }

    fn open_document(&mut self)
    {
        match SourceDocument::open_dialog()
        {
            Ok(Some(document)) =>
            {
                // Loading another document resets runtime overrides; it does
                // not carry facts from the previously opened file.
                self.facts = FactControls::from_document(&document.text);
                self.document = document;
                self.runner.clear();
                self.file_error = None;
            }

            Ok(None) => {} // File chooser cancelled; keep current document.

            Err(error) =>
            {
                self.file_error = Some(error);
            }
        }
    }

    fn save_document(&mut self, as_new_file: bool)
    {
        let result = if as_new_file
        {
            self.document.save_as_dialog()
        }
        else
        {
            self.document.save()
        };

        match result
        {
            Ok(true) => self.file_error = None,
            Ok(false) => {} // Save dialog cancelled; leave dirty state.
            Err(error) => self.file_error = Some(error),
        }
    }

    /// Fact checkbox changes are executed immediately, using the editor's
    /// current text and the selected sequential/parallel mode.
    fn on_facts_changed(&mut self, context: egui::Context)
    {
        if self.document.has_input() && !self.runner.is_running()
        {
            self.run_engine(context);
        }
        else
        {
            self.runner.clear();
        }
    }

    fn run_engine(&mut self, context: egui::Context)
    {
        let mode = if self.parallel
        {
            SolveMode::Parallel
        }
        else
        {
            SolveMode::Sequential
        };

        self.file_error = None;
        self.runner.start(
            self.document.text.clone(),
            mode,
            self.facts.enabled_mask(),
            context,
        );
    }
}

impl eframe::App for ExpertSystemApp
{
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame)
    {
        self.runner.poll();

        egui::Panel::top("toolbar").show(ui, |ui|
        {
            ui.horizontal_wrapped(|ui|
            {
                ui.heading("Expert System");
                ui.separator();

                let enabled = !self.runner.is_running();
                if ui.add_enabled(enabled, egui::Button::new("Open .txt...")).clicked()
                {
                    self.request_open();
                }

                let can_save = enabled && self.document.has_input();
                if ui.add_enabled(can_save, egui::Button::new("Save")).clicked()
                {
                    self.save_document(false);
                }

                if ui.add_enabled(can_save, egui::Button::new("Save As...")).clicked()
                {
                    self.save_document(true);
                }

                ui.separator();
                ui.add_enabled_ui(enabled, |ui|
                {
                    ui.checkbox(&mut self.parallel, "Parallel");
                });

                if ui.add_enabled(can_save, egui::Button::new("Run engine")).clicked()
                {
                    self.run_engine(ui.ctx().clone());
                }

                if self.runner.is_running()
                {
                    ui.spinner();
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui|
            {
                ui.selectable_value(
                    &mut self.workspace,
                    WorkspaceView::Rules,
                    "Rules / Results",
                );
                ui.selectable_value(
                    &mut self.workspace,
                    WorkspaceView::Graph,
                    "Reasoning graph",
                );
            });

            ui.add_space(7.0);
            let fact_controls_enabled = !self.runner.is_running() && self.document.has_input();
            if views::fact_editor(ui, &mut self.facts, fact_controls_enabled)
            {
                // Re-evaluate immediately, including after Reset to source.
                self.on_facts_changed(ui.ctx().clone());
            }

            if let Some(error) = &self.file_error
            {
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::from_rgb(244, 101, 101), error);
            }
        });

        match self.workspace
        {
            WorkspaceView::Rules =>
            {
                egui::Panel::left("source")
                    .resizable(true)
                    .default_size(650.0)
                    .min_size(300.0)
                    .show(ui, |ui|
                    {
                        if views::source_editor(
                            ui,
                            &mut self.document,
                            !self.runner.is_running(),
                        )
                        {
                            self.facts.sync_source(&self.document.text);
                            self.runner.clear();
                            self.file_error = None;
                        }
                    });

                egui::CentralPanel::default().show(ui, |ui|
                {
                    views::results(ui, &self.runner.state);
                });
            }

            WorkspaceView::Graph =>
            {
                // The graph fills the entire workspace rather than being
                // squeezed beside the rule editor. The layout is read-only.
                egui::CentralPanel::default().show(ui, |ui|
                {
                    match &self.runner.state
                    {
                        RunState::Completed(run) =>
                        {
                            self.graph_viewer.show(
                                ui,
                                &run.execution,
                                self.runner.revision(),
                            );
                        }
                        RunState::Running =>
                        {
                            ui.horizontal(|ui|
                            {
                                ui.spinner();
                                ui.label("Building reasoning graphs...");
                            });
                            ui.ctx().request_repaint_after(
                                std::time::Duration::from_millis(100),
                            );
                        }
                        RunState::Failed(error) =>
                        {
                            ui.colored_label(
                                egui::Color32::from_rgb(244, 101, 101),
                                format!("Inference failed: {error}"),
                            );
                            ui.weak("Return to Rules / Results to correct the source.");
                        }
                        RunState::Idle =>
                        {
                            ui.weak("Run the engine to generate reasoning graphs.");
                        }
                    }
                });
            }
        }

        if self.confirm_discard_before_open
        {
            egui::Window::new("Discard unsaved changes?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui|
                {
                    ui.label("The current rules have unsaved edits.");
                    ui.label("Opening another file will discard those edits.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui|
                    {
                        if ui.button("Cancel").clicked()
                        {
                            self.confirm_discard_before_open = false;
                        }
                        if ui.button("Discard and open...").clicked()
                        {
                            self.confirm_discard_before_open = false;
                            self.open_document();
                        }
                    });
                });
        }
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use super::super::runner::RunState;

    #[test]
    fn toggling_fact_starts_engine_and_updates_result()
    {
        let mut app = ExpertSystemApp::default();
        app.document.text = "A=>B\n=A\n?B".to_owned();
        app.facts = FactControls::from_document(&app.document.text);

        // The file still contains '=A', but the runtime checkbox disables A.
        app.facts.set_enabled(0, false);
        app.on_facts_changed(egui::Context::default());
        assert!(matches!(app.runner.state, RunState::Running));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.runner.is_running() && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
            app.runner.poll();
        }

        match &app.runner.state
        {
            RunState::Completed(report) =>
            {
                assert_eq!(report.results.len(), 1);
                assert_eq!(report.results[0].value, Ok(crate::trace::LogicValue::False));
            }
            RunState::Failed(error) => panic!("Unexpected inference failure: {error}"),
            _ => panic!("Auto-run did not finish"),
        }

        assert_eq!(app.document.text, "A=>B\n=A\n?B");
    }

    #[test]
    fn resetting_facts_can_rerun_in_parallel()
    {
        let mut app = ExpertSystemApp::default();
        app.parallel = true;
        app.document.text = "A=>B\n=A\n?B".to_owned();
        app.facts = FactControls::from_document(&app.document.text);
        app.facts.set_enabled(0, false);
        app.facts.restore_source();
        app.on_facts_changed(egui::Context::default());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.runner.is_running() && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
            app.runner.poll();
        }

        match &app.runner.state
        {
            RunState::Completed(report) =>
            {
                assert_eq!(report.mode, SolveMode::Parallel);
                assert_eq!(report.results[0].value, Ok(crate::trace::LogicValue::True));
            }
            RunState::Failed(error) => panic!("Unexpected inference failure: {error}"),
            _ => panic!("Auto-run did not finish"),
        }
    }
}
