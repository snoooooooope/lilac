use config::{Config, ConfigError, File, Environment};
use serde::Deserialize;
use tempfile::{tempdir, TempDir};
use std::path::PathBuf;
use dirs;
use std::fs;

const DEFAULT_AUR_BASE_URL: &str = "https://aur.archlinux.org";
const DEFAULT_CONFIG_CONTENT: &str = r#"
# Base URL for the AUR RPC interface
aur_base_url = "https://aur.archlinux.org"
"#;

const DEFAULT_CACHE_DIR: &str = ".cache/lilac";

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_aur_base_url")]
    pub aur_base_url: String,
    #[serde(skip)]
    pub temp_dir: Option<TempDir>,
}

fn default_aur_base_url() -> String {
    DEFAULT_AUR_BASE_URL.to_string()
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let temp_dir = tempdir().map_err(|e| {
            ConfigError::Message(format!("Failed to create temp directory: {}", e))
        })?;

        let user_config_path: Option<PathBuf> = dirs::config_dir()
            .map(|mut path| {
                path.push("lilac");
                path.push("config.toml");
                path
            });

        if let Some(ref path) = user_config_path {
            if let Some(dir_path) = path.parent() {
                fs::create_dir_all(dir_path).map_err(|e| {
                    ConfigError::Message(format!("Failed to create config directory {}: {}", dir_path.display(), e))
                })?;

                if !path.exists() {
                    fs::write(path, DEFAULT_CONFIG_CONTENT).map_err(|e| {
                        ConfigError::Message(format!("Failed to create default config file {}: {}", path.display(), e))
                    })?;
                }
            }
        }

        let mut config_builder = Config::builder();

        if let Some(ref path) = user_config_path {
             config_builder = config_builder.add_source(
                 File::from(path.clone()).required(false)
             );
        }

        // Add default config file and environment variables
        let mut config = config_builder
            .add_source(File::with_name("config/default").required(false))
            .add_source(Environment::with_prefix("LILAC"))
            .build()?
            .try_deserialize::<Self>()?;

        config.temp_dir = Some(temp_dir);
        Ok(config)
    }

    /// Gets the path to the temp directory
    pub fn temp_path(&self) -> &std::path::Path {
        self.temp_dir.as_ref()
            .expect("Temp directory should exist")
            .path()
    }

    /// Gets the path to the cache directory and ensures it exists
    pub fn cache_path(&self) -> Result<PathBuf, ConfigError> {
        let cache_dir = dirs::home_dir()
            .ok_or_else(|| ConfigError::Message("Failed to resolve home directory".into()))?
            .join(DEFAULT_CACHE_DIR);

        fs::create_dir_all(&cache_dir).map_err(|e| {
            ConfigError::Message(format!("Failed to create cache directory: {}", e))
        })?;

        Ok(cache_dir)
    }
}
