use std::{env, fs, path::PathBuf, sync::LazyLock};

use serde::{Deserialize, Serialize};

/// Application's configuration
#[derive(Deserialize, Serialize)]
pub struct Config {
    /// Path to the directory where app data is stored.
    pub soar_path: String,

    /// A list of remote repositories to fetch packages from.
    pub repositories: Vec<Repository>,

    /// Indicates whether downloads should be performed in parallel.
    pub parallel: bool,
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
}

impl Config {
    /// Creates a new configuration by loading it from the configuration file.
    /// If the configuration file is not found, it generates a new default configuration.
    pub fn new() -> Self {
        let home_config = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            env::var("HOME").map_or_else(
                |_| panic!("Failed to retrieve HOME environment variable"),
                |home| format!("{}/.config", home),
            )
        });
        let pkg_config = PathBuf::from(home_config).join(env!("CARGO_PKG_NAME"));
        let config_path = pkg_config.join("config.json");
        let content = match fs::read(&config_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(pkg_config).unwrap();
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
        let def_config = Self {
            soar_path: "$HOME/.soar".to_owned(),
            repositories: vec![Repository {
                name: "ajam".to_owned(),
                url: "https://pkg.ajam.dev".to_owned(),
                registry: Some("METADATA.AIO.json".to_owned()),
            }],
            parallel: true,
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
