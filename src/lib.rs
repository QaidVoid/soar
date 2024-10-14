use anyhow::Result;
use clap::Parser;
use cli::{Args, Commands};
use misc::download::download_and_save;
use registry::PackageRegistry;

use core::{
    config,
    constant::BIN_PATH,
    util::{cleanup, setup_required_paths},
};
use std::{env, path::Path};

mod cli;
mod core;
mod misc;
mod registry;

pub async fn init() -> Result<()> {
    config::init();
    let args = Args::parse();
    let registry = PackageRegistry::new().await?;
    setup_required_paths().await?;

    let path_env = env::var("PATH")?;
    if !path_env.split(':').any(|p| Path::new(p) == *BIN_PATH) {
        eprintln!(
            "{} is not in PATH. Please add it to PATH to use installed binaries.",
            &*BIN_PATH.to_string_lossy()
        );
    }

    match args.command {
        Commands::Install {
            packages,
            force,
            portable,
            portable_home,
            portable_config,
        } => {
            if portable.is_some() && (portable_home.is_some() || portable_config.is_some()) {
                eprintln!(
                    "Error: --portable cannot be used with --portable-home or --portable-config"
                );
                std::process::exit(1);
            }
            registry
                .install_packages(
                    &packages,
                    force,
                    false,
                    portable,
                    portable_home,
                    portable_config,
                )
                .await?;
        }
        Commands::Sync => {
            cleanup().await?;
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
            registry.list(root_path.as_deref()).await?;
        }
        Commands::Inspect { package } => {
            registry.inspect(&package).await?;
        }
        Commands::Run { command } => {
            registry.run(command.as_ref()).await?;
        }
        Commands::Use { package } => {
            registry.use_package(&package).await?;
        }
        Commands::Download { links } => {
            download_and_save(links.as_ref()).await?;
        }
    };

    Ok(())
}
