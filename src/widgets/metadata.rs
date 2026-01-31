use crate::tui::{TuiEngine, TuiMetrics};
use tiny_skia::{Color, PixmapMut};

pub struct MetadataWidget {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    // We will pass the selected item string to the draw function
}

impl MetadataWidget {
    pub fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Self { x, y, w, h }
    }

    pub fn draw(&self, pixmap: &mut PixmapMut, engine: &TuiEngine, metrics: &TuiMetrics, selected_item: &str) {
        // COLORS (BGR for Zero-Copy)
        let cyan = Color::from_rgba8(255, 255, 0, 255);
        let green = Color::from_rgba8(0, 255, 0, 255);
        let white = Color::from_rgba8(255, 255, 255, 255);
        let dark_bg = Color::from_rgba8(5, 15, 5, 255);
        let bg_main = Color::from_rgba8(5, 10, 0, 255);

        // 1. Draw Container Frame
        engine.draw_box(pixmap, metrics, self.x, self.y, self.w, self.h, cyan);
        engine.draw_string_ex(pixmap, metrics, " MODULE DETAILS ", self.x + 2, self.y, cyan, Some(bg_main),1);

        // 2. Parse the dummy string (e.g. "MAME: PAC-MAN (1980)")
        // We split by ": " to simulate extracting metadata
        let parts: Vec<&str> = selected_item.split(": ").collect();
        let system = parts.get(0).unwrap_or(&"UNKNOWN");
        let title = parts.get(1).unwrap_or(&"UNKNOWN MODULE");

        // 3. Draw Large Title
        engine.draw_string_ex(
            pixmap, metrics, title, 
            self.x + 2, self.y + 2, 
            white, None, 2 // 2x Scale
        );

        // 4. Draw System Info
        engine.draw_string(pixmap, metrics, &format!("PLATFORM: {}", system), self.x + 2, self.y + 5, green);
        engine.draw_string(pixmap, metrics, "STATUS:   INSTALLED", self.x + 2, self.y + 6, green);
        engine.draw_string(pixmap, metrics, "VERSION:  REV 1.0", self.x + 2, self.y + 7, green);


	// 5. Draw "Image" Placeholder Box
	    let img_w = self.w.saturating_sub(4); // Use dynamic self.w
	    let img_h = 14;
	    let img_y = self.y + 10;
	    
	    // Fill
	    engine.draw_string_ex(
	        pixmap, metrics, 
	        &" ".repeat(img_w), 
	        self.x + 2, img_y, 
	        Color::TRANSPARENT, Some(dark_bg), 1
	    );
	    
	    // Outline (re-uses dynamic width)
	    engine.draw_box(pixmap, metrics, self.x + 2, img_y, img_w, img_h, cyan);
	    
	    // Centered "NO SIGNAL"
	    let no_sig = "NO VISUAL FEED";
	    let text_x = self.x + 2 + (img_w / 2).saturating_sub(no_sig.len() / 2);
	    engine.draw_string(
	        pixmap, metrics, no_sig, 
	        text_x, 
	        img_y + (img_h / 2), 
	        Color::from_rgba8(100, 100, 100, 255)
	    );
        
        // 6. Stats Footer
        let play_count = (selected_item.len() * 3) % 99; // Fake random number
        let stats = format!("PLAY COUNT: {:03} | RATING: A+", play_count);
        engine.draw_string(pixmap, metrics, &stats, self.x + 2, self.y + self.h - 2, white);
    }
}