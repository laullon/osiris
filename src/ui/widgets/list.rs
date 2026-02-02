use crate::commands::{NavCommand, UiEvent};
use crate::models::RomLibrary;
use crate::tui::{TuiEngine, TuiMetrics};
use std::rc::Rc;
use tiny_skia::{Color, PixmapMut};

pub struct ListWidget {
    pub title: String,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub library: Rc<RomLibrary>,
    pub selected_system: usize,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

impl ListWidget {
    pub fn new(library: Rc<RomLibrary>) -> Self {
        Self {
            title: "GAME LIST".to_string(),
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            library,
            selected_system: 0,
            selected_index: 0,
            scroll_offset: 0,
        }
    }
}

impl crate::ui::widgets::common::Widget for ListWidget {
    fn handle_command(&mut self, cmd: NavCommand) -> UiEvent {
        let old_idx = self.selected_index;
        match cmd {
            NavCommand::Up if self.selected_index > 0 => self.selected_index -= 1,
            NavCommand::Down
                if self.selected_index
                    < self.library.systems[self.selected_system]
                        .games
                        .len()
                        .saturating_sub(1) =>
            {
                self.selected_index += 1
            }
            NavCommand::Select => {
                if let Some(_item) = self.library.systems[self.selected_system]
                    .games
                    .get(self.selected_index)
                {
                    return UiEvent::LaunchGame(self.selected_index, self.selected_index);
                }
            }
            _ => return UiEvent::None,
        }

        if old_idx != self.selected_index {
            let vis_h = self.h.saturating_sub(2);
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            } else if self.selected_index >= self.scroll_offset + vis_h {
                self.scroll_offset = self.selected_index - vis_h + 1;
            }
            return UiEvent::GameChanged(self.selected_index);
        }
        UiEvent::None
    }

    fn draw(&self, pixmap: &mut PixmapMut, engine: &TuiEngine, metrics: &TuiMetrics) {
        // COLORS (BGR SWAP applied for direct buffer writing)
        // If Cyan looks Yellow, we must write (255, 255, 0) to get (0, 255, 255)
        let cyan = Color::from_rgba8(255, 255, 0, 255);
        let green = Color::from_rgba8(0, 255, 0, 255);
        let white = Color::from_rgba8(255, 255, 255, 255);
        let grey = Color::from_rgba8(180, 180, 180, 255);

        // Dark Cyan Highlight (Swapped R/B)
        let highlight_bg = Color::from_rgba8(60, 60, 0, 255);

        // Main Background (Dark Green/Black)
        let bg_main = Color::from_rgba8(5, 10, 0, 255);

        // 1. Draw Outer Frame
        engine.draw_box(pixmap, metrics, self.x, self.y, self.w, self.h, cyan);

        // 2. Draw Title
        engine.draw_string_ex(
            pixmap,
            metrics,
            &format!(" {} ", self.title),
            self.x + 2,
            self.y,
            green,
            Some(bg_main),
            1,
        );

        // 3. Draw Items
        let vis_h = self.h.saturating_sub(2);
        for i in 0..vis_h {
            let idx = i + self.scroll_offset;
            if idx >= self.library.systems[self.selected_system].games.len() {
                break;
            }

            let text_w = self.w.saturating_sub(4);
            let raw_text = &self.library.systems[self.selected_system].games[idx].name;
            let display_text = if raw_text.len() > text_w {
                format!("{}…", &raw_text[..text_w.saturating_sub(1)])
            } else {
                format!("{:<width$}", raw_text, width = text_w)
            };

            if idx == self.selected_index {
                // Highlighted Item
                engine.draw_string_ex(
                    pixmap,
                    metrics,
                    &display_text,
                    self.x + 2,
                    self.y + 1 + i,
                    white,
                    Some(highlight_bg),
                    1,
                );
            } else {
                // Normal Item
                engine.draw_string(
                    pixmap,
                    metrics,
                    &display_text,
                    self.x + 2,
                    self.y + 1 + i,
                    grey,
                );
            }
        }

        // 4. Draw Scrollbar
        if self.library.systems[self.selected_system].games.len() > vis_h {
            let bar_x = self.x + self.w - 1;
            let total_items = self.library.systems[self.selected_system].games.len() as f32;
            let track_h = vis_h as f32;

            // Calculate handle relative position
            let scroll_pct = self.scroll_offset as f32 / (total_items - track_h).max(1.0);
            let handle_y = (scroll_pct * (track_h - 1.0)).round() as usize;

            for i in 0..vis_h {
                let symbol = if i == handle_y { "█" } else { "▒" };

                // Draw with a background to overwrite the border line
                engine.draw_string_ex(
                    pixmap,
                    metrics,
                    symbol,
                    bar_x,
                    self.y + 1 + i,
                    cyan,
                    Some(bg_main),
                    1,
                );
            }
        }
    }

    fn set_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        self.x = x;
        self.y = y;
        self.w = w;
        self.h = h;
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::SystemChanged(system_idx) => {
                self.selected_system = system_idx;
                self.selected_index = 0;
                self.scroll_offset = 0;
            }
            _ => {}
        }
    }
}
