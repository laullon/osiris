use crate::{
    commands::{NavCommand, UiEvent},
    ui::widgets::common::{Container, Widget},
};

pub struct SplitPanelWidget<L: Widget, R: Widget> {
    left: L,
    right: R,
    split_ratio: u32,
    percentage_mode: bool,
    is_vertical: bool,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl<L: Widget, R: Widget> SplitPanelWidget<L, R> {
    pub fn new(
        left: L,
        right: R,
        split_ratio: u32,
        percentage_mode: bool,
        is_vertical: bool,
    ) -> Self {
        let mut panel = Self {
            left,
            right,
            split_ratio,
            percentage_mode,
            is_vertical,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
        panel.arrange_widgets();
        panel
    }
}

impl<L: Widget, R: Widget> Container for SplitPanelWidget<L, R> {
    fn arrange_widgets(&mut self) {
        if self.w == 0 || self.h == 0 {
            return;
        }

        if self.is_vertical {
            let split_point = if self.percentage_mode {
                (self.h * self.split_ratio as usize) / 100
            } else {
                self.split_ratio as usize
            };

            self.left.set_rect(self.x, self.y, self.w, split_point);
            self.right.set_rect(
                self.x,
                self.y + split_point,
                self.w,
                self.h.saturating_sub(split_point),
            );
        } else {
            let split_point = if self.percentage_mode {
                (self.w * self.split_ratio as usize) / 100
            } else {
                self.split_ratio as usize
            };

            self.left.set_rect(self.x, self.y, split_point, self.h);
            self.right.set_rect(
                self.x + split_point,
                self.y,
                self.w.saturating_sub(split_point),
                self.h,
            );
        }
    }
}

impl<L: Widget, R: Widget> Widget for SplitPanelWidget<L, R> {
    fn draw(
        &self,
        pixmap: &mut tiny_skia::PixmapMut,
        engine: &crate::ui::tui::TuiEngine,
        metrics: &crate::ui::tui::TuiMetrics,
    ) {
        self.left.draw(pixmap, engine, metrics);
        self.right.draw(pixmap, engine, metrics);
    }

    fn set_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        self.x = x;
        self.y = y;
        self.w = w;
        self.h = h;
        self.arrange_widgets();
    }

    fn handle_command(&mut self, cmd: NavCommand) -> UiEvent {
        let e1 = self.left.handle_command(cmd.clone());
        if e1 != UiEvent::None {
            return e1;
        }
        self.right.handle_command(cmd)
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        self.left.handle_ui_event(event.clone());
        self.right.handle_ui_event(event);
    }
}
