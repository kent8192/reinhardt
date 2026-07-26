//! Integration test proving `startproject` produces a usable project tree
//! from embedded templates alone — no CARGO_MANIFEST_DIR dependency.

use reinhardt_commands::start_commands::StartProjectCommand;
use reinhardt_commands::{BaseCommand, CommandContext};
use rstest::*;
use serial_test::serial;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

// Assert that Cargo can fully parse the generated manifest.
//
// Uses `cargo metadata --no-deps` so no registry access is required; the
// command still exercises the same manifest-parsing step that rejects
// misconfigurations (e.g. a `default-run` pointing at a nonexistent bin)
// which would break the scaffold for a real user on `cargo run`.
fn assert_manifest_parses(manifest: &Path) {
	let output = Command::new(env!("CARGO"))
		.args(["metadata", "--no-deps", "--format-version", "1"])
		.arg("--manifest-path")
		.arg(manifest)
		.output()
		.expect("cargo metadata command spawns");
	assert!(
		output.status.success(),
		"generated manifest failed to parse: {}\nstderr:\n{}",
		manifest.display(),
		String::from_utf8_lossy(&output.stderr),
	);
}

fn assert_reinhardt_dependency_features(cargo_toml: &str, expected: &[&str]) {
	let document = cargo_toml
		.parse::<toml_edit::DocumentMut>()
		.expect("generated Cargo.toml must parse as TOML");
	let features = document["target"][r#"cfg(not(target_arch = "wasm32"))"#]["dependencies"]
		["reinhardt"]["features"]
		.as_array()
		.expect("generated native reinhardt dependency must declare features");

	for feature in expected {
		assert!(
			features.iter().any(|value| value.as_str() == Some(feature)),
			"generated reinhardt dependency must include `{feature}`:\n{cargo_toml}"
		);
	}
}

fn assert_generated_rust_sources_do_not_use_tab_indents(root: &Path) {
	for relative in [
		"build.rs",
		"src/bin/manage.rs",
		"src/client/components/nav.rs",
		"src/client/lib.rs",
		"src/config/settings.rs",
		"src/config/wasm.rs",
		"src/lib.rs",
		"tests/integration.rs",
	] {
		let content = std::fs::read_to_string(root.join(relative)).unwrap();
		assert!(
			!content.contains('\t'),
			"generated Rust source should be rustfmt-clean before cargo make dev: {relative}"
		);
	}
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_restful_from_embedded_only() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["sample_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("restful".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	// Act
	let res = StartProjectCommand.execute(&ctx).await;

	// Assert
	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject succeeds from embedded templates");
	let generated = tmp.path().join("sample_proj");
	assert!(
		generated.join("Cargo.toml").exists(),
		"Cargo.toml must be generated"
	);
	assert!(
		generated.join("src").is_dir(),
		"src/ directory must be generated"
	);
	assert_manifest_parses(&generated.join("Cargo.toml"));
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_restful_honors_dependency_selection_flags() {
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["feature_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("restful".to_string(), vec!["true".to_string()]);
	opts.insert(
		"reinhardt-version".to_string(),
		vec!["0.2.0-rc.4".to_string()],
	);
	opts.insert(
		"features".to_string(),
		vec!["minimal,db-sqlite".to_string()],
	);
	opts.insert("no-interactive".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	let res = StartProjectCommand.execute(&ctx).await;

	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject succeeds with dependency selection flags");
	let cargo_toml = std::fs::read_to_string(tmp.path().join("feature_proj/Cargo.toml")).unwrap();
	assert!(cargo_toml.contains("version = \"0.2.0-rc.4\""));
	assert!(cargo_toml.contains("default-features = false"));
	assert!(
		cargo_toml
			.contains("features = [\"minimal\", \"db-sqlite\", \"conf\", \"commands\", \"api\"]")
	);
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_pages_from_embedded_only() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["sample_pages_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	// Act
	let res = StartProjectCommand.execute(&ctx).await;

	// Assert
	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject --with-pages succeeds from embedded templates");
	let generated = tmp.path().join("sample_pages_proj");
	assert!(
		generated.join("Cargo.toml").exists(),
		"Cargo.toml must be generated"
	);
	assert!(
		generated.join("src").is_dir(),
		"src/ directory must be generated"
	);
	let index_html = std::fs::read_to_string(generated.join("index.html")).unwrap();
	assert!(index_html.contains("{{ static_url(\"__reinhardt__/components.css\") }}"));
	let cargo_toml = std::fs::read_to_string(generated.join("Cargo.toml")).unwrap();
	assert!(cargo_toml.contains(
		"package = \"reinhardt-web\", default-features = false, features = [\"pages\", \"client-router\"]"
	));
	assert_reinhardt_dependency_features(
		&cargo_toml,
		&[
			"minimal",
			"pages",
			"client-router",
			"admin",
			"conf",
			"commands",
			"commands-server",
			"commands-autoreload",
			"server",
			"db-sqlite",
			"forms",
			"auth-session",
			"middleware",
			"argon2-hasher",
		],
	);
	assert!(
		!cargo_toml.contains("\"standard\"") && !cargo_toml.contains("\"db-postgres\""),
		"generated pages manifest must not require PostgreSQL defaults:\n{cargo_toml}"
	);
	let base_toml = std::fs::read_to_string(generated.join("settings/base.toml")).unwrap();
	assert!(
		base_toml.contains("engine = \"sqlite\"")
			&& base_toml.contains("name = \"db.sqlite3\"")
			&& !base_toml.contains("engine = \"postgresql\""),
		"generated pages settings must match the SQLite feature default:\n{base_toml}"
	);
	assert!(
		cargo_toml.contains("required-features = [\"with-reinhardt\"]"),
		"generated pages manage binary must be native-feature gated:\n{cargo_toml}"
	);
	assert!(
		cargo_toml.contains("default = [\"with-reinhardt\", \"client-router\"]")
			&& cargo_toml.contains("msw = [\"reinhardt/msw\"]"),
		"generated pages Cargo.toml must declare local feature gates used by WASM tests:\n{cargo_toml}"
	);
	let makefile_toml = std::fs::read_to_string(generated.join("Makefile.toml")).unwrap();
	assert!(
		makefile_toml.contains("\"--no-input\""),
		"generated pages Makefile must use collectstatic's non-interactive flag"
	);
	assert!(
		!makefile_toml.contains("\"--noinput\""),
		"generated pages Makefile must not use the createsuperuser-only --noinput spelling"
	);
	assert!(
		makefile_toml.contains("command = \"wasm-pack\"")
			&& makefile_toml.contains("\"--out-dir\", \"dist-wasm\""),
		"generated pages Makefile must build the browser bundle with wasm-pack into dist-wasm:\n{makefile_toml}"
	);
	assert!(
		makefile_toml
			.contains("args = [\"build\", \"--target\", \"wasm32-unknown-unknown\", \"--lib\"]")
			&& makefile_toml.contains(
				"args = [\"build\", \"--target\", \"wasm32-unknown-unknown\", \"--release\", \"--lib\"]"
			),
		"generated pages Makefile must compile only the library for WASM pre-checks:\n{makefile_toml}"
	);
	assert!(
		!makefile_toml.contains("ls target/wasm32-unknown-unknown")
			&& !makefile_toml.contains("head -1"),
		"generated pages Makefile must not pick an arbitrary .wasm file such as manage.wasm:\n{makefile_toml}"
	);
	assert!(
		generated.join("scripts/wasm-build-dev.sh").exists()
			&& generated.join("scripts/wasm-build-release.sh").exists(),
		"generated pages project must include WASM post-build scripts"
	);
	let build_rs = std::fs::read_to_string(generated.join("build.rs")).unwrap();
	for cfg in ["with_reinhardt", "client", "server"] {
		assert!(
			build_rs.contains(&format!("cargo::rustc-check-cfg=cfg({cfg})")),
			"generated pages build.rs must declare cfg({cfg}) for Rust 2024 check-cfg:\n{build_rs}"
		);
	}
	for removed_cfg in ["wasm"] {
		assert!(
			!build_rs.contains(&format!("cfg({removed_cfg})")),
			"generated pages build.rs must not declare the removed cfg({removed_cfg}) alias:\n{build_rs}"
		);
	}
	assert!(
		build_rs.contains("cargo::rustc-check-cfg=cfg(native)")
			&& build_rs.contains(
				"native: { not(all(target_family = \"wasm\", target_os = \"unknown\")) }"
			),
		"generated pages build.rs must declare and define the native cfg alias:\n{build_rs}"
	);
	assert!(
		cargo_toml.contains("[workspace]") && cargo_toml.contains("members = ["),
		"generated pages Cargo.toml must be a nested-workspace-safe root:\n{cargo_toml}"
	);
	assert!(
		!generated.join("src/shared.rs").exists() && !generated.join("src/shared").exists(),
		"generated pages project must not create a root shared module"
	);
	assert_generated_rust_sources_do_not_use_tab_indents(&generated);
	assert_manifest_parses(&generated.join("Cargo.toml"));
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_pages_adds_required_pages_features() {
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["pages_feature_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	opts.insert(
		"features".to_string(),
		vec!["minimal,pages,client-router,server-fn,db-sqlite".to_string()],
	);
	opts.insert("no-interactive".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	let res = StartProjectCommand.execute(&ctx).await;

	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject --with-pages succeeds with dependency selection flags");
	let cargo_toml =
		std::fs::read_to_string(tmp.path().join("pages_feature_proj/Cargo.toml")).unwrap();
	assert_reinhardt_dependency_features(
		&cargo_toml,
		&[
			"minimal",
			"pages",
			"client-router",
			"db-sqlite",
			"admin",
			"conf",
			"commands",
			"commands-server",
			"commands-autoreload",
			"server",
			"forms",
			"auth-session",
			"middleware",
			"argon2-hasher",
		],
	);
	assert!(
		!cargo_toml.contains("\"server-fn\""),
		"stale server-fn feature alias must not be written to generated Cargo.toml:\n{cargo_toml}"
	);
	assert!(
		!cargo_toml.contains("\"db-postgres\""),
		"explicit db-sqlite selection must not be overwritten by db-postgres:\n{cargo_toml}"
	);
	assert_manifest_parses(&tmp.path().join("pages_feature_proj/Cargo.toml"));
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_pages_explicit_tutorial_features_get_minimal_runtime() {
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["pages_tutorial_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	opts.insert(
		"features".to_string(),
		vec![
			"pages,admin,conf,commands-server,commands-autoreload,db-sqlite,forms,auth-session,middleware,argon2-hasher,static-files"
				.to_string(),
		],
	);
	opts.insert("default-features".to_string(), vec!["false".to_string()]);
	opts.insert("no-interactive".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	let res = StartProjectCommand.execute(&ctx).await;

	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject --with-pages succeeds with tutorial-style explicit features");
	let cargo_toml =
		std::fs::read_to_string(tmp.path().join("pages_tutorial_proj/Cargo.toml")).unwrap();
	assert!(
		cargo_toml.contains("\"minimal\""),
		"explicit Pages feature selections must be augmented with the minimal runtime facade:\n{cargo_toml}"
	);
	assert!(
		cargo_toml.contains("\"server\""),
		"explicit Pages feature selections must be augmented with the HTTP server facade:\n{cargo_toml}"
	);
	assert!(
		cargo_toml.contains("\"db-sqlite\"") && !cargo_toml.contains("\"db-postgres\""),
		"explicit SQLite selection must not be overwritten by db-postgres:\n{cargo_toml}"
	);
	assert_manifest_parses(&tmp.path().join("pages_tutorial_proj/Cargo.toml"));
}
