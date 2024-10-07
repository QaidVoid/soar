use std::{
    collections::HashMap,
    sync::{atomic::{AtomicU64, Ordering}, Arc},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore};

use crate::{
    core::config::CONFIG,
    registry::{
        installed::InstalledPackages,
        package::{parse_package_query, ResolvedPackage},
    },
};

use super::{
    package::{Package, PackageQuery, RootPath},
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
    ) -> Result<()> {
        let resolved_packages: Vec<ResolvedPackage> = package_names
            .iter()
            .filter_map(|package_name| self.resolve_package(package_name).ok())
            .collect();

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

                let handle = tokio::spawn(async move {
                    if package
                        .install(idx, pkgs_len, force, is_update, installed_packages)
                        .await
                        .is_ok()
                    {
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
                if package
                    .install(
                        idx,
                        resolved_packages.len(),
                        force,
                        is_update,
                        installed_packages.clone(),
                    )
                    .await
                    .is_ok()
                {
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
        let pkg_name = query.name.trim().to_lowercase();

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
                            } else if pkg.name.contains(&pkg_name) {
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
}
