//! Consumer-project smoke coverage for the `manage verify` launcher path.

#![cfg(feature = "contract")]

use serial_test::serial;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const FIXTURE: &str = "tests/fixtures/contract_verify_project";

#[derive(Clone, Copy)]
enum ConsumerKind {
	Clean,
	Violating,
}

fn materialize(kind: ConsumerKind) -> TempDir {
	let temp = TempDir::new().expect("create consumer tempdir");
	write_fixture(temp.path(), kind);
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

fn run_verify(root: &Path, target: &Path) -> std::process::Output {
	run_verify_with_args(root, target, &[])
}

fn run_verify_with_args(root: &Path, target: &Path, cargo_args: &[&str]) -> std::process::Output {
	let mut command = Command::new(env!("CARGO").to_owned());
	command
		.current_dir(root)
		.env("CARGO_TARGET_DIR", target)
		.env("RUSTUP_TOOLCHAIN", active_toolchain())
		.env("CARGO_BUILD_JOBS", "2")
		.env("CARGO_INCREMENTAL", "0");
	for argument in cargo_args {
		command.arg(argument);
	}
	command
		.args(["run", "--quiet", "--target-dir"])
		.arg(target)
		.args(["--bin", "manage", "--", "verify"])
		.output()
		.expect("run consumer verify")
}

fn run_built_manage(root: &Path, target: &Path) -> std::process::Output {
	Command::new(target.join("debug/manage"))
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
fn consumer_processes_clean_violating_and_short_circuit_paths() {
	let target = TempDir::new().expect("create consumer target tempdir");
	let consumer_dir = materialize(ConsumerKind::Clean);

	let clean = run_verify(consumer_dir.path(), target.path());
	assert!(clean.status.success());
	assert_eq!(clean.stdout, b"Verification passed.\n");

	write_fixture(consumer_dir.path(), ConsumerKind::Violating);
	let violating = run_verify(consumer_dir.path(), target.path());
	let violating_stdout = String::from_utf8_lossy(&violating.stdout);
	let violating_stderr = String::from_utf8_lossy(&violating.stderr);
	assert!(!violating.status.success());
	assert!(violating_stderr.ends_with("Execution error: contract verification found issues\n"));
	assert_eq!(
		violating_stdout,
		"finding: schema.missing_migration sample:sample (Create table sample)\n\
finding: authorization.missing_declaration GET /mounted (contract_verify_consumer/mounted_endpoint)\n\
finding: settings.map_key_type_mismatch at verification.secrets.* expected=u16 actual=Some(String) ordinal=2\n\
finding: settings.missing_required at verification.secret expected=String actual=None ordinal=0\n\
finding: settings.type_mismatch at verification.secrets.* expected=u32 actual=Some(String) ordinal=3\n\
finding: settings.type_mismatch at verification.values expected=sequence actual=Some(String) ordinal=1\n"
	);
	assert!(!violating_stderr.contains("dynamic-secret"));
	assert!(!violating_stderr.contains("secret-sentinel-5986"));

	let violating_again = run_verify(consumer_dir.path(), target.path());
	assert_eq!(violating.stdout, violating_again.stdout);
	assert_eq!(violating.stderr, violating_again.stderr);

	let settings_path = consumer_dir.path().join("settings/base.toml");
	let valid_settings = fs::read_to_string(&settings_path).expect("read valid settings source");
	fs::write(
		&settings_path,
		"secret = \"settings-source-secret-sentinel-5986\n",
	)
	.expect("write malformed settings source");
	let source_failure = run_built_manage(consumer_dir.path(), target.path());
	assert!(!source_failure.status.success());
	assert_eq!(
		source_failure.stdout,
		b"error: contract state resolution unavailable (settings source)\n"
	);
	assert!(
		source_failure
			.stderr
			.ends_with(b"Execution error: contract verification found issues\n")
	);
	assert!(!String::from_utf8_lossy(&source_failure.stdout).contains("sentinel"));
	assert!(!String::from_utf8_lossy(&source_failure.stderr).contains("sentinel"));

	fs::write(
		&settings_path,
		valid_settings.replace(
			"migration_features = []",
			"migration_features = \"invalid\"",
		),
	)
	.expect("write malformed migration section");
	let aggregate_failure = run_built_manage(consumer_dir.path(), target.path());
	assert!(!aggregate_failure.status.success());
	assert_eq!(
		aggregate_failure.stdout,
		b"error: contract state resolution unavailable (settings section migrations)\n"
	);
	assert!(
		aggregate_failure
			.stderr
			.ends_with(b"Execution error: contract verification found issues\n")
	);

	write_fixture(consumer_dir.path(), ConsumerKind::Clean);
	let source = consumer_dir.path().join("src/lib.rs");
	let clean_source = fs::read_to_string(&source).expect("read clean consumer source");
	fs::write(
		&source,
		format!("{clean_source}\ncompile_error!(\"deliberately broken consumer source\");\n"),
	)
	.expect("break clean consumer source");
	let broken = run_built_manage(consumer_dir.path(), target.path());
	let broken_stdout = String::from_utf8_lossy(&broken.stdout);
	let broken_stderr = String::from_utf8_lossy(&broken.stderr);
	assert!(!broken.status.success());
	assert_eq!(broken_stdout, "");
	assert!(broken_stderr.contains("deliberately broken consumer source"));
	assert!(broken_stderr.contains("cargo check failed; contract verification was not run"));
	assert!(!broken_stderr.contains("contract state resolution"));
	assert!(!broken_stderr.contains("finding:"));

	let unsupported_dir = materialize(ConsumerKind::Clean);
	let unsupported = run_verify_with_args(
		unsupported_dir.path(),
		target.path(),
		&["--config", "build.jobs=2", "--offline"],
	);
	assert!(!unsupported.status.success());
	assert_eq!(unsupported.stdout, b"");
	assert!(
		unsupported
			.stderr
			.ends_with(b"Execution error: Cargo replay configuration is unsupported\n")
	);
}
