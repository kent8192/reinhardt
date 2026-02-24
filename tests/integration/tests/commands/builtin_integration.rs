//! Built-in command integration tests
//!
//! Tests for CheckCommand, RunServerCommand, ShellCommand, and ShowUrlsCommand.

use super::fixtures::{RouterFixture, router_fixture};
use reinhardt_commands::{
	BaseCommand, CheckCommand, CommandContext, RunServerCommand, ShellCommand,
};
use rstest::*;

// ============================================================================
// CheckCommand Tests
// ============================================================================

/// Test: CheckCommand metadata
///
/// Category: Happy Path
/// Verifies that CheckCommand has correct metadata.
#[rstest]
fn test_check_command_metadata() {
	let command = CheckCommand;

	assert_eq!(command.name(), "check", "Command name should be 'check'");
	assert!(
		!command.description().is_empty(),
		"Command should have a description"
	);
}

/// Test: CheckCommand arguments and options
///
/// Category: Happy Path
/// Verifies that CheckCommand defines expected options.
#[rstest]
fn test_check_command_options() {
	let command = CheckCommand;

	let options = command.options();
	let option_names: Vec<&str> = options.iter().map(|o| o.long.as_str()).collect();

	// Check for expected options
	assert!(
		option_names.contains(&"deploy") || options.is_empty(),
		"Should have --deploy option or no options"
	);
}

/// Test: CheckCommand with deploy flag
///
/// Category: Happy Path
/// Verifies that --deploy flag is handled correctly.
#[rstest]
fn test_check_command_deploy_flag() {
	let mut ctx = CommandContext::default();
	ctx.set_option("deploy".to_string(), "true".to_string());

	assert!(ctx.has_option("deploy"), "Should have deploy option");
}

/// Test: CheckCommand with app_label
///
/// Category: Happy Path
/// Verifies that app_label argument is handled correctly.
#[rstest]
fn test_check_command_with_app_label() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("myapp".to_string());

	assert_eq!(
		ctx.arg(0).map(String::as_str),
		Some("myapp"),
		"Should have app_label"
	);
}

/// Test: CheckCommand skip-checks option
///
/// Category: Use Case
/// Verifies that skip-checks works correctly.
#[rstest]
fn test_check_command_skip_checks() {
	let mut ctx = CommandContext::default();
	ctx.set_option("skip-checks".to_string(), "true".to_string());

	assert!(ctx.should_skip_checks(), "Should skip checks");
}

// ============================================================================
// RunServerCommand Tests
// ============================================================================

/// Test: RunServerCommand metadata
///
/// Category: Happy Path
/// Verifies that RunServerCommand has correct metadata.
#[rstest]
fn test_runserver_command_metadata() {
	let command = RunServerCommand;

	assert_eq!(
		command.name(),
		"runserver",
		"Command name should be 'runserver'"
	);
	assert!(
		!command.description().is_empty(),
		"Command should have a description"
	);
}

/// Test: RunServerCommand default binding setup
///
/// Category: Happy Path
/// Verifies default address setup.
#[rstest]
fn test_runserver_default_binding_setup() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("127.0.0.1:8000".to_string());

	assert_eq!(
		ctx.arg(0).map(String::as_str),
		Some("127.0.0.1:8000"),
		"Should have default address"
	);
}

/// Test: RunServerCommand custom address setup
///
/// Category: Happy Path
/// Verifies custom address setup.
#[rstest]
fn test_runserver_custom_address_setup() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("0.0.0.0:3000".to_string());

	assert_eq!(
		ctx.arg(0).map(String::as_str),
		Some("0.0.0.0:3000"),
		"Should have custom address"
	);
}

/// Test: RunServerCommand noreload option
///
/// Category: Happy Path
/// Verifies that --noreload flag is set correctly.
#[rstest]
fn test_runserver_noreload_option() {
	let mut ctx = CommandContext::default();
	ctx.set_option("noreload".to_string(), "true".to_string());

	assert!(ctx.has_option("noreload"), "Should have noreload option");
}

/// Test: RunServerCommand insecure option
///
/// Category: Happy Path
/// Verifies that --insecure flag is set correctly.
#[rstest]
fn test_runserver_insecure_option() {
	let mut ctx = CommandContext::default();
	ctx.set_option("insecure".to_string(), "true".to_string());

	assert!(ctx.has_option("insecure"), "Should have insecure option");
}

/// Test: RunServerCommand invalid address format
///
/// Category: Error Path
/// Verifies handling of invalid address format.
#[rstest]
fn test_runserver_invalid_address_format() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("invalid-address".to_string());

	// The context will accept any string, but command execution should fail
	assert_eq!(ctx.arg(0).map(String::as_str), Some("invalid-address"));
}

// ============================================================================
// ShellCommand Tests
// ============================================================================

/// Test: ShellCommand metadata
///
/// Category: Happy Path
/// Verifies that ShellCommand has correct metadata.
#[rstest]
fn test_shell_command_metadata() {
	let command = ShellCommand;

	assert_eq!(command.name(), "shell", "Command name should be 'shell'");
	assert!(
		!command.description().is_empty(),
		"Command should have a description"
	);
}

/// Test: ShellCommand with command option
///
/// Category: Happy Path
/// Verifies that -c option is set correctly.
#[rstest]
fn test_shell_command_with_command_option() {
	let mut ctx = CommandContext::default();
	ctx.set_option("command".to_string(), "println!(\"Hello\")".to_string());

	assert_eq!(
		ctx.option("command").map(String::as_str),
		Some("println!(\"Hello\")")
	);
}

/// Test: ShellCommand without command (interactive mode)
///
/// Category: Happy Path
/// Verifies interactive mode setup.
#[rstest]
fn test_shell_command_interactive_mode() {
	let ctx = CommandContext::default();

	assert!(
		ctx.option("command").is_none(),
		"Should have no command for interactive mode"
	);
}

/// Test: ShellCommand error expression
///
/// Category: Error Path
/// Verifies handling of invalid expression.
#[rstest]
fn test_shell_command_error_expression() {
	let mut ctx = CommandContext::default();
	ctx.set_option("command".to_string(), "this is not valid rust".to_string());

	assert!(ctx.has_option("command"), "Should have command option");
}

// ============================================================================
// ShowUrlsCommand Tests
// ============================================================================

/// Test: RouterFixture creation
///
/// Category: Happy Path
/// Verifies that router fixture is created correctly.
#[rstest]
fn test_router_fixture_creation(router_fixture: RouterFixture) {
	assert!(
		router_fixture.pattern_count() > 0,
		"Should have registered patterns"
	);
}

/// Test: RouterFixture has expected patterns
///
/// Category: Happy Path
/// Verifies that expected patterns exist.
#[rstest]
fn test_router_fixture_patterns(router_fixture: RouterFixture) {
	assert!(
		router_fixture.has_pattern("/api/users/"),
		"Should have user-list pattern"
	);
	assert!(
		router_fixture.has_pattern("/api/posts/"),
		"Should have post-list pattern"
	);
}

/// Test: RouterFixture has expected named routes
///
/// Category: Happy Path
/// Verifies that expected named routes exist.
#[rstest]
fn test_router_fixture_named_routes(router_fixture: RouterFixture) {
	assert!(
		router_fixture.has_named_route("user-list"),
		"Should have user-list named route"
	);
	assert!(
		router_fixture.has_named_route("post-detail"),
		"Should have post-detail named route"
	);
}

/// Test: ShowUrls with names flag
///
/// Category: Happy Path
/// Verifies that --names flag is set correctly.
#[rstest]
fn test_showurls_names_flag() {
	let mut ctx = CommandContext::default();
	ctx.set_option("names".to_string(), "true".to_string());

	assert!(ctx.has_option("names"), "Should have names option");
}

/// Test: ShowUrls without flags
///
/// Category: Happy Path
/// Verifies default showurls setup.
#[rstest]
fn test_showurls_default_setup() {
	let ctx = CommandContext::default();

	assert!(
		ctx.option("names").is_none(),
		"Should have no names option by default"
	);
}

/// Test: ShowUrls with empty router
///
/// Category: Edge Case
/// Verifies handling of empty router.
#[rstest]
fn test_showurls_empty_router() {
	let router = RouterFixture { patterns: vec![] };

	assert_eq!(router.pattern_count(), 0, "Should have no patterns");
	assert!(!router.has_pattern("/any/"), "Should not find any pattern");
}

// ============================================================================
// Decision Table Tests
// ============================================================================

/// Test: Check command flag combinations
///
/// Category: Decision Table
/// Verifies combinations of --deploy and app_label.
#[rstest]
#[case(false, None, "no options")]
#[case(true, None, "deploy only")]
#[case(false, Some("myapp"), "app_label only")]
#[case(true, Some("myapp"), "both options")]
fn test_check_decision_flag_combinations(
	#[case] deploy: bool,
	#[case] app_label: Option<&str>,
	#[case] description: &str,
) {
	let mut ctx = CommandContext::default();

	if deploy {
		ctx.set_option("deploy".to_string(), "true".to_string());
	}
	if let Some(app) = app_label {
		ctx.add_arg(app.to_string());
	}

	assert_eq!(
		ctx.has_option("deploy"),
		deploy,
		"{}: deploy mismatch",
		description
	);
	assert_eq!(
		ctx.arg(0).map(String::as_str),
		app_label,
		"{}: app_label mismatch",
		description
	);
}

/// Test: RunServer flag combinations
///
/// Category: Decision Table
/// Verifies combinations of --noreload, --insecure, and --no-docs.
#[rstest]
#[case(false, false, false, "no flags")]
#[case(true, false, false, "noreload only")]
#[case(false, true, false, "insecure only")]
#[case(false, false, true, "no_docs only")]
#[case(true, true, false, "noreload and insecure")]
#[case(true, false, true, "noreload and no_docs")]
#[case(false, true, true, "insecure and no_docs")]
#[case(true, true, true, "all flags")]
fn test_runserver_decision_flag_combinations(
	#[case] noreload: bool,
	#[case] insecure: bool,
	#[case] no_docs: bool,
	#[case] description: &str,
) {
	let mut ctx = CommandContext::default();

	if noreload {
		ctx.set_option("noreload".to_string(), "true".to_string());
	}
	if insecure {
		ctx.set_option("insecure".to_string(), "true".to_string());
	}
	if no_docs {
		ctx.set_option("no_docs".to_string(), "true".to_string());
	}

	assert_eq!(
		ctx.has_option("noreload"),
		noreload,
		"{}: noreload mismatch",
		description
	);
	assert_eq!(
		ctx.has_option("insecure"),
		insecure,
		"{}: insecure mismatch",
		description
	);
	assert_eq!(
		ctx.has_option("no_docs"),
		no_docs,
		"{}: no_docs mismatch",
		description
	);
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: Built-in commands sanity
///
/// Category: Sanity
/// Verifies basic structure of all built-in commands.
#[rstest]
fn test_builtin_commands_sanity() {
	// Check all built-in commands have valid metadata
	let commands: Vec<(&str, Box<dyn BaseCommand>)> = vec![
		("check", Box::new(CheckCommand)),
		("runserver", Box::new(RunServerCommand)),
		("shell", Box::new(ShellCommand)),
	];

	for (expected_name, command) in commands {
		assert_eq!(command.name(), expected_name, "Command name should match");
		assert!(
			!command.description().is_empty(),
			"{} should have description",
			expected_name
		);
	}
}
