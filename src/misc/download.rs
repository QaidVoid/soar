use std::{fs::Permissions, os::unix::fs::PermissionsExt, path::Path};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::StreamExt;
use reqwest::Url;
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
};

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
    let filename = Path::new(&filename);
    let temp_path = format!("{}.tmp", filename.display());

    println!(
        "Downloading file from {} [{}]",
        url.color(Color::Blue),
        format_bytes(response.content_length().unwrap_or_default()).color(Color::Yellow)
    );

    let mut stream = response.bytes_stream();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(&temp_path)
        .await
        .context("Failed to open temp file for writing")?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read chunk")?;
        file.write_all(&chunk).await?;
    }

    fs::rename(&temp_path, &filename).await?;

    if is_elf(filename).await {
        fs::set_permissions(&filename, Permissions::from_mode(0o755)).await?;
    }

    success!("Downloaded {}", filename.display().color(Color::Blue));

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
