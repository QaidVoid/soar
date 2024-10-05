use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::constant::INSTALL_TRACK_PATH;

use super::registry::{ResolvedPackage, RootPath};

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
        let path = INSTALL_TRACK_PATH.join("latest");

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

        let content = rmp_serde::to_vec(&self)
            .context("Failed to serialize installed packages to MessagePack")?;

        tokio::fs::write(&path, content)
            .await
            .with_context(|| format!("Failed to write to {}", path.to_string_lossy()))?;

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
}
