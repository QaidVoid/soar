use std::{fmt::Display, io::Write, sync::Arc};

use anyhow::Result;

use fetcher::RegistryFetcher;
use installed::InstalledPackages;
use loader::RegistryLoader;
use package::{update::Updater, ResolvedPackage, RootPath};
use serde::Deserialize;
use storage::{PackageStorage, RepositoryPackages};
use tokio::sync::Mutex;

use crate::core::config::CONFIG;

mod fetcher;
pub mod installed;
mod loader;
pub mod package;
mod storage;

pub struct PackageRegistry {
    fetcher: RegistryFetcher,
    pub storage: PackageStorage,
    pub installed_packages: Arc<Mutex<InstalledPackages>>,
}

impl PackageRegistry {
    pub async fn new() -> Result<Self> {
        let loader = RegistryLoader::new();
        let fetcher = RegistryFetcher::new();
        let mut storage = PackageStorage::new();
        let installed_packages = Arc::new(Mutex::new(InstalledPackages::new().await?));

        Self::load_or_fetch_packages(&loader, &fetcher, &mut storage).await?;

        Ok(Self {
            fetcher,
            storage,
            installed_packages,
        })
    }

    pub async fn load_or_fetch_packages(
        loader: &RegistryLoader,
        fetcher: &RegistryFetcher,
        storage: &mut PackageStorage,
    ) -> Result<()> {
        for repo in &CONFIG.repositories {
            let path = repo.get_path();
            let content = if path.exists() {
                loader.execute(repo).await?
            } else {
                fetcher.execute(repo).await?
            };

            let mut de = rmp_serde::Deserializer::new(&content[..]);
            let packages = RepositoryPackages::deserialize(&mut de)?;

            storage.add_repository(&repo.name, packages);
        }

        Ok(())
    }

    pub async fn fetch(&mut self) -> Result<()> {
        for repo in &CONFIG.repositories {
            let content = self.fetcher.execute(repo).await?;

            let mut de = rmp_serde::Deserializer::new(&content[..]);
            let packages = RepositoryPackages::deserialize(&mut de)?;

            self.storage.add_repository(&repo.name, packages);
        }

        Ok(())
    }

    pub async fn install_packages(
        &self,
        package_names: &[String],
        force: bool,
        is_update: bool,
    ) -> Result<()> {
        self.storage
            .install_packages(
                package_names,
                force,
                is_update,
                self.installed_packages.clone(),
            )
            .await
    }

    pub async fn remove_packages(&self, package_names: &[String]) -> Result<()> {
        self.storage.remove_packages(package_names).await
    }

    pub async fn search(&self, package_name: &str) -> Result<()> {
        let installed_guard = self.installed_packages.lock().await;
        let result = self.storage.search(package_name).await;

        if result.is_empty() {
            Err(anyhow::anyhow!("No packages found"))
        } else {
            result.iter().for_each(|pkg| {
                let installed = if installed_guard.is_installed(pkg) {
                    "[Installed]"
                } else {
                    ""
                };
                println!(
                    "[{}] {}: {} {}",
                    pkg.root_path,
                    pkg.package.full_name('/'),
                    pkg.package.description,
                    installed
                );
            });
            Ok(())
        }
    }

    pub async fn query(&self, package_name: &str) -> Result<()> {
        let installed_guard = self.installed_packages.lock().await;
        let result = self.storage.search(package_name).await;

        if result.is_empty() {
            Err(anyhow::anyhow!("No packages found"))
        } else {
            for pkg in result {
                let installed_pkg = installed_guard.find_package(&pkg);
                let print_data = |key: &str, value: &dyn Display| {
                    println!("{:<16}: {}", key, value);
                };
                let data: Vec<(&str, &dyn Display)> = vec![
                    ("Root Path", &pkg.root_path),
                    ("Name", &pkg.package.name),
                    ("Binary", &pkg.package.bin_name),
                    ("Description", &pkg.package.description),
                    ("Version", &pkg.package.version),
                    ("Download URL", &pkg.package.download_url),
                    ("Size", &pkg.package.size),
                    ("Checksum", &pkg.package.bsum),
                    ("Build Date", &pkg.package.build_date),
                    ("Build Log", &pkg.package.build_log),
                    ("Build Script", &pkg.package.build_script),
                    ("Category", &pkg.package.category),
                    ("Extra Bins", &pkg.package.extra_bins),
                ];

                data.iter().for_each(|(k, v)| {
                    print_data(k, v);
                });

                if let Some(installed) = installed_pkg {
                    print_data(
                        "Install Path",
                        &pkg.package
                            .get_install_path(&installed.checksum)
                            .to_string_lossy(),
                    );
                    print_data(
                        "Install Date",
                        &installed.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    );
                }
                println!();
            }
            Ok(())
        }
    }

    pub async fn update(&self, package_names: Option<&[String]>) -> Result<()> {
        let mut installed_guard = self.installed_packages.lock().await;
        let updater = Updater::new(package_names);
        updater.execute(self, &mut installed_guard).await
    }

    pub async fn info(&self, package_names: Option<&[String]>) -> Result<()> {
        let installed_guard = self.installed_packages.lock().await;
        installed_guard.info(package_names, &self.storage)
    }

    pub async fn list(&self, root_path: Option<&str>) -> Result<()> {
        let root_path = match root_path {
            Some(rp) => match rp.to_lowercase().as_str() {
                "base" => Ok(Some(RootPath::Base)),
                "bin" => Ok(Some(RootPath::Bin)),
                "pkg" => Ok(Some(RootPath::Pkg)),
                _ => Err(anyhow::anyhow!("Invalid root path: {}", rp)),
            },
            None => Ok(None),
        }?;

        let packages = self.storage.list_packages(root_path);
        for resolved_package in packages {
            let package = resolved_package.package.clone();
            let variant_prefix = package
                .variant
                .map(|variant| format!("{}-", variant))
                .unwrap_or_default();
            let installed_guard = self.installed_packages.lock().await;
            let install_prefix = if installed_guard.is_installed(&resolved_package) {
                "+"
            } else {
                "-"
            };
            println!(
                "[{}] [{}] {}{}:{}-{} ({})",
                install_prefix,
                resolved_package.root_path,
                variant_prefix,
                package.name,
                package.name,
                package.version,
                package.size
            );
        }
        Ok(())
    }

    pub async fn inspect(&self, package_name: &str) -> Result<()> {
        self.storage.inspect(package_name).await
    }

    pub async fn run(&self, command: &[String]) -> Result<()> {
        self.storage.run(command).await
    }
}

pub fn select_package_variant(packages: &[ResolvedPackage]) -> Result<&ResolvedPackage> {
    println!(
        "Multiple packages available for {}",
        packages[0].package.name
    );
    for (i, package) in packages.iter().enumerate() {
        println!(
            "  [{}] [{}] {}: {}",
            i + 1,
            package.root_path,
            package.package.full_name('/'),
            package.package.description
        );
    }

    let selection = loop {
        print!("Select a variant (1-{}): ", packages.len());
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        match input.trim().parse::<usize>() {
            Ok(n) if n > 0 && n <= packages.len() => break n - 1,
            _ => println!("Invalid selection, please try again."),
        }
    };
    println!();

    Ok(&packages[selection])
}
