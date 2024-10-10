mod install;
mod remove;
pub mod run;
pub mod update;

use std::{fmt::Display, path::PathBuf, sync::Arc};

use anyhow::Result;
use install::Installer;
use remove::Remover;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::core::constant::PACKAGES_PATH;

use super::installed::InstalledPackages;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub bin_name: String,
    pub description: String,
    pub version: String,
    pub download_url: String,
    pub size: String,
    pub bsum: String,
    pub build_date: String,
    pub src_url: String,
    pub web_url: String,
    pub build_script: String,
    pub build_log: String,
    pub category: String,
    pub extra_bins: String,
    pub variant: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct ResolvedPackage {
    pub repo_name: String,
    pub root_path: RootPath,
    pub package: Package,
}

impl ResolvedPackage {
    pub async fn install(
        &self,
        idx: usize,
        total: usize,
        force: bool,
        is_update: bool,
        installed_packages: Arc<Mutex<InstalledPackages>>,
    ) -> Result<()> {
        let install_path = self.package.get_install_path(&self.package.bsum);
        let mut installer = Installer::new(self, install_path);
        installer
            .execute(idx, total, installed_packages, force, is_update)
            .await?;
        Ok(())
    }

    pub async fn remove(&self) -> Result<()> {
        let remover = Remover::new(self).await?;
        let mut installed_packages = InstalledPackages::new().await?;
        remover.execute(&mut installed_packages).await?;
        Ok(())
    }
}

impl Package {
    pub fn get_install_dir(&self, checksum: &str) -> PathBuf {
        PACKAGES_PATH.join(format!("{}-{}", checksum, self.full_name('-')))
    }

    pub fn get_install_path(&self, checksum: &str) -> PathBuf {
        self.get_install_dir(checksum)
            .join("bin")
            .join(&self.bin_name)
    }

    pub fn full_name(&self, join_char: char) -> String {
        let variant_prefix = self
            .variant
            .to_owned()
            .map(|variant| format!("{}{}", variant, join_char))
            .unwrap_or_default();
        format!("{}{}", variant_prefix, self.name)
    }
}

#[derive(Debug)]
pub struct PackageQuery {
    pub name: String,
    pub variant: Option<String>,
    pub root_path: Option<RootPath>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum RootPath {
    #[default]
    Bin,
    Base,
    Pkg,
}

pub fn parse_package_query(query: &str) -> PackageQuery {
    let (base_query, root_path) = query
        .rsplit_once('#')
        .map(|(n, r)| {
            (
                n.to_owned(),
                match r.to_lowercase().as_str() {
                    "base" => Some(RootPath::Base),
                    "bin" => Some(RootPath::Bin),
                    "pkg" => Some(RootPath::Pkg),
                    _ => {
                        eprintln!("Invalid root path provided for {}", query);
                        std::process::exit(-1);
                    }
                },
            )
        })
        .unwrap_or((query.to_owned(), None));

    let (name, variant) = base_query
        .split_once('/')
        .map(|(v, n)| (n.to_owned(), Some(v.to_owned())))
        .unwrap_or((base_query, None));

    PackageQuery {
        name,
        variant,
        root_path,
    }
}

impl Display for RootPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RootPath::Bin => write!(f, "bin"),
            RootPath::Base => write!(f, "base"),
            RootPath::Pkg => write!(f, "pkg"),
        }
    }
}
