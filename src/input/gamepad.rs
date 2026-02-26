use hidapi::{HidApi, HidDevice};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

// Minimal compatibility types to replace gilrs where used in the project
pub type GamepadId = u32;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Button {
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Axis {
    LeftStickX,
    LeftStickY,
    LeftZ,
    RightStickX,
    RightStickY,
    RightZ,
    DPadX,
    DPadY,
}

#[derive(Debug, Clone)]
pub enum EventType {
    Connected,
    Disconnected,
    /// (button, value, timestamp_ms)
    ButtonChanged(Button, f32, u32),
    /// (axis, value, timestamp_ms)
    AxisChanged(Axis, f32, u32),
}

#[derive(Debug, Clone)]
pub struct Event {
    pub id: GamepadId,
    pub event: EventType,
}

/// Stores the current state of a connected gamepad
#[derive(Debug, Clone)]
pub struct GamepadDeviceState {
    pub id: GamepadId,
    pub name: String,
    pub button_states: HashMap<Button, f32>,
    pub axis_states: HashMap<Axis, f32>,
    pub dpad_neutral: (f32, f32), // (DPadX neutral, DPadY neutral)
    last_report: Vec<u8>,
    // Pending full-report debounce
    pending_report: Vec<u8>,
    pending_count: u8,
    // Observed axis ranges for normalization (min, max)
    axis_range: HashMap<Axis, (f32, f32)>,
    // Debounce counters for buttons
    pending_button_counts: HashMap<Button, u8>,
    // Hat stability
    last_hat: Option<u8>,
    hat_stable_count: u8,
    // Axis range type: Some(true) = signed (-1..1), Some(false) = unsigned (0..1)
    axis_signed: Option<bool>,
}

/// Shared status available to the UI for rendering controllers and logs
pub struct GamepadStatus {
    pub controllers: HashMap<GamepadId, String>,
    pub recent_logs: Vec<String>,
    pub last_event_time_ms: HashMap<GamepadId, u128>,
}

impl GamepadStatus {
    pub fn new() -> Self {
        Self {
            controllers: HashMap::new(),
            recent_logs: Vec::new(),
            last_event_time_ms: HashMap::new(),
        }
    }

    fn push_log(&mut self, s: String) {
        println!("{}", s);
    }
}

pub static GAMEPAD_STATUS: Lazy<Arc<Mutex<GamepadStatus>>> =
    Lazy::new(|| Arc::new(Mutex::new(GamepadStatus::new())));

impl GamepadDeviceState {
    pub fn new(id: GamepadId, name: String, report_len: usize) -> Self {
        let mut button_states = HashMap::new();
        let mut pending_button_counts = HashMap::new();
        for b in ALL_BUTTONS.iter() {
            button_states.insert(*b, 0.5); // neutral
            pending_button_counts.insert(*b, 0u8);
        }
        let mut axis_states = HashMap::new();
        for a in ALL_AXES.iter() {
            axis_states.insert(*a, 0.0);
        }
        let mut axis_range = HashMap::new();
        for a in ALL_AXES.iter() {
            // initialize with inverted range so first sample sets them
            axis_range.insert(*a, (1.0, -1.0));
        }
        Self {
            id,
            name,
            button_states,
            axis_states,
            dpad_neutral: (0.5, 0.5),
            last_report: vec![0u8; report_len],
            pending_report: vec![0u8; report_len],
            pending_count: 0,
            axis_range,
            pending_button_counts,
            last_hat: None,
            hat_stable_count: 0,
            axis_signed: None,
        }
    }

    /// Log minimal info - just controller connection with D-pad state
    pub fn log_state(&self) {
        println!("[GAMEPAD {}] CONNECTED: {}", self.id, self.name);
        println!(
            "  D-Pad State: Up={:.3} Down={:.3} Left={:.3} Right={:.3}",
            self.button_states.get(&Button::DPadUp).unwrap_or(&0.0),
            self.button_states.get(&Button::DPadDown).unwrap_or(&0.0),
            self.button_states.get(&Button::DPadLeft).unwrap_or(&0.0),
            self.button_states.get(&Button::DPadRight).unwrap_or(&0.0)
        );
    }
}

/// Wrapper around hidapi that maintains gamepad state
pub struct GamepadStateWrapper {
    api: HidApi,
    devices: HashMap<GamepadId, HidDevice>,
    gamepad_states: HashMap<GamepadId, GamepadDeviceState>,
    next_id: GamepadId,
}

impl GamepadStateWrapper {
    /// Create a new wrapper around hidapi and open candidate gamepad devices
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let api = HidApi::new()?;
        // clear shared status
        if let Ok(mut st) = GAMEPAD_STATUS.lock() {
            st.controllers.clear();
            st.last_event_time_ms.clear();
        }
        let mut devices = HashMap::new();
        let mut gamepad_states = HashMap::new();
        let mut next_id: GamepadId = 0;

        // Enumerate HID devices: prefer usage_page 0x01 (Generic Desktop) with usage 0x05 (Game Pad) or 0x04 (Joystick)
        for devinfo in api.device_list() {
            let usage_page = devinfo.usage_page();
            let usage = devinfo.usage();
            let is_gamepad = if usage_page == 0x01 && (usage == 0x05 || usage == 0x04) {
                true
            } else {
                // Heuristic: look for product/manufacturer strings
                if let Some(p) = devinfo.product_string() {
                    let low = p.to_lowercase();
                    if low.contains("gamepad")
                        || low.contains("controller")
                        || low.contains("joystick")
                        || low.contains("wireless")
                    {
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            if is_gamepad {
                let path = devinfo.path();
                if let Ok(device) = api.open_path(path) {
                    let mut dev = device; // keep mutable to read initial report
                    let id = next_id;
                    next_id += 1;
                    let name = devinfo
                        .product_string()
                        .unwrap_or("Unknown Controller".into())
                        .to_string();
                    let report_len = 64; // increase buffer to capture larger reports

                    // try to read an initial report to seed last_report and avoid spurious events
                    let mut init_state = GamepadDeviceState::new(id, name.clone(), report_len);
                    let mut init_buf = [0u8; 64];
                    match dev.read_timeout(&mut init_buf, 50) {
                        Ok(len) if len > 0 => {
                            let data = &init_buf[..len];
                            init_state.last_report.clear();
                            init_state.last_report.extend_from_slice(data);
                            init_state.pending_report.clear();
                            init_state.pending_report.extend_from_slice(data);
                            init_state.pending_count = 0;

                            // seed axis states if available and detect signed/unsigned range
                            if data.len() >= 1 {
                                let raw = data[0] as f32;
                                // Detect: if raw is close to 128, it's likely signed (center at 127)
                                // if raw is close to 0 or 1, it's likely unsigned (center at ~128)
                                let is_signed = raw > 64.0 && raw < 192.0;
                                init_state.axis_signed = Some(is_signed);
                                if is_signed {
                                    let value = (raw - 128.0) / 127.0;
                                    init_state.axis_states.insert(Axis::LeftStickX, value);
                                } else {
                                    let value = raw / 255.0;
                                    init_state.axis_states.insert(Axis::LeftStickX, value);
                                }
                            }
                            if data.len() >= 2 {
                                let raw = data[1] as f32;
                                let is_signed = init_state.axis_signed.unwrap_or(true);
                                if is_signed {
                                    let value = (raw - 128.0) / 127.0;
                                    init_state.axis_states.insert(Axis::LeftStickY, value);
                                } else {
                                    let value = raw / 255.0;
                                    init_state.axis_states.insert(Axis::LeftStickY, value);
                                }
                            }
                            if data.len() >= 3 {
                                let bits = data[2];
                                for i in 0..8 {
                                    let mask = 1u8 << i;
                                    let pressed = (bits & mask) != 0;
                                    let btn = match i {
                                        0 => Button::South,
                                        1 => Button::East,
                                        2 => Button::North,
                                        3 => Button::West,
                                        4 => Button::LeftTrigger,
                                        5 => Button::RightTrigger,
                                        6 => Button::Select,
                                        7 => Button::Start,
                                        _ => Button::South,
                                    };
                                    init_state
                                        .button_states
                                        .insert(btn, if pressed { 1.0 } else { 0.0 });
                                }
                            }
                            if data.len() >= 4 {
                                let hat = data[3];
                                match hat {
                                    0 => {
                                        init_state.button_states.insert(Button::DPadUp, 1.0);
                                    }
                                    2 => {
                                        init_state.button_states.insert(Button::DPadRight, 1.0);
                                    }
                                    4 => {
                                        init_state.button_states.insert(Button::DPadDown, 1.0);
                                    }
                                    6 => {
                                        init_state.button_states.insert(Button::DPadLeft, 1.0);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {
                            // leave defaults (neutral)
                        }
                    }

                    devices.insert(id, dev);
                    gamepad_states.insert(id, init_state);

                    // update shared status
                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                        st.controllers.insert(id, name);
                        st.push_log(format!("[GAMEPAD {}] detected", id));
                    }
                }
            }
        }

        Ok(Self {
            api,
            devices,
            gamepad_states,
            next_id,
        })
    }

    /// Get the next event, updating state internally
    pub fn next_event(&mut self) -> Option<Event> {
        // Poll all devices once and return the first event found
        for (id, device) in self.devices.iter_mut() {
            // read_timeout with 0 for non-blocking
            let mut buf = [0u8; 64];
            match device.read_timeout(&mut buf, 0) {
                Ok(len) if len > 0 => {
                    let data = &buf[..len];
                    // log raw report for debugging multi-controller issues
                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                        st.push_log(format!("[GAMEPAD RAW {}] len={} {:?}", id, len, &data));
                    }
                    // Compare with last report to detect changes
                    let state = self.gamepad_states.get_mut(id).unwrap();
                    // Ensure last_report/pending_report are large enough
                    if state.last_report.len() < data.len() {
                        state.last_report.resize(data.len(), 0);
                    }
                    if state.pending_report.len() < data.len() {
                        state.pending_report.resize(data.len(), 0);
                    }

                    // Simple parsing heuristic:
                    // - byte 0: left stick X (0..255 -> -1..1)
                    // - byte 1: left stick Y
                    // - byte 2: button bitfield
                    // - byte 3: hat (0..7) where 0=up, 2=right, 4=down, 6=left, 8=center

                    let now_ms_u128 = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or(Duration::from_secs(0))
                        .as_millis();
                    let now_ms = now_ms_u128 as u32;

                    // Full-report debouncing: require two identical reads before accepting
                    let mut accept_report = false;
                    if state.pending_count == 0 {
                        state.pending_report[..data.len()].copy_from_slice(data);
                        state.pending_count = 1;
                    } else {
                        // Compare pending_report with incoming data
                        if state.pending_report[..data.len()] == data[..] {
                            // stable across two reads -> accept
                            accept_report = true;
                            state.pending_count = 0;
                        } else {
                            // not stable, reset pending_report to new sample
                            state.pending_report[..data.len()].copy_from_slice(data);
                            state.pending_count = 1;
                        }
                    }

                    if !accept_report {
                        // don't process transient reports
                        continue;
                    }

                    // Axis X -> emit analog DPadLeft/DPadRight ButtonChanged events based on stick position
                    if data.len() >= 1 {
                        let raw = data[0] as f32;
                        let is_signed = state.axis_signed.unwrap_or(true);
                        let value = if is_signed {
                            (raw - 128.0) / 127.0 // signed: -1..1
                        } else {
                            raw / 255.0 // unsigned: 0..1
                        };

                        // update observed range for this axis
                        if let Some(r) = state.axis_range.get_mut(&Axis::LeftStickX) {
                            r.0 = r.0.min(value);
                            r.1 = r.1.max(value);
                        }

                        let prev = *state.axis_states.get(&Axis::LeftStickX).unwrap_or(&0.0);
                        if (value - prev).abs() > 0.02 {
                            state.axis_states.insert(Axis::LeftStickX, value);
                            state.last_report[0] = data[0];
                            if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                st.last_event_time_ms.insert(*id, now_ms_u128);
                                st.push_log(format!(
                                    "[GAMEPAD {}] Axis LeftStickX = {:.3}",
                                    id, value
                                ));
                            }

                            // Determine normalization to 0..1 using detected signed/unsigned type
                            let is_signed = state.axis_signed.unwrap_or(true);
                            let n = if is_signed {
                                // signed: -1..1 -> 0..1
                                ((value + 1.0) / 2.0).clamp(0.0, 1.0)
                            } else {
                                // unsigned: 0..1 already
                                value.clamp(0.0, 1.0)
                            };

                            // Quantize to help UI navigation: strong press -> 1.0, neutral -> 0.5, opposite -> 0.0
                            let quantize = |v: f32| {
                                if v > 0.75 {
                                    1.0
                                } else if v < 0.25 {
                                    0.0
                                } else {
                                    0.5
                                }
                            };
                            let dpad_right = quantize(n);
                            let dpad_left = quantize(1.0 - n);

                            // emit for left
                            let prev_left =
                                *state.button_states.get(&Button::DPadLeft).unwrap_or(&0.5);
                            if (dpad_left - prev_left).abs() > 0.01 {
                                state.button_states.insert(Button::DPadLeft, dpad_left);
                                if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                    st.push_log(format!(
                                        "[GAMEPAD {}] Synth DPadLeft = {:.3}",
                                        id, dpad_left
                                    ));
                                }
                                // log press/release only when meaningful
                                if dpad_left > 0.75 {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD LEFT", id));
                                    }
                                } else if prev_left > 0.75 {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD LEFT RELEASE", id));
                                    }
                                }
                                return Some(Event {
                                    id: *id,
                                    event: EventType::ButtonChanged(
                                        Button::DPadLeft,
                                        dpad_left,
                                        now_ms,
                                    ),
                                });
                            }

                            // emit for right
                            let prev_right =
                                *state.button_states.get(&Button::DPadRight).unwrap_or(&0.5);
                            if (dpad_right - prev_right).abs() > 0.01 {
                                state.button_states.insert(Button::DPadRight, dpad_right);
                                if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                    st.push_log(format!(
                                        "[GAMEPAD {}] Synth DPadRight = {:.3}",
                                        id, dpad_right
                                    ));
                                }
                                if dpad_right > 0.75 {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD RIGHT", id));
                                    }
                                } else if prev_right > 0.75 {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD RIGHT RELEASE", id));
                                    }
                                }
                                return Some(Event {
                                    id: *id,
                                    event: EventType::ButtonChanged(
                                        Button::DPadRight,
                                        dpad_right,
                                        now_ms,
                                    ),
                                });
                            }

                            // fallback: emit axis change
                            return Some(Event {
                                id: *id,
                                event: EventType::AxisChanged(Axis::LeftStickX, value, now_ms),
                            });
                        }
                    }

                    // Axis Y -> emit analog DPadUp/DPadDown
                    if data.len() >= 2 {
                        let raw = data[1] as f32;
                        let is_signed = state.axis_signed.unwrap_or(true);
                        let value = if is_signed {
                            (raw - 128.0) / 127.0 // signed: -1..1
                        } else {
                            raw / 255.0 // unsigned: 0..1
                        };

                        // update observed range for this axis
                        if let Some(r) = state.axis_range.get_mut(&Axis::LeftStickY) {
                            r.0 = r.0.min(value);
                            r.1 = r.1.max(value);
                        }

                        let prev = *state.axis_states.get(&Axis::LeftStickY).unwrap_or(&0.0);
                        if (value - prev).abs() > 0.02 {
                            state.axis_states.insert(Axis::LeftStickY, value);
                            state.last_report[1] = data[1];
                            if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                st.last_event_time_ms.insert(*id, now_ms_u128);
                                st.push_log(format!(
                                    "[GAMEPAD {}] Axis LeftStickY = {:.3}",
                                    id, value
                                ));
                            }

                            let is_signed = state.axis_signed.unwrap_or(true);
                            let n = if is_signed {
                                ((value + 1.0) / 2.0).clamp(0.0, 1.0)
                            } else {
                                value.clamp(0.0, 1.0)
                            };

                            let quantize = |v: f32| {
                                if v > 0.75 {
                                    1.0
                                } else if v < 0.25 {
                                    0.0
                                } else {
                                    0.5
                                }
                            };
                            let dpad_down = quantize(n);
                            let dpad_up = quantize(1.0 - n); // assume negative = up

                            let prev_up = *state.button_states.get(&Button::DPadUp).unwrap_or(&0.5);
                            if (dpad_up - prev_up).abs() > 0.01 {
                                state.button_states.insert(Button::DPadUp, dpad_up);
                                if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                    st.push_log(format!(
                                        "[GAMEPAD {}] Synth DPadUp = {:.3}",
                                        id, dpad_up
                                    ));
                                }
                                if dpad_up > 0.75 {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD UP", id));
                                    }
                                } else if prev_up > 0.75 {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD UP RELEASE", id));
                                    }
                                }
                                return Some(Event {
                                    id: *id,
                                    event: EventType::ButtonChanged(
                                        Button::DPadUp,
                                        dpad_up,
                                        now_ms,
                                    ),
                                });
                            }

                            let prev_down =
                                *state.button_states.get(&Button::DPadDown).unwrap_or(&0.5);
                            if (dpad_down - prev_down).abs() > 0.01 {
                                state.button_states.insert(Button::DPadDown, dpad_down);
                                if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                    st.push_log(format!(
                                        "[GAMEPAD {}] Synth DPadDown = {:.3}",
                                        id, dpad_down
                                    ));
                                }
                                if dpad_down > 0.75 {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD DOWN", id));
                                    }
                                } else if prev_down > 0.75 {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD DOWN RELEASE", id));
                                    }
                                }
                                return Some(Event {
                                    id: *id,
                                    event: EventType::ButtonChanged(
                                        Button::DPadDown,
                                        dpad_down,
                                        now_ms,
                                    ),
                                });
                            }

                            return Some(Event {
                                id: *id,
                                event: EventType::AxisChanged(Axis::LeftStickY, value, now_ms),
                            });
                        }
                    }

                    // Buttons (debounced): only emit when a bit change is stable across two reads
                    if data.len() >= 3 {
                        let bits = data[2];
                        let prev_bits = state.last_report.get(2).copied().unwrap_or(0u8);
                        if bits != prev_bits {
                            // Check individual bits
                            for i in 0..8 {
                                let mask = 1u8 << i;
                                let prev_pressed = (prev_bits & mask) != 0;
                                let pressed = (bits & mask) != 0;
                                if pressed != prev_pressed {
                                    let btn = match i {
                                        0 => Button::South,
                                        1 => Button::East,
                                        2 => Button::North,
                                        3 => Button::West,
                                        4 => Button::LeftTrigger,
                                        5 => Button::RightTrigger,
                                        6 => Button::Select,
                                        7 => Button::Start,
                                        _ => Button::South,
                                    };

                                    let desired_value = if pressed { 1.0 } else { 0.0 };
                                    let emitted_prev =
                                        *state.button_states.get(&btn).unwrap_or(&0.5);

                                    if (desired_value - emitted_prev).abs() > 0.001 {
                                        let cnt =
                                            state.pending_button_counts.entry(btn).or_insert(0u8);
                                        *cnt = cnt.saturating_add(1);
                                        if *cnt >= 2 {
                                            // stable change -> emit
                                            state.button_states.insert(btn, desired_value);
                                            state.last_report[2] = bits;
                                            *cnt = 0;
                                            if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                                st.last_event_time_ms.insert(*id, now_ms_u128);
                                                st.push_log(format!(
                                                    "[GAMEPAD {}] Button {:?} = {:.1}",
                                                    id, btn, desired_value
                                                ));
                                            }
                                            return Some(Event {
                                                id: *id,
                                                event: EventType::ButtonChanged(
                                                    btn,
                                                    desired_value,
                                                    now_ms,
                                                ),
                                            });
                                        }
                                    } else {
                                        // no change vs emitted value
                                        state.pending_button_counts.insert(btn, 0u8);
                                    }
                                }
                            }
                        }
                    }

                    // Hat (dpad) in byte 3 (debounced)
                    if data.len() >= 4 {
                        let hat = data[3];
                        let last_hat = state
                            .last_hat
                            .unwrap_or(state.last_report.get(3).copied().unwrap_or(8u8));
                        if hat != last_hat {
                            state.hat_stable_count = state.hat_stable_count.saturating_add(1);
                            if state.hat_stable_count >= 2 {
                                state.last_hat = Some(hat);
                                state.hat_stable_count = 0;
                                state.last_report[3] = hat;
                                if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                    st.last_event_time_ms.insert(*id, now_ms_u128);
                                    st.push_log(format!("[GAMEPAD {}] Hat = {}", id, hat));
                                }

                                match hat {
                                    0 => {
                                        state.button_states.insert(Button::DPadUp, 1.0);
                                        state.button_states.insert(Button::DPadDown, 0.0);
                                        state.button_states.insert(Button::DPadLeft, 0.0);
                                        state.button_states.insert(Button::DPadRight, 0.0);
                                        if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                            st.push_log(format!("[GAMEPAD {}] DPAD UP", id));
                                        }
                                        return Some(Event {
                                            id: *id,
                                            event: EventType::ButtonChanged(
                                                Button::DPadUp,
                                                1.0,
                                                now_ms,
                                            ),
                                        });
                                    }
                                    2 => {
                                        state.button_states.insert(Button::DPadRight, 1.0);
                                        state.button_states.insert(Button::DPadLeft, 0.0);
                                        state.button_states.insert(Button::DPadUp, 0.0);
                                        state.button_states.insert(Button::DPadDown, 0.0);
                                        if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                            st.push_log(format!("[GAMEPAD {}] DPAD RIGHT", id));
                                        }
                                        return Some(Event {
                                            id: *id,
                                            event: EventType::ButtonChanged(
                                                Button::DPadRight,
                                                1.0,
                                                now_ms,
                                            ),
                                        });
                                    }
                                    4 => {
                                        state.button_states.insert(Button::DPadDown, 1.0);
                                        state.button_states.insert(Button::DPadUp, 0.0);
                                        state.button_states.insert(Button::DPadLeft, 0.0);
                                        state.button_states.insert(Button::DPadRight, 0.0);
                                        if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                            st.push_log(format!("[GAMEPAD {}] DPAD DOWN", id));
                                        }
                                        return Some(Event {
                                            id: *id,
                                            event: EventType::ButtonChanged(
                                                Button::DPadDown,
                                                1.0,
                                                now_ms,
                                            ),
                                        });
                                    }
                                    6 => {
                                        state.button_states.insert(Button::DPadLeft, 1.0);
                                        state.button_states.insert(Button::DPadRight, 0.0);
                                        state.button_states.insert(Button::DPadUp, 0.0);
                                        state.button_states.insert(Button::DPadDown, 0.0);
                                        if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                            st.push_log(format!("[GAMEPAD {}] DPAD LEFT", id));
                                        }
                                        return Some(Event {
                                            id: *id,
                                            event: EventType::ButtonChanged(
                                                Button::DPadLeft,
                                                1.0,
                                                now_ms,
                                            ),
                                        });
                                    }
                                    8 => {
                                        // center - release all
                                        state.button_states.insert(Button::DPadUp, 0.5);
                                        state.button_states.insert(Button::DPadDown, 0.5);
                                        state.button_states.insert(Button::DPadLeft, 0.5);
                                        state.button_states.insert(Button::DPadRight, 0.5);
                                        if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                            st.push_log(format!("[GAMEPAD {}] DPAD CENTER", id));
                                        }
                                        return Some(Event {
                                            id: *id,
                                            event: EventType::ButtonChanged(
                                                Button::DPadUp,
                                                0.5,
                                                now_ms,
                                            ),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            state.hat_stable_count = 0;
                        }
                    }

                    // No recognized change from known layouts — attempt generic diff parsing
                    // Look for any byte that changed and heuristically emit events
                    for i in 0..data.len() {
                        // Skip primary indices handled above: axes 0/1, buttons 2, hat 3
                        if i < 4 {
                            continue;
                        }

                        let newb = data[i];
                        let oldb = *state.last_report.get(i).unwrap_or(&0u8);
                        if newb == oldb {
                            continue;
                        }

                        // Update stored report only when we decide to accept it
                        if state.last_report.len() <= i {
                            state.last_report.resize(i + 1, 0);
                        }

                        // If small integer (<=8) treat as hat
                        if newb <= 8 {
                            let hat = newb;
                            // Generic hat debouncing: require two consecutive reads matching
                            if state
                                .last_hat
                                .unwrap_or(state.last_report.get(i).copied().unwrap_or(8u8))
                                != hat
                            {
                                state.hat_stable_count = state.hat_stable_count.saturating_add(1);
                                if state.hat_stable_count < 2 {
                                    continue;
                                }
                                state.last_hat = Some(hat);
                                state.hat_stable_count = 0;
                            } else {
                                state.hat_stable_count = 0;
                            }

                            if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                st.last_event_time_ms.insert(*id, now_ms_u128);
                                st.push_log(format!(
                                    "[GAMEPAD {}] Generic Hat@{} = {}",
                                    id, i, hat
                                ));
                            }
                            match hat {
                                0 => {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD UP", id));
                                    }
                                    return Some(Event {
                                        id: *id,
                                        event: EventType::ButtonChanged(
                                            Button::DPadUp,
                                            1.0,
                                            now_ms,
                                        ),
                                    });
                                }
                                2 => {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD RIGHT", id));
                                    }
                                    return Some(Event {
                                        id: *id,
                                        event: EventType::ButtonChanged(
                                            Button::DPadRight,
                                            1.0,
                                            now_ms,
                                        ),
                                    });
                                }
                                4 => {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD DOWN", id));
                                    }
                                    return Some(Event {
                                        id: *id,
                                        event: EventType::ButtonChanged(
                                            Button::DPadDown,
                                            1.0,
                                            now_ms,
                                        ),
                                    });
                                }
                                6 => {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD LEFT", id));
                                    }
                                    return Some(Event {
                                        id: *id,
                                        event: EventType::ButtonChanged(
                                            Button::DPadLeft,
                                            1.0,
                                            now_ms,
                                        ),
                                    });
                                }
                                8 => {
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.push_log(format!("[GAMEPAD {}] DPAD CENTER", id));
                                    }
                                    return Some(Event {
                                        id: *id,
                                        event: EventType::ButtonChanged(
                                            Button::DPadUp,
                                            0.0,
                                            now_ms,
                                        ),
                                    });
                                }
                                _ => continue,
                            }
                        }

                        // If value swings widely, treat as axis
                        let delta = (newb as i16 - oldb as i16).abs();
                        if delta > 4 {
                            let value = (newb as f32 - 128.0) / 127.0;
                            if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                st.last_event_time_ms.insert(*id, now_ms_u128);
                                st.push_log(format!(
                                    "[GAMEPAD {}] Generic Axis@{} = {:.3}",
                                    id, i, value
                                ));
                            }
                            // map first generic axis found to LeftStickX, others fallback to LeftStickY
                            if i % 2 == 0 {
                                // update observed range
                                if let Some(r) = state.axis_range.get_mut(&Axis::LeftStickX) {
                                    r.0 = r.0.min(value);
                                    r.1 = r.1.max(value);
                                }
                                return Some(Event {
                                    id: *id,
                                    event: EventType::AxisChanged(Axis::LeftStickX, value, now_ms),
                                });
                            } else {
                                if let Some(r) = state.axis_range.get_mut(&Axis::LeftStickY) {
                                    r.0 = r.0.min(value);
                                    r.1 = r.1.max(value);
                                }
                                return Some(Event {
                                    id: *id,
                                    event: EventType::AxisChanged(Axis::LeftStickY, value, now_ms),
                                });
                            }
                        }

                        // Otherwise, treat as button bitfield
                        let bits = newb;
                        let prev_bits = oldb;
                        if bits != prev_bits {
                            for bit in 0..8 {
                                let mask = 1u8 << bit;
                                let prev_pressed = (prev_bits & mask) != 0;
                                let pressed = (bits & mask) != 0;
                                if pressed != prev_pressed {
                                    let btn = match bit {
                                        0 => Button::South,
                                        1 => Button::East,
                                        2 => Button::North,
                                        3 => Button::West,
                                        4 => Button::LeftTrigger,
                                        5 => Button::RightTrigger,
                                        6 => Button::Select,
                                        7 => Button::Start,
                                        _ => Button::South,
                                    };
                                    let value = if pressed { 1.0 } else { 0.0 };
                                    if let Ok(mut st) = GAMEPAD_STATUS.lock() {
                                        st.last_event_time_ms.insert(*id, now_ms_u128);
                                        st.push_log(format!(
                                            "[GAMEPAD {}] Generic Btn@{} bit{} = {}",
                                            id, i, bit, value
                                        ));
                                    }
                                    return Some(Event {
                                        id: *id,
                                        event: EventType::ButtonChanged(btn, value, now_ms),
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Get the current state of a specific gamepad
    pub fn gamepad_state(&self, id: GamepadId) -> Option<&GamepadDeviceState> {
        self.gamepad_states.get(&id)
    }

    /// Get states of all connected gamepads
    pub fn all_gamepads_state(&self) -> Vec<&GamepadDeviceState> {
        self.gamepad_states.values().collect()
    }

    /// Log all currently connected gamepads and their state
    pub fn log_all_states(&self) {
        if self.gamepad_states.is_empty() {
            println!("[GAMEPADS] No gamepads connected");
            return;
        }

        println!(
            "\n[GAMEPADS] {} gamepad(s) connected:",
            self.gamepad_states.len()
        );
        for state in self.gamepad_states.values() {
            state.log_state();
        }
    }
}

// All possible buttons
const ALL_BUTTONS: &[Button] = &[
    Button::South,
    Button::East,
    Button::North,
    Button::West,
    Button::C,
    Button::Z,
    Button::LeftTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::Mode,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];

// All possible axes
const ALL_AXES: &[Axis] = &[
    Axis::LeftStickX,
    Axis::LeftStickY,
    Axis::LeftZ,
    Axis::RightStickX,
    Axis::RightStickY,
    Axis::RightZ,
    Axis::DPadX,
    Axis::DPadY,
];
