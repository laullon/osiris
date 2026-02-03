mod app;
mod commands;
mod models;
mod storage;
mod ui;

use std::thread;

use crate::ui::{renderer, tui};
use gilrs::Event;
use winit::event_loop::{ControlFlow, EventLoop};

#[derive(Debug)]
enum GilrsEvent {
    GamepadInput(Event),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<GilrsEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();

    println!("OSIRIS: INITIATING ROM SCAN...");
    let library = storage::scan_roms("./roms");
    println!(
        "OSIRIS: SCAN COMPLETE. SYSTEMS DETECTED: {}",
        library.systems.len()
    );

    // Call TuiEngine from the tui module
    let tui_instance =
        ui::tui::TuiEngine::new(include_bytes!("../fonts/JetBrainsMono-Regular.ttf"));

    let mut app = app::OsirisApp::new(renderer::Renderer::new(tui_instance), library);

    thread::spawn(move || {
        let mut gilrs = gilrs::Gilrs::new().unwrap();
        loop {
            while let Some(event) = gilrs.next_event() {
                println!("Gamepad event: {:?}", event);
                // Send the event to wake the loop
                let _ = proxy.send_event(GilrsEvent::GamepadInput(event));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });

    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;

    Ok(())
}
