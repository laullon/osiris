use crate::models::{Game, RomLibrary, System};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn scan_roms<P: AsRef<Path>>(root_path: P) -> RomLibrary {
    let mut library = RomLibrary::new();
    let root = root_path.as_ref();

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let system_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("UNKNOWN")
                .to_uppercase();

            let games = if system_name == "MAME" {
                parse_mame_metadata(&path)
            } else {
                scan_generic_dir(&path)
            };

            if !games.is_empty() {
                library.systems.push(System {
                    name: system_name,
                    games,
                });
            }
        }
    }

    library.systems.sort_by(|a, b| a.name.cmp(&b.name));
    library
}

fn scan_generic_dir(path: &Path) -> Vec<Game> {
    let mut games = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && !p.file_name().unwrap().to_str().unwrap().starts_with('.') {
                let id = p.file_stem().unwrap().to_str().unwrap().to_string();
                games.push(Game {
                    id: id.clone(),
                    name: id,
                    path: p,
                    year: "UNKNOWN".into(),
                    manufacturer: "UNKNOWN".into(),
                    players: "1".into(),
                });
            }
        }
    }
    games
}

fn parse_mame_metadata(mame_path: &Path) -> Vec<Game> {
    println!("📡 OSIRIS: INTERROGATING MAME SUBSYSTEM...");

    // 1. Run "mame -listxml"
    let output = Command::new("mame").arg("-listxml").output();

    let xml_data = match output {
        Ok(out) => out.stdout,
        Err(_) => {
            println!("⚠️ OSIRIS: MAME BINARY NOT FOUND IN PATH. FALLING BACK TO FILE SCAN.");
            return scan_generic_dir(mame_path);
        }
    };

    let mut reader = Reader::from_reader(xml_data.as_slice());
    reader.trim_text(true);

    let mut games = Vec::new();
    let mut buf = Vec::new();

    let mut current_game: Option<Game> = None;
    let mut current_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if tag == "machine" {
                    let mut id = String::new();
                    let mut runnable = true;

                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"name" => {
                                id = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                            }
                            b"runnable" => {
                                if attr.value.as_ref() == b"no" {
                                    runnable = false;
                                }
                            }
                            _ => {}
                        }
                    }

                    if runnable && !id.is_empty() {
                        current_game = Some(Game {
                            id: id.clone(),
                            name: id.clone(),
                            // We assume ROMs are in the mame dir with .zip extension
                            path: mame_path.join(format!("{}.zip", id)),
                            year: "".into(),
                            manufacturer: "".into(),
                            players: "1".into(),
                        });
                    }
                } else if tag == "input" {
                    if let Some(g) = &mut current_game {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"players" {
                                g.players =
                                    String::from_utf8_lossy(attr.value.as_ref()).to_string();
                            }
                        }
                    }
                }
                current_tag = tag;
            }
            Ok(Event::Text(e)) => {
                if let Some(g) = &mut current_game {
                    // Correct unescape logic for quick-xml 0.31
                    let val = e
                        .unescape()
                        .map(|c| c.into_owned())
                        .unwrap_or_else(|_| "".into());
                    match current_tag.as_str() {
                        "description" => g.name = val,
                        "year" => g.year = val,
                        "manufacturer" => g.manufacturer = val,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"machine" {
                    if let Some(g) = current_game.take() {
                        // CRITICAL: Only add the game if the actual ROM file exists on disk
                        if g.path.exists() {
                            games.push(g);
                        }
                    }
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                println!("⚠️ OSIRIS: XML PARSE ERROR: {}", e);
                break;
            }
            _ => (),
        }
        buf.clear();
    }

    println!(
        "✅ OSIRIS: MAME SCAN COMPLETE. {} MODULES VERIFIED.",
        games.len()
    );

    // Sort MAME games by their clean description/name
    games.sort_by(|a, b| a.name.cmp(&b.name));
    games
}
