use std::sync::Arc;

use anyhow::Result;
use indicatif::MultiProgress;
use tokio::sync::MutexGuard;

use crate::{
    core::color::{Color, ColorExt},
    error,
    registry::{installed::InstalledPackages, PackageRegistry},
    success,
};

use super::{parse_package_query, PackageQuery, ResolvedPackage};

pub struct Updater {
    package_names: Option<Vec<String>>,
}

impl Updater {
    pub fn new(package_names: Option<&[String]>) -> Self {
        Self {
            package_names: package_names.map(|names| names.to_vec()),
        }
    }

    pub async fn execute(
        &self,
        registry: &PackageRegistry,
        installed_packages: &mut MutexGuard<'_, InstalledPackages>,
    ) -> Result<()> {
        let packages = match &self.package_names {
            Some(r) => {
                let resolved_packages: Result<Vec<ResolvedPackage>> = r
                    .iter()
                    .map(|package_name| registry.storage.resolve_package(package_name))
                    .collect();
                resolved_packages?
            }
            None => installed_packages
                .packages
                .iter()
                .filter_map(|installed| {
                    let pkg = parse_package_query(&installed.name);
                    let query = PackageQuery {
                        collection: Some(installed.collection.clone()),
                        ..pkg
                    };
                    registry
                        .storage
                        .get_packages(&query)
                        .and_then(|v| v.into_iter().next())
                })
                .collect::<Vec<_>>(),
        };

        let mut packages_to_update: Vec<ResolvedPackage> = Vec::new();

        let multi_progress = Arc::new(MultiProgress::new());
        for package in packages {
            if let Some(installed_package) = installed_packages.packages.iter().find(|installed| {
                installed.repo_name == package.repo_name
                    && installed.name == package.package.full_name('/')
                    && installed.collection == package.collection
            }) {
                if installed_package.checksum != package.package.bsum {
                    packages_to_update.push(package);
                }
            } else {
                error!(
                    "Package {} is not installed.",
                    package.package.full_name('/').color(Color::Blue)
                );
            }
        }

        if packages_to_update.is_empty() {
            error!("No updates available");
        } else {
            let mut update_count = 0;
            for (idx, package) in packages_to_update.iter().enumerate() {
                package
                    .install(
                        idx,
                        packages_to_update.len(),
                        true,
                        registry.installed_packages.clone(),
                        None,
                        None,
                        None,
                        multi_progress.clone()
                    )
                    .await?;
                update_count += 1;
            }
            success!(
                "{} packages updated.",
                update_count.color(Color::BrightMagenta)
            );
        }

        Ok(())
    }
}
