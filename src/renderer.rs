use std::{num::NonZeroU32, rc::Rc, time::{Duration, Instant}};
use softbuffer::{Context, Surface};
use winit::window::Window;
use tiny_skia::{Color, PixmapMut};

use crate::{
    commands::NavCommand, 
    tui::{TuiEngine, GRID_ROWS}, 
    widgets::{ListWidget, MetadataWidget} 
};

pub struct Renderer {
    pub tui: TuiEngine,
    pub context: Option<Context<Rc<Window>>>,
    pub surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    pub last_frame_time: Duration,
    pub current_fps: u32,
    pub fps_counter: u32,
    pub fps_timer: Instant,
}

impl Renderer {
    pub fn new(tui: TuiEngine) -> Self {
        Self { 
            tui, 
            context: None, 
            surface: None,
            last_frame_time: Duration::ZERO,
            fps_timer: Instant::now(),
            current_fps: 0,
            fps_counter: 0,
        }
    }
    
    pub fn paint(
        &mut self, 
        window: &Rc<Window>, 
        last_cmd: &mut Option<(NavCommand, Instant)>, 
        game_list: &ListWidget,
        metadata: &mut MetadataWidget
    ) {
        let start_time = Instant::now();
        let size = window.inner_size();
        
        // Prevent crashes on minimize (0 width)
        if size.width == 0 || size.height == 0 { return; }
        
        if self.surface.is_none() {
            let context = Context::new(window.clone()).unwrap();
            let surface = Surface::new(&context, window.clone()).unwrap();
            self.context = Some(context);
            self.surface = Some(surface);
        }
        
        let surface = self.surface.as_mut().unwrap();
        
        // 1. NATIVE RESOLUTION (Fixes "Small Box" issue)
        // We use the exact physical pixels of the window.
        let buf_width = size.width;
        let buf_height = size.height;
        
        surface.resize(
            NonZeroU32::new(buf_width).unwrap(),
            NonZeroU32::new(buf_height).unwrap()
        ).unwrap();
        
        // 2. ZERO-COPY ACCESS
        let mut buffer = surface.buffer_mut().unwrap();
        
        let raw_bytes: &mut [u8] = unsafe {
            std::slice::from_raw_parts_mut(
                buffer.as_mut_ptr() as *mut u8,
                buffer.len() * 4
            )
        };
        
        if let Some(mut pixmap) = PixmapMut::from_bytes(raw_bytes, buf_width, buf_height) {
            
            // 3. COLOR SETUP (BGR for Direct Writing)
            let bg_color = Color::from_rgba8(5, 10, 0, 255);
            let cyan = Color::from_rgba8(255, 255, 0, 255); // R/B Swapped
            let green = Color::from_rgba8(0, 255, 0, 255);
            let white = Color::from_rgba8(255, 255, 255, 255);
            let status_bg = Color::from_rgba8(20, 20, 20, 255);
            
            // 4. DRAW
            pixmap.fill(bg_color);
            let metrics = self.tui.calculate_metrics(buf_width, buf_height);
            
            self.tui.draw_box(&mut pixmap, &metrics, 1, 1, metrics.cols - 2, GRID_ROWS - 2, cyan);
            
            let title = " OSIRIS MISSION TERMINAL ";
            self.tui.draw_string(&mut pixmap, &metrics, title, (metrics.cols / 2).saturating_sub(title.len() / 2), 0, cyan);
            
            game_list.draw(&mut pixmap, &self.tui, &metrics);
            
            // DYNAMIC LAYOUT FOR METADATA
            // It starts 2 cols after the list ends
            let meta_x = game_list.x + game_list.w + 2;
            // It fills the rest of the screen minus a small margin
            let meta_w = metrics.cols.saturating_sub(meta_x + 2)-1;
            
            // Apply dimensions
            metadata.x = meta_x;
            metadata.y = game_list.y;
            metadata.w = meta_w;
            metadata.h = game_list.h;
            
            // Draw Metadata
            if let Some(selected_text) = game_list.items.get(game_list.selected_index) {
                metadata.draw(&mut pixmap, &self.tui, &metrics, selected_text);
            }
            let bar_y = GRID_ROWS - 1;
            let stats_msg = format!(
                " [ RENDER: {:>5.2?} | FPS: {:>3} ] ", 
                self.last_frame_time, 
                self.current_fps
            );
            
            self.tui.draw_string_ex(&mut pixmap, &metrics, &" ".repeat(metrics.cols), 0, bar_y, Color::TRANSPARENT, Some(status_bg), 1);
            self.tui.draw_string(&mut pixmap, &metrics, &stats_msg, metrics.cols.saturating_sub(stats_msg.len()), bar_y, cyan);
            self.tui.draw_string(&mut pixmap, &metrics, " OSIRIS MISSION CONTROL", 0, bar_y, green);
            
            // if let Some((cmd, instant)) = last_cmd {
            //     if instant.elapsed() < Duration::from_secs(2) {
            //         let msg = format!(">> CMD: {:?}", cmd);
            //         self.tui.draw_string(&mut pixmap, &metrics, &msg, 4, GRID_ROWS - 5, white);
            //     } else { *last_cmd = None; }
            // }
        }
        
        // 5. PRESENT
        self.last_frame_time = start_time.elapsed();
        buffer.present().unwrap();
        
        self.fps_counter += 1;
        if self.fps_timer.elapsed() >= Duration::from_secs(1) {
            self.current_fps = self.fps_counter;
            self.fps_counter = 0;
            self.fps_timer = Instant::now();
        }
    }
}