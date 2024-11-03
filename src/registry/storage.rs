use std::{
    collections::HashMap,
    io::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use futures::{future::join_all, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    sync::{Mutex, Semaphore},
};

use crate::{
    core::{
        color::{Color, ColorExt},
        config::CONFIG,
        constant::CACHE_PATH,
        util::format_bytes,
    },
    error,
    package::{parse_package_query, run::Runner, Package, PackageQuery, ResolvedPackage},
    registry::installed::InstalledPackages,
    warn,
};

use super::select_single_package;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageStorage {
    repository: HashMap<String, RepositoryPackages>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepositoryPackages {
    #[serde(flatten)]
    pub collection: HashMap<String, HashMap<String, Vec<Package>>>,
}

impl PackageStorage {
    pub fn new() -> Self {
        Self {
            repository: HashMap::new(),
        }
    }

    pub fn add_repository(&mut self, repo_name: &str, packages: RepositoryPackages) {
        self.repository.insert(repo_name.to_owned(), packages);
    }

    pub fn resolve_package(&self, package_name: &str, yes: bool) -> Result<ResolvedPackage> {
        let pkg_query = parse_package_query(package_name);
        let packages = self
            .get_packages(&pkg_query)
            .ok_or_else(|| anyhow::anyhow!("Package {} not found", package_name))?;

        let package = if yes || packages.len() == 1 {
            &packages[0]
        } else {
            select_single_package(&packages)?
        };

        Ok(package.to_owned())
    }

    pub async fn install_packages(
        &self,
        package_names: &[String],
        force: bool,
        installed_packages: Arc<Mutex<InstalledPackages>>,
        portable: Option<String>,
        portable_home: Option<String>,
        portable_config: Option<String>,
        yes: bool,
    ) -> Result<()> {
        let resolved_packages: Result<Vec<ResolvedPackage>> = package_names
            .iter()
            .map(|package_name| self.resolve_package(package_name, yes))
            .collect();
        let resolved_packages = resolved_packages?;

        let results: Vec<_> = join_all(resolved_packages.iter().map(|package| {
            let installed_packages = Arc::clone(&installed_packages);
            let package = package.clone();

            async move {
                let is_installed = installed_packages.lock().await.is_installed(&package);
                (package, is_installed)
            }
        }))
        .await;

        let resolved_packages: Vec<ResolvedPackage> = results
            .into_iter()
            .filter_map(|(package, is_installed)| {
                if is_installed {
                    warn!(
                        "{} is already installed - {}",
                        package.package.full_name('/'),
                        if force { "reinstalling" } else { "skipping" }
                    );

                    if force {
                        Some(package)
                    } else {
                        None
                    }
                } else {
                    Some(package)
                }
            })
            .collect();
        let installed_count = Arc::new(AtomicU64::new(0));

        let multi_progress = Arc::new(MultiProgress::new());
        let total_progress_bar =
            multi_progress.add(ProgressBar::new(resolved_packages.len() as u64));

        total_progress_bar
            .set_style(ProgressStyle::with_template("Installing {pos}/{len}").unwrap());

        if CONFIG.parallel.unwrap_or_default() {
            let semaphore = Arc::new(Semaphore::new(CONFIG.parallel_limit.unwrap_or(2) as usize));
            let mut handles = Vec::new();

            let pkgs_len = resolved_packages.len();
            for (idx, package) in resolved_packages.iter().enumerate() {
                let permit = semaphore.clone().acquire_owned().await.unwrap();
                let package = package.clone();
                let ic = installed_count.clone();
                let installed_packages = installed_packages.clone();
                let portable = portable.clone();
                let portable_home = portable_home.clone();
                let portable_config = portable_config.clone();
                let total_pb = total_progress_bar.clone();
                let multi_progress = multi_progress.clone();

                let handle = tokio::spawn(async move {
                    if let Err(e) = package
                        .install(
                            idx,
                            pkgs_len,
                            installed_packages,
                            portable,
                            portable_home,
                            portable_config,
                            multi_progress,
                            yes,
                        )
                        .await
                    {
                        error!("{}", e);
                    } else {
                        ic.fetch_add(1, Ordering::Relaxed);
                        total_pb.inc(1);
                    };
                    drop(permit);
                });

                handles.push(handle);
            }

            for handle in handles {
                handle.await?;
            }
        } else {
            for (idx, package) in resolved_packages.iter().enumerate() {
                if let Err(e) = package
                    .install(
                        idx,
                        resolved_packages.len(),
                        installed_packages.clone(),
                        portable.clone(),
                        portable_home.clone(),
                        portable_config.clone(),
                        multi_progress.clone(),
                        yes,
                    )
                    .await
                {
                    error!("{}", e);
                } else {
                    installed_count.fetch_add(1, Ordering::Relaxed);
                    total_progress_bar.inc(1);
                };
            }
        }

        total_progress_bar.finish_and_clear();
        println!(
            "Installed {}/{} packages",
            installed_count.load(Ordering::Relaxed).color(Color::Blue),
            resolved_packages.len().color(Color::BrightBlue)
        );
        Ok(())
    }

    pub async fn remove_packages(
        &self,
        package_names: &[String],
        installed_packages: Arc<Mutex<InstalledPackages>>,
        exact: bool,
    ) -> Result<()> {
        let mut mut_guard = installed_packages.lock().await;
        let installed_packages = &mut_guard.packages;

        let mut packages_to_remove = Vec::new();
        for package_name in package_names.iter() {
            let query = parse_package_query(package_name);
            let mut matching_packages = Vec::new();

            for package in installed_packages {
                if package.name != query.name {
                    continue;
                }
                if let Some(ref ckey) = query.collection {
                    if package.collection != *ckey {
                        continue;
                    }
                }

                let family_matches = match (&query.family, &package.family) {
                    (None, None) => true,
                    (None, Some(_)) => !exact,
                    (Some(ref query_family), Some(ref package_family)) => {
                        query_family == package_family
                    }
                    _ => false,
                };

                if family_matches {
                    matching_packages.push(package.clone());
                }
            }

            if matching_packages.is_empty() {
                error!("{} is not installed.", package_name);
            } else {
                packages_to_remove.extend(matching_packages);
            }
        }

        for package in packages_to_remove {
            mut_guard.remove(&package).await?;
        }

        Ok(())
    }

    pub fn list_packages(&self, collection: Option<&str>) -> Vec<ResolvedPackage> {
        let mut packages: Vec<ResolvedPackage> = self
            .repository
            .iter()
            .flat_map(|(repo_name, repo_packages)| {
                repo_packages
                    .collection
                    .iter()
                    .filter(|(key, _)| collection.is_none() || Some(key.as_str()) == collection)
                    .flat_map(|(key, collections)| {
                        collections.iter().flat_map(|(_, packages)| {
                            packages.iter().map(|package| ResolvedPackage {
                                repo_name: repo_name.clone(),
                                collection: key.clone(),
                                package: package.clone(),
                            })
                        })
                    })
            })
            .collect();

        packages.sort_by(|a, b| {
            let collection_cmp = a.collection.cmp(&b.collection);
            if collection_cmp == std::cmp::Ordering::Equal {
                a.package.full_name('-').cmp(&b.package.full_name('-'))
            } else {
                collection_cmp
            }
        });
        packages
    }

    pub fn get_packages(&self, query: &PackageQuery) -> Option<Vec<ResolvedPackage>> {
        let pkg_name = query.name.trim();
        let resolved_packages: Vec<ResolvedPackage> = self
            .repository
            .iter()
            .flat_map(|(repo_name, packages)| {
                packages
                    .collection
                    .iter()
                    .filter(|(collection_key, _)| {
                        query.collection.is_none()
                            || Some(collection_key.as_str()) == query.collection.as_deref()
                    })
                    .flat_map(|(collection_key, map)| {
                        map.get(pkg_name).into_iter().flat_map(|pkgs| {
                            pkgs.iter().filter_map(|pkg| {
                                if pkg.name == pkg_name
                                    && (query.family.is_none()
                                        || pkg.family.as_ref() == query.family.as_ref())
                                {
                                    Some(ResolvedPackage {
                                        repo_name: repo_name.to_owned(),
                                        package: pkg.clone(),
                                        collection: collection_key.clone(),
                                    })
                                } else {
                                    None
                                }
                            })
                        })
                    })
            })
            .collect();

        if !resolved_packages.is_empty() {
            Some(resolved_packages)
        } else {
            None
        }
    }

    pub async fn search(&self, query: &str, case_sensitive: bool) -> Vec<ResolvedPackage> {
        let query = parse_package_query(query);
        let pkg_name = if case_sensitive {
            query.name.trim().to_owned()
        } else {
            query.name.trim().to_lowercase()
        };
        let mut resolved_packages: Vec<(u32, Package, String, String)> = Vec::new();

        for (repo_name, packages) in &self.repository {
            for (collection_name, collection_packages) in &packages.collection {
                let pkgs: Vec<(u32, Package, String, String)> = collection_packages
                    .iter()
                    .flat_map(|(_, packages)| {
                        packages.iter().filter_map(|pkg| {
                            let mut score = 0;
                            let found_pkg_name = if case_sensitive {
                                pkg.name.clone()
                            } else {
                                pkg.name.to_lowercase()
                            };

                            if found_pkg_name == pkg_name {
                                score += 2;
                            } else if found_pkg_name.contains(&pkg_name) {
                                score += 1;
                            } else {
                                return None;
                            }
                            if query.family.is_none()
                                || pkg.family.as_ref() == query.family.as_ref()
                            {
                                Some((
                                    score,
                                    pkg.to_owned(),
                                    collection_name.to_owned(),
                                    repo_name.to_owned(),
                                ))
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                resolved_packages.extend(pkgs);
            }
        }

        resolved_packages.sort_by(|(a, _, _, _), (b, _, _, _)| b.cmp(a));
        resolved_packages
            .into_iter()
            .filter(|(score, _, _, _)| *score > 0)
            .map(|(_, pkg, collection, repo_name)| ResolvedPackage {
                repo_name,
                package: pkg,
                collection,
            })
            .collect()
    }

    pub async fn inspect(&self, package_name: &str, inspect_type: &str) -> Result<()> {
        let resolved_pkg = self.resolve_package(package_name, false)?;

        let client = reqwest::Client::new();
        let url = if inspect_type == "log" {
            resolved_pkg.package.build_log
        } else if resolved_pkg
            .package
            .build_script
            .starts_with("https://github.com")
        {
            resolved_pkg
                .package
                .build_script
                .replacen("/tree/", "/raw/refs/heads/", 1)
                .replacen("/blob/", "/raw/refs/heads/", 1)
        } else {
            resolved_pkg.package.build_script
        };

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Error fetching build {} from {} [{}]",
                inspect_type,
                url.color(Color::Blue),
                response.status().color(Color::Red)
            ));
        }

        let content_length = response.content_length().unwrap_or_default();
        if content_length > 1_048_576 {
            warn!(
                "The build {} file is too large ({}). Do you really want to download and view it (y/N)? ",
                inspect_type,
                format_bytes(content_length).color(Color::Magenta)
            );

            std::io::stdout().flush()?;
            let mut response = String::new();

            std::io::stdin().read_line(&mut response)?;

            if !response.trim().eq_ignore_ascii_case("y") {
                return Err(anyhow::anyhow!(""));
            }
        }

        println!(
            "Fetching {} from {} [{}]",
            inspect_type,
            url.color(Color::Blue),
            format_bytes(response.content_length().unwrap_or_default()).color(Color::Magenta)
        );

        let mut stream = response.bytes_stream();

        let mut content = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;
            content.extend_from_slice(&chunk);
        }
        let output = String::from_utf8_lossy(&content).replace("\r", "\n");

        println!("\n{}", output);

        Ok(())
    }

    pub async fn run(&self, command: &[String], yes: bool) -> Result<()> {
        fs::create_dir_all(&*CACHE_PATH).await?;

        let package_name = &command[0];
        let args = if command.len() > 1 {
            &command[1..]
        } else {
            &[]
        };
        let runner = if let Ok(resolved_pkg) = self.resolve_package(package_name, yes) {
            let package_path = CACHE_PATH.join(&resolved_pkg.package.bin_name);
            Runner::new(&resolved_pkg, package_path, args)
        } else {
            let query = parse_package_query(package_name);
            let package_path = CACHE_PATH.join(&query.name);
            let mut resolved_pkg = ResolvedPackage::default();
            resolved_pkg.package.name = query.name;
            resolved_pkg.package.family = query.family;

            // TODO: check all the repo for package instead of choosing the first
            let base_url = CONFIG
                .repositories
                .iter()
                .find_map(|repo| {
                    if let Some(collection) = &query.collection {
                        repo.sources.get(collection).cloned()
                    } else {
                        repo.sources.values().next().cloned()
                    }
                })
                .ok_or_else(|| anyhow::anyhow!("No repository found for the package"))?;

            resolved_pkg.collection = query.collection.unwrap_or_else(|| {
                CONFIG
                    .repositories
                    .iter()
                    .find_map(|repo| repo.sources.keys().next().cloned())
                    .unwrap_or_default()
            });

            let download_url = format!("{}/{}", base_url, resolved_pkg.package.full_name('/'));
            resolved_pkg.package.download_url = download_url;
            Runner::new(&resolved_pkg, package_path, args)
        };

        runner.execute().await?;

        Ok(())
    }
}
