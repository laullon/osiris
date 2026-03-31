mod app;
mod commands;
mod models;
mod storage;
mod ui;

use crate::ui::{renderer, tui};
use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;

    let tui_instance =
        ui::tui::TuiEngine::new(include_bytes!("../fonts/JetBrainsMono-Regular.ttf"));

    println!("OSIRIS: INITIATING ROM SCAN...");
    let library = storage::scan_roms("./roms");
    println!(
        "OSIRIS: SCAN COMPLETE. SYSTEMS DETECTED: {}",
        library.systems.len()
    );

    let mut app = app::OsirisApp::new(renderer::Renderer::new(tui_instance), library);
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;

    Ok(())
}
