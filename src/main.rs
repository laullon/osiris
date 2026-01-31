mod commands; mod tui; mod renderer; mod app; mod widgets;
use winit::event_loop::EventLoop;
use widgets::{ListWidget, MetadataWidget};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    
    // Call TuiEngine from the tui module
    let tui_instance = tui::TuiEngine::new(include_bytes!("../fonts/JetBrainsMono-Regular.ttf"));

    let items = (1..100).map(|i| format!("MISSION MODULE {:03}", i)).collect();
    let game_list = widgets::ListWidget::new(" MODULE SELECTOR ", 3, 2, 40, 41, items);
    let metadata = MetadataWidget::new(0, 0, 0, 0);
        let mut app = app::OsirisApp::new(
        gilrs::Gilrs::new().unwrap(),
        renderer::Renderer::new(tui_instance),
        game_list,
        metadata
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}