use config::{Config, Environment};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::ops::Deref;
use std::sync::RwLock;

use super::error::Result;

// CONFIG static variable. It's actually an AppConfig
// inside an RwLock.
lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config::new());
}

#[derive(Debug, Deserialize)]
pub struct Database {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub debug: bool,
    pub database: Database,
}

impl AppConfig {
    pub fn init(default_config: Option<&str>) -> Result<()> {
        let mut settings = Config::new();

        // Embed file into executable
        // This macro will embed the configuration file into the
        // executable. Check include_str! for more info.
        if let Some(config_contents) = default_config {
            //let contents = include_str!(config_file_path);
            settings.merge(config::File::from_str(&config_contents, config::FileFormat::Toml))?;
        }

        // Merge settings with env variables
        settings.merge(Environment::with_prefix("APP"))?;

        // TODO: Merge settings with Clap Settings Arguments

        // Save Config to RwLoc
        {
            let mut w = CONFIG.write().unwrap();
            *w = settings;
        }

        Ok(())
    }

    pub fn merge_config(config_file: Option<&str>) -> Result<()> {
        // Merge settings with config file if there is one
        if let Some(config_file_path) = config_file {
            {
                CONFIG
                    .write()
                    .unwrap()
                    .merge(config::File::with_name(config_file_path))?;
            }
        }
        Ok(())
    }

    // Set CONFIG
    pub fn set(key: &str, value: &str) -> Result<()> {
        {
            // Set Property
            CONFIG.write().unwrap().set(key, value)?;
        }

        Ok(())
    }

    // Get a single value
    pub fn get<'de, T>(key: &'de str) -> Result<T>
    where
        T: serde::Deserialize<'de>,
    {
        Ok(CONFIG.read().unwrap().get::<T>(key)?)
    }

    // Get CONFIG
    // This clones Config (from RwLock<Config>) into a new AppConfig object.
    // This means you have to fetch this again if you changed the configuration.
    pub fn fetch() -> Result<AppConfig> {
        // Get a Read Lock from RwLock
        let r = CONFIG.read().unwrap();

        // Clone the Config object
        let config_clone = r.deref().clone();

        // Coerce Config into AppConfig
        Ok(config_clone.try_into()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_CONFIG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/test_config.toml"));

    #[test]
    fn fetch_config() {
        // Initialize configuration
        AppConfig::init(Some(TEST_CONFIG)).unwrap();

        // Fetch an instance of Config
        let config = AppConfig::fetch().unwrap();

        // Check the values
        assert_eq!(config.debug, false);
        assert_eq!(config.database.url, "custom database url");
    }

    #[test]
    fn verify_get() {
        // Initialize configuration
        AppConfig::init(Some(TEST_CONFIG)).unwrap();

        // Check value with get
        assert_eq!(AppConfig::get::<bool>("debug").unwrap(), false);
        assert_eq!(AppConfig::get::<String>("database.url").unwrap(), "custom database url");
    }

    #[test]
    fn verify_set() {
        // Initialize configuration
        AppConfig::init(Some(TEST_CONFIG)).unwrap();

        // Set a field
        AppConfig::set("database.url", "new url").unwrap();

        // Fetch a new instance of Config
        let config = AppConfig::fetch().unwrap();

        // Check value was modified
        assert_eq!(config.database.url, "new url");
    }
}
