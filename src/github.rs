use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

const GITHUB_ORG: &str = "MotherSphere";
const CACHE_PATH: &str = "config/cache/mothersphere.json";
const CACHE_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Deserialize)]
struct GithubRepo {
    name: String,
    html_url: String,
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonySoftware {
    pub name: String,
    pub html_url: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    fetched_at: u64,
    items: Vec<ColonySoftware>,
}

pub async fn scan_mothersphere_colony_software() -> Result<Vec<ColonySoftware>> {
    if let Some(cached) = load_cache()? {
        return Ok(cached);
    }

    let client = github_client()?;
    let repos = fetch_repos(&client).await?;

    let mut colonies = Vec::new();
    for repo in repos {
        if has_colony_manifest(&client, &repo.name).await? {
            colonies.push(ColonySoftware {
                name: repo.name,
                html_url: repo.html_url,
                description: repo.description,
            });
        }
    }

    colonies.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    write_cache(&colonies)?;
    Ok(colonies)
}

fn github_client() -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("colony-launcher"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        let value = format!("Bearer {token}");
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&value).context("Invalid GitHub token")?,
        );
    }

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("Unable to build GitHub client")
}

async fn fetch_repos(client: &reqwest::Client) -> Result<Vec<GithubRepo>> {
    let mut repos = Vec::new();
    let mut page = 1;

    loop {
        let url = format!(
            "https://api.github.com/orgs/{}/repos?per_page=100&page={}",
            GITHUB_ORG, page
        );
        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to query GitHub repositories")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "GitHub API returned {} while listing repos",
                response.status()
            ));
        }

        let page_repos: Vec<GithubRepo> = response
            .json()
            .await
            .context("Failed to decode GitHub repositories")?;

        if page_repos.is_empty() {
            break;
        }

        repos.extend(page_repos);
        page += 1;
    }

    Ok(repos)
}

async fn has_colony_manifest(client: &reqwest::Client, repo: &str) -> Result<bool> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/contents/colony.json",
        GITHUB_ORG, repo
    );

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to query colony.json")?;

    match response.status() {
        StatusCode::OK => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        status => Err(anyhow::anyhow!(
            "Unexpected status {} while checking colony.json",
            status
        )),
    }
}

fn load_cache() -> Result<Option<Vec<ColonySoftware>>> {
    let path = Path::new(CACHE_PATH);
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None)
        }
        Err(error) => return Err(error).context("Failed to read GitHub cache"),
    };

    let cached: CacheData = serde_json::from_str(&content).context("Invalid cache data")?;

    let fetched_at = UNIX_EPOCH
        .checked_add(Duration::from_secs(cached.fetched_at))
        .unwrap_or(UNIX_EPOCH);
    let age = SystemTime::now()
        .duration_since(fetched_at)
        .unwrap_or(Duration::from_secs(0));

    if age <= CACHE_TTL {
        Ok(Some(cached.items))
    } else {
        Ok(None)
    }
}

fn write_cache(items: &[ColonySoftware]) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();
    let cache = CacheData {
        fetched_at: now,
        items: items.to_vec(),
    };

    let path = Path::new(CACHE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache dir {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(&cache).context("Failed to serialize cache")?;
    fs::write(path, json).context("Failed to write GitHub cache")
}
