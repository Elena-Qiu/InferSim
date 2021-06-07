pub mod app_config;
mod batcher;
mod box_iter;
mod error;
pub mod float;
pub mod logging;
pub mod panic;

pub mod prelude {
    // for now `use super as utils` does not work
    pub use super::super::utils;

    pub use super::app_config::prelude::*;
    pub use super::error::{Error, ErrorKind, Kind as _, Result};
    pub use super::logging::prelude::*;
}

pub use batcher::Batcher;
pub use box_iter::{BoxIterator, IntoBoxIter};
