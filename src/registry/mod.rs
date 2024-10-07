use std::{io::Write, sync::Arc};

use anyhow::Result;

use fetcher::RegistryFetcher;
use installed::InstalledPackages;
use loader::RegistryLoader;
use package::ResolvedPackage;
use serde::Deserialize;
use storage::{PackageStorage, RepositoryPackages};
use tokio::sync::Mutex;

use crate::core::config::CONFIG;

mod fetcher;
pub mod installed;
mod loader;
mod package;
mod storage;

pub struct PackageRegistry {
    fetcher: RegistryFetcher,
    storage: PackageStorage,
    installed_packages: Arc<Mutex<InstalledPackages>>,
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
            .install_packages(package_names, force, is_update, self.installed_packages.clone())
            .await
    }

    pub async fn remove_packages(&self, package_names: &[String]) -> Result<()> {
        self.storage.remove_packages(package_names).await
    }

    pub async fn search(&self, package_name: &str) -> Vec<ResolvedPackage> {
        self.storage.search(package_name).await
    }

    pub async fn update(&self, package_names: Option<&[String]>) -> Result<()> {
        self.installed_packages.lock().await.update(self, package_names).await
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
            package.package.full_name(),
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
