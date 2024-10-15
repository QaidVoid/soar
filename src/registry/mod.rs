use std::{fmt::Display, io::Write, path::PathBuf, sync::Arc};

use anyhow::Result;

use fetcher::RegistryFetcher;
use installed::InstalledPackages;
use loader::RegistryLoader;
use package::{parse_package_query, update::Updater, ResolvedPackage, RootPath};
use serde::Deserialize;
use storage::{PackageStorage, RepositoryPackages};
use tokio::sync::Mutex;

use crate::{
    core::{
        color::{Color, ColorExt},
        config::CONFIG,
    },
    error, info, success,
};

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
            let packages = match RepositoryPackages::deserialize(&mut de) {
                Ok(packages) => packages,
                Err(_) => {
                    error!("Registry is invalid. Refetching...");
                    let content = fetcher.execute(repo).await?;
                    let mut de = rmp_serde::Deserializer::new(&content[..]);
                    RepositoryPackages::deserialize(&mut de)?
                }
            };
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
        portable: Option<String>,
        portable_home: Option<String>,
        portable_config: Option<String>,
    ) -> Result<()> {
        self.storage
            .install_packages(
                package_names,
                force,
                is_update,
                self.installed_packages.clone(),
                portable,
                portable_home,
                portable_config,
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
                    "+"
                } else {
                    "-"
                };
                println!(
                    "[{}] [{}] {}: {}",
                    installed,
                    pkg.root_path.clone().color(Color::BrightGreen),
                    pkg.package.full_name('/').color(Color::Blue),
                    pkg.package.description,
                );
            });
            Ok(())
        }
    }

    pub async fn query(&self, package_name: &str) -> Result<()> {
        let installed_guard = self.installed_packages.lock().await;
        let query = parse_package_query(package_name);
        let result = self.storage.get_packages(&query);

        let Some(result) = result else {
            return Err(anyhow::anyhow!("No packages found"));
        };

        for pkg in result {
            let installed_pkg = installed_guard.find_package(&pkg);
            let print_data = |key: &str, value: &dyn Display| {
                println!("{}: {}", key, value);
            };
            let root_path = pkg.root_path.clone().to_string();
            let data: Vec<(String, &str)> = vec![
                ("Root Path".color(Color::Blue), &root_path),
                ("Name".color(Color::Green), &pkg.package.name),
                ("Binary".color(Color::Red), &pkg.package.bin_name),
                ("Description".color(Color::Yellow), &pkg.package.description),
                ("Version".color(Color::Magenta), &pkg.package.version),
                (
                    "Download URL".color(Color::Green),
                    &pkg.package.download_url,
                ),
                ("Size".color(Color::Blue), &pkg.package.size),
                ("Checksum".color(Color::Yellow), &pkg.package.bsum),
                ("Build Date".color(Color::Magenta), &pkg.package.build_date),
                ("Build Log".color(Color::Red), &pkg.package.build_log),
                ("Build Script".color(Color::Blue), &pkg.package.build_script),
                ("Category".color(Color::Yellow), &pkg.package.category),
                ("Extra Bins".color(Color::Green), &pkg.package.extra_bins),
            ];

            data.iter().for_each(|(k, v)| {
                if !v.is_empty() && v != &"null" {
                    print_data(k.as_str(), v);
                }
            });

            if let Some(installed) = installed_pkg {
                print_data(
                    &"Install Path".color(Color::Magenta),
                    &pkg.package
                        .get_install_path(&installed.checksum)
                        .to_string_lossy(),
                );
                print_data(
                    &"Install Date".color(Color::Red),
                    &installed.timestamp.format("%Y-%m-%d %H:%M:%S"),
                );
            }
            println!();
        }
        Ok(())
    }

    pub async fn update(&self, package_names: Option<&[String]>) -> Result<()> {
        let mut installed_guard = self.installed_packages.lock().await;
        let updater = Updater::new(package_names);
        updater.execute(self, &mut installed_guard).await
    }

    pub async fn info(&self, package_names: Option<&[String]>) -> Result<()> {
        if let Some([package]) = package_names {
            return self.query(package).await;
        }
        let installed_guard = self.installed_packages.lock().await;
        installed_guard.info(package_names, &self.storage).await
    }

    pub async fn list(&self, root_path: Option<&str>) -> Result<()> {
        let root_path = match root_path {
            Some(rp) => match rp.to_lowercase().as_str() {
                "base" => Ok(Some(RootPath::Base)),
                "bin" => Ok(Some(RootPath::Bin)),
                "pkg" => Ok(Some(RootPath::Pkg)),
                _ => Err(anyhow::anyhow!(
                    "Invalid root path: {}",
                    rp.color(Color::BrightGreen)
                )),
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
                "[{0}] [{1}] {2}{3}:{3}-{4} ({5})",
                install_prefix.color(Color::Red),
                resolved_package.root_path.color(Color::BrightGreen),
                variant_prefix.color(Color::Blue),
                package.name.color(Color::Blue),
                package.version.color(Color::Green),
                package.size.color(Color::Magenta)
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

    pub async fn use_package(&self, package_name: &str) -> Result<()> {
        let installed_guard = self.installed_packages.lock().await;
        let resolved_package = self.storage.resolve_package(package_name)?;
        let result = installed_guard.use_package(&resolved_package).await;
        drop(installed_guard);
        match result {
            Ok(_) => {
                success!(
                    "{} is linked to binary path",
                    package_name.color(Color::Blue)
                );
                Ok(())
            }
            Err(e) => {
                if e.to_string() == "NOT_INSTALLED" {
                    error!("Package is not yet installed.");
                    let package_name = resolved_package.package.full_name('/');
                    self.storage
                        .install_packages(
                            &[package_name.to_owned()],
                            true,
                            false,
                            self.installed_packages.clone(),
                            None,
                            None,
                            None,
                        )
                        .await?;

                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }
}

pub fn select_package_variant(packages: &[ResolvedPackage]) -> Result<&ResolvedPackage> {
    info!(
        "Multiple packages available for {}",
        packages[0].package.name.clone().color(Color::Blue)
    );
    for (i, package) in packages.iter().enumerate() {
        println!(
            "  [{}] [{}] {}: {}",
            i + 1,
            package.root_path.clone().color(Color::BrightGreen),
            package.package.full_name('/').color(Color::Blue),
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
            _ => error!("Invalid selection, please try again."),
        }
    };
    println!();

    Ok(&packages[selection])
}
