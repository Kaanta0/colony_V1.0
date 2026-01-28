use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ColonyManifest {
    pub name: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default, alias = "releaseFiles")]
    pub release_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ColonyAppManifest {
    pub name: String,
    pub platforms: Vec<String>,
    pub release_files: Vec<String>,
}

pub fn parse_manifest(contents: &str) -> Result<ColonyAppManifest> {
    let manifest: ColonyManifest =
        serde_json::from_str(contents).context("decoding colony.json manifest")?;
    manifest
        .into_app_manifest()
        .context("colony.json missing required fields")
}

impl ColonyManifest {
    fn into_app_manifest(self) -> Option<ColonyAppManifest> {
        let name = self.name.trim();
        if name.is_empty() {
            return None;
        }

        let platforms: Vec<String> = self
            .platforms
            .into_iter()
            .map(|platform| platform.trim().to_string())
            .filter(|platform| !platform.is_empty())
            .collect();
        let release_files: Vec<String> = self
            .release_files
            .into_iter()
            .map(|file| file.trim().to_string())
            .filter(|file| !file.is_empty())
            .collect();

        if platforms.is_empty() || release_files.is_empty() {
            return None;
        }

        Some(ColonyAppManifest {
            name: name.to_string(),
            platforms,
            release_files,
        })
    }
}
