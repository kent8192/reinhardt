//! Pre-flight check system for deployment validation.
//!
//! Provides tools to verify that required CLI tools and services
//! are available before starting a deployment.

use crate::providers::traits::PreflightCheck;

/// Result of a single pre-flight check execution.
#[derive(Debug, Clone)]
pub struct PreflightCheckResult {
	pub name: String,
	pub passed: bool,
	pub message: String,
}

/// Runner for executing pre-flight checks before deployment.
pub struct PreflightRunner;

impl PreflightRunner {
	/// Checks if a tool exists in PATH by building a check result.
	///
	/// Returns a `PreflightCheckResult` indicating whether the tool
	/// was found. This builds the result structure without actually
	/// executing the tool.
	pub fn check_tool_exists(tool_name: &str) -> PreflightCheckResult {
		match which::which(tool_name) {
			Ok(path) => PreflightCheckResult {
				name: tool_name.to_string(),
				passed: true,
				message: format!("found at {}", path.display()),
			},
			Err(_) => PreflightCheckResult {
				name: tool_name.to_string(),
				passed: false,
				message: format!("{} not found in PATH", tool_name),
			},
		}
	}

	/// Execute pre-flight checks by running their commands.
	///
	/// Runs each check's command via the system shell and compares the
	/// exit code against the expected value. Returns a result for each
	/// check indicating pass/fail.
	pub fn execute_checks(checks: &[PreflightCheck]) -> Vec<PreflightCheckResult> {
		checks.iter().map(Self::run_check).collect()
	}

	/// Run a single pre-flight check command.
	fn run_check(check: &PreflightCheck) -> PreflightCheckResult {
		let parts: Vec<&str> = check.command.split_whitespace().collect();
		if parts.is_empty() {
			return PreflightCheckResult {
				name: check.name.clone(),
				passed: false,
				message: "empty command".to_string(),
			};
		}

		match std::process::Command::new(parts[0])
			.args(&parts[1..])
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.status()
		{
			Ok(status) => {
				let exit_code = status.code().unwrap_or(-1);
				let passed = exit_code == check.expected_exit_code;
				PreflightCheckResult {
					name: check.name.clone(),
					passed,
					message: if passed {
						format!("{}: exit code {}", check.description, exit_code)
					} else {
						format!(
							"{}: expected exit code {}, got {}",
							check.description, check.expected_exit_code, exit_code
						)
					},
				}
			}
			Err(e) => PreflightCheckResult {
				name: check.name.clone(),
				passed: false,
				message: format!("failed to execute '{}': {}", check.command, e),
			},
		}
	}

	/// Returns true if all checks in the results passed.
	pub fn all_passed(results: &[PreflightCheckResult]) -> bool {
		results.iter().all(|r| r.passed)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn preflight_check_result_pass() {
		// Arrange
		let check = PreflightCheckResult {
			name: "terraform_version".to_string(),
			passed: true,
			message: "Terraform v1.11.2".to_string(),
		};

		// Act & Assert
		assert!(check.passed);
		assert_eq!(check.name, "terraform_version");
	}

	#[rstest]
	fn preflight_check_result_fail() {
		// Arrange
		let check = PreflightCheckResult {
			name: "docker_daemon".to_string(),
			passed: false,
			message: "Docker daemon is not running".to_string(),
		};

		// Act & Assert
		assert!(!check.passed);
	}

	#[rstest]
	fn all_checks_passed_returns_true() {
		// Arrange
		let results = vec![
			PreflightCheckResult {
				name: "terraform".to_string(),
				passed: true,
				message: "OK".to_string(),
			},
			PreflightCheckResult {
				name: "docker".to_string(),
				passed: true,
				message: "OK".to_string(),
			},
		];

		// Act
		let result = PreflightRunner::all_passed(&results);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn all_checks_passed_returns_false() {
		// Arrange
		let results = vec![
			PreflightCheckResult {
				name: "terraform".to_string(),
				passed: true,
				message: "OK".to_string(),
			},
			PreflightCheckResult {
				name: "docker".to_string(),
				passed: false,
				message: "Not running".to_string(),
			},
		];

		// Act
		let result = PreflightRunner::all_passed(&results);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn empty_checks_returns_true() {
		// Arrange
		let results: Vec<PreflightCheckResult> = vec![];

		// Act
		let result = PreflightRunner::all_passed(&results);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn check_tool_exists_finds_cargo() {
		// Arrange & Act
		let result = PreflightRunner::check_tool_exists("cargo");

		// Assert
		assert!(result.passed);
		assert_eq!(result.name, "cargo");
		assert!(result.message.starts_with("found at "));
	}

	#[rstest]
	fn check_tool_exists_missing_tool() {
		// Arrange & Act
		let result = PreflightRunner::check_tool_exists("nonexistent_tool_xyz_12345");

		// Assert
		assert!(!result.passed);
		assert_eq!(result.name, "nonexistent_tool_xyz_12345");
		assert_eq!(
			result.message,
			"nonexistent_tool_xyz_12345 not found in PATH"
		);
	}

	#[rstest]
	fn execute_checks_runs_true_command() {
		// Arrange
		let checks = vec![PreflightCheck {
			name: "true_check".to_string(),
			description: "Run true".to_string(),
			command: "true".to_string(),
			expected_exit_code: 0,
		}];

		// Act
		let results = PreflightRunner::execute_checks(&checks);

		// Assert
		assert_eq!(results.len(), 1);
		assert!(results[0].passed);
		assert_eq!(results[0].name, "true_check");
	}

	#[rstest]
	fn execute_checks_handles_failing_command() {
		// Arrange
		let checks = vec![PreflightCheck {
			name: "false_check".to_string(),
			description: "Run false".to_string(),
			command: "false".to_string(),
			expected_exit_code: 0,
		}];

		// Act
		let results = PreflightRunner::execute_checks(&checks);

		// Assert
		assert_eq!(results.len(), 1);
		assert!(!results[0].passed);
	}

	#[rstest]
	fn execute_checks_handles_missing_command() {
		// Arrange
		let checks = vec![PreflightCheck {
			name: "missing_check".to_string(),
			description: "Run nonexistent".to_string(),
			command: "nonexistent_command_xyz_99999".to_string(),
			expected_exit_code: 0,
		}];

		// Act
		let results = PreflightRunner::execute_checks(&checks);

		// Assert
		assert_eq!(results.len(), 1);
		assert!(!results[0].passed);
		assert!(results[0].message.contains("failed to execute"));
	}

	#[rstest]
	fn execute_checks_empty_list() {
		// Arrange
		let checks: Vec<PreflightCheck> = vec![];

		// Act
		let results = PreflightRunner::execute_checks(&checks);

		// Assert
		assert!(results.is_empty());
	}
}
