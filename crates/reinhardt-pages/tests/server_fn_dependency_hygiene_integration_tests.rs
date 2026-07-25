#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::process::{Command, Output};

use tempfile::TempDir;

fn run_cargo_check(
	manifest_path: &std::path::Path,
	target_dir: &std::path::Path,
	target: Option<&str>,
) -> Output {
	let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
	command
		.arg("check")
		.arg("--manifest-path")
		.arg(manifest_path)
		.arg("--target-dir")
		.arg(target_dir)
		.arg("--features")
		.arg("msw")
		.arg("--offline");
	if let Some(target) = target {
		command.arg("--target").arg(target);
	}
	command.output().expect("run downstream cargo check")
}

fn assert_check_succeeded(output: &Output, target: &str) {
	assert!(
		output.status.success(),
		"server_fn consumer without a direct serde dependency should compile for {target}\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

#[test]
fn server_fn_consumer_does_not_require_direct_serde_dependency() {
	let crate_dir = TempDir::new().expect("create downstream fixture");
	let target_dir = TempDir::new().expect("create downstream target dir");
	let reinhardt_pages_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");

	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			r#"[package]
name = "server-fn-dependency-hygiene-fixture"
version = "0.0.0"
edition = "2024"

[features]
msw = []

[dependencies]
pages-api = {{ package = "reinhardt-pages", path = "{reinhardt_pages_dir}", features = ["msw"] }}
"#
		),
	)
	.expect("write downstream manifest");
	fs::create_dir(crate_dir.path().join("src")).expect("create downstream src dir");
	fs::write(
		crate_dir.path().join("src/main.rs"),
		r#"use pages_api::server_fn::{ServerFnError, server_fn};

#[server_fn(auto_register = false)]
pub async fn dependency_hygiene(value: u32) -> Result<u32, ServerFnError> {
	Ok(value)
}

fn main() {}
"#,
	)
	.expect("write downstream source");

	let manifest_path = crate_dir.path().join("Cargo.toml");
	let native = run_cargo_check(&manifest_path, target_dir.path(), None);
	assert_check_succeeded(&native, "native");

	let wasm = run_cargo_check(
		&manifest_path,
		target_dir.path(),
		Some("wasm32-unknown-unknown"),
	);
	assert_check_succeeded(&wasm, "wasm32-unknown-unknown");
}
