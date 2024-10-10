use std::{
    env::{
        self,
        consts::{ARCH, OS},
    },
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use futures::StreamExt;
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt},
};

use super::constant::{BIN_PATH, INSTALL_TRACK_PATH};

/// Expands the environment variables and user home directory in a given path.
pub fn build_path(path: &str) -> Result<PathBuf> {
    let mut result = String::new();
    let mut chars = path.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            let mut var_name = String::new();
            while let Some(&c) = chars.peek() {
                if !c.is_alphanumeric() && c != '_' {
                    break;
                }
                var_name.push(chars.next().unwrap());
            }
            if !var_name.is_empty() {
                let expanded = env::var(&var_name)
                    .with_context(|| format!("Environment variable ${} not found", var_name))?;
                result.push_str(&expanded);
            } else {
                result.push('$');
            }
        } else if c == '~' && result.is_empty() {
            if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
                result.push_str(home.to_string_lossy().as_ref());
            } else {
                result.push('~');
            }
        } else {
            result.push(c);
        }
    }

    Ok(PathBuf::from(result))
}

/// Retrieves the platform string in the format `ARCH-Os`.
///
/// This function combines the architecture (e.g., `x86_64`) and the operating
/// system (e.g., `Linux`) into a single string to identify the platform.
pub fn get_platform() -> String {
    format!("{ARCH}-{}{}", &OS[..1].to_uppercase(), &OS[1..])
}

pub fn format_bytes(bytes: u64) -> String {
    let kb = 1024u64;
    let mb = kb * 1024;
    let gb = mb * 1024;

    match bytes {
        b if b >= gb => format!("{:.2} GiB", b as f64 / gb as f64),
        b if b >= mb => format!("{:.2} MiB", b as f64 / mb as f64),
        b if b >= kb => format!("{:.2} KiB", b as f64 / kb as f64),
        _ => format!("{} B", bytes),
    }
}

pub fn parse_size(size_str: &str) -> Option<u64> {
    let size_str = size_str.trim();
    let units = [
        ("B", 1u64),
        ("KB", 1024u64),
        ("MB", 1024u64 * 1024),
        ("GB", 1024u64 * 1024 * 1024),
    ];

    for (unit, multiplier) in &units {
        let size_str = size_str.to_uppercase();
        if size_str.ends_with(unit) {
            let number_part = size_str.trim_end_matches(unit).trim();
            if let Ok(num) = number_part.parse::<f64>() {
                return Some((num * (*multiplier as f64)) as u64);
            }
        }
    }

    None
}

pub async fn validate_checksum(checksum: &str, file_path: &Path) -> Result<()> {
    let mut file = File::open(&file_path).await?;

    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 8192];

    while let Ok(n) = file.read(&mut buffer).await {
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    file.flush().await?;

    let final_checksum = hasher.finalize().to_hex().to_string();
    if final_checksum == *checksum {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Checksum verification failed."))
    }
}

pub async fn setup_required_paths() -> Result<()> {
    if !BIN_PATH.exists() {
        fs::create_dir_all(&*BIN_PATH).await.context(format!(
            "Failed to create bin directory {}",
            BIN_PATH.to_string_lossy()
        ))?;
    }

    if !INSTALL_TRACK_PATH.exists() {
        fs::create_dir_all(&*INSTALL_TRACK_PATH)
            .await
            .context(format!(
                "Failed to create path: {}",
                INSTALL_TRACK_PATH.to_string_lossy()
            ))?;
    }

    Ok(())
}

pub async fn download(url: &str, what: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    let mut content = Vec::new();

    println!(
        "Fetching {} from {} [{}]",
        what,
        url,
        format_bytes(response.content_length().unwrap_or_default())
    );

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read chunk")?;
        content.extend_from_slice(&chunk);
    }

    Ok(content)
}
