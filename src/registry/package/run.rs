use std::{fs::Permissions, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

use anyhow::{Context, Result};
use futures::StreamExt;
use tokio::{fs, io::AsyncWriteExt};

use crate::{
    core::{
        color::{Color, ColorExt},
        util::{format_bytes, validate_checksum},
    },
    error, info, warn,
};

use super::ResolvedPackage;

pub struct Runner {
    args: Vec<String>,
    resolved_package: ResolvedPackage,
    install_path: PathBuf,
    temp_path: PathBuf,
}

impl Runner {
    pub fn new(package: &ResolvedPackage, install_path: PathBuf, args: &[String]) -> Self {
        let temp_path = install_path.with_extension("part");
        Self {
            args: args.to_owned(),
            resolved_package: package.to_owned(),
            install_path,
            temp_path,
        }
    }

    pub async fn execute(&self) -> Result<()> {
        let package = &self.resolved_package.package;
        let package_name = &package.full_name('/');

        if self.install_path.exists() {
            if xattr::get(&self.install_path, "user.managed_by")?.as_deref() != Some(b"soar") {
                return Err(anyhow::anyhow!(
                    "Path {} is not managed by soar. Exiting.",
                    self.install_path.to_string_lossy().color(Color::Blue)
                ));
            } else {
                info!(
                    "Found existing cache for {}",
                    package_name.color(Color::Blue)
                );
                return self.run().await;
            }
        }

        let client = reqwest::Client::new();
        let downloaded_bytes = if self.temp_path.exists() {
            let meta = fs::metadata(&self.temp_path).await?;
            meta.len()
        } else {
            0
        };

        let response = client
            .get(&package.download_url)
            .header("Range", format!("bytes={}-", downloaded_bytes))
            .send()
            .await
            .context(format!("{} Failed to download package", package_name))?;
        let total_size = response
            .content_length()
            .map(|cl| cl + downloaded_bytes)
            .unwrap_or(0);
        println!(
            "{}: Downloading package [{}]",
            package_name.color(Color::Blue),
            format_bytes(total_size).color(Color::Yellow)
        );

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "{}: Download failed {:?}",
                package_name.color(Color::Blue),
                response.status().color(Color::Red)
            ));
        }

        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(&self.temp_path)
                .await
                .context(format!(
                    "{}: Failed to open temp file for writing",
                    package_name.color(Color::Blue)
                ))?;

            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.context(format!(
                    "{}: Failed to read chunk",
                    package_name.color(Color::Blue)
                ))?;
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
        }

        if package.bsum == "null" {
            warn!(
                "Missing checksum for {}. Installing anyway.",
                package.full_name('/').color(Color::Blue)
            );
        } else {
            let result = validate_checksum(&package.bsum, &self.temp_path).await;
            if result.is_err() {
                error!(
                    "{}: Checksum verification failed.",
                    package_name.color(Color::Blue)
                );
            }
        }

        self.save_file().await?;
        self.run().await?;

        Ok(())
    }

    async fn save_file(&self) -> Result<()> {
        let install_path = &self.install_path;
        let temp_path = &self.temp_path;
        if install_path.exists() {
            tokio::fs::remove_file(&install_path).await?;
        }
        tokio::fs::rename(&temp_path, &install_path).await?;
        tokio::fs::set_permissions(&install_path, Permissions::from_mode(0o755)).await?;
        xattr::set(install_path, "user.managed_by", b"soar")?;

        Ok(())
    }

    async fn run(&self) -> Result<()> {
        Command::new(&self.install_path).args(&self.args).status()?;

        Ok(())
    }
}
