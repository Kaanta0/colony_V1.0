use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct Repository {
    pub name: String,
    pub path: String,
    pub display_path: String,
}

pub fn scan_repositories(config: &Config) -> Result<Vec<Repository>> {
    let mut repos = Vec::new();
    for directory in &config.scan.directories {
        let path = Path::new(directory);
        if !path.exists() {
            continue;
        }
        scan_directory(path, 0, config.scan.max_depth, &mut repos)
            .with_context(|| format!("Scan error for {directory}"))?;
    }

    repos.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    repos.dedup_by(|a, b| a.path == b.path);
    Ok(repos)
}

fn scan_directory(
    path: &Path,
    depth: usize,
    max_depth: usize,
    repos: &mut Vec<Repository>,
) -> Result<()> {
    if depth > max_depth {
        return Ok(());
    }

    for entry in std::fs::read_dir(path).with_context(|| format!("Cannot read: {path:?}"))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let entry_path = entry.path();
            if is_git_repo(&entry_path) {
                let name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                repos.push(Repository {
                    name,
                    path: entry_path.display().to_string(),
                    display_path: display_path(&entry_path),
                });
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
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}
