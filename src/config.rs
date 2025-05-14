use config::{Config, ConfigError, File, Environment};
use serde::Deserialize;
use tempfile::{tempdir, TempDir};

const DEFAULT_AUR_BASE_URL: &str = "https://aur.archlinux.org";

/// Represents the application configuration.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_aur_base_url")]
    pub aur_base_url: String,
    #[serde(skip)] // This field won't be loaded from config files
    pub temp_dir: Option<TempDir>, // Managed by tempfile
}

fn default_aur_base_url() -> String {
    DEFAULT_AUR_BASE_URL.to_string()
}

impl AppConfig {
    /// Loads the configuration and sets up temporary directory
    pub fn load() -> Result<Self, ConfigError> {
        // Create temp directory that will auto-delete when dropped
        let temp_dir = tempdir().map_err(|e| {
            ConfigError::Message(format!("Failed to create temp directory: {}", e))
        })?;

        let mut config = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(Environment::with_prefix("LILAC"))
            .build()?
            .try_deserialize::<Self>()?;

        // Store the temp dir handle to prevent deletion
        config.temp_dir = Some(temp_dir);
        Ok(config)
    }

    /// Gets the path to the temp directory
    pub fn temp_path(&self) -> &std::path::Path {
        self.temp_dir.as_ref()
            .expect("Temp directory should exist")
            .path()
    }
}
