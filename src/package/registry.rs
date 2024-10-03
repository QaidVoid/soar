use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::core::config::CONFIG;
use crate::core::util::build_path;

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
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageRegistry {
    pub bin: HashMap<String, Package>,
    pub base: HashMap<String, Package>,
    pub pkg: HashMap<String, Package>,
}

/// Represents the different sections of the package registry.
#[derive(Debug)]
pub enum RootPath {
    Bin,
    Base,
    Pkg,
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
    /// * `path` - PathBuf pointing to the registry file
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
    pub async fn load_from_file(path: &PathBuf) -> Result<PackageRegistry> {
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
    ///
    /// # Example
    ///
    /// ```
    /// let registry = PackageRegistry::new().await?;
    ///
    /// // Search in all sections
    /// if let Some(package) = registry.get("git", None) {
    ///     println!("Found git package: {}", package.version);
    /// }
    ///
    /// // Search only in binary packages
    /// if let Some(package) = registry.get("git", Some(RootPath::Bin)) {
    ///     println!("Found git binary: {}", package.version);
    /// }
    /// ```
    pub fn get(&self, name: &str, root_path: Option<RootPath>) -> Option<&Package> {
        if let Some(path) = root_path {
            match path {
                RootPath::Bin => self.bin.get(name),
                RootPath::Base => self.base.get(name),
                RootPath::Pkg => self.pkg.get(name),
            }
        } else {
            self.bin
                .get(name)
                .or_else(|| self.base.get(name))
                .or_else(|| self.pkg.get(name))
        }
    }
}
