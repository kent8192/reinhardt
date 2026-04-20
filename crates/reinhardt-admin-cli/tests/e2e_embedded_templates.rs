//! End-to-end tests verifying that templates are embedded in the binary and are
//! correctly rendered when `reinhardt-admin startproject` / `startapp` is invoked.
//!
//! These tests spawn the actual compiled binary (not in-process Rust function calls),
//! so they prove:
//!   1. The binary is self-contained — no external template files are needed at runtime.
//!   2. All Tera template variables (`{{ variable }}`) are substituted in every generated file.
//!   3. The `.example.` dual-output mechanism works correctly (real values vs. placeholder values).
//!   4. Generated project structure is correct.

use rstest::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use walkdir::WalkDir;

// `CARGO_BIN_EXE_reinhardt-admin` is set by Cargo at test-compilation time to the
// absolute path of the compiled binary, so no manual `cargo build` is required.
const REINHARDT_ADMIN: &str = env!("CARGO_BIN_EXE_reinhardt-admin");

/// Walk `dir` and return all files that still contain an unrendered Tera
/// placeholder (`{{`).  Returns a list of `(relative_path, offending_line)`.
///
/// Uses the `walkdir` crate so that every yielded entry is already scoped to
/// the subtree rooted at `dir` — no manual path canonicalization required.
fn find_unrendered_variables(dir: &Path) -> Vec<(PathBuf, String)> {
	let mut hits = Vec::new();
	for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
		if !entry.file_type().is_file() {
			continue;
		}
		let Ok(content) = fs::read_to_string(entry.path()) else {
			continue; // skip non-UTF-8 (compiled artifacts, etc.)
		};
		if let Some(bad_line) = content.lines().find(|l| l.contains("{{")) {
			hits.push((entry.path().to_path_buf(), bad_line.to_string()));
		}
	}
	hits
}

// ─────────────────────────────────────────────────────────────────────────────
// startproject (RESTful)
// ─────────────────────────────────────────────────────────────────────────────

#[rstest]
fn startproject_restful_renders_all_variables() {
	// Arrange
	let tmp = TempDir::new().expect("tempdir");
	let project_name = "e2e_restful_proj";

	// Act: invoke the real binary with no --template / --template-dir flags
	let output = Command::new(REINHARDT_ADMIN)
		.args(["startproject", project_name])
		.current_dir(tmp.path())
		.output()
		.expect("failed to spawn reinhardt-admin");

	let stderr = String::from_utf8_lossy(&output.stderr);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		output.status.success(),
		"startproject failed (exit {:?})\nstdout: {stdout}\nstderr: {stderr}",
		output.status.code()
	);

	let project_dir = tmp.path().join(project_name);

	// Assert — structural checks
	assert!(
		project_dir.join("Cargo.toml").is_file(),
		"Cargo.toml missing"
	);
	assert!(project_dir.join("src").is_dir(), "src/ dir missing");
	assert!(
		project_dir.join("settings").is_dir(),
		"settings/ dir missing"
	);

	// Assert — Cargo.toml has the correct project name (template variable substituted)
	let cargo_toml = fs::read_to_string(project_dir.join("Cargo.toml")).expect("read Cargo.toml");
	assert!(
		cargo_toml.contains(&format!("name = \"{project_name}\"")),
		"Cargo.toml missing `name = \"{project_name}\"`, got:\n{cargo_toml}"
	);

	// Assert — .example.toml file carries the placeholder secret key
	let local_example = project_dir.join("settings").join("local.example.toml");
	assert!(
		local_example.is_file(),
		"settings/local.example.toml missing"
	);
	let example_content = fs::read_to_string(&local_example).expect("read local.example.toml");
	assert!(
		example_content.contains("CHANGE_THIS_IN_PRODUCTION_MUST_BE_KEPT_SECRET"),
		"local.example.toml should contain placeholder secret key"
	);

	// Assert — actual local.toml has a real (non-placeholder) secret key
	let local_toml = project_dir.join("settings").join("local.toml");
	assert!(local_toml.is_file(), "settings/local.toml missing");
	let local_content = fs::read_to_string(&local_toml).expect("read local.toml");
	assert!(
		!local_content.contains("CHANGE_THIS_IN_PRODUCTION_MUST_BE_KEPT_SECRET"),
		"local.toml must NOT contain the placeholder secret key"
	);
	assert!(
		local_content.contains("secret_key"),
		"local.toml must contain a secret_key entry"
	);

	// Assert — no unrendered Tera variables remain in any generated file
	let unrendered = find_unrendered_variables(&project_dir);
	assert!(
		unrendered.is_empty(),
		"unrendered Tera variables found in generated project:\n{}",
		unrendered
			.iter()
			.map(|(p, l)| format!("  {}: {}", p.display(), l))
			.collect::<Vec<_>>()
			.join("\n")
	);
}

// ─────────────────────────────────────────────────────────────────────────────
// startproject (pages)
// ─────────────────────────────────────────────────────────────────────────────

#[rstest]
fn startproject_pages_renders_all_variables() {
	// Arrange
	let tmp = TempDir::new().expect("tempdir");
	let project_name = "e2e_pages_proj";

	// Act: use `-t mtv` to select the pages (WASM + SSR) template
	let output = Command::new(REINHARDT_ADMIN)
		.args(["startproject", project_name, "-t", "mtv"])
		.current_dir(tmp.path())
		.output()
		.expect("failed to spawn reinhardt-admin");

	let stderr = String::from_utf8_lossy(&output.stderr);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		output.status.success(),
		"startproject --pages failed (exit {:?})\nstdout: {stdout}\nstderr: {stderr}",
		output.status.code()
	);

	let project_dir = tmp.path().join(project_name);

	// Assert — structure
	assert!(
		project_dir.join("Cargo.toml").is_file(),
		"Cargo.toml missing"
	);
	assert!(project_dir.join("src").is_dir(), "src/ dir missing");

	// Assert — project name substituted
	let cargo_toml = fs::read_to_string(project_dir.join("Cargo.toml")).expect("read Cargo.toml");
	assert!(
		cargo_toml.contains(&format!("name = \"{project_name}\"")),
		"Cargo.toml missing correct project name, got:\n{cargo_toml}"
	);

	// Assert — no unrendered variables
	let unrendered = find_unrendered_variables(&project_dir);
	assert!(
		unrendered.is_empty(),
		"unrendered Tera variables found in pages project:\n{}",
		unrendered
			.iter()
			.map(|(p, l)| format!("  {}: {}", p.display(), l))
			.collect::<Vec<_>>()
			.join("\n")
	);
}

// ─────────────────────────────────────────────────────────────────────────────
// startapp inside a generated project
// ─────────────────────────────────────────────────────────────────────────────

#[rstest]
fn startapp_renders_all_variables() {
	// Arrange: first create a project, then add an app to it
	let tmp = TempDir::new().expect("tempdir");
	let project_name = "e2e_app_host";
	let app_name = "blog";

	// Create the project first
	let proj_output = Command::new(REINHARDT_ADMIN)
		.args(["startproject", project_name])
		.current_dir(tmp.path())
		.output()
		.expect("failed to spawn reinhardt-admin for startproject");
	assert!(
		proj_output.status.success(),
		"startproject pre-step failed: {}",
		String::from_utf8_lossy(&proj_output.stderr)
	);

	let project_dir = tmp.path().join(project_name);

	// Act: run startapp inside the generated project directory
	let app_output = Command::new(REINHARDT_ADMIN)
		.args(["startapp", app_name])
		.current_dir(&project_dir)
		.output()
		.expect("failed to spawn reinhardt-admin for startapp");

	let stderr = String::from_utf8_lossy(&app_output.stderr);
	let stdout = String::from_utf8_lossy(&app_output.stdout);
	assert!(
		app_output.status.success(),
		"startapp failed (exit {:?})\nstdout: {stdout}\nstderr: {stderr}",
		app_output.status.code()
	);

	let app_dir = project_dir.join("src").join("apps").join(app_name);
	assert!(
		app_dir.exists(),
		"app directory missing at {}",
		app_dir.display()
	);

	// Assert — no unrendered variables in the entire project (project + app)
	let unrendered = find_unrendered_variables(&project_dir);
	assert!(
		unrendered.is_empty(),
		"unrendered Tera variables found after startapp:\n{}",
		unrendered
			.iter()
			.map(|(p, l)| format!("  {}: {}", p.display(), l))
			.collect::<Vec<_>>()
			.join("\n")
	);
}

// ─────────────────────────────────────────────────────────────────────────────
// --template-dir override
// ─────────────────────────────────────────────────────────────────────────────

#[rstest]
fn startproject_template_dir_override_wins_for_overridden_file() {
	// Arrange: create a partial override directory with a custom Cargo.toml.tpl
	let tmp = TempDir::new().expect("tempdir");
	let override_dir = tmp
		.path()
		.join("my_templates")
		.join("project_restful_template");
	fs::create_dir_all(&override_dir).expect("create override dir");
	fs::write(
		override_dir.join("Cargo.toml.tpl"),
		b"# CUSTOM CARGO TOML FOR {{ project_name }}\n",
	)
	.expect("write custom template");

	let project_name = "e2e_override_proj";

	// Act
	let output = Command::new(REINHARDT_ADMIN)
		.args([
			"startproject",
			project_name,
			"--template-dir",
			tmp.path().join("my_templates").to_str().unwrap(),
		])
		.current_dir(tmp.path())
		.output()
		.expect("failed to spawn reinhardt-admin");

	let stderr = String::from_utf8_lossy(&output.stderr);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		output.status.success(),
		"startproject --template-dir failed (exit {:?})\nstdout: {stdout}\nstderr: {stderr}",
		output.status.code()
	);

	let project_dir = tmp.path().join(project_name);

	// Assert — overridden Cargo.toml comes from our custom template
	let cargo_toml = fs::read_to_string(project_dir.join("Cargo.toml")).expect("read Cargo.toml");
	assert!(
		cargo_toml.starts_with("# CUSTOM CARGO TOML FOR"),
		"expected custom Cargo.toml header, got:\n{cargo_toml}"
	);
	// Template variable in the custom file was also rendered
	assert!(
		cargo_toml.contains(project_name),
		"custom Cargo.toml must have project name substituted"
	);

	// Assert — non-overridden files still come from embedded templates (src/ exists)
	assert!(
		project_dir.join("src").is_dir(),
		"src/ dir should come from embedded fallback"
	);
}
