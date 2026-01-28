use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct ColonyManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    pub colony_category: String,
    pub platforms: Vec<PlatformSupport>,
    pub downloads: Vec<DownloadFile>,
}

#[derive(Debug, Deserialize)]
pub struct PlatformSupport {
    pub id: String,
    pub os: String,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub min_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadFile {
    pub url: String,
    pub path: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub platform: Option<String>,
}

impl ColonyManifest {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Impossible de lire {}", path.display()))?;
        let manifest: Self = serde_json::from_str(&contents)
            .with_context(|| format!("Manifest JSON invalide: {}", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn load_default() -> Result<Self> {
        Self::load_from_path(Path::new("colony.json"))
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("Le champ 'id' est requis");
        }
        if self.name.trim().is_empty() {
            bail!("Le champ 'name' est requis");
        }
        if self.colony_category.trim().is_empty() {
            bail!("Le champ 'colony_category' est requis");
        }
        if self.platforms.is_empty() {
            bail!("Au moins une plateforme compatible est requise");
        }
        if self.downloads.is_empty() {
            bail!("Au moins un fichier à télécharger est requis");
        }

        let mut platform_ids = HashSet::new();
        for platform in &self.platforms {
            platform.validate()?;
            platform_ids.insert(platform.id.as_str());
        }

        for download in &self.downloads {
            download.validate()?;
            if let Some(platform_id) = download.platform.as_deref() {
                if !platform_ids.contains(platform_id) {
                    bail!(
                        "La plateforme '{}' référencée dans les téléchargements est inconnue",
                        platform_id
                    );
                }
            }
        }

        Ok(())
    }
}

impl PlatformSupport {
    fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("Le champ 'platforms[].id' est requis");
        }
        if self.os.trim().is_empty() {
            bail!("Le champ 'platforms[].os' est requis");
        }
        Ok(())
    }
}

impl DownloadFile {
    fn validate(&self) -> Result<()> {
        if self.url.trim().is_empty() {
            bail!("Le champ 'downloads[].url' est requis");
        }
        if self.path.trim().is_empty() {
            bail!("Le champ 'downloads[].path' est requis");
        }
        Ok(())
    }
}
