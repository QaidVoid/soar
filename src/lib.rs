use anyhow::Result;
use clap::Parser;
use cli::{Args, Commands};
use registry::PackageRegistry;

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
        Commands::ListInstalledPackages { packages } => {
            registry.info(packages.as_deref()).await?;
        }
        Commands::Search { query } => {
            registry.search(&query).await?;
        }
        Commands::Query { query } => {
            registry.query(&query).await?;
        }
        Commands::ListPackages { root_path } => {
            registry.list(root_path.as_deref())?;
        }
        Commands::Inspect { package } => {
            registry.inspect(&package).await?;
        },
    };

    Ok(())
}
