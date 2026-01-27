use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Application {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub category: AppCategory,
    pub desktop_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppCategory {
    Development,
    Graphics,
    Network,
    Office,
    Multimedia,
    System,
    Utility,
    Game,
    Other,
}

impl AppCategory {
    fn from_categories(categories: &str) -> Self {
        let cats: Vec<&str> = categories.split(';').collect();

        if cats.iter().any(|c| matches!(*c, "Development" | "IDE")) {
            AppCategory::Development
        } else if cats.iter().any(|c| matches!(*c, "Graphics" | "Photography" | "2DGraphics" | "3DGraphics")) {
            AppCategory::Graphics
        } else if cats.iter().any(|c| matches!(*c, "Network" | "WebBrowser" | "Email" | "Chat")) {
            AppCategory::Network
        } else if cats.iter().any(|c| matches!(*c, "Office" | "WordProcessor" | "Spreadsheet")) {
            AppCategory::Office
        } else if cats.iter().any(|c| matches!(*c, "AudioVideo" | "Audio" | "Video" | "Player")) {
            AppCategory::Multimedia
        } else if cats.iter().any(|c| matches!(*c, "System" | "Settings" | "Monitor")) {
            AppCategory::System
        } else if cats.iter().any(|c| matches!(*c, "Utility" | "FileManager" | "Archiving")) {
            AppCategory::Utility
        } else if cats.iter().any(|c| matches!(*c, "Game")) {
            AppCategory::Game
        } else {
            AppCategory::Other
        }
    }
}

pub fn scan_applications() -> Result<Vec<Application>> {
    let mut apps = Vec::new();
    let mut seen_names: HashMap<String, bool> = HashMap::new();

    let search_dirs = get_application_dirs();

    for dir in &search_dirs {
        if dir.exists() {
            eprintln!("[scan] Scanning: {:?}", dir);
            match scan_directory(dir, &mut apps, &mut seen_names) {
                Ok(_) => eprintln!("[scan] Found {} apps so far", apps.len()),
                Err(e) => eprintln!("[scan] Error scanning {:?}: {}", dir, e),
            }
        } else {
            eprintln!("[scan] Directory does not exist: {:?}", dir);
        }
    }

    eprintln!("[scan] Total applications found: {}", apps.len());
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

fn get_application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User applications
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(format!("{}/.local/share/applications", home)));
    }

    // XDG data dirs
    if let Ok(xdg_data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in xdg_data_dirs.split(':') {
            dirs.push(PathBuf::from(format!("{}/applications", dir)));
        }
    } else {
        // Default locations
        dirs.push(PathBuf::from("/usr/share/applications"));
        dirs.push(PathBuf::from("/usr/local/share/applications"));
    }

    // Flatpak applications
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(format!(
            "{}/.local/share/flatpak/exports/share/applications",
            home
        )));
    }

    // Snap applications
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    dirs
}

fn scan_directory(
    dir: &Path,
    apps: &mut Vec<Application>,
    seen: &mut HashMap<String, bool>,
) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("Cannot read: {:?}", dir))?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "desktop") {
            if let Ok(app) = parse_desktop_file(&path) {
                // Skip duplicates by name
                if !seen.contains_key(&app.name) {
                    seen.insert(app.name.clone(), true);
                    apps.push(app);
                }
            }
        }
    }

    Ok(())
}

fn parse_desktop_file(path: &Path) -> Result<Application> {
    let content = fs::read_to_string(path)?;

    let mut name = None;
    let mut exec = None;
    let mut icon = None;
    let mut categories = String::new();
    let mut no_display = false;
    let mut hidden = false;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "Name" if name.is_none() => name = Some(value.to_string()),
                "Exec" => exec = Some(clean_exec(value)),
                "Icon" => icon = Some(value.to_string()),
                "Categories" => categories = value.to_string(),
                "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
                "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
                _ => {}
            }
        }
    }

    // Skip hidden/nodisplay apps
    if no_display || hidden {
        anyhow::bail!("Application is hidden");
    }

    let name = name.ok_or_else(|| anyhow::anyhow!("No name found"))?;
    let exec = exec.ok_or_else(|| anyhow::anyhow!("No exec found"))?;

    Ok(Application {
        name,
        exec,
        icon,
        category: AppCategory::from_categories(&categories),
        desktop_file: path.to_path_buf(),
    })
}

fn clean_exec(exec: &str) -> String {
    // Remove field codes like %f, %F, %u, %U, etc.
    let mut result = String::new();
    let mut chars = exec.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Skip the field code character
            chars.next();
        } else {
            result.push(c);
        }
    }

    result.trim().to_string()
}
