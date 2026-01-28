use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::header::{ETAG, IF_NONE_MATCH};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::manifest::{ColonyAppManifest, parse_manifest};
use crate::metadata::load_local_metadata;
use crate::scan::{AppCategory, AppOrigin, Application, UpdateProposal};

const DEFAULT_GITHUB_USER: &str = "MotherSphere";
const MAX_RATE_LIMIT_RETRIES: usize = 3;
const RATE_LIMIT_BACKOFF_BASE_SECS: u64 = 2;
const RATE_LIMIT_BACKOFF_MAX_SECS: u64 = 60;
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
    let mut etag_cache = GithubEtagCache::load();
    let repos = fetch_repos(&user, &mut etag_cache).await?;
    let local_metadata = load_local_metadata();
    let manifests = fetch_colony_manifests(&user, repos, &mut etag_cache).await?;
    let client = reqwest::Client::builder()
        .user_agent("colony-launcher")
        .build()
        .context("creating GitHub client")?;
    let mut apps = Vec::new();

    for manifest in manifests {
        let readme = fetch_repo_readme(&client, &user, &manifest.repo_name, &mut etag_cache).await;
        let description = readme
            .as_deref()
            .and_then(normalize_description)
            .or_else(|| {
                manifest
                    .description
                    .as_deref()
                    .and_then(normalize_description)
            });
        let language =
            fetch_repo_language(&client, &user, &manifest.repo_name, &mut etag_cache).await;
        let update = build_update_proposal(
            &client,
            &user,
            &manifest,
            local_metadata.get(&manifest.repo_name.to_lowercase()),
            &mut etag_cache,
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
    etag_cache.save()?;
    Ok(apps)
}

pub fn scan_github_apps_with_runtime() -> Result<Vec<Application>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating GitHub scan runtime")?;
    runtime.block_on(scan_github_apps())
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

async fn fetch_repos(user: &str, etag_cache: &mut GithubEtagCache) -> Result<Vec<GithubRepo>> {
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
        let response = get_cached_body(&client, &url, None, etag_cache)
            .await
            .context("requesting GitHub repositories")?;
        let chunk: Vec<GithubRepo> = match response {
            CachedBody::Body(body) => {
                serde_json::from_str(&body).context("decoding GitHub repository list")?
            }
            CachedBody::NotFound => {
                bail!("GitHub API returned a not found status");
            }
        };
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
    etag_cache: &mut GithubEtagCache,
) -> Result<Vec<ColonyRepoManifest>> {
    let client = reqwest::Client::builder()
        .user_agent("colony-launcher")
        .build()
        .context("creating GitHub client")?;
    let mut manifests = Vec::new();

    for repo in repos {
        let manifest = fetch_colony_manifest(&client, user, &repo, etag_cache).await?;
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
    etag_cache: &mut GithubEtagCache,
) -> Option<UpdateProposal> {
    let platform = current_platform();
    if !platform_supported(platform, &manifest.platforms) {
        return None;
    }
    let latest_release =
        fetch_latest_compatible_release(
            client,
            user,
            &manifest.repo_name,
            &manifest.release_files,
            etag_cache,
        )
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
    etag_cache: &mut GithubEtagCache,
) -> Option<CompatibleRelease> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page={}",
        user, repo, PER_PAGE
    );
    let response = get_cached_body(client, &url, None, etag_cache).await.ok()?;
    let releases: Vec<GithubRelease> = match response {
        CachedBody::Body(body) => serde_json::from_str(&body).ok()?,
        CachedBody::NotFound => return None,
    };

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
    etag_cache: &mut GithubEtagCache,
) -> Result<Option<ColonyAppManifest>> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/contents/colony.json",
        user, repo.name
    );
    let response = get_cached_body(
        client,
        &url,
        Some("application/vnd.github.raw"),
        etag_cache,
    )
    .await
    .with_context(|| format!("requesting colony.json for {}", repo.name))?;
    let contents = match response {
        CachedBody::Body(body) => body,
        CachedBody::NotFound => return Ok(None),
    };

    match parse_manifest(&contents) {
        Ok(manifest) => Ok(Some(manifest)),
        Err(error) => {
            eprintln!("[github] Invalid colony.json for {}: {}", repo.name, error);
            Ok(None)
        }
    }
}

async fn fetch_repo_readme(
    client: &reqwest::Client,
    user: &str,
    repo: &str,
    etag_cache: &mut GithubEtagCache,
) -> Option<String> {
    let url = format!("https://api.github.com/repos/{}/{}/readme", user, repo);
    let response = get_cached_body(
        client,
        &url,
        Some("application/vnd.github.raw"),
        etag_cache,
    )
    .await
    .ok()?;
    let contents = match response {
        CachedBody::Body(body) => body,
        CachedBody::NotFound => return None,
    };
    if contents.trim().is_empty() {
        None
    } else {
        Some(contents)
    }
}

async fn fetch_repo_language(
    client: &reqwest::Client,
    user: &str,
    repo: &str,
    etag_cache: &mut GithubEtagCache,
) -> Option<String> {
    let url = format!("https://api.github.com/repos/{}/{}/languages", user, repo);
    let response = get_cached_body(client, &url, None, etag_cache).await.ok()?;
    let languages: HashMap<String, u64> = match response {
        CachedBody::Body(body) => serde_json::from_str(&body).ok()?,
        CachedBody::NotFound => return None,
    };

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

#[derive(Debug, Serialize, Deserialize, Default)]
struct GithubEtagCache {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    etag: String,
    body: String,
}

impl GithubEtagCache {
    fn load() -> Self {
        let path = cache_path();
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => return Self::default(),
        };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    fn save(&self) -> Result<()> {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating cache directory {}", parent.display()))?;
        }
        let contents =
            serde_json::to_string_pretty(self).context("serializing GitHub ETag cache")?;
        fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    fn get(&self, url: &str) -> Option<&CacheEntry> {
        self.entries.get(url)
    }

    fn update(&mut self, url: &str, etag: String, body: String) {
        self.entries.insert(url.to_string(), CacheEntry { etag, body });
    }
}

fn cache_path() -> PathBuf {
    Path::new("config").join("github_etag_cache.json")
}

enum CachedBody {
    Body(String),
    NotFound,
}

async fn get_cached_body(
    client: &reqwest::Client,
    url: &str,
    accept: Option<&str>,
    etag_cache: &mut GithubEtagCache,
) -> Result<CachedBody> {
    for attempt in 0..=MAX_RATE_LIMIT_RETRIES {
        let mut request = client.get(url);
        if let Some(accept) = accept {
            request = request.header("Accept", accept);
        }
        if let Some(entry) = etag_cache.get(url) {
            request = request.header(IF_NONE_MATCH, entry.etag.clone());
        }
        let response = request.send().await.context("sending GitHub request")?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_MODIFIED {
            if let Some(entry) = etag_cache.get(url) {
                return Ok(CachedBody::Body(entry.body.clone()));
            }
            return Err(anyhow!("received 304 without a cached body for {}", url));
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(CachedBody::NotFound);
        }
        let headers = response.headers().clone();
        let rate_limit_remaining = headers
            .get("X-RateLimit-Remaining")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        let rate_limit_reset = headers
            .get("X-RateLimit-Reset")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());

        if matches!(
            status,
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::TOO_MANY_REQUESTS
        ) && rate_limit_remaining == Some(0)
        {
            if attempt < MAX_RATE_LIMIT_RETRIES {
                let backoff =
                    RATE_LIMIT_BACKOFF_BASE_SECS.saturating_mul(1_u64 << attempt);
                let fallback_delay =
                    Duration::from_secs(std::cmp::min(backoff, RATE_LIMIT_BACKOFF_MAX_SECS));
                let delay = rate_limit_reset
                    .and_then(|reset| {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
                        reset.checked_sub(now).map(Duration::from_secs)
                    })
                    .unwrap_or(fallback_delay);
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                continue;
            }
        }

        let etag = headers
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        let body = response
            .text()
            .await
            .with_context(|| format!("reading response body for {}", url))?;
        if !status.is_success() {
            let rate_limit_remaining = rate_limit_remaining
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let rate_limit_reset = rate_limit_reset
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            return Err(anyhow!(
                "GitHub API error for {url}: status={status}, rate_limit_remaining={rate_limit_remaining}, rate_limit_reset={rate_limit_reset}, body={body}"
            ));
        }
        if let Some(etag) = etag {
            etag_cache.update(url, etag, body.clone());
        }
        return Ok(CachedBody::Body(body));
    }

    Err(anyhow!(
        "GitHub API rate limit exceeded for {url} after {} retries",
        MAX_RATE_LIMIT_RETRIES
    ))
}
