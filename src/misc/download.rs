use std::{fs::Permissions, os::unix::fs::PermissionsExt, path::Path};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::StreamExt;
use reqwest::Url;
use tokio::fs;

use crate::core::util::format_bytes;

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
    let elf_bytes = [0x7f, 0x45, 0x4c, 0x46];
    magic_bytes == elf_bytes
}

async fn download(url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Error fetching {} [{}]",
            url,
            response.status()
        ));
    }

    let filename = extract_filename(url);
    let mut content = Vec::new();

    println!(
        "Downloading file from {} [{}]",
        url,
        format_bytes(response.content_length().unwrap_or_default())
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

    println!("Downloaded {}", filename);

    Ok(())
}

pub async fn download_and_save(links: &[String]) -> Result<()> {
    for link in links {
        if let Ok(url) = Url::parse(link) {
            download(url.as_str()).await?;
        } else {
            eprintln!("{} is not a valid URL", link);
        };
    }

    Ok(())
}
