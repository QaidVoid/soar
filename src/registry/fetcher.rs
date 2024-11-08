use std::{collections::HashMap, env::consts::ARCH, path::PathBuf};

use anyhow::{Context, Result};
use futures::future::try_join_all;
use serde::Deserialize;
use tokio::fs;

use crate::{
    core::{
        color::{Color, ColorExt},
        config::Repository,
        constant::REGISTRY_PATH,
        util::download,
    },
    package::Package,
};

pub struct MetadataFetcher;

#[derive(Deserialize)]
struct RepositoryResponse {
    #[serde(flatten)]
    collection: HashMap<String, Vec<Package>>,
}

impl MetadataFetcher {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, repository: &Repository) -> Result<Vec<u8>> {
        let url = format!(
            "{}/{}",
            repository.url,
            repository
                .metadata
                .to_owned()
                .unwrap_or("metadata.json".to_owned())
        );

        let content = download(&url, "metadata", false).await?;

        let parsed: RepositoryResponse =
            serde_json::from_slice(&content).context("Failed to parse metadata json")?;

        let metadata: HashMap<String, HashMap<String, Vec<Package>>> = parsed
            .collection
            .iter()
            .map(|(key, packages)| {
                let package_map: HashMap<String, Vec<Package>> =
                    packages.iter().fold(HashMap::new(), |mut acc, package| {
                        acc.entry(package.pkg.to_lowercase().clone()).or_default().push(Package {
                            family: package
                                .download_url
                                .split('/')
                                .rev()
                                .nth(1)
                                .map(|v| v.to_owned())
                                .filter(|v| v != ARCH),
                            ..package.clone()
                        });
                        acc
                    });

                (key.clone(), package_map)
            })
            .collect();

        let content = rmp_serde::to_vec(&metadata)
            .context("Failed to serialize package metadata to MessagePack")?;

        let path = repository.get_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create registry directory")?;
        }

        fs::write(&path, &content).await.with_context(|| {
            format!(
                "Failed to write metadata for {}",
                repository.name.clone().color(Color::Yellow)
            )
        })?;

        self.fetch_icons(repository).await?;

        Ok(content)
    }

    async fn fetch_icon(
        &self,
        icon_path: PathBuf,
        base_url: String,
        key: &str,
    ) -> Result<Option<(String, Vec<u8>)>> {
        if fs::metadata(&icon_path).await.is_ok() {
            Ok(None)
        } else {
            let content = download(&base_url, "icon", true).await?;
            Ok(Some((key.to_owned(), content))) // Return the key and icon data if downloaded
        }
    }

    pub async fn fetch_icons(&self, repository: &Repository) -> Result<()> {
        // fetch default icons
        let icon_futures: Vec<_> = repository
            .sources
            .iter()
            .map(|(key, base_url)| {
                let base_url = format!("{}/{}.default.png", base_url, key);

                let icon_path = REGISTRY_PATH
                    .join("icons")
                    .join(format!("{}-{}.png", repository.name, key));
                self.fetch_icon(icon_path, base_url, key)
            })
            .collect();

        let icons = try_join_all(icon_futures).await?;
        let icons_to_save: Vec<_> = icons.into_iter().flatten().collect();

        for (key, icon) in icons_to_save {
            let icon_path = REGISTRY_PATH
                .join("icons")
                .join(format!("{}-{}.png", repository.name, key));

            if let Some(parent) = icon_path.parent() {
                fs::create_dir_all(parent).await.context(anyhow::anyhow!(
                    "Failed to create icon directory at {}",
                    parent.to_string_lossy().color(Color::Blue)
                ))?;
            }

            fs::write(icon_path, icon).await?;
        }

        Ok(())
    }

    pub async fn checksum(&self, repository: &Repository) -> Result<Vec<u8>> {
        let url = format!(
            "{}/{}",
            repository.url,
            repository
                .metadata
                .to_owned()
                .map(|file| format!("{file}.bsum"))
                .unwrap_or("metadata.json.bsum".to_owned())
        );

        let content = download(&url, "metadata", true).await?;

        Ok(content)
    }
}
