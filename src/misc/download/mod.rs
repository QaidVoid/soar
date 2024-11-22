use std::{fs::Permissions, os::unix::fs::PermissionsExt, path::Path};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::StreamExt;
use github::{handle_github_download, GITHUB_URL_REGEX};
use gitlab::{handle_gitlab_download, GITLAB_URL_REGEX};
use indicatif::ProgressBar;
use regex::Regex;
use reqwest::{header::USER_AGENT, StatusCode, Url};
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
};
use tracing::{error, info};

mod github;
mod gitlab;

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::ELF_MAGIC_BYTES,
        util::{download_progress_style, format_bytes},
    },
    package::parse_package_query,
    registry::{select_single_package, PackageRegistry},
};

pub enum ApiType {
    PkgForge,
    Primary,
}

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

pub async fn download(url: &str, output: Option<String>) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header(USER_AGENT, "pkgforge/soar")
        .send()
        .await?;

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

fn should_fallback(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::UNAUTHORIZED
        || status == StatusCode::FORBIDDEN
        || status.is_server_error()
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
    let github_re = Regex::new(GITHUB_URL_REGEX).unwrap();
    let gitlab_re = Regex::new(GITLAB_URL_REGEX).unwrap();
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
        let link = link.trim();
        if github_re.is_match(link) {
            info!(
                "GitHub repository URL detected: {}",
                link.color(Color::Blue)
            );
            handle_github_download(
                &github_re,
                link,
                output.clone(),
                match_keywords,
                exclude_keywords,
                &asset_regexes,
                yes,
            )
            .await?;
        } else if gitlab_re.is_match(link) {
            info!(
                "Gitlab repository URL detected: {}",
                link.color(Color::Blue)
            );
            handle_gitlab_download(
                &gitlab_re,
                link,
                output.clone(),
                match_keywords,
                exclude_keywords,
                &asset_regexes,
                yes,
            )
            .await?;
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
