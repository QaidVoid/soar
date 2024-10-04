use std::{
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::Semaphore};

use crate::core::{
    config::CONFIG,
    constant::{ERROR, PAPER, TRUCK},
    util::{parse_package_query, parse_size},
};

use super::{
    download_tracker::DownloadTracker,
    registry::{Package, PackageRegistry, ResolvedPackage},
    util::{setup_symlink, verify_checksum},
};

pub trait InstallPackage {
    async fn install(&self, package_names: &[String], force: bool) -> Result<()>;
}

impl InstallPackage for PackageRegistry {
    async fn install(&self, package_names: &[String], force: bool) -> Result<()> {
        let packages = self.get_packages_to_install(package_names)?;
        if CONFIG.parallel.unwrap_or_default() {
            self.install_parallel(&packages, force).await
        } else {
            self.install_sequential(&packages, force).await
        }
    }
}

struct InstallContext {
    install_path: PathBuf,
    temp_file_path: PathBuf,
}

impl InstallContext {
    async fn new(package: &ResolvedPackage) -> Result<Self> {
        let install_path = package.install_path()?;

        if let Some(parent) = install_path.parent() {
            tokio::fs::create_dir_all(parent).await.context(format!(
                "Failed to create install directory {:#?}",
                install_path
            ))?;
        }

        Ok(Self {
            temp_file_path: install_path.with_extension("part"),
            install_path,
        })
    }
}

impl PackageRegistry {
    async fn install_sequential(&self, packages: &[ResolvedPackage], force: bool) -> Result<()> {
        let total_packages = packages.len();
        let total_bytes = self.calculate_total_bytes(packages).await;

        let multi_progress = MultiProgress::new();
        let tracker = DownloadTracker::new(total_packages, total_bytes, &multi_progress);

        for (index, package) in packages.iter().enumerate() {
            self.process_single_package(
                package,
                force,
                index,
                total_packages,
                &multi_progress,
                tracker.clone(),
            )
            .await?;
        }

        tracker.finish_install().await;
        Ok(())
    }

    async fn install_parallel(&self, packages: &[ResolvedPackage], force: bool) -> Result<()> {
        let total_packages = packages.len();
        let total_bytes = self.calculate_total_bytes(packages).await;

        let registry = Arc::new(self.clone());
        let semaphore = Arc::new(Semaphore::new(CONFIG.parallel_limit.unwrap_or(2) as usize));
        let multi_progress = Arc::new(MultiProgress::new());
        let tracker = DownloadTracker::new(total_packages, total_bytes, &multi_progress);

        let mut handles = Vec::new();

        for (index, package) in packages.iter().enumerate() {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let registry = registry.clone();
            let multi_progress = multi_progress.clone();
            let tracker = tracker.clone();
            let package = package.clone();

            let handle = tokio::spawn(async move {
                let result = registry
                    .process_single_package(
                        &package,
                        force,
                        index,
                        total_packages,
                        &multi_progress,
                        tracker.clone(),
                    )
                    .await;
                drop(permit);
                result
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await??;
        }

        tracker.finish_install().await;
        Ok(())
    }

    async fn process_single_package(
        &self,
        package: &ResolvedPackage,
        force: bool,
        index: usize,
        total: usize,
        multi_progress: &MultiProgress,
        tracker: Arc<DownloadTracker>,
    ) -> Result<()> {
        let ctx = InstallContext::new(package).await?;

        if !force && self.check_existing_installation(&ctx, package).await? {
            return Ok(());
        }

        self.download_and_install_package(&ctx, package, multi_progress, tracker, index, total)
            .await?;
        setup_symlink(&ctx.install_path, package).await?;
        Ok(())
    }

    async fn calculate_total_bytes(&self, packages: &[ResolvedPackage]) -> u64 {
        packages
            .iter()
            .filter_map(|pkg| parse_size(&pkg.package.size))
            .sum()
    }

    async fn check_existing_installation(
        &self,
        ctx: &InstallContext,
        package: &ResolvedPackage,
    ) -> Result<bool> {
        if ctx.install_path.exists() && verify_checksum(&ctx.install_path, &package.package).await?
        {
            println!("  {PAPER}Package {} is already installed", package);
            return Ok(true);
        }

        if ctx.temp_file_path.exists()
            && verify_checksum(&ctx.temp_file_path, &package.package).await?
        {
            println!("Package {} is already downloaded. Installing...", package);
            tokio::fs::rename(&ctx.temp_file_path, &ctx.install_path).await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn download_and_install_package(
        &self,
        ctx: &InstallContext,
        resolved_package: &ResolvedPackage,
        multi_progress: &MultiProgress,
        tracker: Arc<DownloadTracker>,
        index: usize,
        total: usize,
    ) -> Result<()> {
        let ResolvedPackage { package, .. } = resolved_package;
        let client = reqwest::Client::new();
        let downloaded_bytes = self.get_downloaded_bytes(&ctx.temp_file_path).await?;
        let response = self
            .make_request(&client, package, downloaded_bytes)
            .await?;

        let progress_bar = self.create_progress_bar(
            multi_progress,
            index,
            total,
            resolved_package,
            tracker.get_progress_bar(),
        );
        let total_bytes = response.content_length().unwrap_or(0) + downloaded_bytes;
        progress_bar.set_position(downloaded_bytes);
        progress_bar.set_length(total_bytes);

        let mut file = self.open_temp_file(&ctx.temp_file_path).await?;
        let mut stream = response.bytes_stream();
        let mut current_bytes = downloaded_bytes;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;
            current_bytes += chunk.len() as u64;
            progress_bar.set_position(current_bytes);
            tracker.add_downloaded_bytes(chunk.len() as u64).await;
            file.write_all(&chunk).await?;
        }

        file.flush().await?;
        self.finalize_installation(
            ctx,
            resolved_package,
            multi_progress,
            &progress_bar,
            total_bytes,
        )
        .await?;
        tracker.mark_package_completed();
        Ok(())
    }

    async fn get_downloaded_bytes(&self, path: &Path) -> Result<u64> {
        if path.exists() {
            let meta = tokio::fs::metadata(path).await?;
            Ok(meta.len())
        } else {
            Ok(0)
        }
    }

    async fn make_request(
        &self,
        client: &reqwest::Client,
        package: &Package,
        downloaded_bytes: u64,
    ) -> Result<reqwest::Response> {
        let response = client
            .get(&package.download_url)
            .header("Range", format!("bytes={}-", downloaded_bytes))
            .send()
            .await
            .context(format!("Failed to download package {}", package.name))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Download failed: {:?}", response.status()));
        }

        Ok(response)
    }

    async fn open_temp_file(&self, path: &Path) -> Result<tokio::fs::File> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(path)
            .await
            .context("Failed to open temp file for writing")
    }

    async fn finalize_installation(
        &self,
        ctx: &InstallContext,
        package: &ResolvedPackage,
        multi_progress: &MultiProgress,
        progress_bar: &ProgressBar,
        total_bytes: u64,
    ) -> Result<()> {
        let ResolvedPackage {
            package, variant, ..
        } = package;

        progress_bar.set_position(total_bytes);
        progress_bar.finish();

        let error = multi_progress.insert_from_back(1, ProgressBar::new(0));

        match package.bsum == "null" {
            true => {
                error.set_style(ProgressStyle::with_template("  {msg}").unwrap());
                error.set_message(format!(
                    "{ERROR} Missing checksum for {}. Installing anyway.",
                    package.name
                ));
                error.finish();
                if ctx.install_path.exists() {
                    tokio::fs::remove_file(&ctx.install_path).await?;
                }
                tokio::fs::rename(&ctx.temp_file_path, &ctx.install_path).await?;
                self.set_executable_permissions(&ctx.install_path).await?;
            }
            false => {
                if verify_checksum(&ctx.temp_file_path, package).await? {
                    if ctx.install_path.exists() {
                        tokio::fs::remove_file(&ctx.install_path).await?;
                    }
                    tokio::fs::rename(&ctx.temp_file_path, &ctx.install_path).await?;
                    self.set_executable_permissions(&ctx.install_path).await?;
                } else {
                    eprint!("Checksum verification failed for {}/{}. Do you want to remove the file? (y/n): ", variant, package.name);
                    std::io::stdout().flush()?;

                    let mut response = String::new();
                    std::io::stdin().read_line(&mut response)?;

                    if response.trim().eq_ignore_ascii_case("y") {
                        tokio::fs::remove_file(&ctx.temp_file_path).await?;
                    }
                    std::process::exit(-1);
                }
            }
        }

        Ok(())
    }

    async fn set_executable_permissions(&self, path: &Path) -> Result<()> {
        let mut permissions = tokio::fs::metadata(path).await?.permissions();
        permissions.set_mode(0o755);
        tokio::fs::set_permissions(path, permissions).await?;
        Ok(())
    }

    fn create_progress_bar(
        &self,
        multi_progress: &MultiProgress,
        index: usize,
        total: usize,
        resolved_package: &ResolvedPackage,
        total_progress_bar: &ProgressBar,
    ) -> ProgressBar {
        let pb = multi_progress.insert_before(total_progress_bar, ProgressBar::new(0));
        pb.set_style(ProgressStyle::with_template(
            "{spinner} {msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})"
        ).unwrap().with_key("eta", |state: &ProgressState, w: &mut dyn std::fmt::Write|
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#-"));
        pb.set_message(format!(
            "{TRUCK}[{}/{}] [{}] {}",
            index + 1,
            total,
            resolved_package.root_path,
            resolved_package
        ));
        pb
    }

    fn get_packages_to_install(&self, package_names: &[String]) -> Result<Vec<ResolvedPackage>> {
        package_names
            .iter()
            .map(|package_name| {
                let pkg_query = parse_package_query(package_name);
                let packages = self
                    .get(&pkg_query)
                    .ok_or_else(|| anyhow::anyhow!("Package {} not found", package_name))?;

                let package = match packages.len() {
                    0 => {
                        return Err(anyhow::anyhow!(
                            "Is it a fish? Is is a frog? On no, it's a fly."
                        ))
                    }
                    1 => &ResolvedPackage {
                        package: packages[0].package.clone(),
                        variant: String::new(),
                        root_path: packages[0].root_path.clone(),
                    },
                    _ => select_package_variant(&packages)?,
                };

                Ok(package.clone())
            })
            .collect()
    }
}

fn select_package_variant(packages: &[ResolvedPackage]) -> Result<&ResolvedPackage> {
    println!(
        "\nMultiple packages available for {}",
        packages[0].package.name
    );
    for (i, package) in packages.iter().enumerate() {
        println!("  {}. {}: {}", i + 1, package, package.package.description);
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

    Ok(&packages[selection])
}
