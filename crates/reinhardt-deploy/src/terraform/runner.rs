use std::path::{Path, PathBuf};

use regex::Regex;

use crate::error::{DeployError, DeployResult};

/// Terraform CLI runner that builds command arguments for terraform operations.
///
/// Wraps a working directory and provides methods to construct argument lists
/// for `terraform init`, `terraform plan`, and `terraform apply` commands.
pub struct TerraformRunner {
	work_dir: PathBuf,
}

/// Parse a Terraform version string from `terraform version` output.
///
/// Expects output containing a line matching `Terraform vX.Y.Z` and extracts
/// the major, minor, and patch components as a tuple.
///
/// # Errors
///
/// Returns [`DeployError::TerraformExecution`] if the output does not contain
/// a recognizable version string.
pub fn parse_terraform_version(output: &str) -> DeployResult<(u32, u32, u32)> {
	let re = Regex::new(r"Terraform v(\d+)\.(\d+)\.(\d+)").map_err(|e| {
		DeployError::TerraformExecution {
			message: format!("failed to compile version regex: {e}"),
		}
	})?;

	let caps = re
		.captures(output)
		.ok_or_else(|| DeployError::TerraformExecution {
			message: format!("could not parse terraform version from output: {output}"),
		})?;

	let major = caps[1]
		.parse::<u32>()
		.map_err(|e| DeployError::TerraformExecution {
			message: format!("invalid major version: {e}"),
		})?;
	let minor = caps[2]
		.parse::<u32>()
		.map_err(|e| DeployError::TerraformExecution {
			message: format!("invalid minor version: {e}"),
		})?;
	let patch = caps[3]
		.parse::<u32>()
		.map_err(|e| DeployError::TerraformExecution {
			message: format!("invalid patch version: {e}"),
		})?;

	Ok((major, minor, patch))
}

/// Check whether a version tuple meets a minimum version requirement.
///
/// Compares `(major, minor, patch)` tuples lexicographically:
/// major is compared first, then minor, then patch.
pub fn version_satisfies(version: (u32, u32, u32), minimum: (u32, u32, u32)) -> bool {
	version >= minimum
}

impl TerraformRunner {
	/// Create a new `TerraformRunner` with the given working directory.
	pub fn new(work_dir: PathBuf) -> Self {
		Self { work_dir }
	}

	/// Build arguments for `terraform init`.
	pub fn build_init_args(&self) -> Vec<String> {
		vec!["init".into()]
	}

	/// Build arguments for `terraform plan`.
	///
	/// Always includes `-detailed-exitcode` for machine-readable exit codes.
	/// In dry-run mode, adds `-lock=false` to avoid acquiring state locks.
	/// In real mode, adds `-out=tfplan` to save the plan for later apply.
	pub fn build_plan_args(&self, dry_run: bool) -> Vec<String> {
		let mut args = vec!["plan".into(), "-detailed-exitcode".into()];

		if dry_run {
			args.push("-lock=false".into());
		} else {
			args.push("-out=tfplan".into());
		}

		args
	}

	/// Build arguments for `terraform apply` using a saved plan file.
	///
	/// Uses `-auto-approve` since the plan was already reviewed.
	pub fn build_apply_args(&self) -> Vec<String> {
		vec!["apply".into(), "tfplan".into(), "-auto-approve".into()]
	}

	/// Get a reference to the working directory path.
	pub fn work_dir(&self) -> &Path {
		&self.work_dir
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn parse_terraform_version_valid() {
		// Arrange
		let output = "Terraform v1.11.2\non linux_amd64\n";

		// Act
		let version = parse_terraform_version(output).unwrap();

		// Assert
		assert_eq!(version, (1, 11, 2));
	}

	#[rstest]
	fn parse_terraform_version_invalid() {
		// Arrange
		let output = "not terraform";

		// Act
		let result = parse_terraform_version(output);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn check_version_satisfies_minimum() {
		// Arrange & Act & Assert
		assert!(version_satisfies((1, 11, 2), (1, 11, 0)));
		assert!(version_satisfies((1, 12, 0), (1, 11, 0)));
		assert!(version_satisfies((2, 0, 0), (1, 11, 0)));
		assert!(!version_satisfies((1, 10, 0), (1, 11, 0)));
		assert!(!version_satisfies((1, 11, 0), (1, 11, 1)));
	}

	#[rstest]
	fn build_plan_command_dry_run() {
		// Arrange
		let runner = TerraformRunner::new("/tmp/tf-workdir".into());

		// Act
		let args = runner.build_plan_args(true);

		// Assert
		assert_eq!(args, vec!["plan", "-detailed-exitcode", "-lock=false"]);
	}

	#[rstest]
	fn build_plan_command_real() {
		// Arrange
		let runner = TerraformRunner::new("/tmp/tf-workdir".into());

		// Act
		let args = runner.build_plan_args(false);

		// Assert
		assert_eq!(args, vec!["plan", "-detailed-exitcode", "-out=tfplan"]);
	}

	#[rstest]
	fn build_init_args_returns_init() {
		// Arrange
		let runner = TerraformRunner::new("/tmp/tf-workdir".into());

		// Act
		let args = runner.build_init_args();

		// Assert
		assert_eq!(args, vec!["init"]);
	}

	#[rstest]
	fn build_apply_args_returns_correct_sequence() {
		// Arrange
		let runner = TerraformRunner::new("/tmp/tf-workdir".into());

		// Act
		let args = runner.build_apply_args();

		// Assert
		assert_eq!(args, vec!["apply", "tfplan", "-auto-approve"]);
	}

	#[rstest]
	fn work_dir_returns_configured_path() {
		// Arrange
		let path = PathBuf::from("/tmp/tf-workdir");
		let runner = TerraformRunner::new(path.clone());

		// Act
		let result = runner.work_dir();

		// Assert
		assert_eq!(result, path);
	}

	#[rstest]
	fn parse_terraform_version_with_extra_text() {
		// Arrange
		let output =
			"Terraform v1.5.0\non darwin_arm64\n\nYour version of Terraform is out of date.\n";

		// Act
		let version = parse_terraform_version(output).unwrap();

		// Assert
		assert_eq!(version, (1, 5, 0));
	}

	#[rstest]
	fn version_satisfies_equal_versions() {
		// Arrange & Act & Assert
		assert!(version_satisfies((1, 11, 0), (1, 11, 0)));
	}
}
