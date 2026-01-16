pub mod station;

pub use station::Config;

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Get the configuration file path
pub fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Failed to get config directory")?
        .join("ok");

    // Create config directory if it doesn't exist
    fs::create_dir_all(&config_dir)
        .context("Failed to create config directory")?;

    Ok(config_dir.join("config.toml"))
}

/// Load configuration from file, or create default if not exists
pub fn load_or_create_config() -> Result<Config> {
    let path = config_path()?;

    if path.exists() {
        // Load existing config
        let content = fs::read_to_string(&path)
            .context("Failed to read config file")?;
        let config: Config = toml::from_str(&content)
            .context("Failed to parse config file")?;
        Ok(config)
    } else {
        // Create default config
        let config = Config::default();
        save_config(&config)?;

        println!("Created default config at: {}", path.display());
        println!("Please edit this file to add your API credentials.");

        Ok(config)
    }
}

/// Save configuration to file
pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(config)
        .context("Failed to serialize config")?;
    fs::write(&path, content)
        .context("Failed to write config file")?;
    Ok(())
}
