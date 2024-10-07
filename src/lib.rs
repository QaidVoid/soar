use anyhow::Result;
use clap::Parser;
use cli::{Args, Commands};
use registry::{installed::InstalledPackages, PackageRegistry};

use core::{config, util::setup_required_paths};

mod cli;
mod core;
mod registry;

pub async fn init() -> Result<()> {
    config::init();
    let args = Args::parse();
    let registry = PackageRegistry::new().await?;
    setup_required_paths().await?;

    match args.command {
        Commands::Install { packages, force } => {
            registry.install_packages(&packages, force, false).await?;
        }
        Commands::Fetch => {
            let mut registry = registry;
            registry.fetch().await?;
        }
        Commands::Remove { packages } => {
            registry.remove_packages(&packages).await?;
        }
        Commands::Update { packages } => {
            registry.update(packages.as_deref()).await?;
        }
        Commands::ListPackages => {
            let installed_packages = InstalledPackages::new().await?;
            installed_packages.packages.iter().for_each(|package| {
                println!(
                    "- [{}] {}:{}",
                    package.root_path, package.name, package.version
                )
            })
        }
        Commands::Search { query } => {
            let result = registry.search(&query).await;

            if result.is_empty() {
                println!("No packages found");
            } else {
                result.iter().for_each(|pkg| {
                    println!(
                        "[{}] {}: {}",
                        pkg.root_path,
                        pkg.package.full_name(),
                        pkg.package.description
                    );
                })
            }
        }
    };

    Ok(())
}
