use anyhow::{Context, Result};
use clap::Parser;
use cli::{Args, Commands};

use core::{config, constant::BIN_PATH, util::parse_package_query};
use package::{
    fetch_repo::FetchRepository, install::InstallPackage, registry::PackageRegistry,
    search::PackageSearch,
};

mod cli;
mod core;
mod package;

pub async fn init() -> Result<()> {
    config::init();
    let args = Args::parse();
    let registry = PackageRegistry::new().await?;

    if !BIN_PATH.exists() {
        tokio::fs::create_dir_all(&*BIN_PATH)
            .await
            .context(format!("Failed to create bin directory {:#?}", BIN_PATH))?;
    }

    match args.command {
        Commands::Install { packages, force } => {
            registry.install(&packages, force).await?;
        }
        Commands::Fetch => {
            PackageRegistry::fetch().await?;
        }
        Commands::Remove { packages } => todo!(),
        Commands::Update { package } => todo!(),
        Commands::ListPackages => todo!(),
        Commands::Search { query } => {
            let pkg_query = parse_package_query(&query);
            let result = registry.search(&pkg_query);

            if result.is_empty() {
                println!("No packages found");
            } else {
                result.iter().for_each(|data| {
                    println!("{}", data);
                })
            }
        }
    };

    Ok(())
}
