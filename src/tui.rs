use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use tiny_skia::{Color, Paint, PixmapMut, Rect, Transform};

pub const GRID_ROWS: usize = 45;

pub struct TuiMetrics {
    pub char_width: f32,
    pub char_height: f32,
    pub font_size: f32,
    pub cols: usize,
}

pub struct TuiEngine {
    pub font: FontRef<'static>,
}

impl TuiEngine {
    pub fn new(font_data: &'static [u8]) -> Self {
        let font = FontRef::try_from_slice(font_data).expect("Failed to parse font");
        Self { font }
    }

    pub fn calculate_metrics(&self, width: u32, height: u32) -> TuiMetrics {
        let char_height = height as f32 / GRID_ROWS as f32;
        let font_size = char_height * 0.90; 
        let scale = PxScale::from(font_size);
        let scaled_font = self.font.as_scaled(scale);
        let char_width = scaled_font.h_advance(self.font.glyph_id('M'));
        let cols = (width as f32 / char_width) as usize;

        TuiMetrics { char_width, char_height, font_size, cols }
    }

    pub fn draw_string(
        &self, 
        pixmap: &mut PixmapMut, 
        metrics: &TuiMetrics, 
        text: &str, 
        col: usize, 
        row: usize, 
        color: Color
    ) {
        self.draw_string_ex(pixmap, metrics, text, col, row, color, None, 1);
    }

    pub fn draw_string_ex(
        &self,
        pixmap: &mut PixmapMut, // Changed to PixmapMut for Zero-Copy
        metrics: &TuiMetrics,
        text: &str,
        col: usize,
        row: usize,
        color: Color,
        bg_color: Option<Color>,
        scale_factor: usize,
    ) {
        let scaled_font_size = metrics.font_size * scale_factor as f32;
        let scale = PxScale::from(scaled_font_size);
        let char_w = metrics.char_width * scale_factor as f32;
        let char_h = metrics.char_height * scale_factor as f32;
        let x_start = col as f32 * metrics.char_width;
        let y_start = row as f32 * metrics.char_height;
        let y_baseline = y_start + (char_h * 0.82);

        // 1. DRAW BACKGROUND (Block Operation)
        // We do this in its own scope so the borrow of 'pixmap' ends immediately
        if let Some(bg) = bg_color {
            let total_w = text.chars().count() as f32 * char_w;
            if let Some(rect) = Rect::from_xywh(x_start, y_start, total_w, char_h) {
                let mut paint = Paint::default();
                paint.set_color(bg);
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
        }

        // 2. DRAW GLYPHS (Direct Pixel Access)
        // This is 50x faster than calling fill_rect for every pixel of text
        let pix_w = pixmap.width() as i32;
        let pix_h = pixmap.height() as i32;
        
        let r = (color.red() * 255.0) as u8;
        let g = (color.green() * 255.0) as u8;
        let b = (color.blue() * 255.0) as u8;

        let mut x_cursor = x_start;

        // Start mutable borrow of raw pixels
        let pixels = pixmap.pixels_mut();

        for c in text.chars() {
            let glyph = self.font.glyph_id(c).with_scale_and_position(scale, point(x_cursor, y_baseline));
            
            if let Some(outlined) = self.font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, coverage| {
                    // Only draw significant pixels to keep text crisp
                    if coverage > 0.2 {
                        let px = bounds.min.x as i32 + x as i32;
                        let py = bounds.min.y as i32 + y as i32;

                        if px >= 0 && px < pix_w && py >= 0 && py < pix_h {
                            let idx = (py * pix_w + px) as usize;
                            let alpha = (coverage * 255.0) as u8;
                            
                            // Simple Integer Blending (Source Over)
                            // This reads the current pixel (likely the background we just drew)
                            // and blends the text color on top.
                            let dest = &mut pixels[idx];
                            
                            if alpha == 255 {
                                // Solid optimization
                                if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255) {
                                    *dest = p;
                                }
                            } else {
                                // Alpha blending logic
                                let a = alpha as u32;
                                let inv_a = 255 - a;
                                
                                let out_r = ((r as u32 * a + dest.red() as u32 * inv_a) / 255) as u8;
                                let out_g = ((g as u32 * a + dest.green() as u32 * inv_a) / 255) as u8;
                                let out_b = ((b as u32 * a + dest.blue() as u32 * inv_a) / 255) as u8;
                                let out_a = ((255 * a + dest.alpha() as u32 * inv_a) / 255) as u8;

                                if let Some(blended) = tiny_skia::PremultipliedColorU8::from_rgba(out_r, out_g, out_b, out_a) {
                                    *dest = blended;
                                }
                            }
                        }
                    }
                });
            }
            x_cursor += char_w;
        }
    }

    pub fn draw_box(
        &self, 
        pixmap: &mut PixmapMut, // Changed to PixmapMut
        metrics: &TuiMetrics, 
        x: usize, y: usize, w: usize, h: usize, 
        color: Color
    ) {
        if w < 2 || h < 2 { return; }
        // We reuse draw_string which now handles PixmapMut correctly
        self.draw_string(pixmap, metrics, "┌", x, y, color);
        self.draw_string(pixmap, metrics, "┐", x + w - 1, y, color);
        self.draw_string(pixmap, metrics, "└", x, y + h - 1, color);
        self.draw_string(pixmap, metrics, "┘", x + w - 1, y + h - 1, color);

        let h_line = "─".repeat(w.saturating_sub(2));
        self.draw_string(pixmap, metrics, &h_line, x + 1, y, color);
        self.draw_string(pixmap, metrics, &h_line, x + 1, y + h - 1, color);

        for i in 1..h.saturating_sub(1) {
            self.draw_string(pixmap, metrics, "│", x, y + i, color);
            self.draw_string(pixmap, metrics, "│", x + w - 1, y + i, color);
        }
    }
}