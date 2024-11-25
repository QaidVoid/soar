use std::env;

use anyhow::{Context, Result};
use regex::Regex;
use reqwest::{
    header::{HeaderMap, AUTHORIZATION, USER_AGENT},
    Response,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, trace};

use crate::{
    core::{
        color::{Color, ColorExt},
        util::{interactive_ask, AskType},
    },
    misc::download::download,
};

use super::{should_fallback, ApiType};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GitlabAsset {
    name: String,
    direct_asset_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GitlabAssets {
    links: Vec<GitlabAsset>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GitlabRelease {
    tag_name: String,
    upcoming_release: bool,
    released_at: String,
    assets: GitlabAssets,
}

pub static GITLAB_URL_REGEX: &str =
    r"^(?i)(?:https?://)?(?:gitlab(?:\.com)?[:/])([^/@]+/[^/@]+)(?:@([^/\s]*)?)?$";

async fn call_gitlab_api(gh_api: &ApiType, user_repo: &str) -> Result<Response> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/api/v4/projects/{}/releases",
        match gh_api {
            ApiType::PkgForge => "https://api.gl.pkgforge.dev",
            ApiType::Primary => "https://gitlab.com",
        },
        user_repo.replace("/", "%2F")
    );
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, "pkgforge/soar".parse()?);
    if matches!(gh_api, ApiType::Primary) {
        if let Ok(token) = env::var("GITLAB_TOKEN") {
            trace!("Using Gitlab token: {}", token);
            headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse()?);
        }
    }
    client
        .get(&url)
        .headers(headers)
        .send()
        .await
        .context("Failed to fetch Gitlab releases")
}

async fn fetch_gitlab_releases(gh_api: &ApiType, user_repo: &str) -> Result<Vec<GitlabRelease>> {
    let response = match call_gitlab_api(gh_api, user_repo).await {
        Ok(resp) => {
            let status = resp.status();
            if should_fallback(status) && matches!(gh_api, ApiType::PkgForge) {
                debug!("Failed to fetch Gitlab asset using pkgforge API. Retrying request using Gitlab API.");
                call_gitlab_api(&ApiType::Primary, user_repo).await?
            } else {
                resp
            }
        }
        Err(e) => return Err(e),
    };

    if !response.status().is_success() {
        anyhow::bail!(
            "Error fetching releases for {}: {}",
            user_repo,
            response.status()
        );
    }

    let releases: Vec<GitlabRelease> = response
        .json()
        .await
        .context("Failed to parse Gitlab response")?;

    Ok(releases)
}

fn select_asset_idx(assets: &[&GitlabAsset], max: usize) -> Result<usize> {
    for (i, asset) in assets.iter().enumerate() {
        info!(" [{}] {}", i + 1, asset.name);
    }
    let selection = loop {
        let response = interactive_ask(
            &format!("Select an asset (1-{}): ", assets.len()),
            AskType::Normal,
        )?;

        match response.parse::<usize>() {
            Ok(n) if n > 0 && n <= max => break n - 1,
            _ => error!("Invalid selection, please try again."),
        }
    };
    Ok(selection)
}

pub async fn handle_gitlab_download(
    re: &Regex,
    link: &str,
    output: Option<String>,
    match_keywords: Option<&[String]>,
    exclude_keywords: Option<&[String]>,
    asset_regexes: &Vec<Regex>,
    yes: bool,
) -> Result<()> {
    if let Some(caps) = re.captures(link) {
        let user_repo = caps.get(1).unwrap().as_str();
        let tag = caps
            .get(2)
            .map(|tag| tag.as_str().trim())
            .filter(|&tag| !tag.is_empty());
        info!("Fetching releases for {}...", user_repo);

        let releases = fetch_gitlab_releases(&ApiType::PkgForge, user_repo).await?;

        let release = if let Some(tag_name) = tag {
            releases
                .iter()
                .find(|release| release.tag_name.starts_with(tag_name))
        } else {
            releases.iter().find(|release| !release.upcoming_release)
        };

        let Some(release) = release else {
            error!(
                "No {} found for repository {}",
                tag.map(|t| format!("tag {}", t))
                    .unwrap_or("stable release".to_owned()),
                user_repo
            );
            return Ok(());
        };

        let assets = &release.assets.links;

        if assets.is_empty() {
            error!("No assets found for the release.");
            return Ok(());
        }

        let selected_asset = {
            let assets: Vec<&GitlabAsset> = assets
                .iter()
                .filter(|asset| {
                    asset_regexes
                        .iter()
                        .all(|regex| regex.is_match(&asset.name))
                        && match_keywords.map_or(true, |keywords| {
                            keywords.iter().all(|keyword| {
                                keyword
                                    .split(',')
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .all(|part| {
                                        asset.name.to_lowercase().contains(&part.to_lowercase())
                                    })
                            })
                        })
                        && exclude_keywords.map_or(true, |keywords| {
                            keywords.iter().all(|keyword| {
                                keyword
                                    .split(',')
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .all(|part| {
                                        !asset.name.to_lowercase().contains(&part.to_lowercase())
                                    })
                            })
                        })
                })
                .collect();

            match assets.len() {
                0 => {
                    error!("No assets matched the provided criteria.");
                    return Ok(());
                }
                1 => assets[0],
                _ => {
                    if yes {
                        assets[0]
                    } else {
                        info!(
                            "Multiple matching assets found for {}{}",
                            release.tag_name,
                            if release.upcoming_release {
                                " [prerelease]".color(Color::BrightRed)
                            } else {
                                " [stable]".color(Color::BrightCyan)
                            }
                        );

                        let asset_idx = select_asset_idx(&assets, assets.len())?;
                        assets[asset_idx]
                    }
                }
            }
        };

        let download_url = &selected_asset.direct_asset_url;
        download(download_url, output.clone()).await?;
    }
    Ok(())
}
