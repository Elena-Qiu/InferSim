use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::utils::app_config::AppConfig;
use crate::utils::prelude::*;

#[derive(Deserialize)]
pub(crate) struct OutputDir(PathBuf);

impl OutputDir {
    pub fn file(&self, name: impl AsRef<Path>) -> Result<PathBuf> {
        fs::create_dir_all(&self.0).kind(ErrorKind::InvalidConfig)?;
        Ok(self.0.join(name))
    }
}

pub(crate) trait AppConfigExt {
    fn output_dir(&self) -> Result<OutputDir>;
}

impl AppConfigExt for AppConfig {
    fn output_dir(&self) -> Result<OutputDir> {
        self.get("output_dir")
    }
}
