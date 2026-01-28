use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalMetadataStore {
    #[serde(default)]
    pub apps: Vec<LocalAppMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAppMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

pub fn load_local_metadata() -> HashMap<String, LocalAppMetadata> {
    let path = metadata_path();
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(_) => return HashMap::new(),
    };

    let store: LocalMetadataStore = match serde_json::from_str(&contents) {
        Ok(store) => store,
        Err(error) => {
            eprintln!(
                "[metadata] Invalid metadata store {}: {}",
                path.display(),
                error
            );
            return HashMap::new();
        }
    };

    store
        .apps
        .into_iter()
        .map(|app| (app.name.to_lowercase(), app))
        .collect()
}

pub fn save_local_metadata(store: &LocalMetadataStore) -> Result<()> {
    let path = metadata_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating metadata directory {}", parent.display()))?;
    }
    let contents = serde_json::to_string_pretty(store).context("encoding metadata store")?;
    fs::write(&path, contents)
        .with_context(|| format!("writing metadata store {}", path.display()))?;
    Ok(())
}

fn metadata_path() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("Colony").join("metadata.json");
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("colony")
                .join("metadata.json");
        }
    }

    PathBuf::from("metadata.json")
}
