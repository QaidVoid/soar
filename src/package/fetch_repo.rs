use std::collections::HashMap;

use anyhow::{Context, Result};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;

use crate::core::{
    config::{Repository, CONFIG},
    constant::{REGISTRY_PATH, SPARKLE, TRUCK},
    util::{get_platform, get_remote_content_length},
};

use super::registry::{Package, PackageRegistry};

#[derive(Deserialize)]
struct PlatformPackages {
    #[serde(flatten)]
    platforms: HashMap<String, Vec<Package>>,
}

#[derive(Deserialize)]
struct RegistryResponse {
    bin: PlatformPackages,
    base: PlatformPackages,
    pkg: PlatformPackages,
}

pub trait FetchRepository {
    /// Fetches all configured repositories.
    /// This method will either fetch repositories in parallel or
    /// sequentially based on the configuration.
    async fn fetch() -> Result<()>;

    /// Fetches a single repository's package metadata.
    ///
    /// # Arguments
    ///
    /// * `repo` - The repository configuration containing URL and other details
    ///
    /// # Returns
    ///
    /// Returns a Result containing the processed PackageRegistry on success
    async fn fetch_repository(repo: &Repository) -> Result<PackageRegistry>;
}

impl FetchRepository for PackageRegistry {
    async fn fetch() -> Result<()> {
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

    async fn fetch_repository(repo: &Repository) -> Result<PackageRegistry> {
        let url = format!(
            "{}/{}/{}",
            repo.url,
            get_platform(),
            repo.registry
                .to_owned()
                .unwrap_or("metadata.json".to_owned())
        );

        let client = reqwest::Client::new();
        let content_length = get_remote_content_length(&client, &url).await?;

        let pb = ProgressBar::new(content_length);
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg} [{bytes}/{total_bytes}]").unwrap(),
        );

        pb.set_message(format!(
            "{TRUCK}Fetching package registry from {}",
            repo.url
        ));

        let res = client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch repository")?;

        let mut downloaded_bytes = 0;
        let mut content = Vec::new();
        let mut stream = res.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;

            downloaded_bytes += chunk.len() as u64;
            pb.set_position(downloaded_bytes);
            content.extend_from_slice(&chunk);
        }

        let parsed: RegistryResponse =
            serde_json::from_slice(&content).context("Failed to parse registry json")?;

        // Helper function to convert PlatformPackages into a flat HashMap
        // where package names are keys and Package objects are values
        let convert_to_hashmap =
            |platform_packages: PlatformPackages| -> HashMap<String, HashMap<String, Package>> {
                let mut result = HashMap::new();
                for package in platform_packages.platforms.into_values().flatten() {
                    let variant = package.download_url.split('/').rev().nth(1).unwrap_or("");
                    let package_entry = result
                        .entry(package.name.clone())
                        .or_insert_with(HashMap::new);
                    package_entry.insert(variant.to_string(), package);
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
