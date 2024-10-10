use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
    core::{
        constant::{BIN_PATH, INSTALL_TRACK_PATH},
        util::{format_bytes, parse_size},
    },
    registry::package::parse_package_query,
};

use super::{
    package::{ResolvedPackage, RootPath},
    storage::PackageStorage,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstalledPackage {
    pub repo_name: String,
    pub root_path: RootPath,
    pub name: String,
    pub bin_name: String,
    pub version: String,
    pub checksum: String,
    pub size: u64,
    pub timestamp: DateTime<Utc>,
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
                && installed.name == package.package.full_name('-')
        })
    }

    fn find_package_mut(&mut self, package: &ResolvedPackage) -> Option<&mut InstalledPackage> {
        self.packages.iter_mut().find(|installed| {
            installed.repo_name == package.repo_name
                && installed.root_path == package.root_path
                && installed.name == package.package.full_name('-')
        })
    }

    pub fn find_package(&self, package: &ResolvedPackage) -> Option<&InstalledPackage> {
        self.packages.iter().find(|installed| {
            installed.repo_name == package.repo_name
                && installed.root_path == package.root_path
                && installed.name == package.package.full_name('-')
        })
    }

    pub async fn register_package(
        &mut self,
        resolved_package: &ResolvedPackage,
        checksum: &str,
    ) -> Result<()> {
        let package = resolved_package.package.to_owned();
        if let Some(installed) = self.find_package_mut(resolved_package) {
            installed.version = package.version.clone();
            installed.checksum = package.bsum.clone();
        } else {
            let new_installed = InstalledPackage {
                repo_name: resolved_package.repo_name.to_owned(),
                root_path: resolved_package.root_path.to_owned(),
                name: package.full_name('-'),
                bin_name: package.bin_name,
                version: package.version,
                checksum: checksum.to_owned(),
                size: parse_size(&package.size).unwrap_or_default(),
                timestamp: Utc::now(),
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
                        && installed.name == resolved_package.package.full_name('-'))
                });
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

    pub async fn info(
        &self,
        packages: Option<&[String]>,
        package_store: &PackageStorage,
    ) -> Result<()> {
        let mut total_base = (0, 0);
        let mut total_bin = (0, 0);
        let mut total_pkg = (0, 0);
        let mut total = (0, 0);

        let resolved_packages = packages
            .map(|pkgs| {
                pkgs.iter()
                    .flat_map(|package| {
                        let query = parse_package_query(package);
                        package_store
                            .get_packages(&query)
                            .unwrap_or_default()
                            .into_iter()
                            .filter_map(|package| self.find_package(&package).cloned())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| self.packages.clone());

        if resolved_packages.is_empty() {
            return Err(anyhow::anyhow!("No installed packages"));
        }

        resolved_packages.iter().for_each(|package| {
            println!(
                "- [{}] {}:{}-{} ({}) ({})",
                package.root_path,
                package.name,
                package.name,
                package.version,
                package.timestamp.format("%Y-%m-%d %H:%M:%S"),
                format_bytes(package.size)
            );

            match package.root_path {
                RootPath::Bin => total_bin = (total_bin.0 + 1, total_bin.1 + package.size),
                RootPath::Base => total_base = (total_base.0 + 1, total_base.1 + package.size),
                RootPath::Pkg => total_pkg = (total_pkg.0 + 1, total_pkg.1 + package.size),
            }
            total = (total.0 + 1, total.1 + package.size);
        });
        println!();
        println!("{:<2} Installed:", "");
        println!(
            "{:<4} base: {} ({})",
            "",
            total_base.0,
            format_bytes(total_base.1)
        );
        println!(
            "{:<4} bin: {} ({})",
            "",
            total_bin.0,
            format_bytes(total_bin.1)
        );
        println!(
            "{:<4} pkg: {} ({})",
            "",
            total_pkg.0,
            format_bytes(total_pkg.1)
        );
        println!("{:<2} Total: {} ({})", "", total.0, format_bytes(total.1));

        Ok(())
    }

    pub fn reverse_package_search(&self, path: &Path) -> Option<InstalledPackage> {
        let path_str = path.to_string_lossy();
        if path_str.len() > 64 {
            let checksum = &path_str[..64];
            self.packages
                .iter()
                .find(|package| package.checksum == checksum)
                .cloned()
        } else {
            None
        }
    }

    pub async fn use_package(&self, resolved_package: &ResolvedPackage) -> Result<()> {
        if let Some(installed) = self.find_package(resolved_package) {
            let install_path = resolved_package
                .package
                .get_install_path(&installed.checksum);
            let symlink_path = &BIN_PATH.join(&installed.bin_name);

            if symlink_path.exists() {
                fs::remove_file(symlink_path).await?;
            }

            fs::symlink(&install_path, symlink_path)
                .await
                .context(format!(
                    "Failed to link {} to {}",
                    install_path.to_string_lossy(),
                    symlink_path.to_string_lossy()
                ))?;
        } else {
            return Err(anyhow::anyhow!("NOT_INSTALLED"));
        }

        Ok(())
    }
}
