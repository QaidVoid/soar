use anyhow::{Context, Result};
use clap::Parser;
use cli::{Args, Commands};

use core::{config, constant::BIN_PATH};
use package::{registry::PackageRegistry, util::parse_package_query};

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
            registry.install_packages(&packages, force).await?;
        }
        Commands::Fetch => {
            PackageRegistry::fetch().await?;
        }
        Commands::Remove { packages: _ } => todo!(),
        Commands::Update { package: _ } => todo!(),
        Commands::ListPackages => todo!(),
        Commands::Search { query } => {
            let pkg_query = parse_package_query(&query);
            let result = registry.search(&pkg_query);

            if result.is_empty() {
                println!("No packages found");
            } else {
                result.iter().for_each(|pkg| {
                    println!("[{}] {}: {}", pkg.root_path, pkg, pkg.package.description);
                })
            }
        }
    };

    Ok(())
}
