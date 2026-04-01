pub mod manifest;
pub mod loader;

#[path = "config.gen.rs"]
#[allow(dead_code)]
pub mod config_gen;

pub use loader::{ConfigError, ConfigStore};
pub use manifest::{Manifest, TableEntry};
