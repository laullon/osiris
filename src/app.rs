use crate::commands::NavCommand;
use crate::models::RomLibrary;
use crate::ui::renderer::Renderer;
use crate::ui::widgets::common::Widget;
use crate::ui::widgets::panel::SplitPanelWidget;
use crate::ui::widgets::{self, CarouselWidget, GameWidget, ListWidget};
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

type MainSplit = SplitPanelWidget<ListWidget, GameWidget>;
type RootLayout = SplitPanelWidget<CarouselWidget, MainSplit>;

pub struct OsirisApp {
    pub gil: Gilrs,
    pub window: Option<Rc<Window>>,
    pub renderer: Renderer,
    root_panel: RootLayout,
    active_commands: HashMap<NavCommand, CommandState>,
    library: Rc<RomLibrary>,
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
                    self.renderer.paint(win, &mut self.root_panel);
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
            if matches!(cmd, NavCommand::Select | NavCommand::Back) {
                continue;
            }
            if !state.repeating && now.duration_since(state.started_at) >= REPEAT_DELAY {
                state.repeating = true;
            }
            if state.repeating && now.duration_since(state.last_trigger) >= REPEAT_INTERVAL {
                state.last_trigger = now;
                let event = self.root_panel.handle_command(cmd.clone());
                self.root_panel.handle_ui_event(event);
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
    pub fn new(gil: Gilrs, renderer: Renderer, library: RomLibrary) -> Self {
        let library = Rc::new(library);
        let carousel = CarouselWidget::new(library.clone());
        let game_list = widgets::ListWidget::new(library.clone());
        let metadata = GameWidget::new(library.clone());

        let main_split = SplitPanelWidget::new(game_list, metadata, 35, true, false);
        let root_panel = SplitPanelWidget::new(carousel, main_split, 20, true, true);

        Self {
            gil,
            window: None,
            renderer,
            root_panel,
            active_commands: HashMap::new(),
            library,
        }
    }

    fn press_command(&mut self, cmd: NavCommand) {
        if !self.active_commands.contains_key(&cmd) {
            let now = Instant::now();
            self.active_commands.insert(
                cmd.clone(),
                CommandState {
                    last_trigger: now,
                    started_at: now,
                    repeating: false,
                },
            );
            let event = self.root_panel.handle_command(cmd);
            self.root_panel.handle_ui_event(event);
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
            KeyCode::ArrowLeft => Some(NavCommand::Left),
            KeyCode::ArrowRight => Some(NavCommand::Right),
            KeyCode::Space => Some(NavCommand::Select),
            KeyCode::Escape => Some(NavCommand::Back),
            _ => None,
        }
    }
    fn map_btn(&self, btn: Button) -> Option<NavCommand> {
        match btn {
            Button::DPadUp => Some(NavCommand::Up),
            Button::DPadDown => Some(NavCommand::Down),
            Button::DPadLeft => Some(NavCommand::Left),
            Button::DPadRight => Some(NavCommand::Right),
            Button::South => Some(NavCommand::Select),
            Button::East => Some(NavCommand::Back),
            _ => None,
        }
    }
}
