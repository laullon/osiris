#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ControlCommand {
    Navigation(NavigationCommand),
    Action(ActionCommand),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NavigationCommand {
    Up,
    Down,
    Left,
    Right,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionCommand {
    Select,
    Back,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    None,
    SystemChanged(usize),
    GameChanged(usize),
    LaunchGame(usize, usize),
}
