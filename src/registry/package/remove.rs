use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::fs;

use crate::{core::constant::BIN_PATH, registry::installed::InstalledPackages};

use super::ResolvedPackage;

pub struct Remover {
    resolved_package: ResolvedPackage,
    install_dir: PathBuf,
}

impl Remover {
    pub async fn new(resolved_package: &ResolvedPackage, install_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            resolved_package: resolved_package.to_owned(),
            install_dir,
        })
    }

    pub async fn execute(&self, installed_packages: &mut InstalledPackages) -> Result<()> {
        let is_installed = installed_packages.is_installed(&self.resolved_package);
        let package = &self.resolved_package.package;

        if !is_installed {
            return Err(anyhow::anyhow!(
                "Package {}-{} is not installed.",
                package.full_name(),
                package.version
            ));
        }

        self.remove_symlink().await?;
        self.remove_package_path().await?;
        installed_packages
            .unregister_package(&self.resolved_package)
            .await?;

        println!("Package {} removed successfully.", package.full_name());

        Ok(())
    }

    pub async fn remove_symlink(&self) -> Result<()> {
        let package = &self.resolved_package.package;
        let install_path = &self.install_dir.join("bin").join(&package.bin_name);
        let symlink_path = BIN_PATH.join(&package.bin_name);
        if symlink_path.exists() {
            let target = fs::read_link(&symlink_path).await?;
            if &target == install_path {
                fs::remove_file(&symlink_path).await?;
            }
        }

        Ok(())
    }

    pub async fn remove_package_path(&self) -> Result<()> {
        let install_dir = &self.install_dir;
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).await.context(format!(
                "Failed to remove package file: {}",
                install_dir.to_string_lossy()
            ))?;
        }

        Ok(())
    }
}
