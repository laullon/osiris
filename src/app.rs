use crate::GilrsEvent;
use crate::commands::{ActionCommand, ControlCommand, NavigationCommand};
use crate::models::RomLibrary;
use crate::ui::renderer::Renderer;
use crate::ui::widgets::common::Widget;
use crate::ui::widgets::panel::SplitPanelWidget;
use crate::ui::widgets::{self, CarouselWidget, GameWidget, ListWidget};
use gilrs::ev::Code;
use gilrs::{Button, EventType};
use rayon::str::Bytes;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{self, ElementState, KeyEvent, WindowEvent};
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
    pub window: Option<Rc<Window>>,
    pub renderer: Renderer,
    root_panel: RootLayout,
    active_command: (NavigationCommand, CommandState),
}

impl ApplicationHandler<GilrsEvent> for OsirisApp {
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
                let command = self.map_key(key, state);
                self.handle_control_command(command);
            }

            WindowEvent::RedrawRequested => {
                if let Some(win) = &self.window {
                    self.renderer.paint(win, &mut self.root_panel);
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: GilrsEvent) {
        match event {
            GilrsEvent::GamepadInput(ev) => {
                match ev.event {
                    EventType::ButtonReleased(_, _) => {
                        self.active_command.0 = NavigationCommand::None;
                    }
                    EventType::ButtonPressed(btn, _) => {
                        let command = self.map_axis(btn);
                        self.handle_control_command(command);
                    }
                    _ => {}
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        let now = Instant::now();
        let (cmd, state) = &mut self.active_command;

        if !state.repeating && now.duration_since(state.started_at) >= REPEAT_DELAY {
            state.repeating = true;
            println!("start repeating {:?}", cmd);
        }

        if state.repeating && now.duration_since(state.last_trigger) >= REPEAT_INTERVAL {
            state.last_trigger = now;
            let event = self
                .root_panel
                .handle_command(ControlCommand::Navigation(cmd.clone()));
            self.root_panel.handle_ui_event(event);
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl OsirisApp {
    pub fn new(renderer: Renderer, library: RomLibrary) -> Self {
        let library = Rc::new(library);
        let carousel = CarouselWidget::new(library.clone());
        let game_list = widgets::ListWidget::new(library.clone());
        let metadata = GameWidget::new(library.clone());

        let main_split = SplitPanelWidget::new(game_list, metadata, 35, true, false);
        let root_panel = SplitPanelWidget::new(carousel, main_split, 20, true, true);

        Self {
            window: None,
            renderer,
            root_panel,
            active_command: (
                NavigationCommand::None,
                CommandState {
                    last_trigger: Instant::now(),
                    started_at: Instant::now(),
                    repeating: false,
                },
            ),
        }
    }

    fn handle_control_command(&mut self, command: Option<ControlCommand>) {
        if let Some(cmd) = command {
            if let ControlCommand::Navigation(nav_cmd) = cmd.clone() {
                self.active_command = (
                    nav_cmd,
                    CommandState {
                        last_trigger: Instant::now(),
                        started_at: Instant::now(),
                        repeating: false,
                    },
                );
            } else {
                self.active_command.0 = NavigationCommand::None;
            }
            let event = self.root_panel.handle_command(cmd);
            self.root_panel.handle_ui_event(event);
            if let Some(win) = &self.window {
                win.request_redraw();
            }
        } else {
            self.active_command.0 = NavigationCommand::None;
        }
    }

    fn map_key(&self, key: KeyCode, state: ElementState) -> Option<ControlCommand> {
        if state == ElementState::Released {
            return None;
        }
        match key {
            KeyCode::ArrowUp => Some(ControlCommand::Navigation(NavigationCommand::Up)),
            KeyCode::ArrowDown => Some(ControlCommand::Navigation(NavigationCommand::Down)),
            KeyCode::ArrowLeft => Some(ControlCommand::Navigation(NavigationCommand::Left)),
            KeyCode::ArrowRight => Some(ControlCommand::Navigation(NavigationCommand::Right)),
            KeyCode::Space => Some(ControlCommand::Action(ActionCommand::Select)),
            KeyCode::Escape => Some(ControlCommand::Action(ActionCommand::Back)),
            _ => None,
        }
    }

    fn map_axis(&self, btn: Button) -> Option<ControlCommand> {
        match btn {
            Button::DPadUp => Some(ControlCommand::Navigation(NavigationCommand::Up)),
            Button::DPadDown => Some(ControlCommand::Navigation(NavigationCommand::Down)),
            Button::DPadLeft => Some(ControlCommand::Navigation(NavigationCommand::Left)),
            Button::DPadRight => Some(ControlCommand::Navigation(NavigationCommand::Right)),
            _ => None,
        }
    }
}
