use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Application {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub category: AppCategory,
    pub origin: AppOrigin,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppOrigin {
    Windows,
    Colony,
    External,
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

#[cfg(windows)]
fn get_application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Common Start Menu (all users)
    if let Ok(programdata) = std::env::var("ProgramData") {
        dirs.push(PathBuf::from(format!(
            "{}\\Microsoft\\Windows\\Start Menu\\Programs",
            programdata
        )));
    }

    // User Start Menu
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(PathBuf::from(format!(
            "{}\\Microsoft\\Windows\\Start Menu\\Programs",
            appdata
        )));
    }

    dirs
}

#[cfg(not(windows))]
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
        dirs.push(PathBuf::from("/usr/share/applications"));
        dirs.push(PathBuf::from("/usr/local/share/applications"));
    }

    // Flatpak
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(format!(
            "{}/.local/share/flatpak/exports/share/applications",
            home
        )));
    }

    // Snap
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    dirs
}

fn scan_directory(
    dir: &Path,
    apps: &mut Vec<Application>,
    seen: &mut HashMap<String, bool>,
) -> Result<()> {
    scan_directory_recursive(dir, apps, seen, 0)
}

fn scan_directory_recursive(
    dir: &Path,
    apps: &mut Vec<Application>,
    seen: &mut HashMap<String, bool>,
    depth: usize,
) -> Result<()> {
    if depth > 3 {
        return Ok(());
    }

    let entries = fs::read_dir(dir)?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // Recurse into subdirectories (for Start Menu folders)
            let _ = scan_directory_recursive(&path, apps, seen, depth + 1);
        } else if let Some(app) = parse_application_file(&path) {
            if !seen.contains_key(&app.name) {
                seen.insert(app.name.clone(), true);
                apps.push(app);
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
fn parse_application_file(path: &Path) -> Option<Application> {
    let ext = path.extension()?.to_str()?;

    if ext.eq_ignore_ascii_case("lnk") {
        parse_lnk_file(path)
    } else if ext.eq_ignore_ascii_case("exe") {
        parse_exe_file(path)
    } else {
        None
    }
}

#[cfg(windows)]
fn parse_lnk_file(path: &Path) -> Option<Application> {
    // Get the name from the filename (without .lnk extension)
    let name = path.file_stem()?.to_str()?.to_string();

    // Skip certain system entries
    let lower = name.to_lowercase();
    if lower.contains("uninstall")
        || lower.contains("readme")
        || lower.contains("help")
        || lower.contains("website")
        || lower.contains("manual")
        || lower.contains("license")
    {
        return None;
    }

    // Use the .lnk file path directly - Windows can execute it
    let exec = path.to_str()?.to_string();

    let category = categorize_windows_app(&name, &exec);

    Some(Application {
        name,
        exec,
        icon: None,
        category,
        origin: AppOrigin::Windows,
    })
}

#[cfg(windows)]
fn parse_exe_file(path: &Path) -> Option<Application> {
    let name = path.file_stem()?.to_str()?.to_string();
    let exec = path.to_str()?.to_string();

    Some(Application {
        name,
        exec,
        icon: None,
        category: AppCategory::Other,
        origin: AppOrigin::Windows,
    })
}

#[cfg(windows)]
fn categorize_windows_app(name: &str, exec: &str) -> AppCategory {
    let lower_name = name.to_lowercase();
    let lower_exec = exec.to_lowercase();

    if lower_name.contains("code") || lower_name.contains("studio")
        || lower_name.contains("developer") || lower_exec.contains("ide")
        || lower_name.contains("python") || lower_name.contains("node")
        || lower_name.contains("git") || lower_name.contains("terminal")
    {
        AppCategory::Development
    } else if lower_name.contains("photoshop") || lower_name.contains("gimp")
        || lower_name.contains("paint") || lower_name.contains("photo")
        || lower_name.contains("image") || lower_name.contains("draw")
    {
        AppCategory::Graphics
    } else if lower_name.contains("chrome") || lower_name.contains("firefox")
        || lower_name.contains("edge") || lower_name.contains("browser")
        || lower_name.contains("mail") || lower_name.contains("outlook")
        || lower_name.contains("teams") || lower_name.contains("slack")
        || lower_name.contains("discord") || lower_name.contains("zoom")
    {
        AppCategory::Network
    } else if lower_name.contains("word") || lower_name.contains("excel")
        || lower_name.contains("powerpoint") || lower_name.contains("office")
        || lower_name.contains("libre") || lower_name.contains("calc")
        || lower_name.contains("writer") || lower_name.contains("document")
    {
        AppCategory::Office
    } else if lower_name.contains("spotify") || lower_name.contains("vlc")
        || lower_name.contains("media") || lower_name.contains("player")
        || lower_name.contains("music") || lower_name.contains("video")
        || lower_name.contains("audio")
    {
        AppCategory::Multimedia
    } else if lower_name.contains("settings") || lower_name.contains("control")
        || lower_name.contains("system") || lower_name.contains("config")
        || lower_name.contains("manager") || lower_name.contains("monitor")
    {
        AppCategory::System
    } else if lower_name.contains("notepad") || lower_name.contains("calculator")
        || lower_name.contains("util") || lower_name.contains("tool")
        || lower_name.contains("7-zip") || lower_name.contains("winrar")
    {
        AppCategory::Utility
    } else if lower_name.contains("game") || lower_name.contains("steam")
        || lower_name.contains("epic") || lower_name.contains("play")
        || lower_exec.contains("game")
    {
        AppCategory::Game
    } else {
        AppCategory::Other
    }
}

#[cfg(not(windows))]
fn parse_application_file(path: &Path) -> Option<Application> {
    let ext = path.extension()?.to_str()?;

    if ext == "desktop" {
        parse_desktop_file(path).ok()
    } else {
        None
    }
}

#[cfg(not(windows))]
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

    if no_display || hidden {
        anyhow::bail!("Application is hidden");
    }

    let name = name.ok_or_else(|| anyhow::anyhow!("No name found"))?;
    let exec = exec.ok_or_else(|| anyhow::anyhow!("No exec found"))?;

    Ok(Application {
        name,
        exec,
        icon,
        category: categorize_linux_app(&categories),
        origin: AppOrigin::Colony,
    })
}

#[cfg(not(windows))]
fn categorize_linux_app(categories: &str) -> AppCategory {
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

#[cfg(not(windows))]
fn clean_exec(exec: &str) -> String {
    let mut result = String::new();
    let mut chars = exec.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            chars.next();
        } else {
            result.push(c);
        }
    }

    let trimmed = result.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    match shell_words::split(trimmed) {
        Ok(parts) => {
            let filtered: Vec<String> = parts.into_iter().filter(|part| !part.is_empty()).collect();
            if filtered.is_empty() {
                return String::new();
            }
            shell_words::join(&filtered)
        }
        Err(_) => trimmed.to_string(),
    }
}
