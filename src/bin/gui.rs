use eframe::egui;
use expert_system::gui::ExpertSystemApp;

fn main() -> eframe::Result
{
    let native_options = eframe::NativeOptions
    {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 540.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Expert System",
        native_options,
        Box::new(|context| Ok(Box::new(ExpertSystemApp::new(context)))),
    )
}
