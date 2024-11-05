use std::{
    fs::{File, Permissions},
    io::{BufReader, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Url;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::{BIN_PATH, PACKAGES_PATH},
        file::{get_file_type, FileType},
        util::{calculate_checksum, download_progress_style, validate_checksum},
    },
    registry::installed::InstalledPackages,
};

use super::{
    appimage::{integrate_appimage, integrate_using_remote_files, setup_portable_dir},
    ResolvedPackage,
};

pub struct Installer {
    resolved_package: ResolvedPackage,
    install_path: PathBuf,
    temp_path: PathBuf,
}

impl Installer {
    pub fn new(package: &ResolvedPackage) -> Self {
        let temp_path = PACKAGES_PATH
            .join("tmp")
            .join(package.package.full_name('-'))
            .with_extension("part");
        Self {
            resolved_package: package.to_owned(),
            install_path: Path::new("").to_path_buf(),
            temp_path,
        }
    }

    pub async fn execute(
        &mut self,
        idx: usize,
        total: usize,
        installed_packages: Arc<Mutex<InstalledPackages>>,
        portable: Option<String>,
        portable_home: Option<String>,
        portable_config: Option<String>,
        multi_progress: Arc<MultiProgress>,
    ) -> Result<()> {
        let package = &self.resolved_package.package;

        let prefix = format!(
            "[{}/{}] {}",
            (idx + 1).color(Color::Green),
            total.color(Color::Cyan),
            package.full_name('/').color(Color::BrightBlue)
        );

        if let Some(parent) = self.temp_path.parent() {
            fs::create_dir_all(parent).await.context(format!(
                "{}: Failed to create temp directory {}",
                prefix,
                self.temp_path.to_string_lossy().color(Color::Blue)
            ))?;
        }

        if Url::parse(&package.download_url).is_ok() {
            self.download_remote_package(multi_progress.clone(), &prefix)
                .await?;
        } else {
            self.copy_local_package(multi_progress.clone(), &prefix)
                .await?;
        }

        let checksum = calculate_checksum(&self.temp_path).await?;

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

        let warn = multi_progress.insert(1, ProgressBar::new(0));
        warn.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
        match file_type {
            FileType::AppImage => {
                if integrate_appimage(&mut file, package, &self.install_path)
                    .await
                    .is_ok()
                {
                    setup_portable_dir(
                        &package.pkg_name,
                        &self.install_path,
                        portable,
                        portable_home,
                        portable_config,
                    )
                    .await?;
                } else {
                    warn.finish_with_message(format!(
                        "{}: {}",
                        prefix,
                        "Failed to integrate AppImage".color(Color::BrightYellow)
                    ));
                };
            }
            FileType::FlatImage => {
                if integrate_using_remote_files(package, &self.install_path)
                    .await
                    .is_ok()
                {
                    setup_portable_dir(
                        &package.pkg_name,
                        Path::new(&format!(".{}", self.install_path.display())),
                        None,
                        None,
                        portable_config,
                    )
                    .await?;
                } else {
                    warn.finish_with_message(format!(
                        "{}: {}",
                        prefix,
                        "Failed to integrate FlatImage".color(Color::BrightYellow)
                    ));
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
        installed_progress.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
        installed_progress.finish_with_message(format!(
            "[{}/{}] Installed {}",
            (idx + 1).color(Color::Green),
            total.color(Color::Cyan),
            package.full_name('/').color(Color::Blue)
        ));

        if !package.note.is_empty() {
            println!(
                "{}: {}",
                prefix,
                package
                    .note
                    .replace("<br>", "\n     ")
                    .color(Color::BrightYellow)
            );
        }

        Ok(())
    }

    async fn download_remote_package(
        &self,
        multi_progress: Arc<MultiProgress>,
        prefix: &str,
    ) -> Result<()> {
        let prefix = prefix.to_owned();
        let package = &self.resolved_package.package;
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
        download_progress.set_style(download_progress_style(true));

        download_progress.set_length(total_size);
        download_progress.set_message(prefix.clone());

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "{} Download failed {:?}",
                prefix,
                response.status().color(Color::Red),
            ));
        }

        let mut file = fs::OpenOptions::new()
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
        download_progress.finish();
        file.flush().await?;

        let warn_bar = multi_progress.insert_from_back(1, ProgressBar::new(0));
        warn_bar.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
        if package.bsum == "null" {
            warn_bar.finish_with_message(format!(
                "{}: {}",
                prefix,
                "Missing checksum. Installing anyway.".color(Color::BrightYellow)
            ));
        } else {
            let result = validate_checksum(&package.bsum, &self.temp_path).await;
            if result.is_err() {
                warn_bar.finish_with_message(format!(
                    "{}: {}",
                    prefix,
                    "Checksum verification failed. Installing anyway.".color(Color::BrightYellow)
                ));
            }
        }

        Ok(())
    }

    async fn copy_local_package(
        &self,
        multi_progress: Arc<MultiProgress>,
        prefix: &str,
    ) -> Result<()> {
        let temp_path = &self.temp_path;
        let prefix = prefix.to_owned();
        let package = &self.resolved_package.package;

        let download_progress = multi_progress.insert_from_back(1, ProgressBar::new(0));
        download_progress.set_style(download_progress_style(true));

        let total_size = package.size.parse::<u64>().unwrap_or_default();
        download_progress.set_length(total_size);
        download_progress.set_message(prefix.clone());

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&temp_path)
            .await
            .context(format!("{}: Failed to open temp file for writing", prefix))?;
        let mut source = fs::File::open(&package.download_url).await?;
        let mut buffer = vec![0u8; 8096];

        while let Ok(n) = source.read(&mut buffer).await {
            if n == 0 {
                break;
            }

            file.write_all(&buffer[..n]).await?;
            download_progress.inc(n as u64);
        }
        download_progress.finish();
        file.flush().await?;

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
        let symlink_path = &BIN_PATH.join(&package.pkg_name);
        if symlink_path.exists() {
            if let Ok(link) = symlink_path.read_link() {
                if let Ok(parent) = link.strip_prefix(&*PACKAGES_PATH) {
                    let package_name = &parent.parent().unwrap().to_string_lossy()[9..];

                    if package_name == package.full_name('-') {
                        fs::remove_dir_all(link.parent().unwrap()).await?;
                    }
                };
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
