//! End-to-end tests for the `migrate-server-fns` command.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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

fn write_file(root: &Path, relative: &str, contents: &str) {
	let path = root.join(relative);
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).expect("create source directory");
	}
	fs::write(path, contents).expect("write project file");
}

fn prepare_project(name: &str, manifest_targets: &str, files: &[(&str, &str)]) -> TempDir {
	let temp = TempDir::new().expect("create temporary project");
	write_file(
		temp.path(),
		"Cargo.toml",
		&format!(
			"[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{manifest_targets}"
		),
	);
	for (path, contents) in files {
		write_file(temp.path(), path, contents);
	}
	temp
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

fn assert_failure(output: &Output) {
	assert!(
		!output.status.success(),
		"command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

fn stdout(output: &Output) -> String {
	String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> String {
	String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
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

#[cfg(unix)]
#[test]
fn write_preserves_source_permissions() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	fs::set_permissions(&router, fs::Permissions::from_mode(0o640))
		.expect("set source permissions");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		fs::metadata(router)
			.expect("read rewritten source metadata")
			.permissions()
			.mode() & 0o777,
		0o640
	);
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

#[test]
fn resolves_alias_self_and_repeated_super_markers() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::get_questions as questions;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::server_fn;
use reinhardt::ServerRouter;

#[server_fn]
async fn local_status() {}

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(questions::marker)
		.server_fn(self::local_status::marker)
		.server_fn(super::super::server_fn::vote::marker)
}
"#,
	);

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		fs::read_to_string(router).expect("read rewritten router"),
		r#"use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::server_fn;
use reinhardt::ServerRouter;

#[server_fn]
async fn local_status() {}

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
"#
	);
}

#[test]
fn inherited_server_function_alias_makes_nested_coverage_complete() {
	let fixture = prepare_project(
		"inherited_server_fn_alias",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"use reinhardt::{app_config, server_fn, ServerRouter};
use reinhardt::pages::server_fn as sf;

#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[server_fn]
pub async fn status() {}

mod child {
	use super::sf;

	#[sf]
	pub async fn hidden() {}
}

pub fn server_url_patterns() -> ServerRouter {
	router()
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(
		stdout(&output).starts_with("skipped mixed registration: src/lib.rs:"),
		"inherited server_fn aliases must make coverage incomplete when unregistered: {}",
		stdout(&output)
	);
	assert_eq!(fs::read(&source).expect("read skipped source"), before);
}

#[test]
fn glob_import_is_reported_as_unresolved_and_left_byte_identical() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::*;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(get_questions::marker)
}
"#,
	);
	let before = fs::read(&router).expect("read glob router");

	let output = run_migrate(fixture.path(), false);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped unresolved marker `get_questions`: src/apps/polls/urls/server_router.rs:7\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn root_escape_is_reported_as_unresolved_and_left_byte_identical() {
	let fixture = prepare_project(
		"root_escape",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"pub fn server_url_patterns() {
	router()
		.server_fn(super::missing::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read root escape source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/lib.rs:1\n"
	);
	assert_eq!(fs::read(source).expect("read skipped source"), before);
}

#[test]
fn ambiguous_server_function_is_reported_and_left_byte_identical() {
	let fixture = prepare_project(
		"ambiguous",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[server_fn]
#[cfg(feature = "first")]
async fn duplicated() {}

#[server_fn]
#[cfg(not(feature = "first"))]
async fn duplicated() {}

pub fn server_url_patterns() {
	router()
		.server_fn(duplicated::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read ambiguous source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/lib.rs:9\n"
	);
	assert_eq!(fs::read(source).expect("read skipped source"), before);
}

#[test]
fn server_fnset_chain_is_reported_as_mixed_and_left_byte_identical() {
	let fixture = prepare_project(
		"server_fnset",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"pub fn server_url_patterns() {
	router()
		.server_fnset(manual_set())
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read server_fnset source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/lib.rs:1\n"
	);
	assert_eq!(fs::read(source).expect("read skipped source"), before);
}

#[test]
fn pre_existing_automatic_chain_is_silent_and_left_byte_identical() {
	let fixture = prepare_project(
		"already_automatic",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[server_fn]
async fn ready() {}

pub fn server_url_patterns() {
	router()
		.server_fn(ready::marker)
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read automatic source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/lib.rs:4\n"
	);
	assert_eq!(fs::read(source).expect("read unchanged source"), before);
}

#[test]
fn import_used_by_pub_use_is_preserved_after_marker_removal() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/server_fn.rs",
		r#"use reinhardt::server_fn;

#[server_fn]
pub async fn get_questions() {}
"#,
	);
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::get_questions;
pub use self::get_questions::marker as QuestionsMarker;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(get_questions::marker)
}
"#,
	);

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"rewrote: src/apps/polls/urls/server_router.rs\n"
	);
	assert_eq!(
		fs::read_to_string(router).expect("read rewritten router"),
		r#"use crate::apps::polls::server_fn::get_questions;
pub use self::get_questions::marker as QuestionsMarker;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
"#
	);
}

#[test]
fn nested_opted_out_chain_skips_the_whole_router_function() {
	let fixture = prepare_fixture("mixed");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::{automatic, manual};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.mount("/nested", ServerRouter::new().server_fn(manual::marker))
		.server_fn(automatic::marker)
}
"#,
	);
	let before = fs::read(&router).expect("read nested mixed router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/apps/polls/urls/server_router.rs:7\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn nested_unresolved_chain_skips_the_whole_router_function() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::automatic;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.mount("/nested", ServerRouter::new().server_fn(missing::marker))
		.server_fn(automatic::marker)
}
"#,
	);
	write_file(
		fixture.path(),
		"src/apps/polls/server_fn.rs",
		r#"use reinhardt::server_fn;

#[server_fn]
pub async fn automatic() {}
"#,
	);
	let before = fs::read(&router).expect("read nested unresolved router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/apps/polls/urls/server_router.rs:7\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn nested_safe_chains_are_skipped_to_preserve_mount_topology() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::{get_questions, vote};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.mount(
			"/nested",
			ServerRouter::new().server_fn(get_questions::marker),
		)
		.server_fn(vote::marker)
}
"#,
	);
	let before = fs::read(&router).expect("read nested safe router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/apps/polls/urls/server_router.rs:9\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn multiple_router_values_are_skipped_byte_identical() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::{get_questions, vote};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	let first = ServerRouter::new()
		.server_fn(get_questions::marker);
	ServerRouter::new()
		.server_fn(vote::marker)
}
"#,
	);
	let before = fs::read(&router).expect("read multiple router values");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/apps/polls/urls/server_router.rs:5\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn branch_router_values_are_skipped_byte_identical() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::{get_questions, vote};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns(condition: bool) -> ServerRouter {
	if condition {
		ServerRouter::new()
			.server_fn(get_questions::marker)
	} else {
		ServerRouter::new()
			.server_fn(vote::marker)
	}
}
"#,
	);
	let before = fs::read(&router).expect("read branch router values");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/apps/polls/urls/server_router.rs:11\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn macro_token_use_preserves_import_after_marker_removal() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/server_fn.rs",
		r#"use reinhardt::server_fn;

#[server_fn]
pub async fn vote() {}
"#,
	);
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::vote;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

macro_rules! keep_marker {
	($marker:path) => {};
}

pub fn server_url_patterns() -> ServerRouter {
	keep_marker!(vote::marker);
	ServerRouter::new()
		.server_fn(vote::marker)
}
"#,
	);

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		fs::read_to_string(router).expect("read rewritten router"),
		r#"use crate::apps::polls::server_fn::vote;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

macro_rules! keep_marker {
	($marker:path) => {};
}

pub fn server_url_patterns() -> ServerRouter {
	keep_marker!(vote::marker);
	ServerRouter::new()
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
"#
	);
}

#[test]
fn attribute_token_use_preserves_import_after_marker_removal() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/server_fn.rs",
		r#"use reinhardt::server_fn;

#[server_fn]
pub async fn vote() {}
"#,
	);
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::vote;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

#[keep_marker(vote::marker)]
fn retained_attribute() {}

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(vote::marker)
}
"#,
	);

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		fs::read_to_string(router).expect("read rewritten router"),
		r#"use crate::apps::polls::server_fn::vote;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

#[keep_marker(vote::marker)]
fn retained_attribute() {}

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
"#
	);
}

#[test]
fn grouped_self_import_is_removed_with_its_effective_binding() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/server_fn.rs",
		r#"use reinhardt::server_fn;

#[server_fn]
pub async fn get_questions() {}
"#,
	);
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::{self};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(server_fn::get_questions::marker)
}
"#,
	);

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		fs::read_to_string(router).expect("read rewritten router"),
		r#"use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
"#
	);
}

#[test]
fn router_without_app_config_is_skipped_without_changing_bytes() {
	// Arrange
	let fixture = prepare_project(
		"unowned_router",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[server_fn]
pub async fn status() {}

pub fn server_url_patterns() {
	router()
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read unowned router");

	// Act
	let output = run_migrate(fixture.path(), true);

	// Assert
	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/lib.rs:4\n"
	);
	assert_eq!(fs::read(source).expect("read skipped router"), before);
}

#[test]
fn router_is_skipped_when_the_server_function_has_another_app_owner() {
	// Arrange
	let fixture = prepare_project(
		"incompatible_owner",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[app_config(name = "root", label = "root")]
pub struct RootConfig;

pub mod other {
	#[app_config(name = "other", label = "other")]
	pub struct OtherConfig;

	#[server_fn]
	pub async fn status() {}
}

pub fn server_url_patterns() {
	router()
		.server_fn(other::status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read incompatible router");

	// Act
	let output = run_migrate(fixture.path(), true);

	// Assert
	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/lib.rs:12\n"
	);
	assert_eq!(fs::read(source).expect("read skipped router"), before);
}

#[test]
fn partial_router_chains_for_one_app_are_skipped_byte_identical() {
	// Arrange
	let fixture = prepare_project(
		"partial_router_chains",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[app_config(name = "root", label = "root")]
pub struct RootConfig;

mod public {
	#[server_fn]
	pub async fn status() {}

	pub fn server_url_patterns() {
		router()
			.server_fn(status::marker)
	}
}

mod admin {
	#[server_fn]
	pub async fn metrics() {}

	pub fn server_url_patterns() {
		router()
			.server_fn(metrics::marker)
	}
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read partial router chains");

	// Act
	let output = run_migrate(fixture.path(), true);

	// Assert
	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/lib.rs:8\nskipped mixed registration: src/lib.rs:18\n"
	);
	assert_eq!(
		fs::read(source).expect("read skipped router chains"),
		before
	);
}

#[test]
fn nonstandard_target_root_resolves_child_module_from_target_parent() {
	let fixture = prepare_project(
		"nonstandard_target",
		"[[test]]\nname = \"social\"\npath = \"tests/social.rs\"\n",
		&[
			("src/lib.rs", ""),
			(
				"tests/social.rs",
				r#"use reinhardt::ServerRouter;

#[app_config(name = "social", label = "social")]
pub struct SocialConfig;

#[path = "support/child.rs"]
mod child;

pub fn server_url_patterns() -> ServerRouter {
	router()
		.server_fn(child::status::marker)
}
"#,
			),
			(
				"tests/support/child.rs",
				r#"#[server_fn]
pub async fn status() {}
"#,
			),
		],
	);
	let source = fixture.path().join("tests/social.rs");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(stdout(&output), "rewrote: tests/social.rs\n");
	assert_eq!(
		fs::read_to_string(source).expect("read rewritten target"),
		r#"use reinhardt::ServerRouter;

#[app_config(name = "social", label = "social")]
pub struct SocialConfig;

#[path = "support/child.rs"]
mod child;

pub fn server_url_patterns() -> ServerRouter {
	router()
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
"#
	);
}

#[test]
fn path_module_named_lib_uses_non_root_child_directory_rules() {
	let fixture = prepare_project(
		"path_module_lib",
		"[[test]]\nname = \"social\"\npath = \"tests/social.rs\"\n",
		&[
			("src/lib.rs", ""),
			(
				"tests/social.rs",
				r#"use reinhardt::ServerRouter;

#[app_config(name = "social", label = "social")]
pub struct SocialConfig;

#[path = "support/lib.rs"]
mod support;

pub fn server_url_patterns() -> ServerRouter {
	router()
		.server_fn(support::child::status::marker)
}
"#,
			),
			("tests/support/lib.rs", "pub mod child;\n"),
			(
				"tests/support/lib/child.rs",
				r#"#[server_fn]
pub async fn status() {}
"#,
			),
		],
	);
	let source = fixture.path().join("tests/social.rs");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(stdout(&output), "rewrote: tests/social.rs\n");
	assert_eq!(
		fs::read_to_string(source).expect("read rewritten target"),
		r#"use reinhardt::ServerRouter;

#[app_config(name = "social", label = "social")]
pub struct SocialConfig;

#[path = "support/lib.rs"]
mod support;

pub fn server_url_patterns() -> ServerRouter {
	router()
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
"#
	);
}

#[test]
fn unavailable_text_edits_preserve_comments_and_skip_the_migration() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::{get_questions, vote};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		. // Keep this registration comment
		server_fn(get_questions::marker)
		.server_fn(vote::marker)
}
"#,
	);
	let before = fs::read(&router).expect("read router with a comment");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped migration because text edits could not be applied: src/apps/polls/urls/server_router.rs\n"
	);
	assert_eq!(
		fs::read(&router).expect("read skipped router"),
		before,
		"a text-edit failure must not fall back to a comment-dropping formatter"
	);
}

#[test]
fn import_pruning_with_comments_skips_the_entire_migration() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::{get_questions, vote, /* Keep this import comment. */ retained};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(get_questions::marker)
		.server_fn(vote::marker)
}
"#,
	);
	let before = fs::read(&router).expect("read router with an import comment");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped migration because text edits could not be applied: src/apps/polls/urls/server_router.rs\n"
	);
	assert_eq!(fs::read(&router).expect("read skipped router"), before);
}

#[test]
fn absolute_marker_path_is_left_unresolved() {
	let fixture = prepare_project(
		"absolute_marker_path",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"use reinhardt::{app_config, server_fn, ServerRouter};

#[app_config(name = "root", label = "root")]
pub struct RootConfig;

pub mod dependency {
	pub mod vote {
		use super::super::server_fn;

		#[server_fn]
		pub async fn endpoint() {}
	}
}

pub fn server_url_patterns() -> ServerRouter {
	router()
		.server_fn(::dependency::vote::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(
		stdout(&output).starts_with("skipped unresolved marker `vote`: src/lib.rs:"),
		"absolute marker paths must not be mistaken for local markers: {}",
		stdout(&output)
	);
	assert_eq!(fs::read(&source).expect("read skipped source"), before);
}

#[test]
fn cfg_gated_app_ownership_skips_the_migration() {
	let fixture = prepare_project(
		"cfg_gated_ownership",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[cfg(feature = "pages")]
#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[server_fn]
pub async fn status() {}

pub fn server_url_patterns() {
	router()
		.server_fn(status::marker)
}

"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read cfg-gated router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/lib.rs:8\n"
	);
	assert_eq!(fs::read(source).expect("read skipped router"), before);
}

#[test]
fn cfg_gated_server_function_skips_the_migration() {
	let fixture = prepare_project(
		"cfg_gated_server_function",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[server_fn]
pub async fn status() {}

#[cfg(feature = "extra")]
#[server_fn]
pub async fn extra() {}

pub fn server_url_patterns() {
	router()
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read cfg-gated server function source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(
		stdout(&output).starts_with("skipped mixed registration: src/lib.rs:"),
		"conditionally compiled server functions must make coverage incomplete: {}",
		stdout(&output)
	);
	assert_eq!(fs::read(source).expect("read skipped source"), before);
}

#[test]
fn unknown_qualified_server_function_attribute_skips_the_migration() {
	let fixture = prepare_project(
		"renamed_reinhardt_dependency",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[server_fn]
pub async fn status() {}

#[rh::pages::server_fn]
pub async fn hidden() {}

pub fn server_url_patterns() {
	router()
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read renamed dependency source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(
		stdout(&output).starts_with("skipped mixed registration: src/lib.rs:"),
		"unknown qualified server_fn paths must keep migration coverage incomplete: {}",
		stdout(&output)
	);
	assert_eq!(fs::read(source).expect("read skipped source"), before);
}

#[test]
fn cfg_gated_module_skips_the_migration() {
	let fixture = prepare_project(
		"cfg_gated_module",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"use reinhardt::{app_config, server_fn};

#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[server_fn]
pub async fn status() {}

#[cfg(feature = "extra")]
mod extra {
	#[server_fn]
	pub async fn deferred() {}
}

pub fn server_url_patterns() {
	router()
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read cfg-gated module source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(
		stdout(&output).starts_with("skipped mixed registration: src/lib.rs:"),
		"conditionally compiled modules must make coverage incomplete: {}",
		stdout(&output)
	);
	assert_eq!(fs::read(source).expect("read skipped source"), before);
}

#[test]
fn renamed_dependency_server_function_alias_skips_the_migration() {
	let fixture = prepare_project(
		"renamed_server_fn_alias",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"use reinhardt::{app_config, server_fn};
use rh::pages::server_fn as sf;

#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[server_fn]
pub async fn status() {}

#[sf]
pub async fn hidden() {}

pub fn server_url_patterns() {
	router()
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read renamed alias source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(
		stdout(&output).starts_with("skipped mixed registration: src/lib.rs:"),
		"unresolved server_fn aliases must make coverage incomplete: {}",
		stdout(&output)
	);
	assert_eq!(fs::read(source).expect("read skipped source"), before);
}

#[test]
fn foreign_unqualified_server_function_attribute_skips_the_migration() {
	let fixture = prepare_project(
		"foreign_server_fn",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"use other_framework::server_fn;
use reinhardt::{app_config, server_fn as reinhardt_server_fn};

#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[reinhardt_server_fn]
pub async fn status() {}

#[server_fn]
pub async fn foreign() {}

pub fn server_url_patterns() {
	router()
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read foreign server_fn source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(
		stdout(&output).starts_with("skipped mixed registration: src/lib.rs:"),
		"foreign server_fn attributes must make coverage incomplete: {}",
		stdout(&output)
	);
	assert_eq!(fs::read(source).expect("read skipped source"), before);
}

#[test]
fn shared_source_is_skipped_during_dry_runs_and_writes() {
	let fixture = prepare_project(
		"shared_source",
		"[lib]\npath = \"src/lib.rs\"\n\n[[bin]]\nname = \"shared-source\"\npath = \"src/lib.rs\"\n",
		&[
			("src/lib.rs", "pub mod apps { pub mod polls; }\n"),
			(
				"src/apps/polls.rs",
				r#"pub mod server_fn;

#[app_config(name = "polls", label = "polls")]
pub struct PollsConfig;

pub mod urls {
	pub mod server_router;
}
"#,
			),
			(
				"src/apps/polls/server_fn.rs",
				r#"use reinhardt::server_fn;

#[server_fn]
pub async fn status() {}
"#,
			),
			(
				"src/apps/polls/urls/server_router.rs",
				r#"use crate::apps::polls::server_fn::status;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new().server_fn(status::marker)
}
"#,
			),
		],
	);
	let source = fixture.path().join("src/apps/polls/urls/server_router.rs");
	let before = fs::read(&source).expect("read shared source");
	let expected = "skipped incompatible app ownership: src/apps/polls/urls/server_router.rs:0\n";

	let dry_run = run_migrate(fixture.path(), false);
	assert_success(&dry_run);
	assert_eq!(stdout(&dry_run), expected);
	assert_eq!(
		fs::read(&source).expect("read source after dry-run"),
		before
	);

	let write = run_migrate(fixture.path(), true);
	assert_success(&write);
	assert_eq!(stdout(&write), expected);
	assert_eq!(fs::read(source).expect("read source after write"), before);
}
#[test]
fn route_middleware_after_a_marker_skips_the_migration() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	write_file(
		fixture.path(),
		"src/apps/polls/urls/server_router.rs",
		r#"use crate::apps::polls::server_fn::{get_questions, vote};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(get_questions::marker)
		.server_fn(vote::marker)
		.with_route_middleware(Auth)
}
"#,
	);
	let before = fs::read(&router).expect("read middleware router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(
		stdout(&output),
		"skipped mixed registration: src/apps/polls/urls/server_router.rs:9\n"
	);
	assert_eq!(fs::read(router).expect("read skipped router"), before);
}

#[test]
fn metadata_failure_exits_nonzero() {
	let missing_manifest = TempDir::new().expect("create metadata failure directory");
	let metadata_output = run_migrate(missing_manifest.path(), false);
	assert_failure(&metadata_output);
	assert!(
		stderr(&metadata_output).starts_with("error: failed to load Cargo metadata:"),
		"unexpected metadata error: {}",
		stderr(&metadata_output)
	);
}

#[test]
fn generic_router_function_is_skipped_without_changing_bytes() {
	let fixture = prepare_project(
		"generic_router",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[server_fn]
pub async fn status() {}

pub fn server_url_patterns<T: MarkerProvider>() -> ServerRouter {
	router()
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read generic router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(stdout(&output).starts_with("skipped mixed registration:"));
	assert_eq!(fs::read(source).expect("read skipped router"), before);
}

#[test]
fn duplicate_server_function_marker_is_skipped_without_changing_bytes() {
	let fixture = prepare_project(
		"duplicate_marker",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[(
			"src/lib.rs",
			r#"#[app_config(name = "root", label = "root")]
pub struct RootConfig;

#[server_fn]
pub async fn status() {}

pub fn server_url_patterns() -> ServerRouter {
	router()
		.server_fn(status::marker)
		.server_fn(status::marker)
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read duplicate router");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert!(stdout(&output).starts_with("skipped mixed registration:"));
	assert_eq!(fs::read(source).expect("read skipped router"), before);
}

#[test]
fn parse_failure_exits_nonzero() {
	let invalid_source = prepare_project(
		"invalid_source",
		"[lib]\npath = \"src/lib.rs\"\n",
		&[("src/lib.rs", "pub fn broken( {\n")],
	);
	let parse_output = run_migrate(invalid_source.path(), false);
	assert_failure(&parse_output);
	assert!(
		stderr(&parse_output).starts_with("error: failed to parse `"),
		"unexpected parse error: {}",
		stderr(&parse_output)
	);
}

#[cfg(unix)]
#[test]
fn write_io_failure_exits_nonzero() {
	struct RestorePermissions {
		path: PathBuf,
		permissions: fs::Permissions,
	}

	impl Drop for RestorePermissions {
		fn drop(&mut self) {
			fs::set_permissions(&self.path, self.permissions.clone())
				.expect("restore router permissions");
		}
	}

	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
	let directory = router
		.parent()
		.expect("router should have a parent directory")
		.to_path_buf();
	let permissions = fs::metadata(&directory)
		.expect("read router directory metadata")
		.permissions();
	let _restore = RestorePermissions {
		path: directory.clone(),
		permissions: permissions.clone(),
	};
	let mut read_only = permissions;
	read_only.set_mode(0o555);
	fs::set_permissions(&directory, read_only).expect("make router directory read-only");

	let output = run_migrate(fixture.path(), true);

	assert_failure(&output);
	assert_eq!(stdout(&output), "");
	assert!(
		stderr(&output).starts_with("error: failed to access `"),
		"unexpected IO error: {}",
		stderr(&output)
	);
}
