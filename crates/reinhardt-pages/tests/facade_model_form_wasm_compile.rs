#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;
use std::process::Command;
use std::process::Output;

use tempfile::TempDir;

#[test]
fn facade_only_named_model_form_compiles_for_wasm() {
	let pages_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let fixture_manifest = pages_dir
		.join("tests/fixtures/facade_model_form_wasm/Cargo.toml")
		.canonicalize()
		.expect("resolve facade-only WASM fixture manifest");
	let manifest = std::fs::read_to_string(&fixture_manifest).expect("read fixture manifest");
	assert!(
		!manifest.contains("reinhardt-core"),
		"the facade-only fixture must not directly depend on reinhardt-core"
	);

	let target_dir = TempDir::new().expect("create facade-only WASM target directory");
	let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(&fixture_manifest)
		.arg("--target")
		.arg("wasm32-unknown-unknown")
		.arg("--target-dir")
		.arg(target_dir.path())
		.arg("--offline")
		.env_remove("CARGO_BUILD_BUILD_DIR")
		.env_remove("CARGO_TARGET_DIR")
		.env_remove("RUSTC_WRAPPER")
		.output()
		.expect("compile facade-only WASM fixture");
	let output = if output.status.success() || !offline_dependency_resolution_failed(&output) {
		output
	} else {
		Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
			.arg("check")
			.arg("--manifest-path")
			.arg(&fixture_manifest)
			.arg("--target")
			.arg("wasm32-unknown-unknown")
			.arg("--target-dir")
			.arg(target_dir.path())
			.env_remove("CARGO_BUILD_BUILD_DIR")
			.env_remove("CARGO_TARGET_DIR")
			.env_remove("RUSTC_WRAPPER")
			.output()
			.expect("compile facade-only WASM fixture without offline mode")
	};

	assert!(
		output.status.success(),
		"facade-only generated model form must compile for WASM\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr),
	);
}

fn offline_dependency_resolution_failed(output: &Output) -> bool {
	let stderr = String::from_utf8_lossy(&output.stderr);
	stderr.contains("--offline")
		|| stderr.contains("no matching package named")
		|| stderr.contains("failed to download")
		|| stderr.contains("candidate versions found which didn't match")
}
