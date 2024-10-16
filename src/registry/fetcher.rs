use std::{collections::HashMap, env::consts::ARCH};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::fs;

use crate::core::{
    color::{Color, ColorExt},
    config::Repository,
    util::{download, get_platform},
};

use super::{package::Package, storage::RepositoryPackages};

pub struct RegistryFetcher;

#[derive(Deserialize)]
struct RepositoryResponse {
    bin: Vec<Package>,
    base: Vec<Package>,
    pkg: Vec<Package>,
}

impl RegistryFetcher {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, repository: &Repository) -> Result<Vec<u8>> {
        let platform = get_platform();
        let url = format!(
            "{}/{}/{}",
            repository.url,
            platform,
            repository
                .registry
                .to_owned()
                .unwrap_or("metadata.json".to_owned())
        );

        let content = download(&url, "registry", false).await?;

        let parsed: RepositoryResponse =
            serde_json::from_slice(&content).context("Failed to parse registry json")?;

        let convert_to_hashmap = |packages: Vec<Package>| -> HashMap<String, Vec<Package>> {
            let mut result = HashMap::new();
            for package in packages {
                let variant = package
                    .download_url
                    .split('/')
                    .rev()
                    .nth(1)
                    .map(|v| v.to_owned())
                    .filter(|v| v != ARCH && v != &platform && v != &platform.replace('-', "_"));
                let package_entry = result
                    .entry(package.name.to_owned())
                    .or_insert_with(Vec::new);
                package_entry.push(Package { variant, ..package });
            }
            result
        };

        let package_registry = RepositoryPackages {
            bin: convert_to_hashmap(parsed.bin),
            base: convert_to_hashmap(parsed.base),
            pkg: convert_to_hashmap(parsed.pkg),
        };

        let content = rmp_serde::to_vec(&package_registry)
            .context("Failed to serialize package registry to MessagePack")?;

        let path = repository.get_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create registry directory")?;
        }

        fs::write(&path, &content).await.with_context(|| {
            format!(
                "Failed to write registry for {}",
                repository.name.clone().color(Color::Yellow)
            )
        })?;

        Ok(content)
    }
}
