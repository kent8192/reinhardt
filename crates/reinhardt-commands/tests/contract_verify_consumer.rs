//! Consumer-project smoke coverage for the `manage verify` launcher path.

#![cfg(feature = "contract")]

use serial_test::serial;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const FIXTURE: &str = "tests/fixtures/contract_verify_project";
const CONSUMER_TARGET: &str = "/tmp/reinhardt-contract-verify-consumer-target";

#[derive(Clone, Copy)]
enum ConsumerKind {
	Clean,
	Violating,
}

fn materialize() -> TempDir {
	let temp = TempDir::new().expect("create consumer tempdir");
	write_fixture(temp.path(), ConsumerKind::Clean);
	temp
}

fn write_fixture(root: &Path, kind: ConsumerKind) {
	let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../..")
		.canonicalize()
		.expect("resolve workspace root");
	for relative in [
		"Cargo.toml",
		"build.rs",
		"src/lib.rs",
		"src/bin/manage.rs",
		"settings/base.toml",
		"migrations/sample/0001_initial.rs",
	] {
		let source = Path::new(env!("CARGO_MANIFEST_DIR"))
			.join(FIXTURE)
			.join(format!("{}.tpl", relative));
		let mut content = fs::read_to_string(&source).expect("read fixture template");
		content = content.replace(
			"__REINHARDT_ROOT__",
			workspace_root.to_str().expect("workspace root is UTF-8"),
		);
		let violating = matches!(kind, ConsumerKind::Violating);
		content = content.replace("__BROKEN__", "");
		content = content.replace(
			"__MOUNTED_AUTH__",
			if violating { "" } else { ", auth = \"public\"" },
		);
		content = content.replace(
			"__UNMOUNTED_AUTH__",
			if violating { "" } else { ", auth = \"public\"" },
		);
		content = content.replace(
			"__SETTINGS__",
			if violating {
				"{\"values\":\"not-a-sequence\",\"secrets\":{\"dynamic-secret\":\"secret-sentinel-5986\"}}"
			} else {
				"{\"secret\":\"contract-verify-secret\",\"values\":[\"ok\"],\"secrets\":{\"1\":2}}"
			},
		);
		content = content.replace(
			"__MODEL__",
			if violating {
				"use reinhardt::core::serde::{Deserialize, Serialize};\n#[model(app_label = \"sample\", table_name = \"sample\")]\n#[derive(Serialize, Deserialize)]\npub struct Sample {\n    #[field(primary_key = true)]\n    pub id: i64,\n}"
			} else {
				""
			},
		);
		let destination = root.join(relative);
		if let Some(parent) = destination.parent() {
			fs::create_dir_all(parent).expect("create fixture parent");
		}
		fs::write(destination, content).expect("write fixture");
	}
}

fn active_toolchain() -> String {
	let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../..")
		.canonicalize()
		.expect("resolve workspace root");
	let output = Command::new("rustup")
		.current_dir(workspace_root)
		.args(["show", "active-toolchain"])
		.output()
		.expect("query active Rust toolchain");
	String::from_utf8(output.stdout)
		.expect("toolchain output is UTF-8")
		.split_whitespace()
		.next()
		.expect("active toolchain is present")
		.to_owned()
}

fn active_target() -> String {
	let output = Command::new("rustc")
		.args(["-vV"])
		.output()
		.expect("query Rust host target");
	String::from_utf8(output.stdout)
		.expect("rustc output is UTF-8")
		.lines()
		.find_map(|line| line.strip_prefix("host: "))
		.expect("rustc host target is present")
		.to_owned()
}

fn run_verify(root: &Path) -> std::process::Output {
	Command::new(env!("CARGO").to_owned())
		.current_dir(root)
		.env("CARGO_TARGET_DIR", CONSUMER_TARGET)
		.env("RUSTUP_TOOLCHAIN", active_toolchain())
		.env("CARGO_BUILD_JOBS", "2")
		.env("CARGO_INCREMENTAL", "0")
		.args([
			"run",
			"--quiet",
			"--offline",
			"--target-dir",
			CONSUMER_TARGET,
			"--bin",
			"manage",
			"--",
			"verify",
		])
		.output()
		.expect("run consumer verify")
}

fn run_built_manage(root: &Path) -> std::process::Output {
	Command::new(Path::new(CONSUMER_TARGET).join("debug/manage"))
		.current_dir(root)
		.env("RUSTUP_TOOLCHAIN", active_toolchain())
		.env("REINHARDT_ENABLED_FEATURES", "")
		.env("REINHARDT_TARGET", active_target())
		.env("REINHARDT_PROFILE", "debug")
		.env("REINHARDT_CARGO_REPLAY", "exact")
		.args(["verify"])
		.output()
		.expect("run built consumer manage")
}

#[test]
#[serial(contract_verify_consumer)]
fn consumer_processes_dynamic_topology_and_independent_checks() {
	let clean_dir = materialize();
	let clean = run_verify(clean_dir.path());
	let clean_stdout = String::from_utf8_lossy(&clean.stdout);
	assert!(!clean.status.success());
	assert!(clean_stdout.contains("route topology"));
	assert!(!clean_stdout.contains("Verification passed."));

	write_fixture(clean_dir.path(), ConsumerKind::Violating);
	let violating = run_verify(clean_dir.path());
	let violating_stdout = String::from_utf8_lossy(&violating.stdout);
	let violating_stderr = String::from_utf8_lossy(&violating.stderr);
	assert!(!violating.status.success());
	assert!(
		violating_stdout.contains("schema.missing_migration"),
		"violating stdout: {violating_stdout}\nviolating stderr: {violating_stderr}"
	);
	assert!(violating_stdout.contains("settings.missing_required"));
	assert!(violating_stdout.contains("settings.type_mismatch"));
	assert!(violating_stdout.contains("settings.map_key_type_mismatch"));
	assert!(violating_stdout.contains("route topology"));
	assert!(!violating_stdout.contains("authorization.missing_declaration"));
	assert!(!violating_stdout.contains("/unmounted"));
	assert!(!violating_stdout.contains("dynamic-secret"));
	assert!(!violating_stdout.contains("secret-sentinel-5986"));
	assert!(!violating_stderr.contains("dynamic-secret"));
	assert!(!violating_stderr.contains("secret-sentinel-5986"));

	let violating_again = run_verify(clean_dir.path());
	assert_eq!(violating.stdout, violating_again.stdout);
	assert_eq!(violating.stderr, violating_again.stderr);

	let broken_dir = clean_dir;
	let source = broken_dir.path().join("src/lib.rs");
	let mut content = fs::read_to_string(&source).expect("read built consumer source");
	content.push_str("\ncompile_error!(\"deliberately broken consumer source\");\n");
	fs::write(source, content).expect("break built consumer source");
	let broken = run_built_manage(broken_dir.path());
	let stdout = String::from_utf8_lossy(&broken.stdout);
	let stderr = String::from_utf8_lossy(&broken.stderr);
	assert!(!broken.status.success());
	assert!(
		stderr.contains("cargo check") || stdout.contains("cargo check"),
		"broken stdout: {stdout}\nbroken stderr: {stderr}"
	);
	assert!(!stdout.contains("Verification passed."));
}
