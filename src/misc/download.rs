use std::{fs::Permissions, os::unix::fs::PermissionsExt, path::Path};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::StreamExt;
use reqwest::Url;
use tokio::fs;

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::ELF_MAGIC_BYTES,
        util::format_bytes,
    },
    error, success,
};

fn extract_filename(url: &str) -> String {
    Path::new(url)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            let dt = Utc::now().timestamp();
            dt.to_string()
        })
}

fn is_elf(content: &[u8]) -> bool {
    let magic_bytes = &content[..4.min(content.len())];
    magic_bytes == ELF_MAGIC_BYTES
}

async fn download(url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Error fetching {} [{}]",
            url.color(Color::Blue),
            response.status().color(Color::Red)
        ));
    }

    let filename = extract_filename(url);
    let mut content = Vec::new();

    println!(
        "Downloading file from {} [{}]",
        url.color(Color::Blue),
        format_bytes(response.content_length().unwrap_or_default()).color(Color::Yellow)
    );

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read chunk")?;
        content.extend_from_slice(&chunk);
    }

    fs::write(&filename, &content).await?;

    if is_elf(&content) {
        fs::set_permissions(&filename, Permissions::from_mode(0o755)).await?;
    }

    success!("Downloaded {}", filename.color(Color::Blue));

    Ok(())
}

pub async fn download_and_save(links: &[String]) -> Result<()> {
    for link in links {
        if let Ok(url) = Url::parse(link) {
            download(url.as_str()).await?;
        } else {
            error!("{} is not a valid URL", link.color(Color::Blue));
        };
    }

    Ok(())
}
