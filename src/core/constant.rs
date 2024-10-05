use std::{path::PathBuf, sync::LazyLock};

use console::Emoji;

use super::{config::CONFIG, util::build_path};

pub static TRUCK: Emoji<'_, '_> = Emoji("🚚 ", "");
pub static CLIP: Emoji<'_, '_> = Emoji("🔗 ", "");
pub static PAPER: Emoji<'_, '_> = Emoji("📃 ", "");
pub static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", ":-)");
pub static ERROR: Emoji<'_, '_> = Emoji("💥 ", "");

pub static REGISTRY_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("registry"));
pub static BIN_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| build_path(&CONFIG.soar_path).unwrap().join("bin"));
