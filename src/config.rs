use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub scan: ScanConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScanConfig {
    pub directories: Vec<String>,
    pub max_depth: usize,
    pub interval_seconds: u64,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Impossible de lire le fichier de configuration {path:?}"))?;
        let mut config: Config = toml::from_str(&raw).context("Configuration TOML invalide")?;
        config.scan.directories = config.scan.directories.iter().map(expand_tilde).collect();
        Ok(config)
    }
}

fn config_path() -> PathBuf {
    std::env::var("COLONY_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/colony.toml"))
}

fn expand_tilde(path: &String) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{stripped}");
        }
    }
    path.clone()
}
