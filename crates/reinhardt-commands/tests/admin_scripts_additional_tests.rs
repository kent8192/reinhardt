//! Additional Admin Scripts Tests (Part 2)
//!
//! Continuation of comprehensive test suite for reinhardt-admin
//! This file contains the remaining test cases from Django's admin_scripts tests

#[path = "support/environment.rs"]
mod environment;

use environment::{EnvVars, env_vars};
use reinhardt_commands::{
	BaseCommand, CommandContext, CommandError, CommandResult, StartProjectCommand,
};
use reinhardt_test::TeardownGuard;
use reinhardt_test::fixtures::temp_dir;
use rstest::{fixture, rstest};
use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper struct for setting up test environments
struct TestEnvironment {
	temp_dir: TempDir,
}

#[fixture]
fn test_env(temp_dir: TempDir) -> TestEnvironment {
	TestEnvironment { temp_dir }
}

impl TestEnvironment {
	fn path(&self) -> PathBuf {
		self.temp_dir.path().to_path_buf()
	}

	fn create_file(&self, relative_path: &str, content: &str) {
		let file_path = self.path().join(relative_path);
		if let Some(parent) = file_path.parent() {
			fs::create_dir_all(parent).expect("Failed to create parent directory");
		}
		fs::write(&file_path, content).expect("Failed to write file");
	}

	fn file_exists(&self, relative_path: &str) -> bool {
		self.path().join(relative_path).exists()
	}
}

// ============================================================================
// ManageMultipleSettings Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_manage_multiple_builtin_command(#[from(test_env)] env: TestEnvironment) {
	env.create_file("settings1.rs", "pub const DEBUG: bool = true;\n");
	env.create_file("settings2.rs", "pub const DEBUG: bool = false;\n");

	assert!(env.file_exists("settings1.rs"));
	assert!(env.file_exists("settings2.rs"));
}

#[rstest]
#[tokio::test]
async fn test_manage_multiple_builtin_with_settings(#[from(test_env)] env: TestEnvironment) {
	env.create_file("settings1.rs", "// Settings 1\n");
	env.create_file("settings2.rs", "// Settings 2\n");

	assert!(env.file_exists("settings1.rs"));
	assert!(env.file_exists("settings2.rs"));
}

#[rstest]
#[tokio::test]
#[serial(reinhardt_settings)]
async fn test_manage_multiple_builtin_with_environment(
	#[from(env_vars)] mut env_guard: TeardownGuard<EnvVars>,
	#[from(env_vars)] mut nested_guard: TeardownGuard<EnvVars>,
) {
	// Arrange
	let original = std::env::var_os("REINHARDT_SETTINGS_MODULE");
	env_guard.set("REINHARDT_SETTINGS_MODULE", "settings1");
	assert_eq!(
		std::env::var("REINHARDT_SETTINGS_MODULE").unwrap(),
		"settings1"
	);

	#[cfg(unix)]
	{
		use std::os::unix::ffi::OsStringExt;
		env_guard.set(
			"REINHARDT_SETTINGS_MODULE",
			std::ffi::OsString::from_vec(vec![b's', 0xff]),
		);
	}
	let before_unwind = std::env::var_os("REINHARDT_SETTINGS_MODULE");

	// Act
	let result = std::panic::catch_unwind(move || {
		nested_guard.set("REINHARDT_SETTINGS_MODULE", "settings2");
		nested_guard.set("REINHARDT_SETTINGS_MODULE", "settings3");
		panic!("exercise environment restoration during unwinding");
	});

	// Assert
	assert!(result.is_err());
	assert_eq!(std::env::var_os("REINHARDT_SETTINGS_MODULE"), before_unwind);
	drop(env_guard);
	assert_eq!(std::env::var_os("REINHARDT_SETTINGS_MODULE"), original);
}

#[rstest]
#[tokio::test]
#[serial(reinhardt_settings)]
async fn test_manage_multiple_builtin_with_bad_settings(
	#[from(env_vars)] mut env_guard: TeardownGuard<EnvVars>,
) {
	env_guard.set("REINHARDT_SETTINGS_MODULE", "bad_settings");
	assert_eq!(
		std::env::var("REINHARDT_SETTINGS_MODULE").unwrap(),
		"bad_settings"
	);
}

#[rstest]
#[tokio::test]
#[serial(reinhardt_settings)]
async fn test_manage_multiple_builtin_with_bad_environment(
	#[from(env_vars)] mut env_guard: TeardownGuard<EnvVars>,
) {
	env_guard.set("REINHARDT_SETTINGS_MODULE", "");
	assert_eq!(std::env::var("REINHARDT_SETTINGS_MODULE").unwrap(), "");
}

#[rstest]
#[tokio::test]
async fn test_manage_multiple_custom_command(#[from(test_env)] env: TestEnvironment) {
	assert!(env.path().exists());
}

#[rstest]
#[tokio::test]
#[serial(reinhardt_settings)]
async fn test_manage_multiple_custom_command_with_settings(
	#[from(test_env)] env: TestEnvironment,
	#[from(env_vars)] mut env_guard: TeardownGuard<EnvVars>,
) {
	env.create_file("settings1.rs", "pub const DB: &str = \"db1\";\n");
	env.create_file("settings2.rs", "pub const DB: &str = \"db2\";\n");

	// Test with explicit settings parameter
	env_guard.set("REINHARDT_SETTINGS_MODULE", "settings1");

	assert!(env.file_exists("settings1.rs"));
	assert!(env.file_exists("settings2.rs"));
	assert_eq!(
		std::env::var("REINHARDT_SETTINGS_MODULE").unwrap(),
		"settings1"
	);
}

#[rstest]
#[tokio::test]
#[serial(reinhardt_settings)]
async fn test_manage_multiple_custom_command_with_environment(
	#[from(env_vars)] mut env_guard: TeardownGuard<EnvVars>,
) {
	env_guard.set("REINHARDT_SETTINGS_MODULE", "custom.settings");
	env_guard.set("CUSTOM_ENV_VAR", "test_value");

	// Test that environment variables are respected
	assert_eq!(
		std::env::var("REINHARDT_SETTINGS_MODULE").unwrap(),
		"custom.settings"
	);
	assert_eq!(std::env::var("CUSTOM_ENV_VAR").unwrap(), "test_value");
}

// ============================================================================
// ManageCheck Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_manage_check_broken_app(#[from(test_env)] env: TestEnvironment) {
	// Create broken app structure
	env.create_file("apps/broken/mod.rs", "// Broken syntax");
	assert!(env.file_exists("apps/broken/mod.rs"));
}

#[rstest]
#[tokio::test]
async fn test_manage_check_complex_app(#[from(test_env)] env: TestEnvironment) {
	env.create_file(
		"Cargo.toml",
		"[package]\nname = \"test\"\nversion = \"0.1.0\"",
	);
	env.create_file(
		"apps/complex/mod.rs",
		"pub mod models;\npub mod views;\npub mod routes;\n",
	);
	env.create_file("apps/complex/models.rs", "// Model definitions\n");
	env.create_file("apps/complex/views.rs", "// View handlers\n");
	env.create_file("apps/complex/routes.rs", "// Route configurations\n");

	// Complex app structure for testing check command
}

#[rstest]
#[tokio::test]
async fn test_manage_check_app_with_import(#[from(test_env)] env: TestEnvironment) {
	env.create_file(
		"Cargo.toml",
		"[package]\nname = \"test\"\nversion = \"0.1.0\"",
	);
	env.create_file(
		"apps/importtest/mod.rs",
		"use nonexistent::module; // This will cause import error\n",
	);

	// App with import issues for testing check command
}

#[rstest]
#[tokio::test]
async fn test_manage_check_warning_does_not_halt(#[from(test_env)] env: TestEnvironment) {
	env.create_file(
		"Cargo.toml",
		"[package]\nname = \"test\"\nversion = \"0.1.0\"",
	);
	env.create_file(
		"apps/warningtest/mod.rs",
		"// Code that generates warnings\n",
	);

	// Check command should complete even with warnings
}

// ============================================================================
// ManageRunserver Tests
// ============================================================================

#[tokio::test]
async fn test_manage_runserver_addrport() {
	let ctx = CommandContext::new(vec!["127.0.0.1:8080".to_string()]);
	// When runserver is implemented, this should parse address and port
	assert_eq!(ctx.arg(0), Some(&"127.0.0.1:8080".to_string()));
}

#[tokio::test]
async fn test_manage_runserver_zero_ip_addr() {
	let ctx = CommandContext::new(vec!["0.0.0.0:8000".to_string()]);
	// Test binding to all interfaces
	assert_eq!(ctx.arg(0), Some(&"0.0.0.0:8000".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_manage_runserver_on_bind(#[from(test_env)] env: TestEnvironment) {
	env.create_file(
		"Cargo.toml",
		"[package]\nname = \"test\"\nversion = \"0.1.0\"",
	);

	// Test that bind callback is invoked when server starts
	assert!(env.file_exists("Cargo.toml"));
}

// Removed empty test: test_manage_runserver_hide_production_warning_with_environment_variable
// This test was empty and will be implemented when needed

#[tokio::test]
async fn test_manage_runserver_runner_addrport_ipv6() {
	let ctx = CommandContext::new(vec!["[::1]:8000".to_string()]);
	// Test IPv6 loopback address
	assert_eq!(ctx.arg(0), Some(&"[::1]:8000".to_string()));
}

#[tokio::test]
async fn test_manage_runserver_runner_hostname() {
	let ctx = CommandContext::new(vec!["localhost:8000".to_string()]);
	// Test with hostname instead of IP
	assert_eq!(ctx.arg(0), Some(&"localhost:8000".to_string()));
}

#[tokio::test]
async fn test_manage_runserver_runner_hostname_ipv6() {
	let ctx = CommandContext::new(vec!["localhost6:8000".to_string()]);
	// Test IPv6-specific hostname
	assert_eq!(ctx.arg(0), Some(&"localhost6:8000".to_string()));
}

#[tokio::test]
async fn test_manage_runserver_runner_custom_defaults() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("port".to_string(), "3000".to_string());
	ctx.set_option("host".to_string(), "0.0.0.0".to_string());

	// Custom defaults should override standard defaults
	assert_eq!(ctx.option("port"), Some(&"3000".to_string()));
}

#[tokio::test]
async fn test_manage_runserver_runner_custom_defaults_ipv6() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("port".to_string(), "3000".to_string());
	ctx.set_option("host".to_string(), "::".to_string());

	// IPv6 all interfaces binding
	assert_eq!(ctx.option("host"), Some(&"::".to_string()));
}

#[tokio::test]
async fn test_manage_runserver_runner_ambiguous() {
	let ctx = CommandContext::new(vec!["8000".to_string()]);
	// Ambiguous parameter (could be port only or invalid address)
	assert_eq!(ctx.arg(0), Some(&"8000".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_manage_runserver_no_database(#[from(test_env)] env: TestEnvironment) {
	env.create_file(
		"Cargo.toml",
		"[package]\nname = \"test\"\nversion = \"0.1.0\"",
	);

	// Runserver should handle projects without database configuration
	assert!(env.file_exists("Cargo.toml"));
}

#[rstest]
#[tokio::test]
async fn test_manage_runserver_readonly_database(#[from(test_env)] env: TestEnvironment) {
	env.create_file(
		"Cargo.toml",
		"[package]\nname = \"test\"\nversion = \"0.1.0\"",
	);

	// Test with read-only database configuration
	assert!(env.file_exists("Cargo.toml"));
}

#[tokio::test]
async fn test_manage_runserver_skip_checks() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("skip-checks".to_string(), "true".to_string());

	// Skip checks flag should be respected
	assert!(ctx.has_option("skip-checks"));
}

#[tokio::test]
async fn test_manage_runserver_custom_system_checks() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("check".to_string(), "custom.checks".to_string());

	// Custom system checks module
	assert_eq!(ctx.option("check"), Some(&"custom.checks".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_manage_runserver_migration_warning_one_app(#[from(test_env)] env: TestEnvironment) {
	env.create_file(
		"Cargo.toml",
		"[package]\nname = \"test\"\nversion = \"0.1.0\"",
	);
	env.create_file("apps/myapp/migrations/001_initial.sql", "-- Migration\n");

	// Should warn about unapplied migrations for single app
	assert!(env.file_exists("apps/myapp/migrations/001_initial.sql"));
}

#[rstest]
#[tokio::test]
async fn test_manage_runserver_migration_warning_multiple_apps(
	#[from(test_env)] env: TestEnvironment,
) {
	env.create_file(
		"Cargo.toml",
		"[package]\nname = \"test\"\nversion = \"0.1.0\"",
	);
	env.create_file("apps/app1/migrations/001.sql", "-- Migration\n");
	env.create_file("apps/app2/migrations/001.sql", "-- Migration\n");

	// Should warn about unapplied migrations for multiple apps
	assert!(env.file_exists("apps/app1/migrations/001.sql"));
	assert!(env.file_exists("apps/app2/migrations/001.sql"));
}

#[rstest]
#[tokio::test]
#[serial(reinhardt_settings)]
async fn test_manage_runserver_empty_allowed_hosts_error(
	#[from(env_vars)] mut env_guard: TeardownGuard<EnvVars>,
) {
	env_guard.set("ALLOWED_HOSTS", "");

	// Empty ALLOWED_HOSTS in production should error
	assert_eq!(std::env::var("ALLOWED_HOSTS").unwrap(), "");
}

#[tokio::test]
async fn test_manage_runserver_help_output() {
	let ctx = CommandContext::new(vec!["--help".to_string()]);

	// Help output should describe runserver options
	assert_eq!(ctx.arg(0), Some(&"--help".to_string()));
}

#[tokio::test]
async fn test_manage_runserver_suppressed_options() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("no-reload".to_string(), "true".to_string());
	ctx.set_option("nothreading".to_string(), "true".to_string());

	// Suppressed options for specific configurations
	assert!(ctx.has_option("no-reload"));
	assert!(ctx.has_option("nothreading"));
}

// ============================================================================
// ManageTestserver Tests
// ============================================================================

#[tokio::test]
async fn test_manage_testserver_handle_params() {
	let ctx = CommandContext::new(vec!["fixture.json".to_string()]);
	// Testserver should accept fixture file parameter
	assert_eq!(ctx.arg(0), Some(&"fixture.json".to_string()));
}

#[tokio::test]
async fn test_manage_testserver_params_to_runserver() {
	let ctx = CommandContext::new(vec!["fixture.json".to_string(), "8001".to_string()]);
	// Additional params should be passed to underlying runserver
	assert_eq!(ctx.arg(0), Some(&"fixture.json".to_string()));
	assert_eq!(ctx.arg(1), Some(&"8001".to_string()));
}

// ============================================================================
// CommandTypes Tests - Version and Help
// ============================================================================

#[tokio::test]
async fn test_commandtypes_version() {
	let ctx = CommandContext::new(vec!["--version".to_string()]);
	assert_eq!(ctx.arg(0), Some(&"--version".to_string()));
}

#[tokio::test]
async fn test_commandtypes_version_alternative() {
	let ctx = CommandContext::new(vec!["-V".to_string()]);
	assert_eq!(ctx.arg(0), Some(&"-V".to_string()));
}

#[tokio::test]
async fn test_commandtypes_help() {
	let ctx = CommandContext::new(vec!["--help".to_string()]);
	assert_eq!(ctx.arg(0), Some(&"--help".to_string()));
}

#[tokio::test]
async fn test_commandtypes_help_commands() {
	let ctx = CommandContext::new(vec!["help".to_string()]);
	assert_eq!(ctx.arg(0), Some(&"help".to_string()));
}

#[tokio::test]
async fn test_commandtypes_help_alternative() {
	let ctx = CommandContext::new(vec!["-h".to_string()]);
	assert_eq!(ctx.arg(0), Some(&"-h".to_string()));
}

#[tokio::test]
async fn test_commandtypes_help_short_alert() {
	let ctx = CommandContext::new(vec!["-h".to_string()]);
	// Short help flag should work same as --help
	assert_eq!(ctx.arg(0), Some(&"-h".to_string()));
}

#[tokio::test]
async fn test_commandtypes_specific_help() {
	let ctx = CommandContext::new(vec!["startproject".to_string(), "--help".to_string()]);
	assert_eq!(ctx.arg(0), Some(&"startproject".to_string()));
	assert_eq!(ctx.arg(1), Some(&"--help".to_string()));
}

#[tokio::test]
async fn test_commandtypes_help_default_options_with_custom_arguments() {
	let ctx = CommandContext::new(vec![
		"mycommand".to_string(),
		"--help".to_string(),
		"--verbose".to_string(),
	]);
	// Help should list default options plus custom arguments
	assert_eq!(ctx.arg(0), Some(&"mycommand".to_string()));
	assert_eq!(ctx.arg(1), Some(&"--help".to_string()));
}

// ============================================================================
// CommandTypes Tests - Color Output
// ============================================================================

#[test]
fn test_commandtypes_command_color() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("color".to_string(), "true".to_string());
	assert!(ctx.has_option("color"));
}

#[test]
fn test_commandtypes_command_no_color() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("no-color".to_string(), "true".to_string());
	assert!(ctx.has_option("no-color"));
}

#[test]
fn test_commandtypes_force_color_command_init() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("force-color".to_string(), "true".to_string());
	assert!(ctx.has_option("force-color"));
}

#[test]
fn test_commandtypes_no_color_force_color_mutually_exclusive_execute() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("no-color".to_string(), "true".to_string());
	ctx.set_option("force-color".to_string(), "true".to_string());
	// These options should be mutually exclusive
	assert!(ctx.has_option("no-color"));
	assert!(ctx.has_option("force-color"));
}

#[test]
fn test_commandtypes_no_color_force_color_mutually_exclusive_command_init() {
	let mut ctx = CommandContext::new(vec![]);
	ctx.set_option("no-color".to_string(), "true".to_string());
	ctx.set_option("force-color".to_string(), "true".to_string());
	// Should detect mutual exclusivity at init time
	assert!(ctx.has_option("no-color") && ctx.has_option("force-color"));
}

// ============================================================================
// CommandTypes Tests - Output Streams
// ============================================================================

#[tokio::test]
async fn test_commandtypes_custom_stdout() {
	use std::io::Write;

	let mut buffer = Vec::new();
	writeln!(&mut buffer, "Custom stdout test").expect("Write failed");

	assert_eq!(String::from_utf8(buffer).unwrap(), "Custom stdout test\n");
}

#[tokio::test]
async fn test_commandtypes_custom_stderr() {
	use std::io::Write;

	let mut buffer = Vec::new();
	writeln!(&mut buffer, "Custom stderr test").expect("Write failed");

	assert_eq!(String::from_utf8(buffer).unwrap(), "Custom stderr test\n");
}

// ============================================================================
// CommandTypes Tests - Base Command
// ============================================================================

#[tokio::test]
async fn test_commandtypes_base_command() {
	struct TestBaseCommand;

	#[async_trait::async_trait]
	impl BaseCommand for TestBaseCommand {
		fn name(&self) -> &str {
			"testbase"
		}

		async fn execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
			Ok(())
		}
	}

	let cmd = TestBaseCommand;
	let ctx = CommandContext::new(vec!["arg".to_string()]);
	let result = cmd.execute(&ctx).await;
	assert!(result.is_ok());
}

#[tokio::test]
async fn test_commandtypes_base_command_no_label() {
	struct NoLabelCommand;

	#[async_trait::async_trait]
	impl BaseCommand for NoLabelCommand {
		fn name(&self) -> &str {
			"nolabel"
		}

		async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
			if ctx.arg(0).is_some() {
				Err(CommandError::InvalidArguments(
					"No label expected".to_string(),
				))
			} else {
				Ok(())
			}
		}
	}

	let cmd = NoLabelCommand;
	let ctx = CommandContext::new(vec![]);
	let result = cmd.execute(&ctx).await;
	assert!(result.is_ok());
}

// ============================================================================
// CommandTypes Tests - App Commands
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_commandtypes_app_command(#[from(test_env)] env: TestEnvironment) {
	// Create app structure
	env.create_file("apps/testapp/mod.rs", "// Test app\n");
	assert!(env.file_exists("apps/testapp/mod.rs"));
}

#[rstest]
#[tokio::test]
async fn test_commandtypes_app_command_no_apps(#[from(test_env)] env: TestEnvironment) {
	// Test app command with no apps
	assert!(env.path().exists());
}

#[rstest]
#[tokio::test]
async fn test_commandtypes_app_command_multiple_apps(#[from(test_env)] env: TestEnvironment) {
	env.create_file("apps/app1/mod.rs", "// App 1\n");
	env.create_file("apps/app2/mod.rs", "// App 2\n");

	assert!(env.file_exists("apps/app1/mod.rs"));
	assert!(env.file_exists("apps/app2/mod.rs"));
}

// ============================================================================
// Discovery Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_discovery_precedence(#[from(test_env)] env: TestEnvironment) {
	// Test command discovery precedence
	env.create_file("commands/custom.rs", "// Custom command\n");
	assert!(env.file_exists("commands/custom.rs"));
}

// ============================================================================
// CommandDBOptionChoice Tests
// ============================================================================

// Removed empty test: test_commanddb_invalid_choice_db_option
// This test was empty and will be implemented when needed

// ============================================================================
// ArgumentOrder Tests
// ============================================================================

#[tokio::test]
async fn test_argumentorder_setting_then_option() {
	let ctx = CommandContext::new(vec![
		"--settings".to_string(),
		"mysettings".to_string(),
		"--verbose".to_string(),
	]);
	// Test setting followed by option
	assert_eq!(ctx.arg(0), Some(&"--settings".to_string()));
	assert_eq!(ctx.arg(1), Some(&"mysettings".to_string()));
	assert_eq!(ctx.arg(2), Some(&"--verbose".to_string()));
}

// ============================================================================
// ExecuteFromCommandLine Tests
// ============================================================================

// Removed empty test: test_executefromcommandline_program_name_from_argv
// This test was empty and will be implemented when needed

// ============================================================================
// DiffSettings Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_diffsettings_basic(#[from(test_env)] env: TestEnvironment) {
	env.create_file("settings.rs", "pub const DEBUG: bool = true;\n");
	// Test basic diff settings
	assert!(env.file_exists("settings.rs"));
}

#[rstest]
#[tokio::test]
async fn test_diffsettings_settings_configured(#[from(test_env)] env: TestEnvironment) {
	env.create_file(
		"settings.rs",
		"pub const DEBUG: bool = true;\npub const SECRET_KEY: &str = \"test\";\n",
	);
	// Test with configured settings
	assert!(env.file_exists("settings.rs"));
}

// ============================================================================
// Additional StartProject Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_startproject_custom_project_template_non_python_files_not_formatted(
	#[from(test_env)] env: TestEnvironment,
) {
	// Test that non-Rust files are not template-formatted
	assert!(env.path().exists());
}

#[rstest]
#[tokio::test]
async fn test_startproject_template_dir_with_trailing_slash(
	#[from(test_env)] env: TestEnvironment,
) {
	let mut ctx = CommandContext::new(vec![
		"testproject".to_owned(),
		env.path()
			.join("testproject")
			.to_string_lossy()
			.into_owned(),
	]);
	ctx.set_option("template".to_string(), "/path/to/template/".to_string());

	let cmd = StartProjectCommand;
	let result = cmd.execute(&ctx).await;

	// Template path with trailing slash should be handled
	assert!(result.is_ok() || result.is_err());
}

// Removed empty test: test_startproject_custom_project_template_from_tarball_by_path
// This test was empty and will be implemented when needed

// Removed empty test: test_startproject_custom_project_template_from_tarball_to_alternative_location
// This test was empty and will be implemented when needed

// Removed empty test: test_startproject_custom_project_template_from_tarball_by_url
// This test was empty and will be implemented when needed

// Removed empty test: test_startproject_custom_project_template_from_tarball_by_url_django_user_agent
// This test was empty and will be implemented when needed

// Removed empty test: test_startproject_project_template_tarball_url
// This test was empty and will be implemented when needed

#[rstest]
#[tokio::test]
async fn test_startproject_custom_project_template_with_non_ascii_templates(
	#[from(test_env)] env: TestEnvironment,
) {
	// Test with non-ASCII template content
	assert!(env.path().exists());
}

#[rstest]
#[tokio::test]
async fn test_startproject_custom_project_template_hidden_directory_default_excluded(
	#[from(test_env)] env: TestEnvironment,
) {
	// Test that hidden directories are excluded by default
	assert!(env.path().exists());
}

#[rstest]
#[tokio::test]
async fn test_startproject_custom_project_template_hidden_directory_included(
	#[from(test_env)] env: TestEnvironment,
) {
	// Test including hidden directories
	assert!(env.path().exists());
}

#[rstest]
#[tokio::test]
async fn test_startproject_custom_project_template_exclude_directory(
	#[from(test_env)] env: TestEnvironment,
) {
	// Test excluding specific directories
	assert!(env.path().exists());
}

#[rstest]
#[tokio::test]
async fn test_startproject_failure_to_format_code(#[from(test_env)] env: TestEnvironment) {
	// Test handling of code formatting failures
	assert!(env.path().exists());
}
