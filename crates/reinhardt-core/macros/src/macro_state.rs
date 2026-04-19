//! Shared state file for cross-macro communication between `installed_apps!`,
//! `#[url_patterns]`, and `#[routes]`.
//!
//! These proc macros expand within the same user crate, but cannot share data through
//! Rust's type system. This module provides file-based state sharing:
//!
//! - `installed_apps!` writes the list of app labels to a state file
//! - `#[url_patterns]` reads that file to validate the app name identifier
//! - `#[routes]` reads that file and generates `url_prelude` directly
//!
//! This eliminates the need for the `#[macro_export]` callback pattern that triggers
//! `macro_expanded_macro_exports_accessed_by_absolute_paths` on Rust 1.94+.
//!
//! ## Why `CARGO_MANIFEST_DIR/target/` instead of `OUT_DIR`?
//!
//! `OUT_DIR` is only available during build-script execution, not during proc-macro
//! expansion. Proc macros have access to `CARGO_MANIFEST_DIR` via `std::env::var`,
//! so the state file is placed under `$CARGO_MANIFEST_DIR/target/reinhardt/`.

use std::path::PathBuf;

/// File name for the installed apps state.
const STATE_FILE_NAME: &str = ".installed_apps";

/// Subdirectory under `target/` for reinhardt state files.
const STATE_SUBDIR: &str = "reinhardt";

/// Returns the directory path for state files: `$CARGO_MANIFEST_DIR/target/reinhardt/`.
fn state_dir_path() -> Result<PathBuf, String> {
	let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
		"CARGO_MANIFEST_DIR not set. Cannot locate installed apps state file".to_string()
	})?;
	Ok(PathBuf::from(manifest_dir)
		.join("target")
		.join(STATE_SUBDIR))
}

/// Writes the installed app labels to the state file.
///
/// Creates the directory structure if it does not exist.
/// Labels are written as newline-separated UTF-8 text.
///
/// Returns an error if the directory cannot be created or the file cannot be written.
pub(crate) fn write_installed_apps(labels: &[String]) -> Result<(), String> {
	let dir = state_dir_path()?;
	std::fs::create_dir_all(&dir)
		.map_err(|e| format!("Cannot create state directory {}: {e}", dir.display()))?;
	let path = dir.join(STATE_FILE_NAME);
	let content = labels.join("\n");
	std::fs::write(&path, content).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}

/// Reads the installed app labels from the state file.
///
/// Returns a vector of label strings, or an error message if the file cannot be read.
pub(crate) fn read_installed_apps() -> Result<Vec<String>, String> {
	let dir = state_dir_path()?;
	let path = dir.join(STATE_FILE_NAME);
	let content = std::fs::read_to_string(&path)
		.map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
	Ok(content
		.lines()
		.filter(|line| !line.is_empty())
		.map(|line| line.to_string())
		.collect())
}

/// Returns true if the current compilation target is WASM.
///
/// Checks `CARGO_CFG_TARGET_FAMILY` and `CARGO_CFG_TARGET_OS` environment
/// variables set by Cargo during crate compilation.
pub(crate) fn is_wasm_target() -> bool {
	let family = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
	let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
	family == "wasm" && os == "unknown"
}
