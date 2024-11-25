use anyhow::Result;
use clap::Parser;
use cli::{Args, Commands, SelfAction};
use misc::{
    download::{download, download_and_save, github::fetch_github_releases, ApiType},
    health::check_health,
};
use package::build;
use registry::PackageRegistry;
use tokio::fs;
use tracing::{debug, error, info, trace, warn};

use core::{
    color::{Color, ColorExt},
    config::{self, generate_default_config},
    constant::BIN_PATH,
    log::setup_logging,
    util::{cleanup, print_env, setup_required_paths},
};
use std::{
    env::{self, consts::ARCH},
    io::Read,
    path::Path,
};

mod cli;
pub mod core;
mod misc;
mod package;
mod registry;

async fn handle_cli() -> Result<()> {
    let mut args = env::args().collect::<Vec<_>>();
    let self_bin = args.get(0).unwrap().clone();
    let self_version = env!("CARGO_PKG_VERSION");

    let mut i = 0;
    while i < args.len() {
        if args[i] == "-" {
            let mut stdin = std::io::stdin();
            let mut buffer = String::new();
            if stdin.read_to_string(&mut buffer).is_ok() {
                let stdin_args = buffer.split_whitespace().collect::<Vec<&str>>();
                args.remove(i);
                args.splice(i..i, stdin_args.into_iter().map(String::from));
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    let args = Args::parse_from(args);

    setup_logging(&args);

    debug!("Initializing soar");
    config::init();

    debug!("Setting up paths");
    setup_required_paths().await?;

    let path_env = env::var("PATH")?;
    if !path_env.split(':').any(|p| Path::new(p) == *BIN_PATH) {
        warn!(
            "{} is not in {1}. Please add it to {1} to use installed binaries.",
            &*BIN_PATH.to_string_lossy().color(Color::Blue),
            "PATH".color(Color::BrightGreen).bold()
        );
    }

    debug!("Initializing package registry");
    let registry = PackageRegistry::new();

    trace!("Running cleanup");
    let _ = cleanup().await;

    match args.command {
        Commands::Install {
            packages,
            force,
            portable,
            portable_home,
            portable_config,
            yes,
        } => {
            if portable.is_some() && (portable_home.is_some() || portable_config.is_some()) {
                error!("--portable cannot be used with --portable-home or --portable-config");
                std::process::exit(1);
            }

            let portable = portable.map(|p| p.unwrap_or_default());
            let portable_home = portable_home.map(|p| p.unwrap_or_default());
            let portable_config = portable_config.map(|p| p.unwrap_or_default());

            registry
                .await?
                .install_packages(
                    &packages,
                    force,
                    portable,
                    portable_home,
                    portable_config,
                    yes,
                    args.quiet,
                )
                .await?;
        }
        Commands::Sync => {
            registry.await?;
        }
        Commands::Remove { packages, exact } => {
            registry.await?.remove_packages(&packages, exact).await?;
        }
        Commands::Update { packages } => {
            registry
                .await?
                .update(packages.as_deref(), args.quiet)
                .await?;
        }
        Commands::ListInstalledPackages { packages } => {
            registry.await?.info(packages.as_deref()).await?;
        }
        Commands::Search {
            query,
            case_sensitive,
            limit,
        } => {
            registry
                .await?
                .search(&query, case_sensitive, limit)
                .await?;
        }
        Commands::Query { query } => {
            registry.await?.query(&query).await?;
        }
        Commands::ListPackages { collection } => {
            registry.await?.list(collection.as_deref()).await?;
        }
        Commands::Inspect { package } => {
            registry.await?.inspect(&package, "script").await?;
        }
        Commands::Log { package } => {
            registry.await?.inspect(&package, "log").await?;
        }
        Commands::Run { command, yes } => {
            registry.await?.run(command.as_ref(), yes).await?;
        }
        Commands::Use { package } => {
            registry.await?.use_package(&package, args.quiet).await?;
        }
        Commands::Download {
            links,
            yes,
            output,
            regex_patterns,
            match_keywords,
            exclude_keywords,
        } => {
            download_and_save(
                registry.await?,
                links.as_ref(),
                yes,
                output,
                regex_patterns.as_deref(),
                match_keywords.as_deref(),
                exclude_keywords.as_deref(),
            )
            .await?;
        }
        Commands::Health => {
            check_health().await;
        }
        Commands::DefConfig => {
            generate_default_config()?;
        }
        Commands::Env => {
            print_env();
        }
        Commands::Build { files } => {
            for file in files {
                build::init(&file).await?;
            }
        }
        Commands::SelfCmd { action } => {
            match action {
                SelfAction::Update => {
                    let is_nightly = self_version.starts_with("nightly");
                    let gh_releases =
                        fetch_github_releases(&ApiType::PkgForge, "pkgforge/soar").await?;

                    let release = gh_releases.iter().find(|rel| {
                        if is_nightly {
                            return rel.name.starts_with("nightly") && rel.name != self_version;
                        } else {
                            rel.tag_name
                                .trim_start_matches('v')
                                .parse::<f32>()
                                .map(|v| v > self_version.parse::<f32>().unwrap())
                                .unwrap_or(false)
                        }
                    });
                    if let Some(release) = release {
                        let asset = release
                            .assets
                            .iter()
                            .find(|a| {
                                a.name.contains(ARCH)
                                    && !a.name.contains("tar")
                                    && !a.name.contains("sum")
                            })
                            .unwrap();
                        download(&asset.browser_download_url, Some(self_bin)).await?;
                        println!("Soar updated to {}", release.tag_name);
                    } else {
                        eprintln!("No updates found.");
                    }
                }
                SelfAction::Uninstall => {
                    match fs::remove_file(self_bin).await {
                        Ok(_) => {
                            info!("Soar has been uninstalled successfully.");
                            info!("You should remove soar config and data files manually.");
                        }
                        Err(err) => {
                            error!("{}\nFailed to uninstall soar.", err.to_string());
                        }
                    };
                }
            };
        }
    };

    Ok(())
}

pub async fn init() {
    if let Err(e) = handle_cli().await {
        error!("{}", e);
    }
}
