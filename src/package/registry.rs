use std::collections::HashMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::core::config::CONFIG;
use crate::core::util::{build_path, PackageQuery};

use super::fetch_repo::FetchRepository;

/// Represents a package in the registry.
///
/// Contains all metadata about a single package
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub bin_name: String,
    pub description: String,
    pub note: String,
    pub version: String,
    pub download_url: String,
    pub size: String,
    pub bsum: String,
    pub shasum: String,
    pub build_date: String,
    pub src_url: String,
    pub web_url: String,
    pub build_script: String,
    pub build_log: String,
    pub category: String,
    pub extra_bins: String,
}

/// Registry containing all available packages.
///
/// Organizes packages into three categories:
/// - bin: Regular binary packages
/// - base: Base system packages
/// - pkg: Application packages (typically AppImages)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageRegistry {
    pub bin: HashMap<String, HashMap<String, Package>>,
    pub base: HashMap<String, HashMap<String, Package>>,
    pub pkg: HashMap<String, HashMap<String, Package>>,
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
    pub variant: String,
    pub package: Package,
}

impl PackageRegistry {
    /// Creates a new PackageRegistry by loading from local files or fetching from repositories.
    ///
    /// This method will:
    /// 1. Try to load packages from local registry files
    /// 2. Fetch from remote repositories if local files don't exist or are invalid
    /// 3. Merge all packages into a single registry
    ///
    /// # Returns
    ///
    /// A Result containing the new PackageRegistry or an error
    ///
    /// # Errors
    ///
    /// Will return an error if:
    /// - Unable to read local registry files
    /// - Unable to fetch from repositories
    /// - Unable to parse registry data
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
    ///
    /// # Arguments
    ///
    /// * `path` - Path pointing to the registry file
    ///
    /// # Returns
    ///
    /// A Result containing the PackageRegistry or an error
    ///
    /// # Errors
    ///
    /// Will return an error if:
    /// - The file cannot be read
    /// - The file contains invalid MessagePack data
    /// - The MessagePack data cannot be deserialized into a PackageRegistry
    pub async fn load_from_file(path: &Path) -> Result<PackageRegistry> {
        let content = tokio::fs::read(path)
            .await
            .context("Failed to read registry file")?;

        let mut de = rmp_serde::Deserializer::new(&content[..]);

        let registry = PackageRegistry::deserialize(&mut de)?;

        Ok(registry)
    }

    /// Retrieves a package by name, optionally filtering by root path.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the package to find
    /// * `root_path` - Optional RootPath to limit the search to a specific section
    ///
    /// # Returns
    ///
    /// An Option containing a reference to the Package if found, or None if not found
    pub fn get(&self, query: &PackageQuery) -> Option<Vec<ResolvedPackage>> {
        let pkg_name = query.name.trim();
        let package_iterators = match query.root_path {
            Some(RootPath::Bin) => vec![(&self.bin, RootPath::Bin)],
            Some(RootPath::Base) => vec![(&self.base, RootPath::Base)],
            Some(RootPath::Pkg) => vec![(&self.pkg, RootPath::Pkg)],
            None => vec![
                (&self.bin, RootPath::Bin),
                (&self.base, RootPath::Base),
                (&self.pkg, RootPath::Pkg),
            ],
        };

        let mut variants = Vec::new();

        for (package_map, root_path) in package_iterators {
            if let Some(variant_map) = package_map.get(pkg_name) {
                for (key, package) in variant_map {
                    let root_path = root_path.clone();
                    let resolved_package = ResolvedPackage {
                        package: package.clone(),
                        root_path,
                        variant: key.clone(),
                    };
                    variants.push(resolved_package);
                }
            }
        }

        if let Some(ref query_variant) = query.variant {
            variants
                .retain(|pkg_query: &ResolvedPackage| pkg_query.variant.contains(query_variant));
        }

        if !variants.is_empty() {
            Some(variants)
        } else {
            None
        }
    }
}

impl Display for ResolvedPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.variant.is_empty() {
            write!(f, "{}", self.package.name)
        } else {
            write!(
                f,
                "{}/{}",
                self.variant, self.package.name
            )
        }
    }
}

impl ResolvedPackage {
    pub fn install_path(&self) -> Result<PathBuf> {
        let Self {
            package, variant, ..
        } = self;
        let variant_prefix = if variant.is_empty() {
            String::new()
        } else {
            format!("{}-", variant)
        };

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
