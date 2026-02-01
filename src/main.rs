mod app;
mod commands;
mod ui;

use crate::ui::{renderer, tui, widgets};
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;

    // Call TuiEngine from the tui module
    let tui_instance =
        ui::tui::TuiEngine::new(include_bytes!("../fonts/JetBrainsMono-Regular.ttf"));

    let mut app = app::OsirisApp::new(
        gilrs::Gilrs::new().unwrap(),
        renderer::Renderer::new(tui_instance),
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}
