use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::manifest::{ColonyAppManifest, parse_manifest};
use crate::scan::{AppCategory, AppOrigin, Application};

const DEFAULT_GITHUB_USER: &str = "MotherSphere";
const PER_PAGE: usize = 100;

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
}

pub async fn scan_github_apps() -> Result<Vec<Application>> {
    let user = load_github_user();
    let repos = fetch_repos(&user).await?;
    let apps = fetch_colony_manifests(&user, repos).await?
        .into_iter()
        .map(|manifest| Application {
            name: manifest.name,
            exec: manifest.repo_url,
            icon: None,
            category: AppCategory::Development,
            origin: AppOrigin::External,
        })
        .collect();
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
            eprintln!(
                "[github] Invalid config {}: {}",
                path.display(),
                error
            );
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
    platforms: Vec<String>,
    release_files: Vec<String>,
    repo_url: String,
}

async fn fetch_colony_manifests(user: &str, repos: Vec<GithubRepo>) -> Result<Vec<ColonyRepoManifest>> {
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
                platforms: manifest.platforms,
                release_files: manifest.release_files,
                repo_url: repo.html_url,
            });
        }
    }

    Ok(manifests)
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
            eprintln!(
                "[github] Invalid colony.json for {}: {}",
                repo.name, error
            );
            Ok(None)
        }
    }
}
