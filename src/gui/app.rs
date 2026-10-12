//! Application shell and composition root for future graph/trace views.

use eframe::egui;

use crate::engine::SolveMode;

use super::document::SourceDocument;
use super::facts::FactControls;
use super::runner::RunController;
use super::views;

#[derive(Default)]
pub struct ExpertSystemApp
{
    document: SourceDocument,
    facts: FactControls,
    runner: RunController,
    parallel: bool,
    file_error: Option<String>,
    confirm_discard_before_open: bool,
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

            ui.add_space(7.0);
            if views::fact_editor(ui, &mut self.facts, !self.runner.is_running())
            {
                // These changes affect only the next solver run.
                self.runner.clear();
            }

            if let Some(error) = &self.file_error
            {
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::from_rgb(244, 101, 101), error);
            }
        });

        egui::Panel::left("source")
            .resizable(true)
            .default_size(650.0)
            .min_size(300.0)
            .show(ui, |ui|
            {
                if views::source_editor(ui, &mut self.document, !self.runner.is_running())
                {
                    // Source edits only update the file-derived defaults.
                    // Explicit fact toggles continue to override those defaults.
                    self.facts.sync_source(&self.document.text);
                    self.runner.clear();
                    self.file_error = None;
                }
            });

        egui::CentralPanel::default().show(ui, |ui|
        {
            views::results(ui, &self.runner.state);
        });

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
