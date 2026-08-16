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

fn run_verify(root: &Path, target: &Path, verify_args: &[&str]) -> std::process::Output {
	run_verify_with_args(root, target, &[], verify_args)
}

fn run_verify_with_args(
	root: &Path,
	target: &Path,
	cargo_args: &[&str],
	verify_args: &[&str],
) -> std::process::Output {
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
		.args(verify_args)
		.output()
		.expect("run consumer verify")
}

fn run_built_manage(root: &Path, target: &Path, verify_args: &[&str]) -> std::process::Output {
	Command::new(target.join("debug/manage"))
		.current_dir(root)
		.env("RUSTUP_TOOLCHAIN", active_toolchain())
		.env("REINHARDT_ENABLED_FEATURES", "")
		.env("REINHARDT_TARGET", active_target())
		.env("REINHARDT_PROFILE", "debug")
		.env("REINHARDT_CARGO_REPLAY", "exact")
		.args(["verify"])
		.args(verify_args)
		.output()
		.expect("run built consumer manage")
}

fn json_report(output: &std::process::Output) -> serde_json::Value {
	assert_eq!(output.stdout.last(), Some(&b'\n'));
	serde_json::from_slice(&output.stdout).expect("stdout is one JSON report")
}

fn json_report_with_shape(output: &std::process::Output, status: &str) -> serde_json::Value {
	let report = json_report(output);
	assert_eq!(report["schema_version"], 1);
	assert_eq!(report["status"], status);
	assert!(report["violations"].is_array());
	let object = report.as_object().expect("JSON report is an object");
	assert_eq!(object.len(), 3);
	for key in ["schema_version", "status", "violations"] {
		assert!(object.contains_key(key), "missing {key}");
	}
	report
}

fn assert_redacted(output: &std::process::Output) {
	for bytes in [&output.stdout, &output.stderr] {
		let text = String::from_utf8_lossy(bytes);
		assert!(!text.contains("dynamic-secret"));
		assert!(!text.contains("secret-sentinel-5986"));
	}
}

#[test]
#[serial(contract_verify_consumer)]
fn consumer_processes_clean_violating_and_short_circuit_paths() {
	let target = TempDir::new().expect("create consumer target tempdir");
	let consumer_dir = materialize(ConsumerKind::Clean);

	let clean = run_verify(consumer_dir.path(), target.path(), &[]);
	assert!(clean.status.success());
	assert_eq!(clean.stdout, b"Verification passed.\n");
	assert_redacted(&clean);

	let clean_json = run_verify(consumer_dir.path(), target.path(), &["--format", "json"]);
	assert_eq!(clean_json.status.code(), Some(0));
	assert_eq!(
		json_report(&clean_json),
		serde_json::json!({
			"schema_version": 1,
			"status": "passed",
			"violations": []
		})
	);
	assert!(!String::from_utf8_lossy(&clean_json.stderr).contains("\"schema_version\""));
	assert_redacted(&clean_json);

	write_fixture(consumer_dir.path(), ConsumerKind::Violating);
	let violating = run_verify(consumer_dir.path(), target.path(), &[]);
	let violating_stdout = String::from_utf8_lossy(&violating.stdout);
	assert_eq!(violating.status.code(), Some(1));
	assert_eq!(
		violating_stdout,
		"finding: schema.missing_migration sample:sample (Create table sample)\n\
finding: authorization.missing_declaration GET /mounted (contract_verify_consumer/mounted_endpoint)\n\
finding: settings.map_key_type_mismatch at verification.secrets.* expected=u16 actual=Some(String) ordinal=2\n\
finding: settings.missing_required at verification.secret expected=String actual=None ordinal=0\n\
finding: settings.type_mismatch at verification.secrets.* expected=u32 actual=Some(String) ordinal=3\n\
finding: settings.type_mismatch at verification.values expected=sequence actual=Some(String) ordinal=1\n"
	);
	assert_redacted(&violating);

	let violating_again = run_verify(consumer_dir.path(), target.path(), &[]);
	assert_eq!(violating.stdout, violating_again.stdout);
	assert_eq!(violating.stderr, violating_again.stderr);
	assert_redacted(&violating_again);

	let violating_json = run_verify(consumer_dir.path(), target.path(), &["--format", "json"]);
	assert_eq!(violating_json.status.code(), Some(1));
	let violating_report = json_report_with_shape(&violating_json, "failed");
	assert_eq!(
		violating_report["violations"]
			.as_array()
			.unwrap()
			.iter()
			.map(|item| item["code"].as_str().unwrap())
			.collect::<Vec<_>>(),
		vec![
			"schema.missing_migration",
			"authorization.missing_declaration",
			"settings.map_key_type_mismatch",
			"settings.missing_required",
			"settings.type_mismatch",
			"settings.type_mismatch",
		]
	);
	for violation in violating_report["violations"].as_array().unwrap() {
		for field in [
			"code",
			"class",
			"severity",
			"target",
			"location",
			"evidence",
			"suggested_fix",
		] {
			assert!(violation.get(field).is_some(), "missing {field}");
		}
	}
	let violating_json_stderr = String::from_utf8_lossy(&violating_json.stderr);
	assert!(!violating_json_stderr.contains("finding:"));
	assert!(!violating_json_stderr.contains("\"schema_version\""));
	assert_redacted(&violating_json);

	let violating_json_again =
		run_verify(consumer_dir.path(), target.path(), &["--format", "json"]);
	assert_eq!(violating_json.stdout, violating_json_again.stdout);
	assert_redacted(&violating_json_again);

	let settings_path = consumer_dir.path().join("settings/base.toml");
	let valid_settings = fs::read_to_string(&settings_path).expect("read valid settings source");
	fs::write(
		&settings_path,
		"secret = \"settings-source-secret-sentinel-5986\n",
	)
	.expect("write malformed settings source");
	let source_failure = run_built_manage(consumer_dir.path(), target.path(), &[]);
	assert!(!source_failure.status.success());
	assert_eq!(
		source_failure.stdout,
		b"error: contract state resolution unavailable (settings source)\n\
finding: authorization.missing_declaration GET /mounted (contract_verify_consumer/mounted_endpoint)\n"
	);
	assert!(source_failure.stderr.ends_with(
		b"Contract verification could not complete: one or more verification checks could not complete\n"
	));
	assert_redacted(&source_failure);

	let source_failure_json =
		run_built_manage(consumer_dir.path(), target.path(), &["--format", "json"]);
	assert_eq!(source_failure_json.status.code(), Some(2));
	let source_failure_report = json_report_with_shape(&source_failure_json, "error");
	assert_eq!(source_failure_report["violations"], serde_json::json!([]));
	let source_failure_json_stderr = String::from_utf8_lossy(&source_failure_json.stderr);
	assert!(
		source_failure_json_stderr
			.contains("error: contract state resolution unavailable (settings source)")
	);
	assert!(!source_failure_json_stderr.contains("\"schema_version\""));
	assert_redacted(&source_failure_json);

	fs::write(
		&settings_path,
		valid_settings.replace(
			"migration_features = []",
			"migration_features = \"invalid\"",
		),
	)
	.expect("write malformed migration section");
	let aggregate_failure = run_built_manage(consumer_dir.path(), target.path(), &[]);
	assert!(!aggregate_failure.status.success());
	assert_eq!(
		aggregate_failure.stdout,
		"error: contract state resolution unavailable (settings section migrations)\n\
finding: authorization.missing_declaration GET /mounted (contract_verify_consumer/mounted_endpoint)\n\
finding: settings.map_key_type_mismatch at verification.secrets.* expected=u16 actual=Some(String) ordinal=3\n\
finding: settings.missing_required at verification.secret expected=String actual=None ordinal=1\n\
finding: settings.type_mismatch at migrations.migration_features expected=sequence actual=Some(String) ordinal=0\n\
finding: settings.type_mismatch at verification.secrets.* expected=u32 actual=Some(String) ordinal=4\n\
finding: settings.type_mismatch at verification.values expected=sequence actual=Some(String) ordinal=2\n"
			.as_bytes()
	);
	assert!(aggregate_failure.stderr.ends_with(
		b"Contract verification could not complete: one or more verification checks could not complete\n"
	));
	assert_redacted(&aggregate_failure);

	let aggregate_failure_json =
		run_built_manage(consumer_dir.path(), target.path(), &["--format", "json"]);
	assert_eq!(aggregate_failure_json.status.code(), Some(2));
	let aggregate_failure_report = json_report_with_shape(&aggregate_failure_json, "error");
	assert_eq!(
		aggregate_failure_report["violations"],
		serde_json::json!([])
	);
	let aggregate_failure_json_stderr = String::from_utf8_lossy(&aggregate_failure_json.stderr);
	assert!(
		aggregate_failure_json_stderr
			.contains("error: contract state resolution unavailable (settings section migrations)")
	);
	assert!(!aggregate_failure_json_stderr.contains("\"schema_version\""));
	assert_redacted(&aggregate_failure_json);

	write_fixture(consumer_dir.path(), ConsumerKind::Clean);
	let source = consumer_dir.path().join("src/lib.rs");
	let clean_source = fs::read_to_string(&source).expect("read clean consumer source");
	fs::write(
		&source,
		format!("{clean_source}\ncompile_error!(\"deliberately broken consumer source\");\n"),
	)
	.expect("break clean consumer source");
	let broken = run_built_manage(consumer_dir.path(), target.path(), &[]);
	let broken_stdout = String::from_utf8_lossy(&broken.stdout);
	let broken_stderr = String::from_utf8_lossy(&broken.stderr);
	assert!(!broken.status.success());
	assert_eq!(broken_stdout, "");
	assert!(broken_stderr.contains("deliberately broken consumer source"));
	assert!(broken_stderr.contains("cargo check failed; contract verification was not run"));
	assert!(!broken_stderr.contains("contract state resolution"));
	assert!(!broken_stderr.contains("finding:"));
	assert_redacted(&broken);

	let broken_json = run_built_manage(consumer_dir.path(), target.path(), &["--format", "json"]);
	assert_eq!(broken_json.status.code(), Some(2));
	let broken_report = json_report_with_shape(&broken_json, "error");
	assert_eq!(broken_report["violations"], serde_json::json!([]));
	let broken_json_stderr = String::from_utf8_lossy(&broken_json.stderr);
	assert!(broken_json_stderr.contains("deliberately broken consumer source"));
	assert!(!broken_json_stderr.contains("\"schema_version\""));
	assert_redacted(&broken_json);

	let unsupported_dir = materialize(ConsumerKind::Clean);
	let unsupported = run_verify_with_args(
		unsupported_dir.path(),
		target.path(),
		&["--config", "build.jobs=2", "--offline"],
		&[],
	);
	assert_eq!(unsupported.status.code(), Some(2));
	assert_eq!(unsupported.stdout, b"");
	assert!(unsupported.stderr.ends_with(
		b"Contract verification could not complete: Cargo replay configuration is unsupported\n"
	));
	assert_redacted(&unsupported);

	let unsupported_json = run_verify_with_args(
		unsupported_dir.path(),
		target.path(),
		&["--config", "build.jobs=2", "--offline"],
		&["--format", "json"],
	);
	assert_eq!(unsupported_json.status.code(), Some(2));
	let unsupported_report = json_report_with_shape(&unsupported_json, "error");
	assert_eq!(unsupported_report["violations"], serde_json::json!([]));
	assert!(!String::from_utf8_lossy(&unsupported_json.stderr).contains("\"schema_version\""));
	assert_redacted(&unsupported_json);
}
