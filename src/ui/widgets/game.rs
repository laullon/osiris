use crate::{
    commands::{ActionCommand, ControlCommand, UiEvent},
    models::RomLibrary,
    tui::{TuiEngine, TuiMetrics},
    ui::widgets::common::Widget,
};
use image::GenericImageView;
use std::{path::PathBuf, rc::Rc};
use tiny_skia::{Color, Pixmap, PixmapMut};

pub struct GameWidget {
    library: Rc<RomLibrary>,
    selected_system: usize,
    selected_game: usize,
    current_image: Option<Pixmap>,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl GameWidget {
    pub fn new(library: Rc<RomLibrary>) -> Self {
        Self {
            library,
            selected_system: 0,
            selected_game: 0,
            current_image: None,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }
    }

    fn load_image(&mut self) {
        let system = &self.library.systems[self.selected_system];
        let game = &system.games[self.selected_game];

        // Path: data/[system]/[id].png
        let img_path = PathBuf::from("roms")
            .join(&system.name.to_lowercase())
            .join("images")
            .join(format!("{}-image.png", game.id));

        self.current_image = if let Ok(img) = image::open(&img_path) {
            let (width, height) = img.dimensions();
            let mut pixmap = Pixmap::new(width, height).unwrap();

            // Convert image to tiny_skia format
            for (x, y, pixel) in img.pixels() {
                let r = pixel[0];
                let g = pixel[1];
                let b = pixel[2];
                let a = pixel[3];
                if let Some(c) = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a) {
                    pixmap.pixels_mut()[(y * width + x) as usize] = c;
                }
            }
            Some(pixmap)
        } else {
            None // File not found or corrupt
        };
    }
}

impl Widget for GameWidget {
    fn draw(&self, pixmap: &mut PixmapMut, engine: &TuiEngine, metrics: &TuiMetrics) {
        // COLORS (BGR for Zero-Copy)
        let cyan = Color::from_rgba8(255, 255, 0, 255);
        let green = Color::from_rgba8(0, 255, 0, 255);
        let white = Color::from_rgba8(255, 255, 255, 255);
        let dark_bg = Color::from_rgba8(5, 15, 5, 255);
        let bg_main = Color::from_rgba8(5, 10, 0, 255);

        // 1. Draw Container Frame
        engine.draw_box(pixmap, metrics, self.x, self.y, self.w, self.h, cyan);
        engine.draw_string_ex(
            pixmap,
            metrics,
            " MODULE DETAILS ",
            self.x + 2,
            self.y,
            cyan,
            Some(bg_main),
            1,
        );

        let system = self
            .library
            .systems
            .get(self.selected_system)
            .map_or("UNKNOWN", |s| &s.name);

        let game = self
            .library
            .systems
            .get(self.selected_system)
            .and_then(|s| s.games.get(self.selected_game))
            .unwrap();

        // 3. Draw Large Title
        engine.draw_string_ex(
            pixmap,
            metrics,
            game.name.as_str(),
            self.x + 2,
            self.y + 2,
            white,
            None,
            2, // 2x Scale
        );

        // 4. Draw System Info
        engine.draw_string(
            pixmap,
            metrics,
            &format!("PLATFORM: {}", system),
            self.x + 2,
            self.y + 5,
            green,
        );
        engine.draw_string(
            pixmap,
            metrics,
            &format!(
                "Filename: {}",
                game.path
                    .to_path_buf()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
            ),
            self.x + 2,
            self.y + 6,
            green,
        );
        engine.draw_string(
            pixmap,
            metrics,
            &format!("id: {}", game.id),
            self.x + 2,
            self.y + 7,
            green,
        );

        // 5. Draw "Image" Placeholder Box
        let img_w = self.w.saturating_sub(4); // Use dynamic self.w
        let img_h = 14;
        let img_y = self.y + 10;

        // Fill
        engine.draw_string_ex(
            pixmap,
            metrics,
            &" ".repeat(img_w),
            self.x + 2,
            img_y,
            Color::TRANSPARENT,
            Some(dark_bg),
            1,
        );

        // Outline (re-uses dynamic width)
        engine.draw_box(pixmap, metrics, self.x + 2, img_y, img_w, img_h, cyan);

        let img_w_cells = self.w.saturating_sub(4);
        let img_h_cells = 14;
        let img_y = self.y + 10;

        // 1. Calculate pixel boundaries of our TUI box
        let target_px_w = img_w_cells as f32 * metrics.char_width;
        let target_px_h = img_h_cells as f32 * metrics.char_height;
        let target_px_x = (self.x + 2) as f32 * metrics.char_width;
        let target_px_y = img_y as f32 * metrics.char_height;

        // 2. Draw the Image (if loaded)
        if let Some(ref char_pixmap) = self.current_image {
            // Calculate scale to fit inside the box while keeping aspect ratio
            let s_w = target_px_w / char_pixmap.width() as f32;
            let s_h = target_px_h / char_pixmap.height() as f32;
            let scale = s_w.min(s_h);

            // Center the image inside the target box
            let actual_w = char_pixmap.width() as f32 * scale;
            let actual_h = char_pixmap.height() as f32 * scale;
            let x_offset = (target_px_w - actual_w) / 2.0;
            let y_offset = (target_px_h - actual_h) / 2.0;

            let transform = tiny_skia::Transform::from_scale(scale, scale)
                .post_translate(target_px_x + x_offset, target_px_y + y_offset);

            // Use the 'pixmap' argument passed to the function
            pixmap.draw_pixmap(
                0,
                0,
                char_pixmap.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                transform,
                None,
            );
        } else {
            // FALLBACK: Centered "NO SIGNAL"
            let no_sig = "NO VISUAL FEED";
            let text_x = self.x + 2 + (img_w_cells / 2).saturating_sub(no_sig.len() / 2);
            engine.draw_string(
                pixmap,
                metrics,
                no_sig,
                text_x,
                img_y + (img_h_cells / 2),
                Color::from_rgba8(100, 100, 100, 255),
            );
        }

        // 6. Stats Footer
        let play_count = (self.selected_game * 3) % 99; // Fake random number
        let stats = format!("PLAY COUNT: {:03} | RATING: A+", play_count);
        engine.draw_string(
            pixmap,
            metrics,
            &stats,
            self.x + 2,
            self.y + self.h - 2,
            white,
        );
    }

    fn set_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        self.x = x;
        self.y = y;
        self.w = w;
        self.h = h;
    }

    fn handle_command(&mut self, cmd: ControlCommand) -> UiEvent {
        match cmd {
            ControlCommand::Action(action_command) => match action_command {
                ActionCommand::Select => {
                    UiEvent::LaunchGame(self.selected_system, self.selected_game)
                }
                ActionCommand::Back => UiEvent::None,
            },
            _ => UiEvent::None,
        }
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        // Handle UI events if necessary
        match event {
            UiEvent::SystemChanged(system_idx) => {
                self.selected_system = system_idx;
                self.selected_game = 0;
                self.load_image();
            }
            UiEvent::GameChanged(game_idx) => {
                self.selected_game = game_idx;
                self.load_image();
            }
            _ => {}
        }
    }
}
