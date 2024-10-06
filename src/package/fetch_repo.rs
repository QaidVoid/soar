use std::{collections::HashMap, env::consts::ARCH};

use anyhow::{Context, Result};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;

use crate::core::{
    config::{Repository, CONFIG},
    constant::{CLIP, REGISTRY_PATH, SPARKLE},
    util::get_platform,
};

use super::registry::{Package, PackageRegistry};

#[derive(Deserialize)]
struct RegistryResponse {
    bin: Vec<Package>,
    base: Vec<Package>,
    pkg: Vec<Package>,
}

impl PackageRegistry {
    /// Fetches all configured repositories.
    /// This method will either fetch repositories in parallel or
    /// sequentially based on the configuration.
    pub async fn fetch() -> Result<()> {
        if CONFIG.parallel.unwrap_or_default() {
            let tasks: Vec<_> = CONFIG
                .repositories
                .iter()
                .map(|repo| {
                    tokio::spawn(async move {
                        Self::fetch_repository(repo)
                            .await
                            .context(format!("Failed to fetch repository: {}", repo.name))
                    })
                })
                .collect();

            for task in tasks {
                task.await??;
            }
        } else {
            for repo in &CONFIG.repositories {
                Self::fetch_repository(repo)
                    .await
                    .context(format!("Failed to fetch repository: {}", repo.name))?;
            }
        }

        Ok(())
    }

    /// Fetches a single repository's package metadata.
    pub async fn fetch_repository(repo: &Repository) -> Result<PackageRegistry> {
        let platform = get_platform();
        let url = format!(
            "{}/{}/{}",
            repo.url,
            platform,
            repo.registry
                .to_owned()
                .unwrap_or("metadata.json".to_owned())
        );

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch repository")?;

        let content_length = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(content_length);
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg} [{bytes}/{total_bytes}]").unwrap(),
        );

        pb.set_message(format!("{CLIP}Fetching package registry from {}", repo.url));

        let mut downloaded_bytes = 0;
        let mut content = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;

            downloaded_bytes += chunk.len() as u64;
            pb.set_position(downloaded_bytes);
            content.extend_from_slice(&chunk);
        }

        let parsed: RegistryResponse =
            serde_json::from_slice(&content).context("Failed to parse registry json")?;

        let convert_to_hashmap =
            |packages: Vec<Package>| -> HashMap<String, Vec<Package>> {
                let mut result = HashMap::new();
                for package in packages {
                    let variant = package
                        .download_url
                        .split('/')
                        .rev()
                        .nth(1)
                        .map(|v| v.to_owned())
                        .filter(|v| {
                            v != ARCH && v != &platform && v != &platform.replace('-', "_")
                        });
                    let package_entry = result
                        .entry(package.name.to_owned())
                        .or_insert_with(Vec::new);
                    package_entry.push(Package { variant, ..package });
                }
                result
            };

        let package_registry = PackageRegistry {
            bin: convert_to_hashmap(parsed.bin),
            base: convert_to_hashmap(parsed.base),
            pkg: convert_to_hashmap(parsed.pkg),
        };

        let content = rmp_serde::to_vec(&package_registry)
            .context("Failed to serialize package registry to MessagePack")?;

        let registry_path = REGISTRY_PATH.join(&repo.name);

        if let Some(parent) = registry_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create registry directory")?;
        }

        tokio::fs::write(&registry_path, content)
            .await
            .with_context(|| format!("Failed to write registry for {}", repo.name))?;

        pb.finish_with_message(format!(
            "{SPARKLE}Fetched package registry from {}",
            repo.url
        ));

        Ok(package_registry)
    }
}
