use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;

use crate::manifest::{ColonyAppManifest, parse_manifest};
use crate::metadata::load_local_metadata;
use crate::scan::{AppCategory, AppOrigin, Application, UpdateProposal};

const DEFAULT_GITHUB_USER: &str = "MotherSphere";
const PER_PAGE: usize = 100;
const DESCRIPTION_LIMIT: usize = 200;

#[derive(Debug, Deserialize)]
struct ColonyConfig {
    github: Option<GithubConfig>,
}

#[derive(Debug, Deserialize)]
struct GithubConfig {
    user: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRepo {
    name: String,
    html_url: String,
    description: Option<String>,
}

pub async fn scan_github_apps() -> Result<Vec<Application>> {
    let user = load_github_user();
    let repos = fetch_repos(&user).await?;
    let local_metadata = load_local_metadata();
    let manifests = fetch_colony_manifests(&user, repos).await?;
    let client = reqwest::Client::builder()
        .user_agent("colony-launcher")
        .build()
        .context("creating GitHub client")?;
    let mut apps = Vec::new();

    for manifest in manifests {
        let readme = fetch_repo_readme(&client, &user, &manifest.repo_name).await;
        let description = readme
            .as_deref()
            .and_then(normalize_description)
            .or_else(|| {
                manifest
                    .description
                    .as_deref()
                    .and_then(normalize_description)
            });
        let language = fetch_repo_language(&client, &user, &manifest.repo_name).await;
        let update = build_update_proposal(
            &client,
            &user,
            &manifest,
            local_metadata.get(&manifest.repo_name.to_lowercase()),
        )
        .await;
        apps.push(Application {
            name: manifest.name,
            exec: manifest.repo_url,
            icon: None,
            category: AppCategory::Development,
            origin: AppOrigin::External,
            update,
            description,
            language,
        });
    }
    Ok(apps)
}

fn load_github_user() -> String {
    let path = Path::new("config/colony.toml");
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(_) => return DEFAULT_GITHUB_USER.to_string(),
    };
    let config: ColonyConfig = match toml::from_str(&contents) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("[github] Invalid config {}: {}", path.display(), error);
            return DEFAULT_GITHUB_USER.to_string();
        }
    };
    config
        .github
        .and_then(|github| github.user)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_GITHUB_USER.to_string())
}

async fn fetch_repos(user: &str) -> Result<Vec<GithubRepo>> {
    let client = reqwest::Client::builder()
        .user_agent("colony-launcher")
        .build()
        .context("creating GitHub client")?;
    let mut page = 1;
    let mut repos = Vec::new();

    loop {
        let url = format!(
            "https://api.github.com/users/{}/repos?per_page={}&page={}",
            user, PER_PAGE, page
        );
        let response = client
            .get(url)
            .send()
            .await
            .context("requesting GitHub repositories")?
            .error_for_status()
            .context("GitHub API returned an error status")?;
        let chunk: Vec<GithubRepo> = response
            .json()
            .await
            .context("decoding GitHub repository list")?;
        if chunk.is_empty() {
            break;
        }
        let is_last_page = chunk.len() < PER_PAGE;
        repos.extend(chunk);
        if is_last_page {
            break;
        }
        page += 1;
    }

    Ok(repos)
}

#[derive(Debug, Clone)]
struct ColonyRepoManifest {
    name: String,
    repo_name: String,
    platforms: Vec<String>,
    release_files: Vec<String>,
    repo_url: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    #[serde(default)]
    browser_download_url: String,
}

async fn fetch_colony_manifests(
    user: &str,
    repos: Vec<GithubRepo>,
) -> Result<Vec<ColonyRepoManifest>> {
    let client = reqwest::Client::builder()
        .user_agent("colony-launcher")
        .build()
        .context("creating GitHub client")?;
    let mut manifests = Vec::new();

    for repo in repos {
        let manifest = fetch_colony_manifest(&client, user, &repo).await?;
        if let Some(manifest) = manifest {
            manifests.push(ColonyRepoManifest {
                name: manifest.name,
                repo_name: repo.name.clone(),
                platforms: manifest.platforms,
                release_files: manifest.release_files,
                repo_url: repo.html_url,
                description: repo.description,
            });
        }
    }

    Ok(manifests)
}

async fn build_update_proposal(
    client: &reqwest::Client,
    user: &str,
    manifest: &ColonyRepoManifest,
    local_metadata: Option<&crate::metadata::LocalAppMetadata>,
) -> Option<UpdateProposal> {
    let platform = current_platform();
    if !platform_supported(platform, &manifest.platforms) {
        return None;
    }
    let latest_release =
        fetch_latest_compatible_release(client, user, &manifest.repo_name, &manifest.release_files)
            .await?;

    let local_version = local_metadata.map(|metadata| metadata.version.clone());
    let update_available = match local_version.as_deref() {
        Some(version) => is_remote_newer(version, &latest_release.tag),
        None => false,
    };

    Some(UpdateProposal {
        local_version,
        latest_version: latest_release.tag,
        update_available,
        platform: platform.to_string(),
    })
}

fn platform_supported(platform: &str, platforms: &[String]) -> bool {
    platforms.iter().any(|value| {
        let value = value.trim().to_lowercase();
        if platform == "windows" {
            matches!(value.as_str(), "windows" | "win")
        } else {
            matches!(value.as_str(), "linux" | "unix")
        }
    })
}

fn current_platform() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}

fn is_remote_newer(local: &str, remote: &str) -> bool {
    let local_version = parse_version(local);
    let remote_version = parse_version(remote);
    match (local_version, remote_version) {
        (Some(local), Some(remote)) => remote > local,
        _ => false,
    }
}

fn parse_version(tag: &str) -> Option<Version> {
    let trimmed = tag.trim().trim_start_matches('v');
    Version::parse(trimmed).ok()
}

struct CompatibleRelease {
    tag: String,
    assets: Vec<GithubReleaseAsset>,
}

async fn fetch_latest_compatible_release(
    client: &reqwest::Client,
    user: &str,
    repo: &str,
    required_files: &[String],
) -> Option<CompatibleRelease> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page={}",
        user, repo, PER_PAGE
    );
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let releases: Vec<GithubRelease> = response.json().await.ok()?;

    let mut best: Option<(Version, CompatibleRelease)> = None;

    for release in releases {
        let version = match parse_version(&release.tag_name) {
            Some(version) => version,
            None => continue,
        };
        if !release_has_required_assets(&release.assets, required_files) {
            continue;
        }

        let candidate = CompatibleRelease {
            tag: release.tag_name,
            assets: release.assets,
        };

        match &best {
            Some((best_version, _)) if &version <= best_version => {}
            _ => best = Some((version, candidate)),
        }
    }

    best.map(|(_, candidate)| candidate)
}

fn release_has_required_assets(assets: &[GithubReleaseAsset], required_files: &[String]) -> bool {
    if required_files.is_empty() {
        return false;
    }

    required_files.iter().all(|required| {
        let required = required.trim().to_lowercase();
        assets
            .iter()
            .any(|asset| asset.name.to_lowercase() == required)
    })
}

async fn fetch_colony_manifest(
    client: &reqwest::Client,
    user: &str,
    repo: &GithubRepo,
) -> Result<Option<ColonyAppManifest>> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/contents/colony.json",
        user, repo.name
    );
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github.raw")
        .send()
        .await
        .with_context(|| format!("requesting colony.json for {}", repo.name))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let response = response
        .error_for_status()
        .with_context(|| format!("GitHub API returned an error status for {}", repo.name))?;
    let contents = response
        .text()
        .await
        .with_context(|| format!("reading colony.json for {}", repo.name))?;

    match parse_manifest(&contents) {
        Ok(manifest) => Ok(Some(manifest)),
        Err(error) => {
            eprintln!("[github] Invalid colony.json for {}: {}", repo.name, error);
            Ok(None)
        }
    }
}

async fn fetch_repo_readme(client: &reqwest::Client, user: &str, repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{}/{}/readme", user, repo);
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github.raw")
        .send()
        .await
        .ok()?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return None;
    }

    let response = response.error_for_status().ok()?;
    let contents = response.text().await.ok()?;
    if contents.trim().is_empty() {
        None
    } else {
        Some(contents)
    }
}

async fn fetch_repo_language(client: &reqwest::Client, user: &str, repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{}/{}/languages", user, repo);
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let languages: std::collections::HashMap<String, u64> = response.json().await.ok()?;

    languages
        .into_iter()
        .max_by_key(|(_, bytes)| *bytes)
        .map(|(language, _)| language)
}

fn normalize_description(value: &str) -> Option<String> {
    let cleaned = value
        .split_whitespace()
        .filter(|chunk| !chunk.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        None
    } else {
        Some(truncate_text(&cleaned, DESCRIPTION_LIMIT))
    }
}

fn truncate_text(value: &str, max_len: usize) -> String {
    let length = value.chars().count();
    if length <= max_len {
        return value.to_string();
    }
    if max_len <= 1 {
        return "…".to_string();
    }
    let mut truncated = value.chars().take(max_len - 1).collect::<String>();
    truncated.push('…');
    truncated
}
