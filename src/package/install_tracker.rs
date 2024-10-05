use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::{
    config::CONFIG,
    constant::{BIN_PATH, INSTALL_TRACK_PATH},
    util::build_path,
};

use super::registry::{PackageRegistry, ResolvedPackage, RootPath};

#[derive(Debug, Deserialize, Serialize)]
pub struct InstalledPackage {
    pub root_path: RootPath,
    pub variant: Option<String>,
    pub package_name: String,
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
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create installs directory")?;
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

    pub async fn register_package(&mut self, resolved_package: &ResolvedPackage) -> Result<()> {
        if let Some(installed) = self.find_package_mut(resolved_package) {
            installed.version = resolved_package.package.version.clone();
            installed.checksum = resolved_package.package.bsum.clone();
        } else {
            let package = resolved_package.package.to_owned();
            let new_installed = InstalledPackage {
                root_path: resolved_package.root_path.to_owned(),
                variant: package.variant,
                package_name: package.name,
                bin_name: package.bin_name,
                version: package.version,
                checksum: package.bsum,
            };
            self.packages.push(new_installed);
        }

        self.save().await?;

        Ok(())
    }

    fn find_package_mut(
        &mut self,
        resolved_package: &ResolvedPackage,
    ) -> Option<&mut InstalledPackage> {
        self.packages.iter_mut().find(|installed| {
            installed.package_name == resolved_package.package.name
                && installed.root_path == resolved_package.root_path
                && installed.variant == resolved_package.package.variant
        })
    }

    pub async fn remove_package(
        &mut self,
        registry: PackageRegistry,
        package_names: &[String],
    ) -> Result<()> {
        let packages = registry.parse_packages_from_names(package_names)?;

        for package in &packages {
            let exists = self.packages.iter().any(|installed| {
                installed.package_name == package.package.name
                    && installed.root_path == package.root_path
                    && installed.variant == package.package.variant
            });

            let variant_prefix = package
                .package
                .variant
                .clone()
                .map(|variant| format!("{}-", variant))
                .unwrap_or_default();

            if !exists {
                eprintln!(
                    "Package {}{}-{} is not installed.",
                    variant_prefix, package.package.name, package.package.version
                );
                continue;
            }

            let package_path = build_path(&CONFIG.soar_path)?
                .join("packages")
                .join(format!(
                    "{}{}-{}",
                    variant_prefix, package.package.name, package.package.version
                ));

            let symlink_path = BIN_PATH.join(&package.package.bin_name);
            if symlink_path.exists() {
                let target = tokio::fs::read_link(&symlink_path).await?;
                if target == package.install_path()? {
                    tokio::fs::remove_file(&symlink_path).await?;
                }
            }

            if package_path.exists() {
                tokio::fs::remove_dir_all(&package_path)
                    .await
                    .with_context(|| {
                        format!("Failed to remove package file: {:?}", package_path)
                    })?;
            }

            self.packages.retain(|installed| {
                !(installed.package_name == package.package.name
                    && installed.root_path == package.root_path
                    && installed.variant == package.package.variant)
            });

            // HACK: not effective but should update install tracker properly
            self.save().await?;
            println!(
                "Package {}{} removed successfully.",
                variant_prefix, package.package.name
            )
        }

        Ok(())
    }

    async fn save(&self) -> Result<()> {
        let path = INSTALL_TRACK_PATH.join("latest");

        let content = rmp_serde::to_vec(&self)
            .context("Failed to serialize installed packages to MessagePack")?;

        tokio::fs::write(&path, content)
            .await
            .with_context(|| format!("Failed to write to {}", path.to_string_lossy()))?;

        Ok(())
    }
}
