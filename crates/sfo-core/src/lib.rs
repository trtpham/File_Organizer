//! Smart File Organizer core engine.
//!
//! Rust port of `smart_file_organizer/core/`. Modules land phase by phase;
//! see `docs/brownfield/RUST_MIGRATION_PLAN.md`.

pub mod archiver;
pub mod cloud;
pub mod config;
pub mod dedup;
pub mod doctor;
pub mod error;
pub mod executor;
pub mod fnmatch;
pub mod fsutil;
pub mod guardrails;
pub mod hasher;
pub mod i18n;
pub mod junk;
pub mod lan_share;
pub mod models;
pub mod organizer;
pub mod paths;
pub mod platform;
pub mod progress;
pub mod pyjson;
pub mod renamer;
pub mod reporter;
pub mod scanner;
pub mod secrets;
pub mod shredder;
pub mod smart_filter;
pub mod syncer;
pub mod treemap;
pub mod undo;
pub mod vault;
pub mod visual_hasher;
pub mod watcher;

pub use error::{Result, SfoError};

/// Crate version, shared by the CLI and GUI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application data directory name under the user's home, shared with the
/// Python v3.0 build so existing history and journals stay readable.
pub const APP_DIR_NAME: &str = ".smart_file_organizer";

/// Environment variable that relocates the data directory (portable installs
/// and isolated tests). Not present in the Python build.
pub const DATA_DIR_ENV: &str = "SFO_DATA_DIR";

/// `$SFO_DATA_DIR` if set, else `~/.smart_file_organizer`, falling back to the
/// current directory when the home directory cannot be determined.
pub fn default_data_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|v| !v.is_empty()) {
        return std::path::PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(APP_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(VERSION.starts_with("4."));
    }

    #[test]
    fn app_dir_matches_python_build() {
        assert_eq!(APP_DIR_NAME, ".smart_file_organizer");
        assert!(default_data_dir().ends_with(APP_DIR_NAME));
    }
}
