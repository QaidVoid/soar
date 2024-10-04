use std::{
    env::{
        self,
        consts::{ARCH, OS},
    },
    path::PathBuf,
};

use anyhow::{Context, Result};
use reqwest::Client;

use crate::package::registry::RootPath;

/// Expands the environment variables and user home directory in a given path.
///
/// - `$VAR` will be replaced with the value of the environment variable `VAR`.
/// - `~` at the beginning of the path will be replaced with the user's home directory.
///
/// # Arguments
///
/// * `path` - A string slice that holds the path to be expanded.
///
/// # Returns
///
/// A `PathBuf` containing the expanded path.
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

/// Fetches the content length of a remote resource using a HEAD request.
///
/// # Arguments
///
/// * `client` - A `reqwest::Client` used to make the request.
/// * `url` - A string slice that holds the URL to fetch.
///
/// # Returns
///
/// A `Result<u64>` containing the content length of the remote resource.
///
/// # Errors
///
/// Returns an error if the request fails or if the `Content-Length` header is not found.
pub async fn get_remote_content_length(client: &Client, url: &str) -> Result<u64> {
    let response = client
        .head(url)
        .send()
        .await
        .context("Failed to send HEAD request")?;

    let content_length = match response.headers().get("Content-Length") {
        Some(length) => length
            .to_str()
            .context("Failed to convert Content-Length header to string")
            .and_then(|s| {
                s.parse::<u64>()
                    .context("Failed to parse Content-Length as u64")
            })?,
        None => return Err(anyhow::anyhow!("Content-Length header not found")),
    };

    Ok(content_length)
}

#[derive(Debug)]
pub struct PackageQuery {
    pub name: String,
    pub variant: Option<String>,
    pub root_path: Option<RootPath>,
}

pub fn parse_package_query(query: &str) -> PackageQuery {
    let (base_query, root_path) = query
        .rsplit_once('#')
        .map(|(n, r)| {
            (
                n.to_owned(),
                match r.to_lowercase().as_str() {
                    "base" => Some(RootPath::Base),
                    "bin" => Some(RootPath::Bin),
                    "pkg" => Some(RootPath::Pkg),
                    _ => {
                        eprintln!("Invalid root path provided for {}", query);
                        std::process::exit(-1);
                    }
                },
            )
        })
        .unwrap_or((query.to_owned(), None));

    let (name, variant) = base_query
        .split_once('/')
        .map(|(v, n)| (n.to_owned(), Some(v.to_owned())))
        .unwrap_or((base_query, None));

    PackageQuery {
        name,
        variant,
        root_path,
    }
}

pub fn format_bytes(bytes: u64) -> String {
    let kb = 1024u64;
    let mb = kb * 1024;
    let gb = mb * 1024;

    match bytes {
        b if b >= gb => format!("{:.2} GB", b as f64 / gb as f64),
        b if b >= mb => format!("{:.2} MB", b as f64 / mb as f64),
        b if b >= kb => format!("{:.2} KB", b as f64 / kb as f64),
        _ => format!("{} B", bytes),
    }
}

pub fn parse_size(size_str: &str) -> Option<u64> {
    let size_str = size_str.trim();
    let units = [
        ("B", 1u64),
        ("KB", 1000u64),
        ("MB", 1000u64 * 1000),
        ("GB", 1000u64 * 1000 * 1000),
    ];

    for (unit, multiplier) in &units {
        if size_str.ends_with(unit) {
            let number_part = size_str.trim_end_matches(unit).trim();
            if let Ok(num) = number_part.parse::<f64>() {
                return Some((num * (*multiplier as f64)) as u64);
            }
        }
    }

    None
}
