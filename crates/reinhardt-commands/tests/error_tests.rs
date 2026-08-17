//! CommandError unit tests
//!
//! Tests for error type variants, conversions, and Display implementations.

use reinhardt_commands::{CommandError, command_error_exit_code};
use rstest::rstest;
use std::io;

#[rstest]
#[case(CommandError::VerificationFailed, 1)]
#[case(
	CommandError::VerificationExecution("Cargo replay failed".to_owned()),
	2
)]
#[case(CommandError::ExecutionError("other command".to_owned()), 1)]
fn verification_exit_codes_are_stable(#[case] error: CommandError, #[case] expected: i32) {
	assert_eq!(command_error_exit_code(&error), expected);
}

// =============================================================================
// Happy Path Tests - Error Creation
// =============================================================================

/// Test NotFound error creation
///
/// **Category**: Happy Path
/// **Verifies**: NotFound variant stores and returns message correctly
#[rstest]
fn test_error_not_found_creation() {
	let message = "command_name";
	let error = CommandError::NotFound(message.to_string());

	let display = format!("{}", error);
	assert!(
		display.contains("Command not found"),
		"Display should contain 'Command not found'"
	);
	assert!(
		display.contains(message),
		"Display should contain the command name"
	);
}

/// Test InvalidArguments error creation
///
/// **Category**: Happy Path
/// **Verifies**: InvalidArguments variant stores and returns message correctly
#[rstest]
fn test_error_invalid_arguments_creation() {
	let message = "Missing required argument: name";
	let error = CommandError::InvalidArguments(message.to_string());

	let display = format!("{}", error);
	assert!(
		display.contains("Invalid arguments"),
		"Display should contain 'Invalid arguments'"
	);
	assert!(
		display.contains(message),
		"Display should contain the error message"
	);
}

/// Test ExecutionError creation
///
/// **Category**: Happy Path
/// **Verifies**: ExecutionError variant stores and returns message correctly
#[rstest]
fn test_error_execution_error_creation() {
	let message = "Database connection failed";
	let error = CommandError::ExecutionError(message.to_string());

	let display = format!("{}", error);
	assert!(
		display.contains("Execution error"),
		"Display should contain 'Execution error'"
	);
	assert!(
		display.contains(message),
		"Display should contain the error message"
	);
}

/// Test ParseError creation
///
/// **Category**: Happy Path
/// **Verifies**: ParseError variant stores and returns message correctly
#[rstest]
fn test_error_parse_error_creation() {
	let message = "Invalid JSON format";
	let error = CommandError::ParseError(message.to_string());

	let display = format!("{}", error);
	assert!(
		display.contains("Parse error"),
		"Display should contain 'Parse error'"
	);
	assert!(
		display.contains(message),
		"Display should contain the error message"
	);
}

/// Test TemplateError creation
///
/// **Category**: Happy Path
/// **Verifies**: TemplateError variant stores and returns message correctly
#[rstest]
fn test_error_template_error_creation() {
	let message = "Undefined variable: project_name";
	let error = CommandError::TemplateError(message.to_string());

	let display = format!("{}", error);
	assert!(
		display.contains("Template error"),
		"Display should contain 'Template error'"
	);
	assert!(
		display.contains(message),
		"Display should contain the error message"
	);
}

// =============================================================================
// Happy Path Tests - Error Conversion
// =============================================================================

/// Test conversion from std::io::Error
///
/// **Category**: Happy Path
/// **Verifies**: IoError variant is created from std::io::Error
#[rstest]
fn test_error_from_io_error() {
	let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
	let error: CommandError = io_error.into();

	let display = format!("{}", error);
	assert!(
		display.contains("IO error"),
		"Display should contain 'IO error'"
	);
	assert!(
		display.contains("File not found"),
		"Display should contain the original error message"
	);

	// Verify it's the correct variant
	assert!(matches!(error, CommandError::IoError(_)));
}

/// Test conversion from String
///
/// **Category**: Happy Path
/// **Verifies**: ExecutionError is created from String
#[rstest]
fn test_error_from_string() {
	let message = "Something went wrong".to_string();
	let error: CommandError = message.clone().into();

	let display = format!("{}", error);
	assert!(
		display.contains("Execution error"),
		"Display should contain 'Execution error'"
	);
	assert!(
		display.contains(&message),
		"Display should contain the original message"
	);

	// Verify it's the correct variant
	assert!(matches!(error, CommandError::ExecutionError(_)));
}

/// Test conversion from tera::Error
///
/// **Category**: Happy Path
/// **Verifies**: TemplateError is created from tera::Error
#[rstest]
fn test_error_from_tera_error() {
	// Create a tera error by trying to render an invalid template
	let mut tera = tera::Tera::default();
	let tera_result = tera.render_str("{{ undefined_var }}", &tera::Context::new());

	if let Err(tera_error) = tera_result {
		let error: CommandError = tera_error.into();

		let display = format!("{}", error);
		assert!(
			display.contains("Template error"),
			"Display should contain 'Template error'"
		);

		// Verify it's the correct variant
		assert!(matches!(error, CommandError::TemplateError(_)));
	}
}

// =============================================================================
// Equivalence Partitioning Tests - Error Display
// =============================================================================

/// Test Display implementation for all error variants
///
/// **Category**: Equivalence Partitioning
/// **Verifies**: Each error variant has a distinct, meaningful display output
#[rstest]
#[case(CommandError::NotFound("test".to_string()), "Command not found", "test")]
#[case(CommandError::InvalidArguments("arg error".to_string()), "Invalid arguments", "arg error")]
#[case(CommandError::ExecutionError("exec error".to_string()), "Execution error", "exec error")]
#[case(CommandError::ParseError("parse error".to_string()), "Parse error", "parse error")]
#[case(CommandError::TemplateError("template error".to_string()), "Template error", "template error")]
fn test_error_display_partitions(
	#[case] error: CommandError,
	#[case] expected_prefix: &str,
	#[case] expected_message: &str,
) {
	let display = format!("{}", error);

	assert!(
		display.contains(expected_prefix),
		"Display '{}' should contain prefix '{}'",
		display,
		expected_prefix
	);
	assert!(
		display.contains(expected_message),
		"Display '{}' should contain message '{}'",
		display,
		expected_message
	);
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test error with empty message
///
/// **Category**: Edge Case
/// **Verifies**: Errors handle empty messages gracefully
#[rstest]
fn test_error_empty_message() {
	let errors = vec![
		CommandError::NotFound(String::new()),
		CommandError::InvalidArguments(String::new()),
		CommandError::ExecutionError(String::new()),
		CommandError::ParseError(String::new()),
		CommandError::TemplateError(String::new()),
	];

	for error in errors {
		// Should not panic
		let display = format!("{}", error);
		assert!(
			!display.is_empty(),
			"Display should not be empty even with empty message"
		);
	}
}

/// Test error with very long message
///
/// **Category**: Edge Case
/// **Verifies**: Errors handle long messages correctly
#[rstest]
fn test_error_long_message() {
	let long_message = "x".repeat(10000);
	let error = CommandError::ExecutionError(long_message.clone());

	let display = format!("{}", error);
	assert!(
		display.contains(&long_message),
		"Display should contain the full long message"
	);
}

/// Test error with special characters
///
/// **Category**: Edge Case
/// **Verifies**: Errors handle special characters correctly
#[rstest]
fn test_error_special_characters() {
	let special_message = "Error: <script>alert('xss')</script> \"quotes\" 'apostrophes' \n\t\r";
	let error = CommandError::ExecutionError(special_message.to_string());

	let display = format!("{}", error);
	assert!(
		display.contains("<script>"),
		"Display should preserve special characters"
	);
	assert!(
		display.contains("\"quotes\""),
		"Display should preserve quotes"
	);
}

/// Test error with Unicode characters
///
/// **Category**: Edge Case
/// **Verifies**: Errors handle Unicode correctly
#[rstest]
fn test_error_unicode_message() {
	let unicode_message = "エラー: ファイルが見つかりません";
	let error = CommandError::NotFound(unicode_message.to_string());

	let display = format!("{}", error);
	assert!(
		display.contains(unicode_message),
		"Display should contain Unicode message"
	);
}

// =============================================================================
// Debug Trait Tests
// =============================================================================

/// Test Debug implementation
///
/// **Category**: Happy Path
/// **Verifies**: Debug output includes variant name and message
#[rstest]
fn test_error_debug() {
	let error = CommandError::NotFound("test_command".to_string());
	let debug = format!("{:?}", error);

	assert!(
		debug.contains("NotFound"),
		"Debug should contain variant name"
	);
	assert!(
		debug.contains("test_command"),
		"Debug should contain message"
	);
}

// =============================================================================
// IoError Tests
// =============================================================================

/// Test various IoError kinds
///
/// **Category**: Combination
/// **Verifies**: Different io::ErrorKind values convert correctly
#[rstest]
#[case(io::ErrorKind::NotFound, "entity not found")]
#[case(io::ErrorKind::PermissionDenied, "permission denied")]
#[case(io::ErrorKind::ConnectionRefused, "connection refused")]
#[case(io::ErrorKind::TimedOut, "operation timed out")]
#[case(io::ErrorKind::InvalidInput, "invalid input")]
fn test_io_error_kinds(#[case] kind: io::ErrorKind, #[case] custom_message: &str) {
	let io_error = io::Error::new(kind, custom_message);
	let error: CommandError = io_error.into();

	let display = format!("{}", error);
	assert!(
		display.contains("IO error"),
		"Display should contain 'IO error'"
	);
	assert!(
		display.contains(custom_message),
		"Display should contain custom message '{}'",
		custom_message
	);
}

// =============================================================================
// Sanity Tests
// =============================================================================

/// Sanity test for basic error workflow
///
/// **Category**: Sanity
/// **Verifies**: Basic error creation and display works
#[rstest]
fn test_error_basic_sanity() {
	// Create each variant
	let errors: Vec<(CommandError, &str)> = vec![
		(CommandError::NotFound("cmd".to_string()), "NotFound"),
		(
			CommandError::InvalidArguments("arg".to_string()),
			"InvalidArguments",
		),
		(
			CommandError::ExecutionError("exec".to_string()),
			"ExecutionError",
		),
		(CommandError::ParseError("parse".to_string()), "ParseError"),
		(
			CommandError::TemplateError("template".to_string()),
			"TemplateError",
		),
	];

	for (error, variant_name) in errors {
		// Display works
		let display = format!("{}", error);
		assert!(
			!display.is_empty(),
			"Display should not be empty for {}",
			variant_name
		);

		// Debug works
		let debug = format!("{:?}", error);
		assert!(
			debug.contains(variant_name),
			"Debug should contain '{}' for {:?}",
			variant_name,
			error
		);
	}
}
