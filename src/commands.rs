#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NavCommand {
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
    SelectItem(String),
}
