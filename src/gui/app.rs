//! Application shell and composition root for future graph/trace views.

use eframe::egui;

use crate::engine::SolveMode;

use super::document::SourceDocument;
use super::runner::RunController;
use super::views;

#[derive(Default)]
pub struct ExpertSystemApp
{
    document: SourceDocument,
    runner: RunController,
    parallel: bool,
    file_error: Option<String>,
}

impl ExpertSystemApp
{
    pub fn new(_creation_context: &eframe::CreationContext<'_>) -> Self
    {
        Self::default()
    }

    fn open_document(&mut self)
    {
        match SourceDocument::open_dialog()
        {
            Ok(Some(document)) =>
            {
                self.document = document;
                self.runner.clear();
                self.file_error = None;
            }

            Ok(None) => {} // User cancelled the picker.

            Err(error) =>
            {
                // Keep the prior document and results on a failed open.
                self.file_error = Some(error);
            }
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
        self.runner.start(self.document.text.clone(), mode, context);
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
                    self.open_document();
                }

                ui.add_enabled_ui(enabled, |ui|
                {
                    ui.checkbox(&mut self.parallel, "Parallel");
                });

                let can_run = enabled && self.document.has_input();
                if ui.add_enabled(can_run, egui::Button::new("Run engine")).clicked()
                {
                    self.run_engine(ui.ctx().clone());
                }

                if self.runner.is_running()
                {
                    ui.spinner();
                }
            });

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
                    self.runner.clear();
                    self.file_error = None;
                }
            });

        egui::CentralPanel::default().show(ui, |ui|
        {
            views::results(ui, &self.runner.state);
        });
    }
}
