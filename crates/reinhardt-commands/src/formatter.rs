//! Code formatter integration
//!
//! Provides integration with code formatters like rustfmt.

use crate::CommandError;
use crate::process::{ProcessRequest, ProcessRunner, SystemProcessRunner};
use std::path::Path;

/// Run code formatters on the specified files
///
/// This function attempts to run rustfmt on the provided file paths.
/// It handles cases where rustfmt is not found or fails to execute.
///
/// # Arguments
///
/// * `paths` - Paths to files to format
/// * `formatter_path` - Optional custom path to the formatter executable
///
/// # Returns
///
/// * `Ok(())` if formatting succeeds
/// * `Err(CommandError)` if formatter is not found or execution fails
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_commands::formatter::run_formatters;
///
/// // Format files with default rustfmt
/// run_formatters(&["src/lib.rs", "src/main.rs"], None)?;
///
/// // Format with custom formatter path
/// run_formatters(&["src/lib.rs"], Some("/usr/local/bin/rustfmt"))?;
/// # Ok::<(), reinhardt_commands::CommandError>(())
/// ```
pub fn run_formatters(paths: &[&str], formatter_path: Option<&str>) -> Result<(), CommandError> {
	run_formatters_with_runner(paths, formatter_path, &SystemProcessRunner)
}

fn run_formatters_with_runner<R: ProcessRunner>(
	paths: &[&str],
	formatter_path: Option<&str>,
	runner: &R,
) -> Result<(), CommandError> {
	if paths.is_empty() {
		return Ok(());
	}

	let formatter = formatter_path.unwrap_or("rustfmt");

	// Check if formatter exists
	let check_result = runner.run(&ProcessRequest::new(formatter).arg("--version"));

	match check_result {
		Ok(output) => {
			if !output.success {
				return Err(CommandError::ExecutionError(format!(
					"Formatter '{}' is not working properly",
					formatter
				)));
			}
		}
		Err(e) => {
			if e.kind() == std::io::ErrorKind::NotFound {
				return Err(CommandError::ExecutionError(format!(
					"Formatter '{}' not found. Please install rustfmt: rustup component add rustfmt",
					formatter
				)));
			} else {
				return Err(CommandError::ExecutionError(format!(
					"Failed to check formatter '{}': {}",
					formatter, e
				)));
			}
		}
	}

	// Run formatter on each file
	for path in paths {
		if !Path::new(path).exists() {
			return Err(CommandError::ExecutionError(format!(
				"File not found: {}",
				path
			)));
		}

		let result = runner.run(&ProcessRequest::new(formatter).arg(path));

		match result {
			Ok(output) => {
				if !output.success {
					let stderr = String::from_utf8_lossy(&output.stderr);
					return Err(CommandError::ExecutionError(format!(
						"Formatter failed for '{}': {}",
						path, stderr
					)));
				}
			}
			Err(e) => {
				return Err(CommandError::ExecutionError(format!(
					"Failed to run formatter on '{}': {}",
					path, e
				)));
			}
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use crate::process::{FakeProcessRunner, ProcessOutcome};

	use super::*;

	#[test]
	fn formatter_reports_version_check_failure_without_formatting_files() {
		let runner = FakeProcessRunner::new([Ok(ProcessOutcome::failure(
			"exit status: 1",
			b"broken toolchain".to_vec(),
		))]);

		let error = run_formatters_with_runner(&["src/lib.rs"], Some("rustfmt"), &runner)
			.expect_err("version failure must stop formatting");

		assert_eq!(
			error.to_string(),
			"Execution error: Formatter 'rustfmt' is not working properly"
		);
		assert_eq!(runner.requests().len(), 1);
	}

	#[test]
	fn formatter_includes_stderr_from_file_format_failure() {
		let temp = tempfile::NamedTempFile::new().expect("create Rust file");
		let runner = FakeProcessRunner::new([
			Ok(ProcessOutcome::success(b"rustfmt 1.8".to_vec())),
			Ok(ProcessOutcome::failure(
				"exit status: 1",
				b"parse error".to_vec(),
			)),
		]);

		let error = run_formatters_with_runner(
			&[temp.path().to_str().expect("UTF-8 path")],
			Some("rustfmt"),
			&runner,
		)
		.expect_err("format failure must propagate");

		assert!(error.to_string().contains("parse error"));
	}

	#[test]
	fn test_run_formatters_empty_paths() {
		let result = run_formatters(&[], None);
		assert!(result.is_ok(), "Should succeed with empty paths");
	}

	#[test]
	fn test_run_formatters_nonexistent_formatter() {
		let result = run_formatters(&["src/lib.rs"], Some("/nonexistent/path/to/formatter"));
		assert!(result.is_err(), "Should fail with nonexistent formatter");
		if let Err(e) = result {
			let error_msg = format!("{:?}", e);
			assert!(
				error_msg.contains("not found"),
				"Error should mention formatter not found"
			);
		}
	}

	#[test]
	fn test_run_formatters_nonexistent_file() {
		let result = run_formatters(&["/nonexistent/file.rs"], None);
		assert!(result.is_err(), "Should fail with nonexistent file");
		if let Err(e) = result {
			let error_msg = format!("{:?}", e);
			assert!(
				error_msg.contains("File not found"),
				"Error should mention file not found"
			);
		}
	}
}
