use std::{fs::Permissions, io::Write, os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use futures::StreamExt;
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::{BIN_PATH, PACKAGES_PATH},
        util::{calculate_checksum, format_bytes, validate_checksum},
    },
    error,
    registry::{
        installed::InstalledPackages,
        package::{
            appimage::{check_user_ns, extract_appimage, setup_portable_dir},
            RootPath,
        },
    },
    warn,
};

use super::ResolvedPackage;

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
        is_update: bool,
        portable: Option<String>,
        portable_home: Option<String>,
        portable_config: Option<String>,
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
            error!("{}: Package is already installed", prefix);
            return Err(anyhow::anyhow!(""));
        }

        if is_installed && !is_update {
            warn!("{}: Reinstalling package", prefix);
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
        println!(
            "{}: Downloading package [{}]",
            prefix,
            format_bytes(total_size).color(Color::Yellow)
        );

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
            }
            file.flush().await?;
        }

        if package.bsum == "null" {
            error!(
                "Missing checksum for {}. Installing anyway.",
                package.full_name('/').color(Color::BrightBlue)
            );
        } else {
            let result = validate_checksum(&package.bsum, &self.temp_path).await;
            if result.is_err() {
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
        self.symlink_bin(&installed_packages).await?;
        if self.resolved_package.root_path == RootPath::Pkg {
            extract_appimage(package, &self.install_path).await?;
            setup_portable_dir(
                &package.bin_name,
                &self.install_path,
                portable,
                portable_home,
                portable_config,
            )
            .await?;
        }

        {
            let mut installed_packages = installed_packages.lock().await;
            installed_packages
                .register_package(&self.resolved_package, &checksum)
                .await?;
        }

        println!("{}: Installed package.", prefix);
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

        if self.resolved_package.root_path == RootPath::Pkg {
            check_user_ns().await;
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

    async fn symlink_bin(&self, installed_packages: &Arc<Mutex<InstalledPackages>>) -> Result<()> {
        let package = &self.resolved_package.package;
        let install_path = &self.install_path;
        let symlink_path = &BIN_PATH.join(&package.bin_name);
        let installed_guard = installed_packages.lock().await;
        if symlink_path.exists() {
            if let Ok(link) = symlink_path.read_link() {
                if &link != install_path {
                    if let Some(path_owner) =
                        installed_guard.reverse_package_search(link.strip_prefix(&*PACKAGES_PATH)?)
                    {
                        warn!(
                            "The package {} owns the binary {}",
                            path_owner.name, &package.bin_name
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
