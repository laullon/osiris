use crate::commands::{NavCommand, UiEvent};
use tiny_skia::PixmapMut;

use crate::ui::tui::{TuiEngine, TuiMetrics};

pub(crate) trait Widget {
    fn draw(&self, pixmap: &mut PixmapMut, engine: &TuiEngine, metrics: &TuiMetrics);
    fn set_rect(&mut self, x: usize, y: usize, w: usize, h: usize);
    fn handle_command(&mut self, cmd: NavCommand) -> UiEvent;
    fn handle_ui_event(&mut self, _event: UiEvent);
}

pub(crate) trait Container {
    fn arrange_widgets(&mut self);
}
