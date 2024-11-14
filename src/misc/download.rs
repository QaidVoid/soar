use std::{env, fs::Permissions, os::unix::fs::PermissionsExt, path::Path};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::StreamExt;
use indicatif::ProgressBar;
use regex::Regex;
use reqwest::{
    header::{HeaderMap, AUTHORIZATION, USER_AGENT},
    Url,
};
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
};
use tracing::{error, info, trace};

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::ELF_MAGIC_BYTES,
        util::{download_progress_style, format_bytes, interactive_ask, AskType},
    },
    package::parse_package_query,
    registry::{select_single_package, PackageRegistry},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GithubAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: String,
    assets: Vec<GithubAsset>,
}

static GITHUB_URL_REGEX: &str =
    r"^(?i)(?:https?://)?(?:github(?:\.com)?[:/])([^/@]+/[^/@]+)(?:@([^/\s]+))?$";

fn extract_filename(url: &str) -> String {
    Path::new(url)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            let dt = Utc::now().timestamp();
            dt.to_string()
        })
}

async fn is_elf(file_path: &Path) -> bool {
    let Ok(file) = File::open(file_path).await else {
        return false;
    };
    let mut file = BufReader::new(file);

    let mut magic_bytes = [0_u8; 4];
    if file.read_exact(&mut magic_bytes).await.is_ok() {
        return magic_bytes == ELF_MAGIC_BYTES;
    }
    false
}

async fn download(url: &str, output: Option<String>) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Error fetching {} [{}]",
            url.color(Color::Blue),
            response.status().color(Color::Red)
        ));
    }

    let filename = output.unwrap_or(extract_filename(url));
    let filename = if filename.ends_with("/") {
        format!(
            "{}/{}",
            filename.trim_end_matches("/"),
            extract_filename(url)
        )
    } else {
        filename
    };
    let output_path = Path::new(&filename);

    if let Some(output_dir) = output_path.parent() {
        if !output_dir.exists() {
            fs::create_dir_all(&output_dir).await.context(format!(
                "Failed to create directory: {}",
                output_dir.display()
            ))?;
        }
    }

    let temp_path = format!("{}.tmp", output_path.display());

    info!(
        "Downloading file from {} [{}]",
        url.color(Color::Blue),
        format_bytes(response.content_length().unwrap_or_default()).color(Color::Yellow)
    );

    let content_length = response.content_length().unwrap_or(0);
    let progress_bar = ProgressBar::new(content_length);
    progress_bar.set_style(download_progress_style(false));

    let mut stream = response.bytes_stream();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(&temp_path)
        .await
        .context("Failed to open temp file for writing")?;

    let mut downloaded_bytes = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read chunk")?;
        file.write_all(&chunk).await?;
        downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as u64);
        progress_bar.set_position(downloaded_bytes);
        if content_length == 0 {
            progress_bar.set_length(downloaded_bytes);
        }
    }
    progress_bar.finish();

    fs::rename(&temp_path, &output_path).await?;

    if is_elf(output_path).await {
        fs::set_permissions(&output_path, Permissions::from_mode(0o755)).await?;
    }

    info!("Downloaded {}", output_path.display().color(Color::Blue));

    Ok(())
}

async fn fetch_github_releases(user_repo: &str) -> Result<Vec<GithubRelease>> {
    let client = reqwest::Client::new();
    let url = format!("https://api.github.com/repos/{}/releases", user_repo);
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, "pkgforge/soar".parse()?);
    if let Ok(token) = env::var("GITHUB_TOKEN") {
        trace!("Using Github token: {}", token);
        headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse()?);
    };
    let response = client
        .get(&url)
        .headers(headers)
        .send()
        .await
        .context("Failed to fetch GitHub releases")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Error fetching releases for {}: {}",
            user_repo,
            response.status()
        );
    }

    let releases: Vec<GithubRelease> = response
        .json()
        .await
        .context("Failed to parse GitHub response")?;

    Ok(releases)
}

fn select_asset_idx(assets: &[&GithubAsset], max: usize) -> Result<usize> {
    for (i, asset) in assets.iter().enumerate() {
        info!(
            " [{}] {:#?} ({})",
            i + 1,
            asset.name,
            format_bytes(asset.size),
        );
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

pub async fn download_and_save(
    registry: PackageRegistry,
    links: &[String],
    yes: bool,
    output: Option<String>,
    regex_patterns: Option<&[String]>,
    match_keywords: Option<&[String]>,
    exclude_keywords: Option<&[String]>,
) -> Result<()> {
    let re = Regex::new(GITHUB_URL_REGEX).unwrap();
    let asset_regexes = regex_patterns
        .map(|patterns| {
            patterns
                .iter()
                .map(|pattern| Regex::new(pattern))
                .collect::<Result<Vec<Regex>, regex::Error>>()
        })
        .transpose()?
        .unwrap_or_default();

    for link in links {
        if re.is_match(link) {
            info!(
                "GitHub repository URL detected: {}",
                link.color(Color::Blue)
            );
            if let Some(caps) = re.captures(link) {
                let user_repo = caps.get(1).unwrap().as_str();
                let tag = caps.get(2).map(|tag| tag.as_str());
                info!("Fetching releases for {}...", user_repo);

                let releases = fetch_github_releases(user_repo).await?;

                let release = if let Some(tag_name) = tag {
                    releases
                        .iter()
                        .find(|release| release.tag_name.starts_with(tag_name))
                } else {
                    releases
                        .iter()
                        .find(|release| !release.prerelease && !release.draft)
                };

                let Some(release) = release else {
                    error!(
                        "No {} found for repository {}",
                        tag.map(|t| format!("tag {}", t))
                            .unwrap_or("stable release".to_owned()),
                        user_repo
                    );
                    continue;
                };

                let assets = &release.assets;

                if assets.is_empty() {
                    error!("No assets found for the release.");
                    continue;
                }

                let selected_asset = {
                    let assets: Vec<&GithubAsset> = assets
                        .iter()
                        .filter(|asset| {
                            asset_regexes
                                .iter()
                                .all(|regex| regex.is_match(&asset.name))
                                && match_keywords.map_or(true, |keywords| {
                                    keywords.iter().all(|keyword| {
                                        asset.name.to_lowercase().contains(&keyword.to_lowercase())
                                    })
                                })
                                && exclude_keywords.map_or(true, |keywords| {
                                    keywords.iter().all(|keyword| {
                                        !asset.name.to_lowercase().contains(&keyword.to_lowercase())
                                    })
                                })
                        })
                        .collect();

                    match assets.len() {
                        0 => {
                            error!("No assets matched the provided criteria.");
                            continue;
                        }
                        1 => assets[0],
                        _ => {
                            if yes {
                                assets[0]
                            } else {
                                info!(
                                    "Multiple matching assets found for {}{}",
                                    release.tag_name,
                                    if release.prerelease {
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

                let download_url = &selected_asset.browser_download_url;
                download(download_url, output.clone()).await?;
            }
        } else if let Ok(url) = Url::parse(link) {
            download(url.as_str(), output.clone()).await?;
        } else {
            error!("{} is not a valid URL", link.color(Color::Blue));
            info!("Searching for package instead..");

            let query = parse_package_query(link);
            let packages = registry.storage.get_packages(&query);

            if let Some(packages) = packages {
                let resolved_pkg = if yes || packages.len() == 1 {
                    &packages[0]
                } else {
                    select_single_package(&packages)?
                };
                download(&resolved_pkg.package.download_url, output.clone()).await?;
            } else {
                error!("No packages found.");
            }
        };
    }

    Ok(())
}
