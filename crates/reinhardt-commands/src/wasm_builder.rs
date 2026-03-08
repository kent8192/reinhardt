//! WASM build utilities for reinhardt-pages projects.
//!
//! This module provides the core functionality for building WASM frontends
//! without relying on Trunk. It uses wasm-bindgen-cli directly for maximum
//! control over the build process.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Configuration for WASM builds.
#[derive(Debug, Clone)]
pub struct WasmBuildConfig {
	/// The project directory containing Cargo.toml
	pub project_dir: PathBuf,
	/// Output directory for generated files (default: "dist")
	pub output_dir: PathBuf,
	/// Build in release mode
	pub release: bool,
	/// Enable wasm-opt optimization (release only)
	pub optimize: bool,
	/// Target name (crate name, used for output file naming)
	pub target_name: Option<String>,
}

impl Default for WasmBuildConfig {
	fn default() -> Self {
		Self {
			project_dir: PathBuf::from("."),
			output_dir: PathBuf::from("dist"),
			release: false,
			optimize: true,
			target_name: None,
		}
	}
}

impl WasmBuildConfig {
	/// Create a new configuration with the given project directory.
	pub fn new(project_dir: impl Into<PathBuf>) -> Self {
		Self {
			project_dir: project_dir.into(),
			..Default::default()
		}
	}

	/// Set the output directory.
	pub fn output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
		self.output_dir = dir.into();
		self
	}

	/// Set release mode.
	pub fn release(mut self, release: bool) -> Self {
		self.release = release;
		self
	}

	/// Set wasm-opt optimization (only applies to release builds).
	pub fn optimize(mut self, optimize: bool) -> Self {
		self.optimize = optimize;
		self
	}

	/// Set the target name explicitly.
	pub fn target_name(mut self, name: impl Into<String>) -> Self {
		self.target_name = Some(name.into());
		self
	}
}

/// Result of a successful WASM build.
#[derive(Debug)]
pub struct WasmBuildOutput {
	/// Path to the generated .wasm file
	pub wasm_file: PathBuf,
	/// Path to the generated .js glue file
	pub js_file: PathBuf,
	/// Path to the TypeScript definitions (if generated)
	pub ts_file: Option<PathBuf>,
	/// Output directory
	pub output_dir: PathBuf,
}

/// Error types for WASM build operations.
#[derive(Debug, thiserror::Error)]
pub enum WasmBuildError {
	/// The `wasm32-unknown-unknown` rustup target is not installed.
	#[error(
		"wasm32-unknown-unknown target not installed. Run: rustup target add wasm32-unknown-unknown"
	)]
	TargetNotInstalled,
	/// The `wasm-bindgen-cli` tool is not installed.
	#[error("wasm-bindgen-cli not installed. Run: cargo install wasm-bindgen-cli")]
	WasmBindgenNotInstalled,
	/// The cargo build step failed.
	#[error("Cargo build failed: {0}")]
	CargoBuildFailed(String),
	/// The wasm-bindgen post-processing step failed.
	#[error("wasm-bindgen failed: {0}")]
	WasmBindgenFailed(String),
	/// The wasm-opt optimization step failed.
	#[error("wasm-opt failed: {0}")]
	WasmOptFailed(String),
	/// Could not determine the crate name from `Cargo.toml`.
	#[error("Failed to determine crate name from Cargo.toml")]
	CrateNameNotFound,
	/// An I/O error occurred during WASM build operations.
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
}

/// WASM build executor.
pub struct WasmBuilder {
	config: WasmBuildConfig,
}

impl WasmBuilder {
	/// Create a new builder with the given configuration.
	pub fn new(config: WasmBuildConfig) -> Self {
		Self { config }
	}

	/// Build the WASM target.
	pub fn build(&self) -> Result<WasmBuildOutput, WasmBuildError> {
		// Ensure wasm32 target is installed
		self.check_target_installed()?;

		// Ensure wasm-bindgen is installed
		self.check_wasm_bindgen_installed()?;

		// Get crate name
		let crate_name = self.get_crate_name()?;
		println!("Building WASM for crate: {}", crate_name);

		// Create output directory
		let output_dir = self.config.project_dir.join(&self.config.output_dir);
		std::fs::create_dir_all(&output_dir)?;

		// Run cargo build
		println!("Running cargo build --target wasm32-unknown-unknown...");
		self.run_cargo_build()?;

		// Determine the WASM file location
		let profile = if self.config.release {
			"release"
		} else {
			"debug"
		};
		let wasm_path = self
			.config
			.project_dir
			.join("target")
			.join("wasm32-unknown-unknown")
			.join(profile)
			.join(format!("{}.wasm", crate_name.replace('-', "_")));

		// Run wasm-bindgen
		println!("Running wasm-bindgen...");
		self.run_wasm_bindgen(&wasm_path, &output_dir)?;

		// Run wasm-opt for release builds
		if self.config.release && self.config.optimize {
			if self.is_wasm_opt_available() {
				println!("Running wasm-opt...");
				self.run_wasm_opt(&output_dir, &crate_name)?;
			} else {
				eprintln!(
					"Warning: wasm-opt not found, skipping optimization. Install with: brew install binaryen"
				);
			}
		}

		let wasm_file = output_dir.join(format!("{}_bg.wasm", crate_name.replace('-', "_")));
		let js_file = output_dir.join(format!("{}.js", crate_name.replace('-', "_")));
		let ts_file = output_dir.join(format!("{}.d.ts", crate_name.replace('-', "_")));

		Ok(WasmBuildOutput {
			wasm_file,
			js_file,
			ts_file: if ts_file.exists() {
				Some(ts_file)
			} else {
				None
			},
			output_dir,
		})
	}

	fn check_target_installed(&self) -> Result<(), WasmBuildError> {
		let output = Command::new("rustup")
			.args(["target", "list", "--installed"])
			.output()?;

		let stdout = String::from_utf8_lossy(&output.stdout);
		if stdout.contains("wasm32-unknown-unknown") {
			Ok(())
		} else {
			eprintln!("Warning: wasm32-unknown-unknown target not installed");
			Err(WasmBuildError::TargetNotInstalled)
		}
	}

	fn check_wasm_bindgen_installed(&self) -> Result<(), WasmBuildError> {
		let result = Command::new("wasm-bindgen").arg("--version").output();

		match result {
			Ok(output) if output.status.success() => Ok(()),
			_ => {
				eprintln!("Warning: wasm-bindgen-cli not installed");
				Err(WasmBuildError::WasmBindgenNotInstalled)
			}
		}
	}

	fn is_wasm_opt_available(&self) -> bool {
		Command::new("wasm-opt")
			.arg("--version")
			.output()
			.map(|o| o.status.success())
			.unwrap_or(false)
	}

	fn get_crate_name(&self) -> Result<String, WasmBuildError> {
		if let Some(name) = &self.config.target_name {
			return Ok(name.clone());
		}

		// Read Cargo.toml to get crate name
		let cargo_toml_path = self.config.project_dir.join("Cargo.toml");
		let content = std::fs::read_to_string(&cargo_toml_path)?;

		// Parse [package] name = "..."
		for line in content.lines() {
			let line = line.trim();
			if line.starts_with("name")
				&& line.contains('=')
				&& let Some(name) = line.split('=').nth(1)
			{
				let name = name.trim().trim_matches('"').trim_matches('\'');
				return Ok(name.to_string());
			}
		}

		Err(WasmBuildError::CrateNameNotFound)
	}

	fn run_cargo_build(&self) -> Result<(), WasmBuildError> {
		let mut cmd = Command::new("cargo");
		cmd.arg("build")
			.arg("--target")
			.arg("wasm32-unknown-unknown")
			.current_dir(&self.config.project_dir);

		if self.config.release {
			cmd.arg("--release");
		}

		let output = cmd.output()?;

		if output.status.success() {
			Ok(())
		} else {
			let stderr = String::from_utf8_lossy(&output.stderr);
			Err(WasmBuildError::CargoBuildFailed(stderr.to_string()))
		}
	}

	fn run_wasm_bindgen(&self, wasm_path: &Path, output_dir: &Path) -> Result<(), WasmBuildError> {
		let output = Command::new("wasm-bindgen")
			.arg("--target")
			.arg("web")
			.arg("--out-dir")
			.arg(output_dir)
			.arg(wasm_path)
			.output()?;

		if output.status.success() {
			Ok(())
		} else {
			let stderr = String::from_utf8_lossy(&output.stderr);
			Err(WasmBuildError::WasmBindgenFailed(stderr.to_string()))
		}
	}

	fn run_wasm_opt(&self, output_dir: &Path, crate_name: &str) -> Result<(), WasmBuildError> {
		let wasm_file = output_dir.join(format!("{}_bg.wasm", crate_name.replace('-', "_")));

		if !wasm_file.exists() {
			return Ok(());
		}

		// Create temp file for optimization
		let temp_file = output_dir.join(format!("{}_bg_opt.wasm", crate_name.replace('-', "_")));

		let output = Command::new("wasm-opt")
			.arg("-O3")
			.arg("--output")
			.arg(&temp_file)
			.arg(&wasm_file)
			.output()?;

		if output.status.success() {
			// Replace original with optimized version
			std::fs::rename(&temp_file, &wasm_file)?;
			Ok(())
		} else {
			// Clean up temp file on failure
			let _ = std::fs::remove_file(&temp_file);
			let stderr = String::from_utf8_lossy(&output.stderr);
			Err(WasmBuildError::WasmOptFailed(stderr.to_string()))
		}
	}
}

/// Check if all WASM build tools are installed.
pub fn check_wasm_tools_installed() -> Result<(), Vec<String>> {
	let mut missing = Vec::new();

	// Check wasm32 target
	if let Ok(output) = Command::new("rustup")
		.args(["target", "list", "--installed"])
		.output()
	{
		let stdout = String::from_utf8_lossy(&output.stdout);
		if !stdout.contains("wasm32-unknown-unknown") {
			missing.push(
				"wasm32-unknown-unknown target (run: rustup target add wasm32-unknown-unknown)"
					.to_string(),
			);
		}
	}

	// Check wasm-bindgen
	if Command::new("wasm-bindgen")
		.arg("--version")
		.output()
		.map(|o| !o.status.success())
		.unwrap_or(true)
	{
		missing.push("wasm-bindgen-cli (run: cargo install wasm-bindgen-cli)".to_string());
	}

	if missing.is_empty() {
		Ok(())
	} else {
		Err(missing)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_config_defaults() {
		let config = WasmBuildConfig::default();
		assert_eq!(config.output_dir, PathBuf::from("dist"));
		assert!(!config.release);
		assert!(config.optimize);
	}

	#[test]
	fn test_config_builder() {
		let config = WasmBuildConfig::new("/path/to/project")
			.output_dir("build")
			.release(true)
			.optimize(false)
			.target_name("my-app");

		assert_eq!(config.project_dir, PathBuf::from("/path/to/project"));
		assert_eq!(config.output_dir, PathBuf::from("build"));
		assert!(config.release);
		assert!(!config.optimize);
		assert_eq!(config.target_name, Some("my-app".to_string()));
	}
}
