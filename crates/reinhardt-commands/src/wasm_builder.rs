//! WASM build utilities for reinhardt-pages projects.
//!
//! This module provides the core functionality for building WASM frontends
//! without relying on Trunk. It uses wasm-bindgen-cli directly for maximum
//! control over the build process.

use crate::process::{ProcessRequest, ProcessRunner, SystemProcessRunner};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
	/// Override for the cargo target directory. When `None`, falls back to
	/// `project_dir/target`. In workspace setups, this should point to the
	/// workspace root's target directory.
	pub target_dir: Option<PathBuf>,
}

impl Default for WasmBuildConfig {
	fn default() -> Self {
		Self {
			project_dir: PathBuf::from("."),
			output_dir: PathBuf::from("dist"),
			release: false,
			optimize: true,
			target_name: None,
			target_dir: None,
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

	/// Set the cargo target directory explicitly.
	///
	/// When building inside a Cargo workspace, the target directory is at
	/// the workspace root, not relative to the member crate. Use this to
	/// point wasm-bindgen at the correct artifact location.
	pub fn target_dir(mut self, dir: impl Into<PathBuf>) -> Self {
		self.target_dir = Some(dir.into());
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

	/// Detect the cargo target directory by running `cargo metadata`.
	///
	/// In workspace setups, Cargo places build artifacts in the workspace root's
	/// `target/` directory, not the individual crate's directory. This method
	/// queries `cargo metadata` for the canonical `target_directory` path, which
	/// respects `CARGO_TARGET_DIR`, `.cargo/config.toml`, and workspace layout.
	///
	/// Falls back to `project_dir/target` if `cargo metadata` is unavailable.
	fn detect_target_dir_with_runner<R: ProcessRunner>(&self, runner: &R) -> PathBuf {
		let output = runner.run(
			&ProcessRequest::new("cargo")
				.args(["metadata", "--no-deps", "--format-version=1"])
				.current_dir(&self.config.project_dir),
		);

		if let Ok(output) = output
			&& output.success
			&& let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
			&& let Some(target_dir) = json.get("target_directory").and_then(|v| v.as_str())
		{
			return PathBuf::from(target_dir);
		}

		self.config.project_dir.join("target")
	}

	/// Build the WASM target.
	pub fn build(&self) -> Result<WasmBuildOutput, WasmBuildError> {
		self.build_with_runner(&SystemProcessRunner)
	}

	fn build_with_runner<R: ProcessRunner>(
		&self,
		runner: &R,
	) -> Result<WasmBuildOutput, WasmBuildError> {
		let target_base = self
			.config
			.target_dir
			.as_ref()
			.cloned()
			.unwrap_or_else(|| self.detect_target_dir_with_runner(runner));

		// Ensure wasm32 target is installed
		self.check_target_installed_with_runner(runner)?;

		// Ensure wasm-bindgen is installed
		self.check_wasm_bindgen_installed_with_runner(runner)?;

		// Get crate name
		let crate_name = self.get_crate_name()?;
		println!("Building WASM for crate: {}", crate_name);

		// Create output directory
		let output_dir = self.config.project_dir.join(&self.config.output_dir);
		std::fs::create_dir_all(&output_dir)?;

		// Run cargo build
		println!("Running cargo build --target wasm32-unknown-unknown...");
		self.run_cargo_build_with_runner(runner)?;

		// Determine the WASM file location
		let profile = if self.config.release {
			"release"
		} else {
			"debug"
		};
		let wasm_path = target_base
			.join("wasm32-unknown-unknown")
			.join(profile)
			.join(format!("{}.wasm", crate_name.replace('-', "_")));

		// Run wasm-bindgen
		println!("Running wasm-bindgen...");
		self.run_wasm_bindgen_with_runner(&wasm_path, &output_dir, runner)?;

		// Run wasm-opt for release builds
		if self.config.release && self.config.optimize {
			if self.is_wasm_opt_available_with_runner(runner) {
				println!("Running wasm-opt...");
				self.run_wasm_opt_with_runner(&output_dir, &crate_name, runner)?;
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

	fn check_target_installed_with_runner<R: ProcessRunner>(
		&self,
		runner: &R,
	) -> Result<(), WasmBuildError> {
		let output =
			runner.run(&ProcessRequest::new("rustup").args(["target", "list", "--installed"]))?;

		let stdout = String::from_utf8_lossy(&output.stdout);
		if stdout.contains("wasm32-unknown-unknown") {
			Ok(())
		} else {
			eprintln!("Warning: wasm32-unknown-unknown target not installed");
			Err(WasmBuildError::TargetNotInstalled)
		}
	}

	fn check_wasm_bindgen_installed_with_runner<R: ProcessRunner>(
		&self,
		runner: &R,
	) -> Result<(), WasmBuildError> {
		let result = runner.run(&ProcessRequest::new("wasm-bindgen").arg("--version"));

		match result {
			Ok(output) if output.success => Ok(()),
			_ => {
				eprintln!("Warning: wasm-bindgen-cli not installed");
				Err(WasmBuildError::WasmBindgenNotInstalled)
			}
		}
	}

	fn is_wasm_opt_available_with_runner<R: ProcessRunner>(&self, runner: &R) -> bool {
		runner
			.run(&ProcessRequest::new("wasm-opt").arg("--version"))
			.map(|outcome| outcome.success)
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

	fn run_cargo_build_with_runner<R: ProcessRunner>(
		&self,
		runner: &R,
	) -> Result<(), WasmBuildError> {
		let mut request = ProcessRequest::new("cargo")
			.args(["build", "--lib", "--target", "wasm32-unknown-unknown"])
			.current_dir(&self.config.project_dir);
		if self.config.release {
			request = request.arg("--release");
		}

		let output = runner.run(&request)?;

		if output.success {
			Ok(())
		} else {
			let stderr = String::from_utf8_lossy(&output.stderr);
			Err(WasmBuildError::CargoBuildFailed(stderr.to_string()))
		}
	}

	fn run_wasm_bindgen_with_runner<R: ProcessRunner>(
		&self,
		wasm_path: &Path,
		output_dir: &Path,
		runner: &R,
	) -> Result<(), WasmBuildError> {
		let output = runner.run(
			&ProcessRequest::new("wasm-bindgen")
				.args(["--target", "web", "--out-dir"])
				.arg(output_dir.to_path_buf())
				.arg(wasm_path.to_path_buf()),
		)?;

		if output.success {
			Ok(())
		} else {
			let stderr = String::from_utf8_lossy(&output.stderr);
			Err(WasmBuildError::WasmBindgenFailed(stderr.to_string()))
		}
	}

	fn run_wasm_opt_with_runner<R: ProcessRunner>(
		&self,
		output_dir: &Path,
		crate_name: &str,
		runner: &R,
	) -> Result<(), WasmBuildError> {
		let wasm_file = output_dir.join(format!("{}_bg.wasm", crate_name.replace('-', "_")));

		if !wasm_file.exists() {
			return Ok(());
		}

		// Create temp file for optimization
		let temp_file = output_dir.join(format!("{}_bg_opt.wasm", crate_name.replace('-', "_")));

		let output = runner.run(
			&ProcessRequest::new("wasm-opt")
				.args(["-O3", "--output"])
				.arg(temp_file.clone())
				.arg(wasm_file.clone()),
		)?;

		if output.success {
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
	check_wasm_tools_installed_with_runner(&SystemProcessRunner)
}

fn check_wasm_tools_installed_with_runner<R: ProcessRunner>(runner: &R) -> Result<(), Vec<String>> {
	let mut missing = Vec::new();

	// Check wasm32 target
	if let Ok(output) =
		runner.run(&ProcessRequest::new("rustup").args(["target", "list", "--installed"]))
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
	if runner
		.run(&ProcessRequest::new("wasm-bindgen").arg("--version"))
		.map(|outcome| !outcome.success)
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

/// Check if a Cargo.toml content string declares `cdylib` in `crate-type`.
///
/// Parses the `[lib]` section looking for a `crate-type` array that
/// includes `"cdylib"`. Returns `false` if no `[lib]` section or no
/// `crate-type` key is found.
pub fn detect_cdylib_in_cargo_toml_content(content: &str) -> bool {
	let mut in_lib_section = false;
	for line in content.lines() {
		let trimmed = line.trim();
		if trimmed.starts_with('[') {
			in_lib_section = trimmed == "[lib]";
			continue;
		}
		if in_lib_section && trimmed.starts_with("crate-type") && trimmed.contains("cdylib") {
			return true;
		}
	}
	false
}

/// Check if the Cargo.toml at the given path declares `cdylib` in `crate-type`.
pub fn detect_cdylib_in_cargo_toml(path: &Path) -> bool {
	std::fs::read_to_string(path)
		.map(|content| detect_cdylib_in_cargo_toml_content(&content))
		.unwrap_or(false)
}

/// Returns the most recent modification time among the WASM crate's tracked
/// source files: every `.rs` under `<crate_dir>/src/` (recursive) and
/// `<crate_dir>/Cargo.toml`.
///
/// Returns `None` if neither `src/` nor `Cargo.toml` is readable.
pub fn latest_source_mtime(crate_dir: &Path) -> Option<SystemTime> {
	let mut latest: Option<SystemTime> = None;
	let mut update = |t: SystemTime| {
		latest = Some(latest.map_or(t, |l| l.max(t)));
	};

	if let Ok(meta) = std::fs::metadata(crate_dir.join("Cargo.toml"))
		&& let Ok(mtime) = meta.modified()
	{
		update(mtime);
	}

	let src = crate_dir.join("src");
	let mut stack = vec![src];
	while let Some(dir) = stack.pop() {
		let entries = match std::fs::read_dir(&dir) {
			Ok(e) => e,
			Err(_) => continue,
		};
		for entry in entries.flatten() {
			let path = entry.path();
			let file_type = match entry.file_type() {
				Ok(t) => t,
				Err(_) => continue,
			};
			if file_type.is_dir() {
				stack.push(path);
			} else if file_type.is_file()
				&& path.extension().and_then(|e| e.to_str()) == Some("rs")
				&& let Ok(meta) = entry.metadata()
				&& let Ok(mtime) = meta.modified()
			{
				update(mtime);
			}
		}
	}

	latest
}

/// Returns `true` if the WASM bundle at `artifact` is missing or older than
/// any tracked source file under `crate_dir` (see [`latest_source_mtime`]).
///
/// On any failure to read metadata, the function returns `true` (rebuild)
/// to fail safely toward freshness rather than serving a potentially stale
/// bundle.
pub fn is_wasm_stale(crate_dir: &Path, artifact: &Path) -> bool {
	let Ok(artifact_meta) = std::fs::metadata(artifact) else {
		return true;
	};
	let Ok(artifact_mtime) = artifact_meta.modified() else {
		return true;
	};
	match latest_source_mtime(crate_dir) {
		Some(src_mtime) => src_mtime > artifact_mtime,
		None => true,
	}
}

#[cfg(test)]
mod tests {
	use crate::process::{FakeProcessRunner, ProcessOutcome};
	use std::ffi::OsString;
	use std::fs;

	use super::*;

	fn make_wasm_crate() -> tempfile::TempDir {
		let temp = tempfile::tempdir().expect("create temporary crate");
		fs::create_dir(temp.path().join("src")).expect("create source directory");
		fs::write(
			temp.path().join("Cargo.toml"),
			b"[package]\nname = \"demo-app\"\nversion = \"0.1.0\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n",
		)
		.expect("write Cargo.toml");
		temp
	}

	fn metadata_outcome(target_dir: &Path) -> ProcessOutcome {
		ProcessOutcome::success(
			format!(r#"{{"target_directory":"{}"}}"#, target_dir.display()).into_bytes(),
		)
	}

	fn successful_build_outcomes(target_dir: &Path) -> [std::io::Result<ProcessOutcome>; 5] {
		[
			Ok(metadata_outcome(target_dir)),
			Ok(ProcessOutcome::success(
				b"wasm32-unknown-unknown\n".to_vec(),
			)),
			Ok(ProcessOutcome::success(b"wasm-bindgen 0.2".to_vec())),
			Ok(ProcessOutcome::success(Vec::new())),
			Ok(ProcessOutcome::success(Vec::new())),
		]
	}

	fn request_args(requests: &[crate::process::ProcessRequest]) -> Vec<Vec<OsString>> {
		requests
			.iter()
			.map(|request| request.args.clone())
			.collect()
	}

	#[test]
	fn build_with_runner_runs_wasm_pipeline_in_order() {
		let project = make_wasm_crate();
		let target_dir = project.path().join("workspace-target");
		let runner = FakeProcessRunner::new(successful_build_outcomes(&target_dir));
		let builder = WasmBuilder::new(WasmBuildConfig::new(project.path()));

		builder
			.build_with_runner(&runner)
			.expect("scripted build succeeds");

		let requests = runner.requests();
		assert_eq!(
			requests
				.iter()
				.map(|request| request.program.clone())
				.collect::<Vec<_>>(),
			vec!["cargo", "rustup", "wasm-bindgen", "cargo", "wasm-bindgen"]
		);
		assert_eq!(
			request_args(&requests),
			vec![
				vec![
					"metadata".into(),
					"--no-deps".into(),
					"--format-version=1".into()
				],
				vec!["target".into(), "list".into(), "--installed".into()],
				vec!["--version".into()],
				vec![
					"build".into(),
					"--lib".into(),
					"--target".into(),
					"wasm32-unknown-unknown".into(),
				],
				vec![
					"--target".into(),
					"web".into(),
					"--out-dir".into(),
					project.path().join("dist").into_os_string(),
					target_dir
						.join("wasm32-unknown-unknown/debug/demo_app.wasm")
						.into_os_string(),
				],
			]
		);
		assert_eq!(requests[0].current_dir.as_deref(), Some(project.path()));
		assert_eq!(requests[3].current_dir.as_deref(), Some(project.path()));
	}

	#[test]
	fn build_with_runner_reports_missing_wasm_target() {
		let project = make_wasm_crate();
		let target_dir = project.path().join("workspace-target");
		let runner = FakeProcessRunner::new([
			Ok(metadata_outcome(&target_dir)),
			Ok(ProcessOutcome::success(b"x86_64-apple-darwin\n".to_vec())),
		]);
		let builder = WasmBuilder::new(WasmBuildConfig::new(project.path()));

		let error = builder
			.build_with_runner(&runner)
			.expect_err("missing target must stop the build");

		assert!(matches!(error, WasmBuildError::TargetNotInstalled));
		assert_eq!(runner.requests().len(), 2);
	}

	#[test]
	fn build_with_runner_propagates_cargo_stderr() {
		let project = make_wasm_crate();
		let target_dir = project.path().join("workspace-target");
		let runner = FakeProcessRunner::new([
			Ok(metadata_outcome(&target_dir)),
			Ok(ProcessOutcome::success(
				b"wasm32-unknown-unknown\n".to_vec(),
			)),
			Ok(ProcessOutcome::success(b"wasm-bindgen 0.2".to_vec())),
			Ok(ProcessOutcome::failure(
				"exit status: 1",
				b"cargo diagnostics".to_vec(),
			)),
		]);
		let builder = WasmBuilder::new(WasmBuildConfig::new(project.path()));

		let error = builder
			.build_with_runner(&runner)
			.expect_err("cargo failure must propagate stderr");

		assert_eq!(error.to_string(), "Cargo build failed: cargo diagnostics");
	}

	#[test]
	fn build_with_runner_propagates_wasm_bindgen_stderr() {
		let project = make_wasm_crate();
		let target_dir = project.path().join("workspace-target");
		let runner = FakeProcessRunner::new([
			Ok(metadata_outcome(&target_dir)),
			Ok(ProcessOutcome::success(
				b"wasm32-unknown-unknown\n".to_vec(),
			)),
			Ok(ProcessOutcome::success(b"wasm-bindgen 0.2".to_vec())),
			Ok(ProcessOutcome::success(Vec::new())),
			Ok(ProcessOutcome::failure(
				"exit status: 1",
				b"bindgen diagnostics".to_vec(),
			)),
		]);
		let builder = WasmBuilder::new(WasmBuildConfig::new(project.path()));

		let error = builder
			.build_with_runner(&runner)
			.expect_err("wasm-bindgen failure must propagate stderr");

		assert_eq!(
			error.to_string(),
			"wasm-bindgen failed: bindgen diagnostics"
		);
	}

	#[test]
	fn release_build_runs_wasm_opt_when_available() {
		let project = make_wasm_crate();
		let target_dir = project.path().join("workspace-target");
		let output_dir = project.path().join("dist");
		fs::create_dir(&output_dir).expect("create output directory");
		fs::write(output_dir.join("demo_app_bg.wasm"), b"wasm").expect("write WASM output");
		fs::write(output_dir.join("demo_app_bg_opt.wasm"), b"optimized wasm")
			.expect("write optimized WASM output");
		let runner = FakeProcessRunner::new([
			Ok(metadata_outcome(&target_dir)),
			Ok(ProcessOutcome::success(
				b"wasm32-unknown-unknown\n".to_vec(),
			)),
			Ok(ProcessOutcome::success(b"wasm-bindgen 0.2".to_vec())),
			Ok(ProcessOutcome::success(Vec::new())),
			Ok(ProcessOutcome::success(Vec::new())),
			Ok(ProcessOutcome::success(b"wasm-opt 1.0".to_vec())),
			Ok(ProcessOutcome::success(Vec::new())),
		]);
		let builder = WasmBuilder::new(WasmBuildConfig::new(project.path()).release(true));

		builder
			.build_with_runner(&runner)
			.expect("release build succeeds with wasm-opt");

		let requests = runner.requests();
		assert_eq!(requests[5].program, "wasm-opt");
		assert_eq!(requests[5].args, vec![OsString::from("--version")]);
		assert_eq!(
			requests[6].args,
			vec![
				OsString::from("-O3"),
				OsString::from("--output"),
				output_dir.join("demo_app_bg_opt.wasm").into_os_string(),
				output_dir.join("demo_app_bg.wasm").into_os_string(),
			]
		);
	}

	#[test]
	fn release_build_skips_wasm_opt_when_unavailable() {
		let project = make_wasm_crate();
		let target_dir = project.path().join("workspace-target");
		let runner = FakeProcessRunner::new([
			Ok(metadata_outcome(&target_dir)),
			Ok(ProcessOutcome::success(
				b"wasm32-unknown-unknown\n".to_vec(),
			)),
			Ok(ProcessOutcome::success(b"wasm-bindgen 0.2".to_vec())),
			Ok(ProcessOutcome::success(Vec::new())),
			Ok(ProcessOutcome::success(Vec::new())),
			Ok(ProcessOutcome::failure("exit status: 1", Vec::new())),
		]);
		let builder = WasmBuilder::new(WasmBuildConfig::new(project.path()).release(true));

		builder
			.build_with_runner(&runner)
			.expect("release build skips unavailable wasm-opt");

		assert_eq!(runner.requests().len(), 6);
	}

	#[test]
	fn check_tools_reports_target_and_bindgen_independently() {
		let missing_bindgen = FakeProcessRunner::new([
			Ok(ProcessOutcome::success(
				b"wasm32-unknown-unknown\n".to_vec(),
			)),
			Ok(ProcessOutcome::failure("exit status: 1", Vec::new())),
		]);
		let missing_target = FakeProcessRunner::new([
			Ok(ProcessOutcome::success(b"x86_64-apple-darwin\n".to_vec())),
			Ok(ProcessOutcome::success(b"wasm-bindgen 0.2".to_vec())),
		]);

		assert_eq!(
			check_wasm_tools_installed_with_runner(&missing_bindgen),
			Err(vec![
				"wasm-bindgen-cli (run: cargo install wasm-bindgen-cli)".to_string()
			])
		);
		assert_eq!(
			check_wasm_tools_installed_with_runner(&missing_target),
			Err(vec![
				"wasm32-unknown-unknown target (run: rustup target add wasm32-unknown-unknown)"
					.to_string()
			])
		);
	}

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

	#[test]
	fn test_detect_cdylib_present() {
		let content = r#"
[lib]
crate-type = ["cdylib", "rlib"]
"#;
		assert!(detect_cdylib_in_cargo_toml_content(content));
	}

	#[test]
	fn test_detect_cdylib_absent() {
		let content = r#"
[lib]
name = "my_lib"
"#;
		assert!(!detect_cdylib_in_cargo_toml_content(content));
	}

	#[test]
	fn test_detect_cdylib_only_rlib() {
		let content = r#"
[lib]
crate-type = ["rlib"]
"#;
		assert!(!detect_cdylib_in_cargo_toml_content(content));
	}

	#[test]
	fn test_detect_cdylib_no_lib_section() {
		let content = r#"
[package]
name = "my-app"
version = "0.1.0"
"#;
		assert!(!detect_cdylib_in_cargo_toml_content(content));
	}

	mod staleness {
		use super::*;
		use std::fs::{self, File};
		use std::time::Duration;

		// Build a minimal cdylib-like crate layout under `dir`:
		//   <dir>/Cargo.toml
		//   <dir>/src/lib.rs
		//   <dir>/src/nested/mod_a.rs
		// Returns the crate directory.
		fn make_crate(dir: &Path) -> PathBuf {
			fs::create_dir_all(dir.join("src/nested")).unwrap();
			fs::write(dir.join("Cargo.toml"), b"[package]\nname=\"x\"\n").unwrap();
			fs::write(dir.join("src/lib.rs"), b"// lib").unwrap();
			fs::write(dir.join("src/nested/mod_a.rs"), b"// nested").unwrap();
			dir.to_path_buf()
		}

		fn set_mtime(path: &Path, t: SystemTime) {
			let f = File::options().write(true).open(path).unwrap();
			f.set_modified(t).unwrap();
		}

		#[test]
		fn returns_true_when_artifact_missing() {
			let tmp = tempfile::tempdir().unwrap();
			let crate_dir = make_crate(tmp.path());
			let artifact = crate_dir.join("dist/x_bg.wasm");
			// Arrange: artifact does not exist.
			// Act + Assert
			assert!(is_wasm_stale(&crate_dir, &artifact));
		}

		#[test]
		fn returns_false_when_artifact_newer_than_sources() {
			let tmp = tempfile::tempdir().unwrap();
			let crate_dir = make_crate(tmp.path());
			let dist = crate_dir.join("dist");
			fs::create_dir_all(&dist).unwrap();
			let artifact = dist.join("x_bg.wasm");
			fs::write(&artifact, b"\0asm").unwrap();

			let base = SystemTime::now() - Duration::from_secs(120);
			set_mtime(&crate_dir.join("Cargo.toml"), base);
			set_mtime(&crate_dir.join("src/lib.rs"), base);
			set_mtime(&crate_dir.join("src/nested/mod_a.rs"), base);
			set_mtime(&artifact, base + Duration::from_secs(60));

			assert!(!is_wasm_stale(&crate_dir, &artifact));
		}

		#[test]
		fn returns_true_when_source_newer_than_artifact() {
			let tmp = tempfile::tempdir().unwrap();
			let crate_dir = make_crate(tmp.path());
			let dist = crate_dir.join("dist");
			fs::create_dir_all(&dist).unwrap();
			let artifact = dist.join("x_bg.wasm");
			fs::write(&artifact, b"\0asm").unwrap();

			let base = SystemTime::now() - Duration::from_secs(120);
			set_mtime(&crate_dir.join("Cargo.toml"), base);
			set_mtime(&crate_dir.join("src/lib.rs"), base);
			set_mtime(&artifact, base);
			// One nested source is newer than the artifact.
			set_mtime(
				&crate_dir.join("src/nested/mod_a.rs"),
				base + Duration::from_secs(60),
			);

			assert!(is_wasm_stale(&crate_dir, &artifact));
		}

		#[test]
		fn treats_cargo_toml_changes_as_stale() {
			let tmp = tempfile::tempdir().unwrap();
			let crate_dir = make_crate(tmp.path());
			let dist = crate_dir.join("dist");
			fs::create_dir_all(&dist).unwrap();
			let artifact = dist.join("x_bg.wasm");
			fs::write(&artifact, b"\0asm").unwrap();

			let base = SystemTime::now() - Duration::from_secs(120);
			set_mtime(&crate_dir.join("src/lib.rs"), base);
			set_mtime(&crate_dir.join("src/nested/mod_a.rs"), base);
			set_mtime(&artifact, base);
			// Cargo.toml updated after artifact (e.g. dependency bump).
			set_mtime(
				&crate_dir.join("Cargo.toml"),
				base + Duration::from_secs(60),
			);

			assert!(is_wasm_stale(&crate_dir, &artifact));
		}
	}
}
