use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use gilrs::{Button, EventType, Gilrs};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

// --- OS PROOF FONT EMBEDDING ---
// This embeds the font bytes directly into your binary at compile time.
// Make sure font.ttf is in the same folder as your Cargo.toml
const FONT_DATA: &[u8] = include_bytes!("../fonts/JetBrainsMono-Regular.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavCommand {
    Up, Down, Left, Right, Select, Back,
}

struct App {
    gil: Gilrs,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    last_cmd: Option<(NavCommand, Instant)>,
    font: FontRef<'static>, // Store the parsed font
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Rc::new(event_loop
                .create_window(
                    Window::default_attributes()
                    .with_title("OSIRIS")
                    .with_decorations(false)
                    .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None))),
                )
                .unwrap());
                
                let context = softbuffer::Context::new(window.clone()).unwrap();
                let surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
                
                self.window = Some(window);
                self.surface = Some(surface);
            }
        }
        
        fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
            while let Some(gil_ev) = self.gil.next_event() {
                match gil_ev.event {
                    EventType::ButtonPressed(Button::South, _) => self.show(NavCommand::Select),
                    EventType::ButtonPressed(Button::East, _) => self.show(NavCommand::Back),
                    _ => {}
                }
            }
            
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput {
                    event: KeyEvent { state: ElementState::Pressed, physical_key: PhysicalKey::Code(key), .. },
                    ..
                } => {
                    let cmd = match key {
                        KeyCode::ArrowUp => Some(NavCommand::Up),
                        KeyCode::ArrowDown => Some(NavCommand::Down),
                        KeyCode::ArrowLeft => Some(NavCommand::Left),
                        KeyCode::ArrowRight => Some(NavCommand::Right),
                        KeyCode::Space => Some(NavCommand::Select),
                        KeyCode::Escape => Some(NavCommand::Back),
                        _ => None,
                    };
                    if let Some(c) = cmd { self.show(c); }
                }
                WindowEvent::RedrawRequested => self.paint(),
                _ => {}
            }
        }
    }
    
    impl App {
        fn new() -> Self {
            // Parse the embedded font once
            let font = FontRef::try_from_slice(FONT_DATA).expect("Failed to parse embedded font");
            Self {
                gil: Gilrs::new().unwrap(),
                window: None,
                surface: None,
                last_cmd: None,
                font,
            }
        }
        
        fn show(&mut self, cmd: NavCommand) {
            println!("{:?}", cmd);
            self.last_cmd = Some((cmd, Instant::now()));
            if let Some(win) = &self.window {
                win.request_redraw();
            }
        }
        
        fn paint(&mut self) {
            let (win, surface) = match (&self.window, &mut self.surface) {
                (Some(w), Some(s)) => (w, s),
                _ => return,
            };
            
            let (width, height) = {
                let size = win.inner_size();
                (size.width, size.height)
            };
            if width == 0 || height == 0 { return; }
            
            surface.resize(NonZeroU32::new(width).unwrap(), NonZeroU32::new(height).unwrap()).unwrap();
            
            let mut pixmap = Pixmap::new(width, height).unwrap();
            pixmap.fill(Color::from_rgba8(0, 0, 0, 255));
            
            // Use the stored font
            let scale = PxScale::from(120.0);
            let scaled_font = self.font.as_scaled(scale);
            let text = "OSIRIS";
            
            let mut total_w = 0.0;
            for c in text.chars() {
                total_w += scaled_font.h_advance(self.font.glyph_id(c));
            }
            
            let mut x_cursor = (width as f32 - total_w) / 2.0;
            let y_cursor = (height as f32) / 2.0;
            
            for c in text.chars() {
                let glyph_id = self.font.glyph_id(c);
                let glyph = glyph_id.with_scale_and_position(scale, point(x_cursor, y_cursor));
                
                if let Some(outlined) = self.font.outline_glyph(glyph) {
                    let bounds = outlined.px_bounds();
                    outlined.draw(|x, y, c| {
                        if c > 0.01 {
                            let paint = Paint {
                                shader: tiny_skia::Shader::SolidColor(Color::from_rgba8(0, 255, 255, (c * 255.0) as u8)),
                                ..Default::default()
                            };
                            let rect = Rect::from_xywh(bounds.min.x + x as f32, bounds.min.y + y as f32, 1.0, 1.0).unwrap();
                            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                        }
                    });
                }
                x_cursor += scaled_font.h_advance(glyph_id);
            }
            
            // Indicator box
            if let Some((_, instant)) = self.last_cmd {
                if instant.elapsed() < Duration::from_secs(2) {
                    let rect = Rect::from_xywh(20.0, height as f32 - 50.0, 100.0, 30.0).unwrap();
                    let paint = Paint {
                        shader: tiny_skia::Shader::SolidColor(Color::from_rgba8(255, 255, 255, 255)),
                        ..Default::default()
                    };
                    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                } else {
                    self.last_cmd = None;
                }
            }
            
            let mut buffer = surface.buffer_mut().unwrap();
            for (i, pixel) in pixmap.pixels().iter().enumerate() {
                let r = pixel.red() as u32;
                let g = pixel.green() as u32;
                let b = pixel.blue() as u32;
                buffer[i] = (r << 16) | (g << 8) | b;
            }
            buffer.present().unwrap();
            
            if self.last_cmd.is_some() {
                win.request_redraw();
            }
        }
    }
    
    fn main() -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        let mut app = App::new();
        event_loop.run_app(&mut app)?;
        Ok(())
    }