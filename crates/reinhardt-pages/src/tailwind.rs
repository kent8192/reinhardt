//! Tailwind CSS Integration for Reinhardt Pages
//!
//! This module provides build-time Tailwind CSS integration for reinhardt-pages
//! applications, including:
//!
//! - **Configuration**: `TailwindConfig` for specifying Tailwind version, paths, and options
//! - **CLI Management**: Automatic download and caching of the Tailwind CSS standalone CLI
//! - **Class Scanning**: Extract CSS class names from RSX templates (`.rs` files)
//! - **Build Pipeline**: Generate optimized CSS output from scanned classes
//!
//! ## Feature Gate
//!
//! This module is only available when the `tailwind` feature is enabled:
//!
//! ```toml
//! [dependencies]
//! reinhardt-pages = { version = "0.1", features = ["tailwind"] }
//! ```
//!
//! ## Quick Start
//!
//! ```ignore
//! use reinhardt_pages::tailwind::{TailwindConfig, TailwindBuilder};
//!
//! let config = TailwindConfig::new()
//!     .with_version("4.1.0")
//!     .with_content_paths(vec!["src/**/*.rs".to_string()])
//!     .with_output_path("static/css/tailwind.css".to_string());
//!
//! let builder = TailwindBuilder::new(config);
//! builder.build()?;
//! ```
//!
//! ## Architecture
//!
//! The Tailwind integration pipeline works in the following stages:
//!
//! 1. **Configuration**: Define content paths, output path, and Tailwind version
//! 2. **CLI Resolution**: Download or locate the Tailwind standalone CLI binary
//! 3. **Class Scanning**: Parse RSX templates to extract class attribute values
//! 4. **CSS Generation**: Run the Tailwind CLI to produce optimized CSS

pub mod class_scanner;
pub mod cli;
pub mod config;

pub use class_scanner::ClassScanner;
pub use cli::TailwindCli;
pub use config::TailwindConfig;

use std::io;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

/// Errors that can occur during Tailwind CSS build operations.
#[derive(Debug, Error)]
pub enum TailwindError {
	/// Failed to download the Tailwind CLI binary.
	#[error("failed to download Tailwind CLI: {0}")]
	DownloadFailed(String),

	/// The Tailwind CLI binary was not found at the expected path.
	#[error("Tailwind CLI not found at {path}")]
	CliNotFound {
		/// The path where the CLI was expected.
		path: String,
	},

	/// The Tailwind CLI process exited with a non-zero status code.
	#[error("Tailwind CLI exited with status {status}: {stderr}")]
	CliFailed {
		/// The exit status code.
		status: i32,
		/// Standard error output from the CLI.
		stderr: String,
	},

	/// An I/O error occurred during file operations.
	#[error("I/O error: {0}")]
	Io(#[from] io::Error),

	/// No content paths were configured for class scanning.
	#[error("no content paths configured")]
	NoContentPaths,

	/// An invalid configuration was provided.
	#[error("invalid configuration: {0}")]
	InvalidConfig(String),
}

/// Result type alias for Tailwind operations.
pub type TailwindResult<T> = Result<T, TailwindError>;

/// Build pipeline for generating Tailwind CSS from RSX templates.
///
/// `TailwindBuilder` orchestrates the entire Tailwind CSS build process:
/// resolving the CLI binary, scanning content files, and generating CSS output.
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::tailwind::{TailwindConfig, TailwindBuilder};
///
/// let config = TailwindConfig::new()
///     .with_version("4.1.0")
///     .with_content_paths(vec!["src/**/*.rs".to_string()])
///     .with_output_path("static/css/tailwind.css".to_string());
///
/// let builder = TailwindBuilder::new(config);
/// builder.build()?;
/// ```
pub struct TailwindBuilder {
	/// The Tailwind configuration.
	config: TailwindConfig,
	/// The CLI manager for binary resolution.
	cli: TailwindCli,
}

impl TailwindBuilder {
	/// Create a new `TailwindBuilder` with the given configuration.
	pub fn new(config: TailwindConfig) -> Self {
		let cli = TailwindCli::new(config.version(), config.cache_dir());
		Self { config, cli }
	}

	/// Execute the Tailwind CSS build pipeline.
	///
	/// This method:
	/// 1. Ensures the Tailwind CLI binary is available (downloading if needed)
	/// 2. Scans content paths for CSS class names
	/// 3. Writes a temporary content file for Tailwind to process
	/// 4. Runs the Tailwind CLI to generate the output CSS
	///
	/// ## Errors
	///
	/// Returns `TailwindError` if any step in the pipeline fails.
	pub fn build(&self) -> TailwindResult<()> {
		if self.config.content_paths().is_empty() {
			return Err(TailwindError::NoContentPaths);
		}

		let cli_path = self.cli.ensure_binary()?;

		// Scan for classes to create an input CSS with @source directive
		let scanner = ClassScanner::new(self.config.content_paths());
		let source_files = scanner.collect_source_files()?;

		if source_files.is_empty() {
			eprintln!("warning: no source files found matching content paths");
		}

		// Build the Tailwind CLI command
		let output_path = self.config.output_path();
		if let Some(parent) = Path::new(output_path).parent() {
			std::fs::create_dir_all(parent)?;
		}

		let mut cmd = Command::new(&cli_path);

		// Tailwind v4 uses --input and --output
		if let Some(input) = self.config.input_css() {
			cmd.arg("--input").arg(input);
		}

		cmd.arg("--output").arg(output_path);

		if self.config.minify() {
			cmd.arg("--minify");
		}

		// Add content glob patterns via --content (Tailwind v4 supports this)
		for content_path in self.config.content_paths() {
			cmd.arg("--content").arg(content_path);
		}

		let output = cmd.output()?;

		if !output.status.success() {
			return Err(TailwindError::CliFailed {
				status: output.status.code().unwrap_or(-1),
				stderr: String::from_utf8_lossy(&output.stderr).to_string(),
			});
		}

		Ok(())
	}

	/// Execute the Tailwind CSS build in watch mode for development.
	///
	/// This runs the Tailwind CLI with `--watch` to continuously rebuild
	/// CSS as source files change.
	///
	/// ## Errors
	///
	/// Returns `TailwindError` if the CLI cannot be started.
	pub fn watch(&self) -> TailwindResult<std::process::Child> {
		if self.config.content_paths().is_empty() {
			return Err(TailwindError::NoContentPaths);
		}

		let cli_path = self.cli.ensure_binary()?;
		let output_path = self.config.output_path();

		if let Some(parent) = Path::new(output_path).parent() {
			std::fs::create_dir_all(parent)?;
		}

		let mut cmd = Command::new(&cli_path);

		if let Some(input) = self.config.input_css() {
			cmd.arg("--input").arg(input);
		}

		cmd.arg("--output").arg(output_path);

		if self.config.minify() {
			cmd.arg("--minify");
		}

		for content_path in self.config.content_paths() {
			cmd.arg("--content").arg(content_path);
		}

		cmd.arg("--watch");

		let child = cmd.spawn()?;
		Ok(child)
	}

	/// Returns a reference to the inner configuration.
	pub fn config(&self) -> &TailwindConfig {
		&self.config
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_builder_rejects_empty_content_paths() {
		// Arrange
		let config = TailwindConfig::new()
			.with_version("4.1.0")
			.with_output_path("out.css".to_string());

		let builder = TailwindBuilder::new(config);

		// Act
		let result = builder.build();

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), TailwindError::NoContentPaths));
	}
}
