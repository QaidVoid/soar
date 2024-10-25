use std::{collections::HashMap, env::consts::ARCH, fs, path::PathBuf, sync::LazyLock};

use serde::{Deserialize, Serialize};

use crate::core::color::{Color, ColorExt};

use super::{constant::REGISTRY_PATH, util::home_config_path};

/// Application's configuration
#[derive(Deserialize, Serialize)]
pub struct Config {
    /// Path to the directory where app data is stored.
    pub soar_path: String,

    /// A list of remote repositories to fetch packages from.
    pub repositories: Vec<Repository>,

    /// Indicates whether downloads should be performed in parallel.
    pub parallel: Option<bool>,

    /// Limit the number of parallel downloads
    pub parallel_limit: Option<u32>,
}

/// Struct representing a repository configuration.
#[derive(Deserialize, Serialize)]
pub struct Repository {
    /// Name of the repository.
    pub name: String,

    /// URL of the repository.
    pub url: String,

    /// Optional field specifying a custom registry file for the repository. Default:
    /// `metadata.json`
    pub registry: Option<String>,

    /// Download Sources for different collections
    pub sources: HashMap<String, String>,
}

impl Repository {
    pub fn get_path(&self) -> PathBuf {
        REGISTRY_PATH.join(&self.name)
    }
}

impl Config {
    /// Creates a new configuration by loading it from the configuration file.
    /// If the configuration file is not found, it generates a new default configuration.
    pub fn new() -> Self {
        let home_config = home_config_path();
        let pkg_config = PathBuf::from(home_config).join(env!("CARGO_PKG_NAME"));
        let config_path = pkg_config.join("config.json");
        let content = match fs::read(&config_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(&pkg_config).unwrap();
                eprintln!(
                    "{}\nGenerating default config at {}",
                    "Config not found".color(Color::BrightRed),
                    config_path.to_string_lossy().color(Color::Green)
                );
                Config::generate(config_path)
            }
            Err(e) => {
                panic!("Error reading config file: {:?}", e);
            }
        };
        serde_json::from_slice(&content)
            .unwrap_or_else(|e| panic!("Failed to parse config file: {:?}", e))
    }

    fn generate(config_path: PathBuf) -> Vec<u8> {
        let sources = HashMap::from([
            ("bin".to_owned(), format!("https://bin.ajam.dev/{ARCH}")),
            (
                "base".to_owned(),
                format!("https://bin.ajam.dev/{ARCH}/Baseutils"),
            ),
            ("pkg".to_owned(), format!("https://pkg.ajam.dev/{ARCH}")),
        ]);

        let def_config = Self {
            soar_path: "$HOME/.soar".to_owned(),
            repositories: vec![Repository {
                name: "ajam".to_owned(),
                url: "https://bin.ajam.dev/{ARCH}".to_owned(),
                registry: Some("METADATA.AIO.json".to_owned()),
                sources,
            }],
            parallel: Some(true),
            parallel_limit: Some(2),
        };
        let serialized = serde_json::to_vec_pretty(&def_config).unwrap();
        fs::write(config_path, &serialized).unwrap();
        serialized
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// Initializes the global configuration by forcing the static `CONFIG` to load.
pub fn init() {
    let _ = &*CONFIG;
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(Config::default);
