use std::path::Path;

use anyhow::{Context, Result};
use tokio::fs;

use crate::{core::constant::BIN_PATH, registry::installed::InstalledPackages};

use super::ResolvedPackage;

pub struct Remover {
    resolved_package: ResolvedPackage,
}

impl Remover {
    pub async fn new(resolved_package: &ResolvedPackage) -> Result<Self> {
        Ok(Self {
            resolved_package: resolved_package.to_owned(),
        })
    }

    pub async fn execute(&self, installed_packages: &mut InstalledPackages) -> Result<()> {
        let package = &self.resolved_package.package;
        let installed = installed_packages.find_package(&self.resolved_package);
        let Some(installed) = installed else {
            return Err(anyhow::anyhow!(
                "Package {}-{} is not installed.",
                package.full_name('/'),
                package.version
            ));
        };

        let install_dir = package.get_install_dir(&installed.checksum);
        let install_path = package.get_install_path(&installed.checksum);
        self.remove_symlink(&install_path).await?;
        self.remove_package_path(&install_dir).await?;
        installed_packages
            .unregister_package(&self.resolved_package)
            .await?;

        println!("Package {} removed successfully.", package.full_name('/'));

        Ok(())
    }

    pub async fn remove_symlink(&self, install_path: &Path) -> Result<()> {
        let package = &self.resolved_package.package;
        let symlink_path = BIN_PATH.join(&package.bin_name);
        if symlink_path.exists() {
            let target = fs::read_link(&symlink_path).await?;
            if target == install_path {
                fs::remove_file(&symlink_path).await?;
            }
        }

        Ok(())
    }

    pub async fn remove_package_path(&self, install_dir: &Path) -> Result<()> {
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).await.context(format!(
                "Failed to remove package file: {}",
                install_dir.to_string_lossy()
            ))?;
        }

        Ok(())
    }
}
