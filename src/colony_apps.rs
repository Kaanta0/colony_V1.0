use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::github;
use crate::manifest::{ColonyManifest, DownloadFile, PlatformSupport};
use crate::scan::{AppCategory, AppOrigin, Application, ColonyAppInfo, ColonyDownload};

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GithubTag {
    name: String,
}

#[derive(Debug)]
struct ReleaseInfo {
    tag: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LocalVersion {
    version: String,
    manifest_version: Option<String>,
}

pub async fn scan_colony_applications() -> Result<Vec<Application>> {
    let client = github::github_client()?;
    let repos = github::scan_mothersphere_colony_software().await?;
    let mut apps = Vec::new();

    for repo in repos {
        match build_app_from_repo(&client, &repo.name).await {
            Ok(Some(app)) => apps.push(app),
            Ok(None) => {}
            Err(error) => {
                eprintln!("[colony_apps] Repo {} ignoré: {error}", repo.name);
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

pub async fn update_and_launch(app: Application) -> Result<()> {
    let colony = app.colony.as_ref().context("Application non Colony")?;
    let exec_path = colony.exec_path.clone();

    ensure_latest(colony).await?;

    launch_exec(exec_path.to_string_lossy().to_string())?;
    Ok(())
}

async fn build_app_from_repo(client: &reqwest::Client, repo: &str) -> Result<Option<Application>> {
    let release = fetch_latest_release(client, repo).await?;
    let manifest = fetch_manifest(client, repo, &release.tag).await?;

    let platform = select_platform(&manifest).context("Aucune plateforme compatible")?;
    let downloads = select_downloads(&manifest, platform.id.as_str(), &release)?;

    let install_dir = install_dir_for_manifest(&manifest.id)?;
    let local_version = load_local_version(&install_dir).ok();
    let latest_version = release.tag.clone();

    let exec_path = install_dir.join(
        downloads
            .first()
            .context("Aucun téléchargement disponible")?
            .path
            .as_path(),
    );

    let app = Application {
        name: manifest.name.clone(),
        exec: exec_path.to_string_lossy().to_string(),
        icon: None,
        category: map_category(&manifest.colony_category),
        origin: AppOrigin::Colony,
        colony: Some(ColonyAppInfo {
            repo: repo.to_string(),
            manifest_id: manifest.id.clone(),
            manifest_version: manifest.version.clone(),
            install_dir,
            exec_path,
            local_version,
            latest_version,
            downloads,
        }),
    };

    Ok(Some(app))
}

async fn fetch_latest_release(client: &reqwest::Client, repo: &str) -> Result<ReleaseInfo> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases",
        github::GITHUB_ORG,
        repo
    );
    let response = client
        .get(&url)
        .send()
        .await
        .context("Impossible de récupérer les releases")?;

    if !response.status().is_success() {
        bail!("GitHub a répondu {}", response.status());
    }

    let releases: Vec<GithubRelease> = response.json().await.context("Réponse release invalide")?;

    if let Some(release) = releases
        .into_iter()
        .find(|release| !release.draft && !release.prerelease)
    {
        return Ok(ReleaseInfo {
            tag: release.tag_name,
            assets: release.assets,
        });
    }

    fetch_latest_tag(client, repo).await
}

async fn fetch_latest_tag(client: &reqwest::Client, repo: &str) -> Result<ReleaseInfo> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/tags?per_page=1",
        github::GITHUB_ORG,
        repo
    );
    let response = client
        .get(&url)
        .send()
        .await
        .context("Impossible de récupérer les tags")?;

    if response.status() == StatusCode::NOT_FOUND {
        bail!("Aucun tag trouvé");
    }
    if !response.status().is_success() {
        bail!("GitHub a répondu {}", response.status());
    }

    let tags: Vec<GithubTag> = response.json().await.context("Réponse tags invalide")?;
    let tag = tags.first().context("Aucun tag disponible")?.name.clone();

    Ok(ReleaseInfo {
        tag,
        assets: Vec::new(),
    })
}

async fn fetch_manifest(client: &reqwest::Client, repo: &str, tag: &str) -> Result<ColonyManifest> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}/colony.json",
        github::GITHUB_ORG,
        repo,
        tag
    );
    let response = client
        .get(&url)
        .send()
        .await
        .context("Impossible de récupérer le manifeste")?;

    if !response.status().is_success() {
        bail!("Manifest introuvable ({})", response.status());
    }

    let manifest: ColonyManifest = response.json().await.context("Manifest invalide")?;
    manifest.validate()?;
    Ok(manifest)
}

fn select_platform(manifest: &ColonyManifest) -> Option<&PlatformSupport> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    manifest.platforms.iter().find(|platform| {
        platform.os.eq_ignore_ascii_case(os)
            && platform
                .arch
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(arch))
                .unwrap_or(true)
    })
}

fn select_downloads(
    manifest: &ColonyManifest,
    platform_id: &str,
    release: &ReleaseInfo,
) -> Result<Vec<ColonyDownload>> {
    let downloads: Vec<ColonyDownload> = manifest
        .downloads
        .iter()
        .filter(|download| {
            download
                .platform
                .as_deref()
                .map(|value| value == platform_id)
                .unwrap_or(true)
        })
        .map(|download| resolve_download(download, release))
        .collect::<Result<Vec<_>>>()?;

    if downloads.is_empty() {
        bail!("Aucun téléchargement compatible");
    }

    Ok(downloads)
}

fn resolve_download(download: &DownloadFile, release: &ReleaseInfo) -> Result<ColonyDownload> {
    let url = if download.url.starts_with("http://") || download.url.starts_with("https://") {
        download.url.clone()
    } else {
        let asset_name = download.url.strip_prefix("asset:").unwrap_or(&download.url);
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .context("Asset GitHub introuvable")?;
        asset.browser_download_url.clone()
    };

    Ok(ColonyDownload {
        url,
        path: PathBuf::from(&download.path),
        sha256: download.sha256.clone(),
        size: download.size,
    })
}

async fn ensure_latest(colony: &ColonyAppInfo) -> Result<()> {
    let current_version = load_local_version(&colony.install_dir).ok();
    if current_version.as_deref() == Some(colony.latest_version.as_str()) {
        return Ok(());
    }

    let client = github::github_client()?;
    download_assets(&client, colony).await?;
    write_local_version(colony)?;
    Ok(())
}

async fn download_assets(client: &reqwest::Client, colony: &ColonyAppInfo) -> Result<()> {
    for download in &colony.downloads {
        let destination = colony.install_dir.join(&download.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Impossible de créer le dossier {}", parent.display()))?;
        }

        let response = client
            .get(&download.url)
            .send()
            .await
            .with_context(|| format!("Téléchargement échoué ({})", download.url))?;

        if !response.status().is_success() {
            bail!(
                "Téléchargement échoué pour {} ({})",
                download.url,
                response.status()
            );
        }

        let bytes = response.bytes().await.context("Lecture binaire échouée")?;
        fs::write(&destination, &bytes)
            .with_context(|| format!("Impossible d'écrire {}", destination.display()))?;
    }

    Ok(())
}

fn load_local_version(install_dir: &Path) -> Result<String> {
    let path = install_dir.join("version.json");
    let data = fs::read_to_string(&path)
        .with_context(|| format!("Impossible de lire {}", path.display()))?;
    let parsed: LocalVersion = serde_json::from_str(&data).context("version.json invalide")?;
    Ok(parsed.version)
}

fn write_local_version(colony: &ColonyAppInfo) -> Result<()> {
    fs::create_dir_all(&colony.install_dir).with_context(|| {
        format!(
            "Impossible de créer le dossier {}",
            colony.install_dir.display()
        )
    })?;
    let payload = LocalVersion {
        version: colony.latest_version.clone(),
        manifest_version: colony.manifest_version.clone(),
    };
    let json = serde_json::to_string_pretty(&payload).context("Impossible de sérialiser")?;
    let path = colony.install_dir.join("version.json");
    fs::write(&path, json).with_context(|| format!("Impossible d'écrire {}", path.display()))?;
    Ok(())
}

fn install_dir_for_manifest(manifest_id: &str) -> Result<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    #[cfg(not(windows))]
    let base = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    let base_path = if cfg!(windows) {
        PathBuf::from(base).join("Colony").join("apps")
    } else {
        PathBuf::from(base)
            .join(".local")
            .join("share")
            .join("colony")
            .join("apps")
    };

    Ok(base_path.join(manifest_id))
}

fn map_category(category: &str) -> AppCategory {
    match category.trim().to_lowercase().as_str() {
        "development" => AppCategory::Development,
        "graphics" => AppCategory::Graphics,
        "network" => AppCategory::Network,
        "office" => AppCategory::Office,
        "multimedia" => AppCategory::Multimedia,
        "system" => AppCategory::System,
        "utility" | "utilities" => AppCategory::Utility,
        "game" | "games" => AppCategory::Game,
        _ => AppCategory::Other,
    }
}

fn launch_exec(exec: String) -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &exec])
            .spawn()
            .map(|_| ())
            .with_context(|| format!("Impossible de lancer {exec}"))?;
    }

    #[cfg(not(windows))]
    {
        match shell_words::split(&exec) {
            Ok(mut parts) => {
                parts.retain(|part| !part.is_empty());
                if let Some((cmd, args)) = parts.split_first() {
                    std::process::Command::new(cmd)
                        .args(args)
                        .spawn()
                        .map(|_| ())
                        .with_context(|| format!("Impossible de lancer {exec}"))?;
                } else {
                    bail!("Commande vide");
                }
            }
            Err(error) => {
                bail!("Impossible de lancer {exec}: {error}");
            }
        }
    }

    Ok(())
}
