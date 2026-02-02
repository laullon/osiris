#[derive(Debug, Clone)]

pub struct Game {
    pub id: String,   // short name (e.g., "atetris")
    pub name: String, // long description (e.g., "Tetris (set 1)")
    pub path: std::path::PathBuf,
    pub year: String,
    pub manufacturer: String,
    pub players: String,
}

#[derive(Debug, Clone)]
pub struct System {
    pub name: String,
    pub games: Vec<Game>,
}

pub struct RomLibrary {
    pub systems: Vec<System>,
}

impl RomLibrary {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }
}
