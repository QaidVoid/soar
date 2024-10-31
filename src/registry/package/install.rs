use std::{
    fs::{File, Permissions},
    io::{BufReader, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::{BIN_PATH, PACKAGES_PATH},
        file::{get_file_type, FileType},
        util::{calculate_checksum, format_bytes, validate_checksum},
    },
    registry::{
        installed::InstalledPackages,
        package::appimage::{integrate_appimage, setup_portable_dir},
    },
    warn,
};

use super::{appimage::integrate_using_remote_files, ResolvedPackage};

pub struct Installer {
    resolved_package: ResolvedPackage,
    install_path: PathBuf,
    temp_path: PathBuf,
}

impl Installer {
    pub fn new(package: &ResolvedPackage, install_path: PathBuf) -> Self {
        let temp_path = PACKAGES_PATH
            .join("tmp")
            .join(package.package.full_name('-'))
            .with_extension("part");
        Self {
            resolved_package: package.to_owned(),
            install_path,
            temp_path,
        }
    }

    pub async fn execute(
        &mut self,
        idx: usize,
        total: usize,
        installed_packages: Arc<Mutex<InstalledPackages>>,
        force: bool,
        portable: Option<String>,
        portable_home: Option<String>,
        portable_config: Option<String>,
        multi_progress: Arc<MultiProgress>,
        yes: bool,
    ) -> Result<()> {
        let package = &self.resolved_package.package;
        let is_installed = installed_packages
            .lock()
            .await
            .is_installed(&self.resolved_package);

        let prefix = format!(
            "[{}/{}] {}",
            (idx + 1).color(Color::Green),
            total.color(Color::Cyan),
            package.full_name('/').color(Color::BrightBlue)
        );

        if !force && is_installed {
            return Err(anyhow::anyhow!("{}: Package is already installed", prefix));
        }

        if let Some(parent) = self.temp_path.parent() {
            fs::create_dir_all(parent).await.context(format!(
                "{}: Failed to create temp directory {}",
                prefix,
                self.temp_path.to_string_lossy().color(Color::Blue)
            ))?;
        }

        let temp_path = &self.temp_path;
        let client = reqwest::Client::new();
        let downloaded_bytes = if temp_path.exists() {
            let meta = fs::metadata(&temp_path).await?;
            meta.len()
        } else {
            0
        };

        let response = client
            .get(&package.download_url)
            .header("Range", format!("bytes={}-", downloaded_bytes))
            .send()
            .await
            .context(format!("{}: Failed to download package", prefix))?;
        let total_size = response
            .content_length()
            .map(|cl| cl + downloaded_bytes)
            .unwrap_or(0);

        let download_progress = multi_progress.insert_from_back(1, ProgressBar::new(0));
        download_progress.set_style(
            ProgressStyle::with_template(
                "{msg:48} [{wide_bar:.green/white}] {speed:14} {computed_bytes:22}",
            )
            .unwrap()
            .with_key(
                "computed_bytes",
                |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                    write!(
                        w,
                        "{}/{}",
                        format_bytes(state.pos()),
                        format_bytes(state.len().unwrap_or_default())
                    )
                    .unwrap()
                },
            )
            .with_key(
                "speed",
                |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                    let pos = state.pos() as f64;
                    let elapsed = state.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (pos / elapsed) as u64
                    } else {
                        0
                    };
                    write!(w, "{}/s", format_bytes(speed)).unwrap()
                },
            )
            .progress_chars("━━"),
        );

        download_progress.set_length(total_size);
        download_progress.set_message(prefix.clone());

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "{} Download failed {:?}",
                prefix,
                response.status().color(Color::Red),
            ));
        }

        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(&temp_path)
                .await
                .context(format!("{}: Failed to open temp file for writing", prefix))?;
            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.context(format!("{}: Failed to read chunk", prefix))?;
                file.write_all(&chunk).await?;
                download_progress.inc(chunk.len() as u64);
            }
            file.flush().await?;
        }

        download_progress.finish();

        if package.bsum == "null" {
            warn!(
                "Missing checksum for {}. Installing anyway.",
                package.full_name('/').color(Color::BrightBlue)
            );
        } else {
            let result = validate_checksum(&package.bsum, &self.temp_path).await;
            if result.is_err() {
                if yes {
                    warn!("Checksum verification failed. Installing anyway.");
                } else {
                    eprint!(
                        "\n{}: Checksum verification failed. Do you want to remove the package? (y/n): ",
                        prefix
                    );
                    std::io::stdout().flush()?;

                    let mut response = String::new();
                    std::io::stdin().read_line(&mut response)?;

                    if response.trim().eq_ignore_ascii_case("y") {
                        tokio::fs::remove_file(&temp_path).await?;
                        return Err(anyhow::anyhow!("Checksum verification failed."));
                    }
                }
            }
        }
        let checksum = calculate_checksum(temp_path).await?;

        self.install_path = package.get_install_path(&checksum);
        if let Some(parent) = self.install_path.parent() {
            fs::create_dir_all(parent).await.context(format!(
                "{}: Failed to create install directory {}",
                prefix,
                self.install_path.to_string_lossy().color(Color::Blue)
            ))?;
        }

        self.save_file().await?;
        self.symlink_bin().await?;

        let mut file = BufReader::new(File::open(&self.install_path)?);
        let file_type = get_file_type(&mut file);

        match file_type {
            FileType::AppImage => {
                if integrate_appimage(&mut file, package, &self.install_path)
                    .await
                    .is_ok()
                {
                    setup_portable_dir(
                        &package.bin_name,
                        &self.install_path,
                        portable,
                        portable_home,
                        portable_config,
                    )
                    .await?;
                } else {
                    warn!("{}: Failed to integrate AppImage", prefix);
                };
            }
            FileType::FlatImage => {
                if integrate_using_remote_files(package, &self.install_path)
                    .await
                    .is_err()
                {
                    warn!("{}: Failed to integrate FlatImage", prefix);
                };
            }
            _ => {}
        }

        {
            let mut installed_packages = installed_packages.lock().await;
            installed_packages
                .register_package(&self.resolved_package, &checksum)
                .await?;
        }

        let installed_progress = multi_progress.insert_from_back(1, ProgressBar::new(0));
        installed_progress.set_style(
            ProgressStyle::default_bar()
                .template("{msg}")
                .unwrap()
                .progress_chars("##-"),
        );
        installed_progress.finish_with_message(format!(
            "[{}/{}] Installed {}",
            (idx + 1).color(Color::Green),
            total.color(Color::Cyan),
            package.full_name('/').color(Color::Blue)
        ));

        if !package.note.is_empty() {
            println!(
                "{}: [{}] {}",
                prefix,
                "Note".color(Color::Magenta),
                package
                    .note
                    .replace("<br>", "\n     ")
                    .color(Color::BrightYellow)
            );
        }

        Ok(())
    }

    async fn save_file(&self) -> Result<()> {
        let install_path = &self.install_path;
        let temp_path = &self.temp_path;
        if install_path.exists() {
            tokio::fs::remove_file(&install_path).await?;
        }
        tokio::fs::rename(&temp_path, &install_path).await?;
        tokio::fs::set_permissions(&install_path, Permissions::from_mode(0o755)).await?;
        xattr::set(install_path, "user.managed_by", b"soar")?;

        Ok(())
    }

    async fn symlink_bin(&self) -> Result<()> {
        let package = &self.resolved_package.package;
        let install_path = &self.install_path;
        let symlink_path = &BIN_PATH.join(&package.bin_name);
        if symlink_path.exists() {
            if let Ok(link) = symlink_path.read_link() {
                if &link != install_path {
                    if let Ok(parent) = link.strip_prefix(&*PACKAGES_PATH) {
                        let package_name =
                            parent.parent().unwrap().to_string_lossy()[9..].replacen("-", "/", 1);

                        if package_name == package.full_name('-') {
                            fs::remove_dir_all(link.parent().unwrap()).await?;
                        } else {
                            warn!(
                                "The package {} owns the binary {}",
                                package_name, &package.bin_name
                            );
                            print!(
                                "Do you want to switch to {} (y/N)? ",
                                package.full_name('/').color(Color::Blue)
                            );
                            std::io::stdout().flush()?;

                            let mut response = String::new();
                            std::io::stdin().read_line(&mut response)?;

                            if !response.trim().eq_ignore_ascii_case("y") {
                                return Ok(());
                            }
                        }
                    };
                }
            }
            fs::remove_file(symlink_path).await?;
        }
        fs::symlink(&install_path, &symlink_path)
            .await
            .context(format!(
                "Failed to link {} to {}",
                install_path.to_string_lossy(),
                symlink_path.to_string_lossy()
            ))?;

        Ok(())
    }
}
