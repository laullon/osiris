use crate::commands::{ActionCommand, ControlCommand, NavigationCommand};
use crate::input::gamepad::{Button, EventType};
use crate::models::RomLibrary;
use crate::ui::renderer::Renderer;
use crate::ui::widgets::common::Widget;
use crate::ui::widgets::panel::SplitPanelWidget;
use crate::ui::widgets::{self, CarouselWidget, GameWidget, ListWidget};
use crate::GilrsEvent;
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
                    EventType::ButtonChanged(btn, value, _) => {
                        // For D-pad, determine navigation direction based on movement from neutral
                        if matches!(
                            btn,
                            Button::DPadUp
                                | Button::DPadDown
                                | Button::DPadLeft
                                | Button::DPadRight
                        ) {
                            println!(
                                "[UI] Received D-pad event from controller {}: {:?} = {:.3}",
                                ev.id, btn, value
                            );
                            let command = self.map_dpad_direction(btn, value);
                            self.handle_control_command(command);
                        } else {
                            // Regular buttons: simple threshold
                            if value > 0.5 {
                                let command = self.map_axis(btn);
                                self.handle_control_command(command);
                            } else {
                                self.active_command.0 = NavigationCommand::None;
                            }
                        }
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

    fn map_dpad_direction(&self, btn: Button, value: f32) -> Option<ControlCommand> {
        // D-pad is analog: ~0.4-0.5 = neutral, 1.0 = one direction, 0.0 = opposite
        // Threshold: > 0.7 = pressed, < 0.3 = opposite direction, between = no action

        let command = match btn {
            Button::DPadUp => {
                if value > 0.7 {
                    Some(ControlCommand::Navigation(NavigationCommand::Up))
                } else if value < 0.3 {
                    Some(ControlCommand::Navigation(NavigationCommand::Down))
                } else {
                    Some(ControlCommand::Navigation(NavigationCommand::None))
                }
            }
            Button::DPadDown => {
                if value > 0.7 {
                    Some(ControlCommand::Navigation(NavigationCommand::Down))
                } else if value < 0.3 {
                    Some(ControlCommand::Navigation(NavigationCommand::Up))
                } else {
                    Some(ControlCommand::Navigation(NavigationCommand::None))
                }
            }
            Button::DPadLeft => {
                if value > 0.7 {
                    Some(ControlCommand::Navigation(NavigationCommand::Left))
                } else if value < 0.3 {
                    Some(ControlCommand::Navigation(NavigationCommand::Right))
                } else {
                    Some(ControlCommand::Navigation(NavigationCommand::None))
                }
            }
            Button::DPadRight => {
                if value > 0.7 {
                    Some(ControlCommand::Navigation(NavigationCommand::Right))
                } else if value < 0.3 {
                    Some(ControlCommand::Navigation(NavigationCommand::Left))
                } else {
                    Some(ControlCommand::Navigation(NavigationCommand::None))
                }
            }
            _ => None,
        };

        if let Some(ControlCommand::Navigation(nav)) = &command {
            println!("[D-PAD] {:?} value={:.3} -> {:?}", btn, value, nav);
        }

        command
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
