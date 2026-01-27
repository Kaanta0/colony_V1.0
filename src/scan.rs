use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use slint::SharedString;

use crate::config::Config;

pub fn scan_repositories(config: &Config) -> Result<Vec<SharedString>> {
    let mut repos = Vec::new();
    for directory in &config.scan.directories {
        let path = Path::new(directory);
        if !path.exists() {
            continue;
        }
        scan_directory(path, 0, config.scan.max_depth, &mut repos)
            .with_context(|| format!("Erreur de scan pour {directory}"))?;
    }

    repos.sort();
    repos.dedup();
    Ok(repos.into_iter().map(SharedString::from).collect())
}

fn scan_directory(
    path: &Path,
    depth: usize,
    max_depth: usize,
    repos: &mut Vec<String>,
) -> Result<()> {
    if depth > max_depth {
        return Ok(());
    }

    for entry in std::fs::read_dir(path).with_context(|| format!("Lecture impossible: {path:?}"))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let entry_path = entry.path();
            if is_git_repo(&entry_path) {
                repos.push(display_path(&entry_path));
            } else {
                scan_directory(&entry_path, depth + 1, max_depth, repos)?;
            }
        }
    }

    Ok(())
}

fn is_git_repo(path: &Path) -> bool {
    path.join(".git").is_dir()
}

fn display_path(path: &PathBuf) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~{}", stripped.display());
        }
    }
    path.display().to_string()
}
