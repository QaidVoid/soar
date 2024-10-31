use std::{path::PathBuf, sync::LazyLock};

use super::{config::CONFIG, util::build_path};

pub static CACHE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("cache"));
pub static REGISTRY_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("registry"));
pub static BIN_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("bin"));
pub static INSTALL_TRACK_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("installs"));
pub static PACKAGES_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("packages"));

pub const ELF_MAGIC_BYTES: [u8; 4] = [0x7f, 0x45, 0x4c, 0x46];
pub const APPIMAGE_MAGIC_BYTES: [u8; 4] = [0x41, 0x49, 0x02, 0x00];
pub const FLATIMAGE_MAGIC_BYTES: [u8; 4] = [0x46, 0x49, 0x01, 0x00];

pub const CAP_SYS_ADMIN: i32 = 21;
pub const CAP_MKNOD: i32 = 27;
