//! Shared state file for cross-macro communication between `installed_apps!` and `#[routes]`.
//!
//! Both proc macros expand within the same user crate, but cannot share data through
//! Rust's type system. This module provides file-based state sharing:
//!
//! - `installed_apps!` writes the list of app labels to a state file
//! - `#[routes]` reads that file and generates `url_prelude` directly
//!
//! This eliminates the need for the `#[macro_export]` callback pattern that triggers
//! `macro_expanded_macro_exports_accessed_by_absolute_paths` on Rust 1.94+.

use std::path::PathBuf;

/// File name for the installed apps state.
const STATE_FILE_NAME: &str = ".installed_apps";

/// Subdirectory under `target/` for reinhardt state files.
const STATE_SUBDIR: &str = "reinhardt";

/// Returns the directory path for state files: `$CARGO_MANIFEST_DIR/target/reinhardt/`.
fn state_dir_path() -> Option<PathBuf> {
	let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
	Some(
		PathBuf::from(manifest_dir)
			.join("target")
			.join(STATE_SUBDIR),
	)
}

/// Writes the installed app labels to the state file.
///
/// Creates the directory structure if it does not exist.
/// Labels are written as newline-separated UTF-8 text.
pub(crate) fn write_installed_apps(labels: &[String]) {
	let Some(dir) = state_dir_path() else {
		return;
	};
	if std::fs::create_dir_all(&dir).is_err() {
		return;
	}
	let content = labels.join("\n");
	// Silently ignore write failures; the error will surface when #[routes] reads the file.
	let _ = std::fs::write(dir.join(STATE_FILE_NAME), content);
}

/// Reads the installed app labels from the state file.
///
/// Returns a vector of label strings, or an error message if the file cannot be read.
pub(crate) fn read_installed_apps() -> Result<Vec<String>, String> {
	let dir = state_dir_path().ok_or_else(|| {
		"CARGO_MANIFEST_DIR not set. Cannot locate installed apps state file".to_string()
	})?;
	let path = dir.join(STATE_FILE_NAME);
	let content = std::fs::read_to_string(&path)
		.map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
	Ok(content
		.lines()
		.filter(|line| !line.is_empty())
		.map(|line| line.to_string())
		.collect())
}
