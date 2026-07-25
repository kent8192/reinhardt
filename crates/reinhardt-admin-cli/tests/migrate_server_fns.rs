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
		stdout(&output),
		"rewrote: src/apps/polls/urls/server_router.rs\n"
	);
	assert_eq!(
		fs::read_to_string(router).expect("read rewritten router"),
		r#"use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::server_fn;
use reinhardt::ServerRouter;

#[server_fn]
async fn local_status() {}

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.auto_server_fns(module_path!())
}
"#
	);
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
		"skipped unresolved marker `missing`: src/lib.rs:3\n"
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
		"skipped unresolved marker `duplicated`: src/lib.rs:11\n"
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
		"skipped mixed registration: src/lib.rs:3\n"
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
		.auto_server_fns(module_path!())
}
"#,
		)],
	);
	let source = fixture.path().join("src/lib.rs");
	let before = fs::read(&source).expect("read automatic source");

	let output = run_migrate(fixture.path(), true);

	assert_success(&output);
	assert_eq!(stdout(&output), "");
	assert_eq!(fs::read(source).expect("read unchanged source"), before);
}

#[test]
fn import_used_by_pub_use_is_preserved_after_marker_removal() {
	let fixture = prepare_fixture("safe");
	let router = router_path(fixture.path());
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
		fs::read_to_string(router).expect("read rewritten router"),
		r#"use crate::apps::polls::server_fn::get_questions;
pub use self::get_questions::marker as QuestionsMarker;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.auto_server_fns(module_path!())
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
		"skipped mixed registration: src/apps/polls/urls/server_router.rs:9\n"
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
		.auto_server_fns(module_path!())
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
		.auto_server_fns(module_path!())
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
		.auto_server_fns(module_path!())
}
"#
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
				r#"#[path = "support/child.rs"]
mod child;

pub fn server_url_patterns() {
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
		r#"#[path = "support/child.rs"]
mod child;

pub fn server_url_patterns() {
	router()
		.auto_server_fns(module_path!())
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
				r#"#[path = "support/lib.rs"]
mod support;

pub fn server_url_patterns() {
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
		r#"#[path = "support/lib.rs"]
mod support;

pub fn server_url_patterns() {
	router()
		.auto_server_fns(module_path!())
}
"#
	);
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
	let permissions = fs::metadata(&router)
		.expect("read router metadata")
		.permissions();
	let _restore = RestorePermissions {
		path: router.clone(),
		permissions: permissions.clone(),
	};
	let mut read_only = permissions;
	read_only.set_mode(0o444);
	fs::set_permissions(&router, read_only).expect("make router read-only");

	let output = run_migrate(fixture.path(), true);

	assert_failure(&output);
	assert_eq!(stdout(&output), "");
	assert!(
		stderr(&output).starts_with("error: failed to access `"),
		"unexpected IO error: {}",
		stderr(&output)
	);
}
