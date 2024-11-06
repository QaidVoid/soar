use std::path::Path;

use anyhow::{Context, Result};
use tokio::fs;

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::BIN_PATH,
    },
    package::appimage::remove_applinks,
    registry::installed::{InstalledPackage, InstalledPackages},
    successln,
};

pub struct Remover {
    package: InstalledPackage,
}

impl Remover {
    pub async fn new(package: &InstalledPackage) -> Result<Self> {
        Ok(Self {
            package: package.clone(),
        })
    }

    pub async fn execute(&self, installed_packages: &mut InstalledPackages) -> Result<()> {
        let package = &self.package;

        let install_dir = package.get_install_dir();
        let install_path = package.get_install_path();
        self.remove_symlink(&install_path).await?;
        remove_applinks(&package.name, &package.bin_name, &install_path).await?;
        self.remove_package_path(&install_dir).await?;
        installed_packages.unregister_package(&self.package).await?;

        successln!(
            "Package {} removed successfully.",
            package.full_name('/').color(Color::Blue)
        );

        Ok(())
    }

    pub async fn remove_symlink(&self, install_path: &Path) -> Result<()> {
        let package = &self.package;
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
                install_dir.to_string_lossy().color(Color::Blue)
            ))?;
        }

        Ok(())
    }
}
