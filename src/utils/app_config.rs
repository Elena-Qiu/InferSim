use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};

use config::{Environment, Source};
use lazy_static::lazy_static;

use super::error::Result;

static DEFAULT_CONFIG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/default_config.toml"));

/// A new type to impl `config::Source`
#[derive(Debug, Clone, serde::Deserialize)]
struct Preset(HashMap<String, config::Value>);

impl config::Source for Preset {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> std::result::Result<HashMap<String, config::Value>, config::ConfigError> {
        let mut kv = self.0.clone();
        // make sure it's not getting endlessly recursive
        kv.remove("presets");
        Ok(kv)
    }
}

/// The main structure holding application config
pub struct AppConfig(config::Config);

impl AppConfig {
    fn new() -> Self {
        // Start with empty
        Self(config::Config::new())
    }

    pub fn setup(&mut self) -> Result<&mut Self> {
        // Merge with default config
        self.0
            .merge(config::File::from_str(&DEFAULT_CONFIG, config::FileFormat::Toml))?;

        // Merge settings with env variables
        self.0.merge(Environment::with_prefix("INFERSIM"))?;

        Ok(self)
    }

    /// Load config from a file
    pub fn use_file(&mut self, path: &Path) -> Result<&mut Self> {
        self.0.merge(config::File::from(path))?;
        Ok(self)
    }

    /// Load preset
    pub fn use_preset(&mut self, name: &str) -> Result<&mut Self> {
        // load the preset
        let preset: Preset = self.get(format!("presets.{}", name))?;
        self.0.merge(preset)?;
        Ok(self)
    }

    /// Get a single value and deserialize to the given type
    pub fn get<T, K>(&self, key: K) -> Result<T>
    where
        // use DeserializeOwned, because we are reading CONFIG using RWLock
        // and the lock is released before returning. So T should not borrow
        // anything from CONFIG.
        T: serde::de::DeserializeOwned,
        K: AsRef<str>,
    {
        Ok(self.0.get(key.as_ref())?)
    }

    /// Get a single value and deserialize to the given type
    pub fn fetch<T>(&self) -> Result<T>
    where
        // use DeserializeOwned, because we are reading CONFIG using RWLock
        // and the lock is released before returning. So T should not borrow
        // anything from CONFIG.
        T: serde::de::DeserializeOwned,
    {
        let t = self.0.clone().try_into()?;
        Ok(t)
    }
}

lazy_static! {
    /// global AppConfig instance
    static ref CONFIG: RwLock<AppConfig> = RwLock::new(AppConfig::new());
}

pub fn setup() -> Result<()> {
    config_mut().setup()?;
    Ok(())
}

/// global AppConfig instance
pub fn config() -> RwLockReadGuard<'static, AppConfig> {
    CONFIG.read().unwrap()
}

/// mutable global AppConfig instance
pub fn config_mut() -> RwLockWriteGuard<'static, AppConfig> {
    CONFIG.write().unwrap()
}

pub mod prelude {
    pub use super::{config, config_mut};
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    fn test_config() -> AppConfig {
        let mut config = AppConfig::new();
        config.setup().unwrap();
        config
            .use_file(Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/test_config.toml"
            )))
            .unwrap();

        config
    }

    #[test]
    fn fetch_config() {
        // Initialize configuration
        let config = test_config();

        #[derive(Deserialize)]
        struct Database {
            url: String,
        }
        #[derive(Deserialize)]
        struct Fragment {
            debug: bool,
            database: Database,
        }

        // Fetch an instance of Config
        let frag: Fragment = config.fetch().unwrap();

        // Check the values
        assert!(!frag.debug);
        assert_eq!(frag.database.url, "custom database url");
    }

    #[test]
    fn verify_get() {
        // Initialize configuration
        let config = test_config();

        let debug: bool = config.get("debug").unwrap();
        let url: String = config.get("database.url").unwrap();

        // Check value with get
        assert!(!debug);
        assert_eq!(url, "custom database url");
    }

    #[test]
    fn profile() {
        let mut config = test_config();

        // the global value
        let debug: bool = config.get("debug").unwrap();
        assert!(!debug);

        config.use_preset("abc").unwrap();
        // value from preset
        let debug: bool = config.get("debug").unwrap();
        assert!(debug);

        let dec: usize = config.get("dec").unwrap();
        assert_eq!(dec, 1);
    }
}
