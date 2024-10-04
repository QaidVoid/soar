use std::path::Path;

use anyhow::Result;
use tokio::{fs::File, io::AsyncReadExt};

use crate::core::constant::BIN_PATH;

use super::registry::{Package, ResolvedPackage};

pub async fn setup_symlink(install_path: &Path, resolved_package: &ResolvedPackage) -> Result<()> {
    let symlink_path = BIN_PATH.join(&resolved_package.package.bin_name);
    if symlink_path.exists() {
        tokio::fs::remove_file(&symlink_path).await?;
    }
    std::os::unix::fs::symlink(install_path, symlink_path)?;
    Ok(())
}

pub async fn verify_checksum(path: &Path, package: &Package) -> Result<bool> {
    let mut file = File::open(path).await?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 8192];

    while let Ok(n) = file.read(&mut buffer).await {
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hasher.finalize().to_hex().to_string() == package.bsum)
}
