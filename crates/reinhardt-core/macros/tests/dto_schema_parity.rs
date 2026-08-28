use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn dto_schema_option_compiles_and_generates_native_schema() {
	let crate_dir = tempfile::tempdir().expect("create temporary fixture directory");
	let target_dir = tempfile::tempdir().expect("create temporary target directory");
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
	let metadata = Command::new(&cargo)
		.arg("metadata")
		.arg("--manifest-path")
		.arg(manifest_dir.join("Cargo.toml"))
		.arg("--no-deps")
		.arg("--offline")
		.arg("--format-version")
		.arg("1")
		.output()
		.expect("resolve workspace metadata");
	assert!(
		metadata.status.success(),
		"workspace metadata should resolve\nstderr:\n{}",
		String::from_utf8_lossy(&metadata.stderr),
	);
	let metadata: serde_json::Value =
		serde_json::from_slice(&metadata.stdout).expect("parse workspace metadata");
	let repo_root = PathBuf::from(
		metadata["workspace_root"]
			.as_str()
			.expect("workspace metadata should include an absolute root"),
	);
	let repo_root_toml = serde_json::to_string(&repo_root.to_string_lossy().into_owned())
		.expect("escape repository root for TOML");
	let rest_root_toml = serde_json::to_string(
		&repo_root
			.join("crates/reinhardt-rest")
			.to_string_lossy()
			.into_owned(),
	)
	.expect("escape reinhardt-rest root for TOML");

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
reinhardt = {{ path = {}, package = "reinhardt-web", default-features = false, features = ["openapi"] }}
reinhardt_rest = {{ path = {}, package = "reinhardt-rest", default-features = false, features = ["openapi"] }}
"#,
			repo_root_toml,
			rest_root_toml
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
#[schema(title = "Login request")]
struct LoginRequest {
	#[schema(description = "Username", example = "alice")]
	username: String,
	#[schema(default_value = "0")]
	attempts: i64,
}

#[dto(schema)]
struct Profile {
	display_name: String,
}

#[dto(schema)]
#[cfg_attr(native, derive(reinhardt_rest::openapi::Schema))]
struct DirectRestSchema {
	name: String,
}

fn main() {
	assert_eq!(LoginRequest::schema_name(), Some(String::from("LoginRequest")));
	assert_eq!(Profile::schema_name(), Some(String::from("Profile")));
	assert_eq!(DirectRestSchema::schema_name(), Some(String::from("DirectRestSchema")));
}
"#,
	)
	.expect("write fixture source");

	let output = Command::new(&cargo)
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
