use std::{path::PathBuf, sync::LazyLock};

use super::{config::CONFIG, util::build_path};

pub static REGISTRY_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("registry"));
pub static BIN_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("bin"));
pub static INSTALL_TRACK_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("installs"));
pub static PACKAGES_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("packages"));
