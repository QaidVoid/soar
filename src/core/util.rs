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

use super::{
    color::{Color, ColorExt},
    constant::{BIN_PATH, INSTALL_TRACK_PATH, PACKAGES_PATH},
};

pub fn home_path() -> String {
    env::var("HOME").unwrap_or_else(|_| {
        panic!("Unable to find home directory.");
    })
}

pub fn home_config_path() -> String {
    env::var("XDG_CONFIG_HOME").unwrap_or(format!("{}/.config", home_path()))
}

pub fn home_cache_path() -> String {
    env::var("XDG_CACHE_HOME").unwrap_or(format!("{}/.cache", home_path()))
}

pub fn home_data_path() -> String {
    env::var("XDG_DATA_HOME").unwrap_or(format!("{}/.local/share", home_path()))
}

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

pub async fn calculate_checksum(file_path: &Path) -> Result<String> {
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

    Ok(hasher.finalize().to_hex().to_string())
}

pub async fn validate_checksum(checksum: &str, file_path: &Path) -> Result<()> {
    let final_checksum = calculate_checksum(file_path).await?;
    if final_checksum == *checksum {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Checksum verification failed."))
    }
}

pub async fn setup_required_paths() -> Result<()> {
    if !BIN_PATH.exists() {
        fs::create_dir_all(&*BIN_PATH).await.with_context(|| {
            format!(
                "Failed to create bin directory {}",
                BIN_PATH.to_string_lossy().color(Color::Blue)
            )
        })?;
    }

    if !INSTALL_TRACK_PATH.exists() {
        fs::create_dir_all(&*INSTALL_TRACK_PATH)
            .await
            .with_context(|| {
                format!(
                    "Failed to create installs directory: {}",
                    INSTALL_TRACK_PATH.to_string_lossy().color(Color::Blue)
                )
            })?;
    }

    if !PACKAGES_PATH.exists() {
        fs::create_dir_all(&*PACKAGES_PATH).await.with_context(|| {
            format!(
                "Failed to create packages directory: {}",
                PACKAGES_PATH.to_string_lossy().color(Color::Blue)
            )
        })?;
    }

    Ok(())
}

pub async fn download(url: &str, what: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Error fetching {} from {} [{}]",
            what.color(Color::Cyan),
            url.color(Color::Blue),
            response.status().color(Color::Red)
        ));
    }

    let mut content = Vec::new();

    println!(
        "Fetching {} from {} [{}]",
        what.color(Color::Cyan),
        url.color(Color::Blue),
        format_bytes(response.content_length().unwrap_or_default())
    );

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read chunk")?;
        content.extend_from_slice(&chunk);
    }

    Ok(content)
}

pub async fn cleanup() -> Result<()> {
    let mut cache_dir = home_cache_path();
    cache_dir.push_str("/soar");
    let cache_dir = build_path(&cache_dir)?;

    if cache_dir.exists() {
        let mut tree = fs::read_dir(&cache_dir).await?;

        while let Some(entry) = tree.next_entry().await? {
            let path = entry.path();
            if xattr::get(&path, "user.managed_by")?.as_deref() != Some(b"soar") {
                continue;
            };

            fs::remove_file(path).await?;
        }
    }

    remove_broken_symlink().await?;

    Ok(())
}

pub async fn remove_broken_symlink() -> Result<()> {
    let mut tree = fs::read_dir(&*BIN_PATH).await?;
    while let Some(entry) = tree.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            fs::remove_file(path).await?;
        }
    }

    Ok(())
}
