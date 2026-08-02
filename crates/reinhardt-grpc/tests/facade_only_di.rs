#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn facade_only_dependency_compiles_grpc_handler_di() {
	run_facade_fixture("renamed", "reinhardt", "reinhardt");
	run_facade_fixture("unrenamed", "reinhardt-web", "reinhardt_web");
}

fn run_facade_fixture(case: &str, dependency_key: &str, crate_ident: &str) {
	let crate_dir = tempfile::tempdir().expect("create downstream fixture directory");
	let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../..")
		.canonicalize()
		.expect("resolve repository root");
	let repo_root_toml = toml::Value::String(repo_root.to_string_lossy().into_owned()).to_string();
	let target_dir = shared_target_dir(&repo_root);

	fs::create_dir(crate_dir.path().join("src")).expect("create downstream src directory");
	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			r#"[package]
name = "reinhardt-grpc-facade-only-di-fixture-{case}"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
{dependency_key} = {{ path = {repo_root_toml}, package = "reinhardt-web", default-features = false, features = ["minimal", "grpc"] }}
tonic = "0.14.2"
"#
		),
	)
	.expect("write downstream manifest");
	fs::write(
		crate_dir.path().join("src/main.rs"),
		format!(
			r#"use {crate_ident}::di::{{Depends, InjectableKey, InjectionContext}};
use {crate_ident}::grpc::{{GrpcRequestExt, grpc_handler}};
use std::sync::Arc;
use tonic::{{Request, Response, Status}};

struct ConfigKey;

impl InjectableKey for ConfigKey {{}}

struct ConfigService;
struct MyRequest;
struct MyResponse;

#[grpc_handler]
async fn handler(
	request: Request<MyRequest>,
	#[inject] _service: Depends<ConfigKey, ConfigService>,
) -> Result<Response<MyResponse>, Status> {{
	let _ = request.into_inner();
	Ok(Response::new(MyResponse))
}}

fn context_from_request(request: &Request<MyRequest>) -> Option<Arc<InjectionContext>> {{
	request.get_di_context::<Arc<InjectionContext>>()
}}

fn main() {{
	let _handler = handler;
	let _extractor = context_from_request;
}}
"#
		),
	)
	.expect("write downstream source");

	let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(crate_dir.path().join("Cargo.toml"))
		.arg("--target-dir")
		.arg(target_dir)
		.arg("--offline")
		.output()
		.expect("run downstream facade-only cargo check");

	assert!(
		output.status.success(),
		"facade-only gRPC DI fixture {case} should compile\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

fn shared_target_dir(repo_root: &Path) -> PathBuf {
	if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
		let target_dir = PathBuf::from(target_dir);
		return if target_dir.is_absolute() {
			target_dir
		} else {
			repo_root.join(target_dir)
		};
	}

	if let Some(target_dir) = current_test_target_dir() {
		return target_dir;
	}

	repo_root.join("target")
}

fn current_test_target_dir() -> Option<PathBuf> {
	let current_exe = std::env::current_exe().ok()?;
	for ancestor in current_exe.ancestors() {
		let Some(name) = ancestor.file_name().and_then(|name| name.to_str()) else {
			continue;
		};
		if matches!(name, "debug" | "release") {
			return ancestor.parent().map(Path::to_path_buf);
		}
	}
	None
}
