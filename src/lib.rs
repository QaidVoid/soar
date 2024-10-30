use anyhow::Result;
use clap::Parser;
use cli::{Args, Commands};
use misc::download::download_and_save;
use registry::PackageRegistry;

use core::{
    color::{Color, ColorExt},
    config,
    constant::BIN_PATH,
    health::check_health,
    util::{cleanup, setup_required_paths},
};
use std::{env, path::Path};

mod cli;
pub mod core;
mod misc;
mod registry;

pub async fn init() -> Result<()> {
    let args = Args::parse();
    config::init();
    setup_required_paths().await?;
    let registry = PackageRegistry::new().await?;

    let path_env = env::var("PATH")?;
    if !path_env.split(':').any(|p| Path::new(p) == *BIN_PATH) {
        warn!(
            "{} is not in {1}. Please add it to {1} to use installed binaries.",
            &*BIN_PATH.to_string_lossy().color(Color::Blue),
            "PATH".color(Color::BrightGreen).bold()
        );
    }

    let _ = cleanup().await;

    match args.command {
        Commands::Install {
            packages,
            force,
            portable,
            portable_home,
            portable_config,
        } => {
            if portable.is_some() && (portable_home.is_some() || portable_config.is_some()) {
                error!("--portable cannot be used with --portable-home or --portable-config");
                std::process::exit(1);
            }

            let portable = portable.map(|p| p.unwrap_or_default());
            let portable_home = portable_home.map(|p| p.unwrap_or_default());
            let portable_config = portable_config.map(|p| p.unwrap_or_default());

            registry
                .install_packages(&packages, force, portable, portable_home, portable_config)
                .await?;
        }
        Commands::Sync => {
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
        Commands::Search {
            query,
            case_sensitive,
        } => {
            registry.search(&query, case_sensitive).await?;
        }
        Commands::Query { query } => {
            registry.query(&query).await?;
        }
        Commands::ListPackages { collection } => {
            registry.list(collection.as_deref()).await?;
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
        Commands::Health => {
            check_health().await;
        }
    };

    Ok(())
}
