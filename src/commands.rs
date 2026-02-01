#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavCommand {
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
}
