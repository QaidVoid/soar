use std::{collections::HashMap, env::consts::ARCH};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::fs;

use crate::core::{
    color::{Color, ColorExt},
    config::Repository,
    util::{download, get_platform},
};

use super::package::Package;

pub struct RegistryFetcher;

#[derive(Deserialize)]
struct RepositoryResponse {
    #[serde(flatten)]
    collection: HashMap<String, Vec<Package>>,
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

        let package_registry: HashMap<String, HashMap<String, Vec<Package>>> = parsed
            .collection
            .iter()
            .map(|(key, packages)| {
                let package_map: HashMap<String, Vec<Package>> =
                    packages.iter().fold(HashMap::new(), |mut acc, package| {
                        acc.entry(package.name.clone()).or_default().push(Package {
                            variant: package
                                .download_url
                                .split('/')
                                .rev()
                                .nth(1)
                                .map(|v| v.to_owned())
                                .filter(|v| {
                                    v != ARCH && v != &platform && v != &platform.replace('-', "_")
                                }),
                            ..package.clone()
                        });
                        acc
                    });

                (key.clone(), package_map)
            })
            .collect();

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

    pub async fn checksum(&self, repository: &Repository) -> Result<Vec<u8>> {
        let platform = get_platform();
        let url = format!(
            "{}/{}/{}",
            repository.url,
            platform,
            repository
                .registry
                .to_owned()
                .map(|file| format!("{file}.bsum"))
                .unwrap_or("metadata.json.bsum".to_owned())
        );

        let content = download(&url, "registry", true).await?;

        Ok(content)
    }
}
