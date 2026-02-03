use crate::commands::{ControlCommand, NavigationCommand, UiEvent};
use crate::models::RomLibrary;
use crate::ui::tui::{TuiEngine, TuiMetrics};
use crate::ui::widgets::common::Widget;
use std::rc::Rc;
use tiny_skia::{Color, PixmapMut};

pub struct CarouselWidget {
    pub library: Rc<RomLibrary>,
    pub selected_index: usize,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl CarouselWidget {
    pub fn new(library: Rc<RomLibrary>) -> Self {
        Self {
            library,
            selected_index: 0,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }
    }
}

impl Widget for CarouselWidget {
    fn set_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        self.x = x;
        self.y = y;
        self.w = w;
        self.h = h;
    }

    fn handle_command(&mut self, cmd: ControlCommand) -> UiEvent {
        if self.library.systems.is_empty() {
            return UiEvent::None;
        }
        let old_idx = self.selected_index;

        match cmd {
            ControlCommand::Navigation(navigation_command) => match navigation_command {
                NavigationCommand::Right => {
                    self.selected_index = (self.selected_index + 1) % self.library.systems.len();
                }
                NavigationCommand::Left => {
                    self.selected_index = (self.selected_index + self.library.systems.len() - 1)
                        % self.library.systems.len();
                }
                _ => return UiEvent::None,
            },
            _ => {
                return UiEvent::None;
            }
        }

        if old_idx != self.selected_index {
            return UiEvent::SystemChanged(self.selected_index);
        }
        UiEvent::None
    }

    fn draw(&self, pixmap: &mut PixmapMut, engine: &TuiEngine, metrics: &TuiMetrics) {
        let cyan = Color::from_rgba8(255, 255, 0, 255);
        let _green = Color::from_rgba8(0, 255, 0, 255);
        let highlight_bg = Color::from_rgba8(0, 60, 60, 255);
        let bg_main = Color::from_rgba8(5, 10, 0, 255);

        // Draw Container
        engine.draw_box(pixmap, metrics, self.x, self.y, self.w, self.h, cyan);
        engine.draw_string_ex(
            pixmap,
            metrics,
            " EMULATOR SUBSYSTEMS ",
            self.x + 2,
            self.y,
            cyan,
            Some(bg_main),
            1,
        );

        if self.library.systems.is_empty() {
            return;
        }

        let item_count = self.library.systems.len();
        let slot_count = 5;
        // Horizontal spacing: Divide widget width into 5 zones
        let slot_w = self.w / slot_count;
        let center_y = self.y + (self.h / 2) - 1; // -1 to account for 2x height

        for i in 0..slot_count {
            // Calculate which index to show in this slot (relative to selection)
            // i=0: index-2, i=1: index-1, i=2: SELECTED, i=3: index+1, i=4: index+2
            let relative_idx = (self.selected_index + item_count + i - 2) % item_count;
            let name = &self.library.systems[relative_idx].name;

            let slot_center_x = self.x + (i * slot_w) + (slot_w / 2);
            let is_selected = i == 2;

            if is_selected {
                let display = format!("[ {} ]", name);
                // 2x text takes double width, so we adjust center math
                let text_x = slot_center_x.saturating_sub(display.len()); // len*2 / 2
                engine.draw_string_ex(
                    pixmap,
                    metrics,
                    &display,
                    text_x,
                    center_y,
                    Color::WHITE,
                    Some(highlight_bg),
                    2, // 2x SIZE
                );
            } else {
                // Faded 1x text for side items
                let text_x = slot_center_x.saturating_sub(name.len() / 2);
                engine.draw_string(
                    pixmap,
                    metrics,
                    name,
                    text_x,
                    center_y + 1,
                    Color::from_rgba8(100, 100, 100, 255),
                );
            }
        }
    }

    fn handle_ui_event(&mut self, _event: UiEvent) {}
}
