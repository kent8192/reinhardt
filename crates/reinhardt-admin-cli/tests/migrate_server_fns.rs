//! End-to-end tests for the `migrate-server-fns` command.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const REINHARDT_ADMIN: &str = env!("CARGO_BIN_EXE_reinhardt-admin");
const FIXTURES: &str = concat!(
	env!("CARGO_MANIFEST_DIR"),
	"/tests/fixtures/migrate_server_fns"
);

fn prepare_fixture(name: &str) -> TempDir {
	let temp = TempDir::new().expect("create temporary project");
	let fixture = Path::new(FIXTURES).join(name);
	copy_tree(&fixture.join("src"), &temp.path().join("src"));
	fs::write(
		temp.path().join("Cargo.toml"),
		format!(
			"[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n"
		),
	)
	.expect("write fixture manifest");
	temp
}

fn copy_tree(source: &Path, destination: &Path) {
	for entry in walkdir::WalkDir::new(source) {
		let entry = entry.expect("walk fixture tree");
		let relative = entry
			.path()
			.strip_prefix(source)
			.expect("relative fixture path");
		let target = destination.join(relative);
		if entry.file_type().is_dir() {
			fs::create_dir_all(&target).expect("create fixture directory");
		} else {
			fs::copy(entry.path(), &target).expect("copy fixture file");
		}
	}
}

fn router_path(root: &Path) -> PathBuf {
	root.join("src/apps/polls/urls/server_router.rs")
}

fn run_migrate(root: &Path, write: bool) -> Output {
	let mut command = Command::new(REINHARDT_ADMIN);
	command.arg("migrate-server-fns").arg(root);
	if write {
		command.arg("--write");
	}
	command
		.output()
		.expect("run reinhardt-admin migrate-server-fns")
}

fn assert_success(output: &Output) {
	assert!(
		output.status.success(),
		"command failed with exit {:?}\nstdout:\n{}\nstderr:\n{}",
		output.status.code(),
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

fn stdout(output: &Output) -> String {
	String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

#[test]
fn dry_run_reports_safe_router_without_changing_bytes() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	let before = fs::read(&router).expect("read safe router");

	let output = run_migrate(fixture.path(), false);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"would rewrite: src/apps/polls/urls/server_router.rs\n"
	);
	assert_eq!(fs::read(router).expect("read router after dry-run"), before);
}

#[test]
fn write_replaces_explicit_markers_and_preserves_unrelated_builder_calls() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	let expected = fs::read(
		Path::new(FIXTURES)
			.join("safe")
			.join("expected/server_router.rs"),
	)
	.expect("read expected router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"rewrote: src/apps/polls/urls/server_router.rs\n"
	);
	assert_eq!(fs::read(router).expect("read rewritten router"), expected);
}

#[test]
fn mixed_opted_out_router_is_skipped_without_changing_bytes() {
	let fixture = prepare_fixture("mixed");
	let router = router_path(fixture.path());
	let before = fs::read(&router).expect("read mixed router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/apps/polls/urls/server_router.rs:8\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn unresolved_alias_is_skipped_without_changing_bytes() {
	let fixture = prepare_fixture("unresolved");
	let router = router_path(fixture.path());
	let before = fs::read(&router).expect("read unresolved router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped unresolved marker `missing_alias`: src/apps/polls/urls/server_router.rs:7\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn write_is_idempotent() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());

	let first = run_migrate(fixture.path(), true);
	assert_success(&first);
	let after_first = fs::read(&router).expect("read router after first rewrite");

	let second = run_migrate(fixture.path(), true);

	assert_success(&second);
	assert_eq!(stdout(&second), "");
	assert_eq!(
		fs::read(router).expect("read router after second rewrite"),
		after_first
	);
}
