use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn dto_schema_option_compiles_and_generates_native_schema() {
	let crate_dir = tempfile::tempdir().expect("create temporary fixture directory");
	let target_dir = tempfile::tempdir().expect("create temporary target directory");
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let repo_root = manifest_dir
		.join("../../..")
		.canonicalize()
		.expect("resolve repository root");

	fs::create_dir(crate_dir.path().join("src")).expect("create fixture src directory");
	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			r#"[package]
name = "reinhardt-dto-schema-parity-fixture"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
reinhardt = {{ path = "{}", package = "reinhardt-web", default-features = false, features = ["core", "openapi"] }}
inventory = "0.3"
"#,
			repo_root.display()
		),
	)
	.expect("write fixture manifest");
	fs::write(
		crate_dir.path().join("build.rs"),
		r#"fn main() {
	println!("cargo::rustc-check-cfg=cfg(native)");
	println!("cargo::rustc-cfg=native");
}
"#,
	)
	.expect("write fixture build script");
	fs::write(
		crate_dir.path().join("src/main.rs"),
		r#"#![deny(unexpected_cfgs)]

use reinhardt::dto;

#[dto(schema)]
struct LoginRequest {
	username: String,
}

#[dto(schema)]
struct Profile {
	display_name: String,
}

fn main() {
	assert_eq!(LoginRequest::schema_name(), Some(String::from("LoginRequest")));
	assert_eq!(Profile::schema_name(), Some(String::from("Profile")));
}
"#,
	)
	.expect("write fixture source");

	let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(crate_dir.path().join("Cargo.toml"))
		.arg("--target-dir")
		.arg(target_dir.path())
		.arg("--offline")
		.output()
		.expect("run native DTO schema parity fixture");

	assert!(
		output.status.success(),
		"native DTO schema parity fixture should compile\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr),
	);
}
