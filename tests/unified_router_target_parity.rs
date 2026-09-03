use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn unified_router_has_a_target_neutral_return_type() {
	let crate_dir = tempfile::tempdir().expect("create temporary fixture directory");
	let target_dir = tempfile::tempdir().expect("create temporary target directory");
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let repo_root = manifest_dir.to_path_buf();
	let fixture_dir = manifest_dir.join("tests/fixtures/unified_router_target_parity");

	fs::create_dir(crate_dir.path().join("src")).expect("create fixture src directory");
	let manifest = format!(
		r#"[package]
name = "reinhardt-unified-router-target-parity-fixture"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[features]
server = []

[dependencies]
reinhardt = {{ path = "{}", package = "reinhardt-web", default-features = false, features = ["client-router"] }}
# Workaround for Lokathor/tinyvec#225 (tracked in reinhardt-web#6260)
# Remove this workaround when tinyvec publishes a release that compiles with
# the `alloc` feature without `std`.
#
# Ideal implementation (without workaround):
# omit this direct pin and let isolated fixtures resolve tinyvec from crates.io.
tinyvec = "=1.12.0"
"#,
		repo_root.display(),
	);
	fs::write(crate_dir.path().join("Cargo.toml"), manifest).expect("write fixture manifest");
	let build_script = r#"fn main() {
	println!("cargo::rustc-check-cfg=cfg(client)");
	println!("cargo::rustc-check-cfg=cfg(server)");
	if std::env::var_os("CARGO_FEATURE_SERVER").is_some() {
		println!("cargo::rustc-cfg=server");
	}
	if std::env::var("TARGET").is_ok_and(|target| target.starts_with("wasm32")) {
		println!("cargo::rustc-cfg=client");
	}
}
"#;
	fs::write(crate_dir.path().join("build.rs"), build_script).expect("write fixture build script");
	fs::copy(
		fixture_dir.join("src/lib.rs"),
		crate_dir.path().join("src/lib.rs"),
	)
	.expect("copy fixture source");

	let manifest_path = crate_dir.path().join("Cargo.toml");
	let target_path = target_dir.path().to_path_buf();
	let scenarios = [
		("native with server cfg", Some("server"), None),
		("native without server cfg", None, None),
		("WASM with client cfg", None, Some("wasm32-unknown-unknown")),
	];

	for (scenario, feature, target) in scenarios {
		let mut command = fixture_check_command(&manifest_path, &target_path);
		if let Some(feature) = feature {
			command.arg("--features").arg(feature);
		}
		if let Some(target) = target {
			command.arg("--target").arg(target);
		}
		let output = command
			.arg("--offline")
			.output()
			.expect("run UnifiedRouter target-parity fixture");

		assert!(
			output.status.success(),
			"{scenario} should compile\nstdout:\n{}\nstderr:\n{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr),
		);
	}
}

fn fixture_check_command(manifest_path: &Path, target_path: &Path) -> Command {
	let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
	command
		.arg("check")
		.arg("--manifest-path")
		.arg(manifest_path)
		.arg("--target-dir")
		.arg(target_path);
	command
}
