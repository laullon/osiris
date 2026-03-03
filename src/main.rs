mod app;
mod commands;
mod input;
mod models;
mod storage;
mod ui;

use std::{thread, time::Duration};

use crate::input::gamepad::{Event, GamepadStateWrapper};
use crate::ui::{renderer, tui};
use winit::event_loop::{ControlFlow, EventLoop};

#[derive(Debug)]
enum GilrsEvent {
    GamepadInput(Event),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<GilrsEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();

    // Call TuiEngine from the tui module
    let tui_instance =
        ui::tui::TuiEngine::new(include_bytes!("../fonts/JetBrainsMono-Regular.ttf"));

    thread::spawn(move || {
        let mut gamepad_wrapper =
            GamepadStateWrapper::new().expect("Failed to initialize gamepads");
        loop {
            while let Some(event) = gamepad_wrapper.next_event() {
                // Send the event to wake the loop (logging handled in wrapper)
                let _ = proxy.send_event(GilrsEvent::GamepadInput(event));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });

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
