use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::core::constant::INSTALL_TRACK_PATH;

use super::package::{ResolvedPackage, RootPath};

#[derive(Debug, Deserialize, Serialize)]
pub struct InstalledPackage {
    pub repo_name: String,
    pub root_path: RootPath,
    pub name: String,
    pub bin_name: String,
    pub version: String,
    pub checksum: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InstalledPackages {
    pub packages: Vec<InstalledPackage>,
}

impl InstalledPackages {
    pub async fn new() -> Result<Self> {
        let path = INSTALL_TRACK_PATH.join("latest");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create installs directory to track installations.")?;
        }

        let packages = if path.exists() {
            let content = tokio::fs::read(&path)
                .await
                .context("Failed to read installed packages")?;

            let mut de = rmp_serde::Deserializer::new(&content[..]);

            InstalledPackages::deserialize(&mut de)?
        } else {
            InstalledPackages {
                packages: Vec::new(),
            }
        };

        Ok(packages)
    }

    pub fn is_installed(&self, package: &ResolvedPackage) -> bool {
        self.packages.iter().any(|installed| {
            installed.repo_name == package.repo_name
                && installed.root_path == package.root_path
                && installed.name == package.package.full_name()
        })
    }

    fn find_package_mut(&mut self, package: &ResolvedPackage) -> Option<&mut InstalledPackage> {
        self.packages.iter_mut().find(|installed| {
            installed.repo_name == package.repo_name
                && installed.root_path == package.root_path
                && installed.name == package.package.full_name()
        })
    }

    pub async fn register_package(&mut self, resolved_package: &ResolvedPackage) -> Result<()> {
        let package = resolved_package.package.to_owned();
        if let Some(installed) = self.find_package_mut(resolved_package) {
            installed.version = package.version.clone();
            installed.checksum = package.bsum.clone();
        } else {
            let new_installed = InstalledPackage {
                repo_name: resolved_package.repo_name.to_owned(),
                root_path: resolved_package.root_path.to_owned(),
                name: package.full_name(),
                bin_name: package.bin_name,
                version: package.version,
                checksum: package.bsum,
            };
            self.packages.push(new_installed);
        }

        self.save().await?;

        Ok(())
    }

    pub async fn unregister_package(&mut self, resolved_package: &ResolvedPackage) -> Result<()> {
        match self.is_installed(resolved_package) {
            true => {
                self.packages.retain(|installed| {
                    !(installed.repo_name == resolved_package.repo_name
                        && installed.root_path == resolved_package.root_path
                        && installed.name == resolved_package.package.full_name())
                });
                println!("NOW: {:#?}", self.packages);
            }
            false => {
                return Err(anyhow::anyhow!(
                    "Package is not registered to install database."
                ))
            }
        };

        self.save().await?;

        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        let path = INSTALL_TRACK_PATH.join("latest");

        let content = rmp_serde::to_vec(&self)
            .context("Failed to serialize installed packages to MessagePack")?;

        fs::write(&path, content)
            .await
            .context(format!("Failed to write to {}", path.to_string_lossy()))?;

        Ok(())
    }
}
