use rstest::rstest;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Output;

#[rstest]
fn model_generated_payload_executes_on_wasm() {
	let crate_dir = tempfile::tempdir().expect("create temporary fixture directory");
	let target_dir = tempfile::tempdir().expect("create temporary target directory");
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let repo_root = manifest_dir
		.join("../../..")
		.canonicalize()
		.expect("resolve repository root");
	let fixture_dir = manifest_dir.join("tests/fixtures/model_wasm_parity");
	let wasm_bindgen_test_runner = "wasm-bindgen-test-runner";
	let wasm_bindgen_version = detect_wasm_bindgen_runner_version(wasm_bindgen_test_runner);
	let wasm_bindgen_test_version = wasm_bindgen_test_version_for(&wasm_bindgen_version);

	fs::create_dir(crate_dir.path().join("src")).expect("create fixture src directory");
	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			r#"[package]
name = "reinhardt-model-wasm-parity-fixture"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[features]
native = ["reinhardt/core", "reinhardt/database", "reinhardt/forms", "reinhardt/image-fields"]

[dependencies]
reinhardt = {{ path = "{}", package = "reinhardt-web", default-features = false }}
reinhardt-core = {{ path = "{}" }}
decimal = {{ package = "rust_decimal", version = "1.0", features = ["serde"] }}
identifier = {{ package = "uuid", version = "1.0", features = ["serde"] }}
json = {{ package = "serde_json", version = "1.0" }}
serde = {{ version = "1.0", features = ["derive"] }}
time = {{ package = "chrono", version = "0.4", features = ["serde"] }}

[target.'cfg(not(all(target_family = "wasm", target_os = "unknown")))'.dependencies]
ctor = "0.8.0"
linkme = "0.3"

[dev-dependencies]
rstest = "0.26"
wasm-bindgen-test = "={}"

[target.'cfg(not(all(target_family = "wasm", target_os = "unknown")))'.dev-dependencies]
serial_test = "3"
"#,
			repo_root.display(),
			repo_root.join("crates/reinhardt-core").display(),
			wasm_bindgen_test_version
		),
	)
	.expect("write fixture manifest");
	fs::write(
		crate_dir.path().join("build.rs"),
		r#"fn main() {
	println!("cargo::rustc-check-cfg=cfg(native)");
}
"#,
	)
	.expect("write fixture build script");
	for source in ["lib.rs", "named_validation.rs", "validation_regressions.rs"] {
		fs::copy(
			fixture_dir.join("src").join(source),
			crate_dir.path().join("src").join(source),
		)
		.expect("copy fixture source");
	}

	let manifest_path = crate_dir.path().join("Cargo.toml");
	let target_path = target_dir.path().to_path_buf();
	let native_output = native_fixture_test_command(&manifest_path, &target_path)
		.arg("--offline")
		.output()
		.expect("run native model macro parity fixture tests");
	let native_output = if native_output.status.success()
		|| !offline_dependency_resolution_failed(&native_output)
	{
		native_output
	} else {
		native_fixture_test_command(&manifest_path, &target_path)
			.output()
			.expect("run native model macro parity fixture tests without offline mode")
	};
	assert!(
		native_output.status.success(),
		"native model macro parity fixture should execute the same declaration\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&native_output.stdout),
		String::from_utf8_lossy(&native_output.stderr),
	);

	let output = wasm_fixture_test_command(&manifest_path, &target_path, wasm_bindgen_test_runner)
		.arg("--offline")
		.arg("--")
		.arg("--nocapture")
		.output()
		.expect("run wasm model macro parity fixture tests");
	let output = if output.status.success() || !offline_dependency_resolution_failed(&output) {
		output
	} else {
		wasm_fixture_test_command(&manifest_path, &target_path, wasm_bindgen_test_runner)
			.arg("--")
			.arg("--nocapture")
			.output()
			.expect("run wasm model macro parity fixture tests without offline mode")
	};

	assert!(
		output.status.success(),
		"WASM model macro parity fixture should execute\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr),
	);
	let runtime_output = format!(
		"{}\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	// wasm-bindgen-test-runner interleaves platform-dependent harness and log
	// output, so the complete process output cannot be compared exactly. These
	// guards instead require each exact generated test name after a successful exit.
	assert!(
		runtime_output.contains("generated_named_contract_executes_in_wasm_runtime"),
		"WASM model macro parity fixture must execute the generated named contract test\n{runtime_output}",
	);
	assert!(
		runtime_output
			.contains("named_contract_normalizes_defaults_and_runs_application_validation"),
		"WASM model macro parity fixture must execute named validation\n{runtime_output}",
	);
	assert!(
		runtime_output.contains("generated_payload_cleans_and_validates_in_wasm_runtime"),
		"WASM model macro parity fixture must execute generated validation\n{runtime_output}",
	);
	assert!(
		runtime_output
			.contains("generated_required_email_uses_the_canonical_message_in_wasm_runtime"),
		"WASM model macro parity fixture must execute required email parity\n{runtime_output}",
	);
	assert!(
		runtime_output.contains(
			"generated_create_and_update_semantics_match_the_server_boundary_in_wasm_runtime"
		),
		"WASM model macro parity fixture must execute create/update parity\n{runtime_output}",
	);
	assert!(
		runtime_output
			.contains("generated_required_scalars_use_canonical_create_errors_in_wasm_runtime"),
		"WASM model macro parity fixture must execute required scalar parity\n{runtime_output}",
	);
	assert!(
		runtime_output.contains("generated_snapshot_deferral_only_accepts_required_uploads"),
		"WASM model macro parity fixture must execute upload deferral validation\n{runtime_output}",
	);

	let dependency_tree = wasm_dependency_tree_command(&manifest_path)
		.output()
		.expect("inspect WASM fixture dependency tree");
	assert!(
		dependency_tree.status.success(),
		"WASM dependency-tree inspection should succeed\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&dependency_tree.stdout),
		String::from_utf8_lossy(&dependency_tree.stderr),
	);
	let dependency_tree = String::from_utf8_lossy(&dependency_tree.stdout);
	for forbidden in [
		"reinhardt-db",
		"sqlx",
		"tokio-postgres",
		"mysql_async",
		"rusqlite",
	] {
		assert!(
			!dependency_tree.contains(forbidden),
			"WASM named-contract fixture must not depend on `{forbidden}`\n{dependency_tree}",
		);
	}
}

fn wasm_fixture_test_command(
	manifest_path: &Path,
	target_path: &Path,
	wasm_bindgen_test_runner: &str,
) -> Command {
	let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
	command
		.env(
			"CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER",
			wasm_bindgen_test_runner,
		)
		.arg("test")
		.arg("--manifest-path")
		.arg(manifest_path)
		.arg("--target")
		.arg("wasm32-unknown-unknown")
		.arg("--target-dir")
		.arg(target_path);
	command
}

fn native_fixture_test_command(manifest_path: &Path, target_path: &Path) -> Command {
	let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
	command
		.arg("test")
		.arg("--lib")
		.arg("--manifest-path")
		.arg(manifest_path)
		.arg("--features")
		.arg("native")
		.arg("--target-dir")
		.arg(target_path);
	command
}

fn wasm_dependency_tree_command(manifest_path: &Path) -> Command {
	let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
	command
		.arg("tree")
		.arg("--manifest-path")
		.arg(manifest_path)
		.arg("--target")
		.arg("wasm32-unknown-unknown");
	command
}

fn detect_wasm_bindgen_runner_version(wasm_bindgen_test_runner: &str) -> String {
	let output = Command::new(wasm_bindgen_test_runner)
		.arg("-V")
		.output()
		.expect("wasm-bindgen-test-runner must be installed for WASM tests");
	assert!(
		output.status.success(),
		"wasm-bindgen-test-runner -V should succeed\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr),
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	stdout
		.split_whitespace()
		.nth(1)
		.expect("wasm-bindgen-test-runner -V output must include a version")
		.to_string()
}

fn wasm_bindgen_test_version_for(wasm_bindgen_version: &str) -> String {
	let mut parts = wasm_bindgen_version.split('.');
	let major = parts.next().expect("wasm-bindgen version has major");
	let minor = parts.next().expect("wasm-bindgen version has minor");
	let patch = parts
		.next()
		.expect("wasm-bindgen version has patch")
		.parse::<u16>()
		.expect("wasm-bindgen patch version is numeric");
	assert_eq!(major, "0", "unexpected wasm-bindgen major version");
	assert_eq!(minor, "2", "unexpected wasm-bindgen minor version");
	assert!(
		patch >= 50,
		"wasm-bindgen patch version must map to wasm-bindgen-test 0.3.x"
	);
	format!("0.3.{}", patch - 50)
}

fn offline_dependency_resolution_failed(output: &Output) -> bool {
	let stderr = String::from_utf8_lossy(&output.stderr);
	stderr.contains("--offline")
		|| stderr.contains("no matching package named")
		|| stderr.contains("failed to download")
		|| stderr.contains("candidate versions found which didn't match")
}
