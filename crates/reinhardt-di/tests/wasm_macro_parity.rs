use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn injectable_macros_compile_as_wasm_parity_stubs() {
	let crate_dir = tempfile::tempdir().expect("create temporary fixture directory");
	let target_dir = tempfile::tempdir().expect("create temporary target directory");
	let fixture_dir =
		Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wasm_macro_parity");

	fs::create_dir(crate_dir.path().join("src")).expect("create fixture src directory");
	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			r#"[package]
name = "reinhardt-di-wasm-macro-parity-fixture"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
reinhardt-di = {{ path = "{}", features = ["macros"] }}
"#,
			env!("CARGO_MANIFEST_DIR")
		),
	)
	.expect("write fixture manifest");
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
		.expect("run wasm macro parity fixture");

	assert!(
		output.status.success(),
		"WASM macro parity fixture should compile\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr),
	);
}
