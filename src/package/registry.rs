use std::collections::HashMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::core::config::CONFIG;
use crate::core::util::build_path;

use super::util::{parse_package_query, select_package_variant, PackageQuery};

/// Represents a package in the registry.
///
/// Contains all metadata about a single package
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub bin_name: String,
    pub description: String,
    pub version: String,
    pub download_url: String,
    pub size: String,
    pub bsum: String,
    pub build_date: String,
    pub src_url: String,
    pub web_url: String,
    pub build_script: String,
    pub build_log: String,
    pub category: String,
    pub extra_bins: String,
    pub variant: Option<String>,
}

/// Registry containing all available packages.
///
/// Organizes packages into three categories:
/// - bin: Regular binary packages
/// - base: Base system packages
/// - pkg: Application packages (typically AppImages)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageRegistry {
    pub bin: HashMap<String, Vec<Package>>,
    pub base: HashMap<String, Vec<Package>>,
    pub pkg: HashMap<String, Vec<Package>>,
}

/// Represents the different sections of the package registry.
#[derive(Debug, Clone)]
pub enum RootPath {
    Bin,
    Base,
    Pkg,
}

#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub root_path: RootPath,
    pub package: Package,
}

impl PackageRegistry {
    /// Creates a new PackageRegistry by loading from local files or fetching from repositories.
    pub async fn new() -> Result<Self> {
        let mut set = JoinSet::new();
        let mut bin_packages = HashMap::new();
        let mut base_packages = HashMap::new();
        let mut appimages = HashMap::new();

        for repo in &CONFIG.repositories {
            let registry_path = build_path(&CONFIG.soar_path)
                .unwrap()
                .join("registry")
                .join(&repo.name);

            if registry_path.exists() {
                match Self::load_from_file(&registry_path).await {
                    Ok(repo_registry) => {
                        bin_packages.extend(repo_registry.bin);
                        base_packages.extend(repo_registry.base);
                        appimages.extend(repo_registry.pkg);
                        continue;
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to read registry from file for {}: {:?}",
                            repo.name, e
                        );
                    }
                }
            }

            set.spawn(Self::fetch_repository(repo));
        }

        while let Some(res) = set.join_next().await {
            match res {
                Ok(result) => match result {
                    Ok(repo_registry) => {
                        bin_packages.extend(repo_registry.bin);
                        base_packages.extend(repo_registry.base);
                        appimages.extend(repo_registry.pkg);
                    }
                    Err(e) => eprintln!("Error fetching repository: {:?}", e),
                },
                Err(e) => eprintln!("Task failed: {:?}", e),
            }
        }

        Ok(PackageRegistry {
            bin: bin_packages,
            base: base_packages,
            pkg: appimages,
        })
    }

    /// Loads a PackageRegistry from a local file.
    pub async fn load_from_file(path: &Path) -> Result<PackageRegistry> {
        let content = tokio::fs::read(path)
            .await
            .context("Failed to read registry file")?;

        let mut de = rmp_serde::Deserializer::new(&content[..]);

        let registry = PackageRegistry::deserialize(&mut de)?;

        Ok(registry)
    }

    /// Retrieves a package by name, optionally filtering by root path.
    pub fn get(&self, query: &PackageQuery) -> Option<Vec<ResolvedPackage>> {
        let pkg_name = query.name.trim();

        let package_iterators = query
            .root_path
            .to_owned()
            .map(|root_path| match root_path {
                RootPath::Bin => vec![(&self.bin, RootPath::Bin)],
                RootPath::Base => vec![(&self.base, RootPath::Base)],
                RootPath::Pkg => vec![(&self.pkg, RootPath::Pkg)],
            })
            .unwrap_or_else(|| {
                vec![
                    (&self.bin, RootPath::Bin),
                    (&self.base, RootPath::Base),
                    (&self.pkg, RootPath::Pkg),
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
                            package: p,
                            root_path: root_path.to_owned(),
                        })
                        .collect::<Vec<ResolvedPackage>>()
                })
            })
            .flatten()
            .collect();

        if !pkgs.is_empty() {
            Some(pkgs)
        } else {
            None
        }
    }

    pub fn parse_packages_from_names(
        &self,
        package_names: &[String],
    ) -> Result<Vec<ResolvedPackage>> {
        package_names
            .iter()
            .map(|package_name| {
                let pkg_query = parse_package_query(package_name);
                let packages = self
                    .get(&pkg_query)
                    .ok_or_else(|| anyhow::anyhow!("Package {} not found", package_name))?;

                let package = match packages.len() {
                    0 => {
                        return Err(anyhow::anyhow!(
                            "Is it a fish? Is is a frog? On no, it's a fly."
                        ))
                    }
                    1 => &ResolvedPackage {
                        package: packages[0].package.to_owned(),
                        root_path: packages[0].root_path.to_owned(),
                    },
                    _ => select_package_variant(&packages)?,
                };

                Ok(package.to_owned())
            })
            .collect()
    }
}

impl Display for ResolvedPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ResolvedPackage { package, .. } = self;
        if let Some(variant) = &package.variant {
            write!(f, "{}/{}", variant, package.name)
        } else {
            write!(f, "{}", package.name)
        }
    }
}

impl ResolvedPackage {
    pub fn install_path(&self) -> Result<PathBuf> {
        let Self { package, .. } = self;

        let variant_prefix = package
            .variant
            .clone()
            .map(|variant| format!("{}-", variant))
            .unwrap_or_default();

        let path = build_path(&CONFIG.soar_path)?
            .join("packages")
            .join(format!(
                "{}{}-{}",
                variant_prefix, package.name, package.version
            ))
            .join("bin")
            .join(&package.bin_name);
        Ok(path)
    }
}

impl Display for RootPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let w = match self {
            RootPath::Bin => "bin",
            RootPath::Base => "base",
            RootPath::Pkg => "pkg",
        };

        write!(f, "{}", w)
    }
}
