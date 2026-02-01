use crate::commands::NavCommand;
use crate::renderer::Renderer;
use crate::widgets::{ListWidget, MetadataWidget};
use gilrs::{Button, EventType, Gilrs};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_INTERVAL: Duration = Duration::from_millis(80);

struct CommandState {
    last_trigger: Instant,
    started_at: Instant,
    repeating: bool,
}

pub struct OsirisApp {
    pub gil: Gilrs,
    pub window: Option<Rc<Window>>,
    pub renderer: Renderer,
    pub last_cmd: Option<(NavCommand, Instant)>,
    pub game_list: ListWidget,
    pub metadata: MetadataWidget,
    active_commands: HashMap<NavCommand, CommandState>,
}

impl ApplicationHandler for OsirisApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Rc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("OSIRIS")
                            .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None))),
                    )
                    .unwrap(),
            );
            self.window = Some(window);
        }
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => _el.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(key),
                        repeat: false,
                        ..
                    },
                ..
            } => {
                if let Some(cmd) = self.map_key(key) {
                    if state == ElementState::Pressed {
                        self.press_command(cmd);
                    } else {
                        self.release_command(cmd);
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(win) = &self.window {
                    self.renderer.paint(
                        win,
                        &mut self.last_cmd,
                        &self.game_list,
                        &mut self.metadata, // Add &mut here
                    );
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        while let Some(ev) = self.gil.next_event() {
            match ev.event {
                EventType::ButtonPressed(btn, _) => {
                    if let Some(cmd) = self.map_btn(btn) {
                        self.press_command(cmd);
                    }
                }
                EventType::ButtonReleased(btn, _) => {
                    if let Some(cmd) = self.map_btn(btn) {
                        self.release_command(cmd);
                    }
                }
                _ => {}
            }
        }

        let mut changed = false;
        let now = Instant::now();
        for (cmd, state) in self.active_commands.iter_mut() {
            if !matches!(cmd, NavCommand::Up | NavCommand::Down) {
                continue;
            }
            if !state.repeating && now.duration_since(state.started_at) >= REPEAT_DELAY {
                state.repeating = true;
            }
            if state.repeating && now.duration_since(state.last_trigger) >= REPEAT_INTERVAL {
                state.last_trigger = now;
                self.game_list.handle_command(*cmd);
                self.last_cmd = Some((*cmd, now));
                changed = true;
            }
        }

        if changed {
            if let Some(win) = &self.window {
                win.request_redraw();
            }
        }
    }
}

impl OsirisApp {
    pub fn new(
        gil: Gilrs,
        renderer: Renderer,
        game_list: ListWidget,
        metadata: MetadataWidget,
    ) -> Self {
        Self {
            gil,
            window: None,
            renderer,
            last_cmd: None,
            game_list,
            active_commands: HashMap::new(),
            metadata,
        }
    }
    fn press_command(&mut self, cmd: NavCommand) {
        if !self.active_commands.contains_key(&cmd) {
            let now = Instant::now();
            self.active_commands.insert(
                cmd,
                CommandState {
                    last_trigger: now,
                    started_at: now,
                    repeating: false,
                },
            );
            self.game_list.handle_command(cmd);
            self.last_cmd = Some((cmd, now));
            if let Some(win) = &self.window {
                win.request_redraw();
            }
        }
    }
    fn release_command(&mut self, cmd: NavCommand) {
        self.active_commands.remove(&cmd);
    }
    fn map_key(&self, key: KeyCode) -> Option<NavCommand> {
        match key {
            KeyCode::ArrowUp => Some(NavCommand::Up),
            KeyCode::ArrowDown => Some(NavCommand::Down),
            KeyCode::Space => Some(NavCommand::Select),
            KeyCode::Escape => Some(NavCommand::Back),
            _ => None,
        }
    }
    fn map_btn(&self, btn: Button) -> Option<NavCommand> {
        match btn {
            Button::DPadUp => Some(NavCommand::Up),
            Button::DPadDown => Some(NavCommand::Down),
            Button::South => Some(NavCommand::Select),
            Button::East => Some(NavCommand::Back),
            _ => None,
        }
    }
}
