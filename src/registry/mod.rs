use std::{io::Write, sync::Arc};

use anyhow::Result;

use fetcher::MetadataFetcher;
use installed::InstalledPackages;
use loader::MetadataLoader;
use serde::Deserialize;
use storage::{PackageStorage, RepositoryPackages};
use termion::cursor;
use tokio::{fs, sync::Mutex};

use crate::{
    core::{
        color::{Color, ColorExt},
        config::CONFIG,
        util::{get_terminal_width, wrap_text},
    },
    error, infoln,
    package::{
        image::get_package_image_string, parse_package_query, update::Updater, ResolvedPackage,
    },
    successln,
};

mod fetcher;
pub mod installed;
mod loader;
mod storage;

pub struct PackageRegistry {
    pub storage: PackageStorage,
    pub installed_packages: Arc<Mutex<InstalledPackages>>,
}

impl PackageRegistry {
    pub async fn new() -> Result<Self> {
        let loader = MetadataLoader::new();
        let fetcher = MetadataFetcher::new();
        let mut storage = PackageStorage::new();
        let installed_packages = Arc::new(Mutex::new(InstalledPackages::new().await?));

        Self::load_or_fetch_packages(&loader, &fetcher, &mut storage).await?;

        Ok(Self {
            storage,
            installed_packages,
        })
    }

    pub async fn load_or_fetch_packages(
        loader: &MetadataLoader,
        fetcher: &MetadataFetcher,
        storage: &mut PackageStorage,
    ) -> Result<()> {
        for repo in &CONFIG.repositories {
            let path = repo.get_path();
            let content = if path.exists() {
                loader.execute(repo, fetcher).await?
            } else {
                let checksum = fetcher.checksum(repo).await?;
                let checksum_path = repo
                    .get_path()
                    .with_file_name(format!("{}.remote.bsum", repo.name));
                fs::write(checksum_path, &checksum).await?;

                fetcher.execute(repo).await?
            };

            let mut de = rmp_serde::Deserializer::new(&content[..]);
            let packages = match RepositoryPackages::deserialize(&mut de) {
                Ok(packages) => packages,
                Err(_) => {
                    error!("Metadata is invalid. Refetching...");
                    let content = fetcher.execute(repo).await?;
                    let mut de = rmp_serde::Deserializer::new(&content[..]);
                    RepositoryPackages::deserialize(&mut de)?
                }
            };
            storage.add_repository(&repo.name, packages);
        }

        Ok(())
    }

    pub async fn install_packages(
        &self,
        package_names: &[String],
        force: bool,
        portable: Option<String>,
        portable_home: Option<String>,
        portable_config: Option<String>,
        yes: bool,
    ) -> Result<()> {
        self.storage
            .install_packages(
                package_names,
                force,
                self.installed_packages.clone(),
                portable,
                portable_home,
                portable_config,
                yes,
            )
            .await
    }

    pub async fn remove_packages(&self, package_names: &[String], exact: bool) -> Result<()> {
        self.storage
            .remove_packages(package_names, self.installed_packages.clone(), exact)
            .await
    }

    pub async fn search(&self, package_name: &str, case_sensitive: bool) -> Result<()> {
        let installed_guard = self.installed_packages.lock().await;
        let result = self.storage.search(package_name, case_sensitive).await;

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
                    pkg.collection.clone().color(Color::BrightGreen),
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
                package.pkg_name.clone().color(Color::BrightGreen),
                package.clone().full_name('/').color(Color::BrightCyan),
                pkg.collection.clone().color(Color::BrightRed)
            );
            let mut data: Vec<(&str, String)> = vec![
                ("Name", formatted_name),
                (
                    "Description",
                    package.description.clone().color(Color::BrightYellow),
                ),
                (
                    "Homepage",
                    package.homepage.clone().color(Color::BrightBlue),
                ),
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
                ("Note", package.note.clone().color(Color::BrightCyan)),
                (
                    "Category",
                    package.category.clone().color(Color::BrightCyan),
                ),
                (
                    "Extra Bins",
                    package.provides.clone().color(Color::BrightBlack),
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

            let pkg_image = get_package_image_string(&pkg).await;

            let mut printable = Vec::new();
            let indent = 32;

            printable.extend(pkg_image.as_bytes());
            printable.extend(cursor::Up(15).to_string().as_bytes());
            printable.extend(cursor::Right(indent).to_string().as_bytes());

            data.iter().for_each(|(k, v)| {
                let value = strip_ansi_escapes::strip_str(v);

                if !value.is_empty() && value != "null" {
                    let available_width = get_terminal_width() - indent as usize;
                    let line = wrap_text(
                        &format!("{}: {}", k.color(Color::Red).bold(), v),
                        available_width,
                        indent,
                    );

                    printable.extend(format!("{}\n", line).as_bytes());
                    printable.extend(cursor::Right(indent).to_string().as_bytes());
                }
            });

            printable.extend(cursor::Down(1).to_string().as_bytes());
            println!("{}", String::from_utf8(printable).unwrap());
        }
        Ok(())
    }

    pub async fn update(&self, package_names: Option<&[String]>) -> Result<()> {
        let updater = Updater::new(package_names);
        updater.execute(self).await
    }

    pub async fn info(&self, package_names: Option<&[String]>) -> Result<()> {
        if let Some([package]) = package_names {
            return self.query(package).await;
        }
        let installed_guard = self.installed_packages.lock().await;
        installed_guard.info(package_names, &self.storage).await
    }

    pub async fn list(&self, collection: Option<&str>) -> Result<()> {
        let packages = self.storage.list_packages(collection);
        if packages.is_empty() {
            anyhow::bail!("No packages found");
        }
        for resolved_package in packages {
            let package = resolved_package.package.clone();
            let installed_guard = self.installed_packages.lock().await;
            let install_prefix = if installed_guard.is_installed(&resolved_package) {
                "+"
            } else {
                "-"
            };
            println!(
                "[{0}] [{1}] {2}:{3}-{4} ({5})",
                install_prefix.color(Color::Red),
                resolved_package.collection.color(Color::BrightGreen),
                package.full_name('/').color(Color::Blue),
                package.pkg.color(Color::Blue),
                package.version.color(Color::Green),
                package.size.color(Color::Magenta)
            );
        }
        Ok(())
    }

    pub async fn inspect(&self, package_name: &str, inspect_type: &str) -> Result<()> {
        self.storage.inspect(package_name, inspect_type).await
    }

    pub async fn run(&self, command: &[String], yes: bool) -> Result<()> {
        self.storage.run(command, yes).await
    }

    pub async fn use_package(&self, package_name: &str) -> Result<()> {
        let installed_guard = self.installed_packages.lock().await;
        let resolved_package = self.storage.resolve_package(package_name, false)?;
        let result = installed_guard.use_package(&resolved_package).await;
        drop(installed_guard);
        match result {
            Ok(_) => {
                successln!(
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
                            self.installed_packages.clone(),
                            None,
                            None,
                            None,
                            false,
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

pub fn select_single_package(packages: &[ResolvedPackage]) -> Result<&ResolvedPackage> {
    infoln!(
        "Multiple packages available for {}",
        packages[0].package.pkg.clone().color(Color::Blue)
    );
    for (i, package) in packages.iter().enumerate() {
        println!(
            "  [{}] [{}] {}: {}",
            i + 1,
            package.collection.clone().color(Color::BrightGreen),
            package.package.full_name('/').color(Color::Blue),
            package.package.description
        );
    }

    let selection = loop {
        print!("Select a package (1-{}): ", packages.len());
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
