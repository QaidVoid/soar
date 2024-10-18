use std::{io::Write, sync::Arc};

use anyhow::{Context, Result};

use fetcher::RegistryFetcher;
use futures::future::try_join_all;
use installed::InstalledPackages;
use loader::RegistryLoader;
use package::{
    image::PackageImage, parse_package_query, update::Updater, ResolvedPackage, RootPath,
};
use serde::Deserialize;
use storage::{PackageStorage, RepositoryPackages};
use tokio::{fs, sync::Mutex};

use crate::{
    core::{
        color::{Color, ColorExt},
        config::CONFIG,
        constant::REGISTRY_PATH,
        util::{download, wrap_text},
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

            // fetch default icons
            let icon_futures: Vec<_> = repo
                .paths
                .iter()
                .map(|(key, base_url)| {
                    let base_url = format!("{}/{}.default.png", base_url, key);
                    async move { download(&base_url, "icon", true).await }
                })
                .collect();
            let icons = try_join_all(icon_futures).await?;

            for (key, icon) in repo.paths.keys().zip(icons) {
                let icon_path = REGISTRY_PATH
                    .join("icons")
                    .join(format!("{}-{}.png", repo.name, key));

                if let Some(parent) = icon_path.parent() {
                    fs::create_dir_all(parent).await.context(anyhow::anyhow!(
                        "Failed to create icon directory at {}",
                        parent.to_string_lossy().color(Color::Blue)
                    ))?;
                }

                fs::write(icon_path, icon).await?;
            }
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
            let package = &pkg.package;

            let formatted_name = format!(
                "{} ({}#{})",
                package.name.clone().color(Color::BrightGreen),
                package.clone().full_name('/').color(Color::BrightCyan),
                pkg.root_path.clone().color(Color::BrightRed)
            );
            let mut data: Vec<(&str, String)> = vec![
                ("Name", formatted_name),
                (
                    "Description",
                    package.description.clone().color(Color::BrightYellow),
                ),
                ("Homepage", package.web_url.clone().color(Color::BrightBlue)),
                ("Source", package.src_url.clone().color(Color::BrightBlue)),
                (
                    "Version",
                    package.version.clone().color(Color::BrightMagenta),
                ),
                ("Checksum", package.bsum.clone().color(Color::BrightMagenta)),
                ("Size", package.size.clone().color(Color::BrightMagenta)),
                (
                    "Download URL",
                    package.download_url.clone().color(Color::BrightBlue),
                ),
                (
                    "Build Date",
                    package.build_date.clone().color(Color::BrightMagenta),
                ),
                (
                    "Build Log",
                    package.build_log.clone().color(Color::BrightBlue),
                ),
                (
                    "Build Script",
                    package.build_script.clone().color(Color::BrightBlue),
                ),
                (
                    "Category",
                    package.category.clone().color(Color::BrightCyan),
                ),
                (
                    "Extra Bins",
                    package.extra_bins.clone().color(Color::BrightBlack),
                ),
            ];

            if let Some(installed) = installed_pkg {
                data.push((
                    "Install Path",
                    package
                        .clone()
                        .get_install_path(&installed.checksum)
                        .to_string_lossy()
                        .to_string()
                        .color(Color::BrightGreen),
                ));
                data.push((
                    "Install Date",
                    installed
                        .timestamp
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                        .color(Color::BrightMagenta),
                ));
            }

            let pkg_image = PackageImage::from(&pkg).await;

            let mut printable = Vec::new();
            match pkg_image {
                PackageImage::Sixel(img) => {
                    printable.extend(format!("{:<2}{}\x1B\\", "", img).as_bytes());
                    printable.extend(format!("\x1B[{}A", 15).as_bytes());
                    printable.extend(format!("\x1B[{}C", 32).as_bytes());

                    data.iter().for_each(|(k, v)| {
                        let value = strip_ansi_escapes::strip(v);
                        let value = String::from_utf8(value).unwrap();

                        if !value.is_empty() && value != "null" {
                            let line =
                                wrap_text(&format!("{}: {}", k.color(Color::Red).bold(), v), 4);
                            printable.extend(format!("\x1B[s{}\x1B[u\x1B[1B", line).as_bytes());
                        }
                    });
                    printable.extend(format!("\n\x1B[{}B", 16).as_bytes());
                    println!("{}", String::from_utf8(printable).unwrap());
                }
                PackageImage::Kitty(img) => {
                    printable.extend(format!("{:<2}{}\x1B\\", "", img).as_bytes());
                    printable.extend(format!("\x1B[{}A", 15).as_bytes());

                    data.iter().for_each(|(k, v)| {
                        let value = strip_ansi_escapes::strip(v);
                        let value = String::from_utf8(value).unwrap();

                        if !value.is_empty() && value != "null" {
                            let line =
                                wrap_text(&format!("{}: {}", k.color(Color::Red).bold(), v), 4);
                            printable.extend(format!("\x1B[s{}\x1B[u\x1B[1B", line).as_bytes());
                        }
                    });
                    printable.extend(format!("\n\x1B[{}B", 16).as_bytes());
                    println!("{}", String::from_utf8(printable).unwrap());
                }
                _ => {
                    data.iter().for_each(|(k, v)| {
                        let value = strip_ansi_escapes::strip(v);
                        let value = String::from_utf8(value).unwrap();

                        if !value.is_empty() && value != "null" {
                            let line =
                                wrap_text(&format!("{}: {}", k.color(Color::Red).bold(), v), 0);
                            println!("{}", line);
                        }
                    });
                }
            };
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
