use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn model_foreign_key_id_accessor_compiles_on_wasm() {
	let crate_dir = tempfile::tempdir().expect("create temporary fixture directory");
	let target_dir = tempfile::tempdir().expect("create temporary target directory");
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let repo_root = manifest_dir
		.join("../../..")
		.canonicalize()
		.expect("resolve repository root");
	let fixture_dir = manifest_dir.join("tests/fixtures/model_wasm_parity");

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

[dependencies]
reinhardt = {{ path = "{}", package = "reinhardt-web", default-features = false }}
reinhardt-core = {{ path = "{}" }}
chrono = {{ version = "0.4", features = ["serde"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#,
			repo_root.display(),
			repo_root.join("crates/reinhardt-core").display()
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
	fs::copy(
		fixture_dir.join("src/lib.rs"),
		crate_dir.path().join("src/lib.rs"),
	)
	.expect("copy fixture source");

	let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(crate_dir.path().join("Cargo.toml"))
		.arg("--target")
		.arg("wasm32-unknown-unknown")
		.arg("--target-dir")
		.arg(target_dir.path())
		.arg("--offline")
		.output()
		.expect("run wasm model macro parity fixture");

	assert!(
		output.status.success(),
		"WASM model macro parity fixture should compile\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr),
	);
}
