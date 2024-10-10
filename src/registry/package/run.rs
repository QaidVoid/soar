use std::{
    fs::Permissions, io::Write, os::unix::fs::PermissionsExt, path::PathBuf, process::Command,
};

use anyhow::{Context, Result};
use futures::StreamExt;
use tokio::{fs, io::AsyncWriteExt};

use crate::core::util::{format_bytes, validate_checksum};

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
        let package_name = &package.full_name();

        if self.install_path.exists() {
            if xattr::get(&self.install_path, "user.ManagedBy")?.as_deref() != Some(b"soar") {
                return Err(anyhow::anyhow!(
                    "Path {} is not managed by soar. Exiting.",
                    self.install_path.to_string_lossy()
                ));
            } else {
                println!("Found existing cache for {}", package_name);
                let result = validate_checksum(&package.bsum, &self.install_path).await;
                if result.is_err() {
                    eprintln!("Checksum validation failed for {}", package_name);
                    eprintln!("The package will be re-downloaded.");
                } else {
                    self.run().await?;
                    return Ok(());
                }
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
            package_name,
            format_bytes(total_size)
        );

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "{}: Download failed with status code {:?}",
                package_name,
                response.status()
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
                    package_name
                ))?;

            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.context(format!("{}: Failed to read chunk", package_name))?;
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
        }

        if package.bsum == "null" {
            eprintln!(
                "Missing checksum for {}. Installing anyway.",
                package.full_name()
            );
        } else {
            let result = validate_checksum(&package.bsum, &self.temp_path).await;
            if result.is_err() {
                eprint!(
                    "\n{}: Checksum verification failed. Do you want to remove the package? (y/n): ",
                    package_name
                );
                std::io::stdout().flush()?;

                let mut response = String::new();
                std::io::stdin().read_line(&mut response)?;

                if response.trim().eq_ignore_ascii_case("y") {
                    tokio::fs::remove_file(&self.temp_path).await?;
                    return Err(anyhow::anyhow!(""));
                }
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
        xattr::set(install_path, "user.ManagedBy", b"soar")?;

        Ok(())
    }

    async fn run(&self) -> Result<()> {
        Command::new(&self.install_path).args(&self.args).spawn()?;

        Ok(())
    }
}
