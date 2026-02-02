#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NavCommand {
    Up,
    Down,
    Left,
    Right,
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
