use std::{
    collections::HashMap,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    sync::{Mutex, Semaphore},
};

use crate::{
    core::{
        config::CONFIG,
        util::{build_path, format_bytes, get_platform, home_cache_path},
    },
    registry::{
        installed::InstalledPackages,
        package::{parse_package_query, ResolvedPackage},
    },
};

use super::{
    package::{run::Runner, Package, PackageQuery, RootPath},
    select_package_variant,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageStorage {
    repository: HashMap<String, RepositoryPackages>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepositoryPackages {
    pub bin: HashMap<String, Vec<Package>>,
    pub base: HashMap<String, Vec<Package>>,
    pub pkg: HashMap<String, Vec<Package>>,
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

    pub fn resolve_package(&self, package_name: &str) -> Result<ResolvedPackage> {
        let pkg_query = parse_package_query(package_name);
        let packages = self
            .get_packages(&pkg_query)
            .ok_or_else(|| anyhow::anyhow!("Package {} not found", package_name))?;
        let package = match packages.len() {
            0 => {
                return Err(anyhow::anyhow!(
                    "Is it a fish? Is is a frog? On no, it's a fly."
                ));
            }
            1 => &ResolvedPackage {
                repo_name: packages[0].repo_name.to_owned(),
                package: packages[0].package.to_owned(),
                root_path: packages[0].root_path.to_owned(),
            },
            _ => select_package_variant(&packages)?,
        };

        Ok(package.to_owned())
    }

    pub async fn install_packages(
        &self,
        package_names: &[String],
        force: bool,
        is_update: bool,
        installed_packages: Arc<Mutex<InstalledPackages>>,
        portable: Option<PathBuf>,
        portable_home: Option<PathBuf>,
        portable_config: Option<PathBuf>,
    ) -> Result<()> {
        let resolved_packages: Result<Vec<ResolvedPackage>> = package_names
            .iter()
            .map(|package_name| self.resolve_package(package_name))
            .collect();
        let resolved_packages = resolved_packages?;

        let installed_count = Arc::new(AtomicU64::new(0));
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

                let handle = tokio::spawn(async move {
                    if let Err(e) = package
                        .install(
                            idx,
                            pkgs_len,
                            force,
                            is_update,
                            installed_packages,
                            portable,
                            portable_home,
                            portable_config,
                        )
                        .await
                    {
                        eprintln!("{}", e);
                    } else {
                        ic.fetch_add(1, Ordering::Relaxed);
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
                        force,
                        is_update,
                        installed_packages.clone(),
                        portable.clone(),
                        portable_home.clone(),
                        portable_config.clone(),
                    )
                    .await
                {
                    eprintln!("{}", e);
                } else {
                    installed_count.fetch_add(1, Ordering::Relaxed);
                };
            }
        }
        println!(
            "Installed {}/{} packages",
            installed_count.load(Ordering::Relaxed),
            resolved_packages.len()
        );
        Ok(())
    }

    pub async fn remove_packages(&self, package_names: &[String]) -> Result<()> {
        let resolved_packages: Vec<ResolvedPackage> = package_names
            .iter()
            .filter_map(|package_name| self.resolve_package(package_name).ok())
            .collect();
        for package in resolved_packages {
            package.remove().await?;
        }

        Ok(())
    }

    pub fn list_packages(&self, root_path: Option<RootPath>) -> Vec<ResolvedPackage> {
        self.repository
            .iter()
            .flat_map(|(repo_name, repo_packages)| {
                let package_iterators = match root_path {
                    Some(ref path) => match path {
                        RootPath::Bin => vec![(&repo_packages.bin, RootPath::Bin)],
                        RootPath::Base => vec![(&repo_packages.base, RootPath::Base)],
                        RootPath::Pkg => vec![(&repo_packages.pkg, RootPath::Pkg)],
                    },
                    None => vec![
                        (&repo_packages.bin, RootPath::Bin),
                        (&repo_packages.base, RootPath::Base),
                        (&repo_packages.pkg, RootPath::Pkg),
                    ],
                };

                package_iterators.into_iter().flat_map(move |(map, path)| {
                    map.iter().flat_map(move |(_, packages)| {
                        let value = path.clone();
                        packages.iter().map(move |package| ResolvedPackage {
                            repo_name: repo_name.clone(),
                            root_path: value.clone(),
                            package: package.clone(),
                        })
                    })
                })
            })
            .collect::<Vec<_>>()
    }

    pub fn get_packages(&self, query: &PackageQuery) -> Option<Vec<ResolvedPackage>> {
        let pkg_name = query.name.trim();

        let mut resolved_packages = Vec::new();
        for (repo_name, packages) in &self.repository {
            let package_iterators = query
                .root_path
                .to_owned()
                .map(|root_path| match root_path {
                    RootPath::Bin => vec![(&packages.bin, RootPath::Bin)],
                    RootPath::Base => vec![(&packages.base, RootPath::Base)],
                    RootPath::Pkg => vec![(&packages.pkg, RootPath::Pkg)],
                })
                .unwrap_or_else(|| {
                    vec![
                        (&packages.bin, RootPath::Bin),
                        (&packages.base, RootPath::Base),
                        (&packages.pkg, RootPath::Pkg),
                    ]
                });

            let pkgs: Vec<ResolvedPackage> = package_iterators
                .iter()
                .filter_map(|(map, root_path)| {
                    map.get(pkg_name).map(|p| {
                        p.iter()
                            .filter(|pkg| {
                                pkg.name == pkg_name
                                    && (query.variant.is_none()
                                        || pkg.variant.as_ref() == query.variant.as_ref())
                            })
                            .cloned()
                            .map(|p| ResolvedPackage {
                                repo_name: repo_name.to_owned(),
                                package: p,
                                root_path: root_path.to_owned(),
                            })
                            .collect::<Vec<ResolvedPackage>>()
                    })
                })
                .flatten()
                .collect();

            resolved_packages.extend(pkgs);
        }

        if !resolved_packages.is_empty() {
            Some(resolved_packages)
        } else {
            None
        }
    }

    pub async fn search(&self, query: &str) -> Vec<ResolvedPackage> {
        let query = parse_package_query(query);
        let pkg_name = query.name.trim();

        let mut resolved_packages: Vec<(u32, Package, RootPath, String)> = Vec::new();
        for (repo_name, packages) in &self.repository {
            let package_iterators = query
                .root_path
                .to_owned()
                .map(|root_path| match root_path {
                    RootPath::Bin => vec![(&packages.bin, RootPath::Bin)],
                    RootPath::Base => vec![(&packages.base, RootPath::Base)],
                    RootPath::Pkg => vec![(&packages.pkg, RootPath::Pkg)],
                })
                .unwrap_or_else(|| {
                    vec![
                        (&packages.bin, RootPath::Bin),
                        (&packages.base, RootPath::Base),
                        (&packages.pkg, RootPath::Pkg),
                    ]
                });
            let pkgs: Vec<(u32, Package, RootPath, String)> = package_iterators
                .iter()
                .flat_map(|(map, root_path)| {
                    map.iter().flat_map(|(_, packages)| {
                        packages.iter().filter_map(|pkg| {
                            let mut score = 0;
                            if pkg.name == pkg_name {
                                score += 2;
                            } else if pkg.name.contains(pkg_name) {
                                score += 1;
                            } else {
                                return None;
                            }

                            if query.variant.is_none()
                                || pkg.variant.as_ref() == query.variant.as_ref()
                            {
                                Some((
                                    score,
                                    pkg.to_owned(),
                                    root_path.to_owned(),
                                    repo_name.to_owned(),
                                ))
                            } else {
                                None
                            }
                        })
                    })
                })
                .collect();

            resolved_packages.extend(pkgs);
        }

        resolved_packages.sort_by(|(a, _, _, _), (b, _, _, _)| b.cmp(a));

        let pkgs: Vec<ResolvedPackage> = resolved_packages
            .into_iter()
            .filter(|(score, _, _, _)| *score > 0)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(_, pkg, root_path, repo_name)| ResolvedPackage {
                repo_name,
                package: pkg,
                root_path,
            })
            .collect();

        pkgs
    }

    pub async fn inspect(&self, package_name: &str) -> Result<()> {
        let resolved_pkg = self.resolve_package(package_name)?;

        let client = reqwest::Client::new();
        let url = resolved_pkg.package.build_log;
        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Error fetching log from {} [{}]",
                url,
                response.status()
            ));
        }

        let content_length = response.content_length().unwrap_or_default();
        if content_length > 1_048_576 {
            print!(
                "The log file is too large ({}). Do you really want to download and view it (y/N)? ",
                format_bytes(content_length)
            );

            std::io::stdout().flush()?;
            let mut response = String::new();

            std::io::stdin().read_line(&mut response)?;

            if !response.trim().eq_ignore_ascii_case("y") {
                return Err(anyhow::anyhow!(""));
            }
        }

        println!(
            "Fetching log from {} [{}]",
            url,
            format_bytes(response.content_length().unwrap_or_default())
        );

        let mut stream = response.bytes_stream();

        let mut content = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;
            content.extend_from_slice(&chunk);
        }
        let log_str = String::from_utf8_lossy(&content).replace("\r", "\n");

        println!("\n{}", log_str);

        Ok(())
    }

    pub async fn run(&self, command: &[String]) -> Result<()> {
        let mut cache_dir = home_cache_path();
        cache_dir.push_str("/soar");
        let cache_dir = build_path(&cache_dir)?;

        fs::create_dir_all(&cache_dir).await?;

        let package_name = &command[0];
        let args = if command.len() > 1 {
            &command[1..]
        } else {
            &[]
        };
        let runner = if let Ok(resolved_pkg) = self.resolve_package(package_name) {
            let package_path = cache_dir.join(&resolved_pkg.package.bin_name);
            Runner::new(&resolved_pkg, package_path, args)
        } else {
            let query = parse_package_query(package_name);
            let package_path = cache_dir.join(&query.name);
            let mut resolved_pkg = ResolvedPackage::default();
            resolved_pkg.package.name = query.name;
            resolved_pkg.package.variant = query.variant;
            resolved_pkg.root_path = query.root_path.unwrap_or_default();

            // TODO: don't use just first repo
            let platform = get_platform();
            let repo = &CONFIG.repositories[0];
            let base_url = format!("{}/{}", repo.url, platform);

            let root_path = if resolved_pkg.root_path == RootPath::Base {
                "/Baseutils"
            } else {
                ""
            };
            let download_url = format!(
                "{}{}/{}",
                base_url,
                root_path,
                resolved_pkg.package.full_name('/')
            );
            resolved_pkg.package.download_url = download_url;
            Runner::new(&resolved_pkg, package_path, args)
        };

        runner.execute().await?;

        Ok(())
    }
}
