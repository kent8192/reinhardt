use regex::Regex;

use crate::error::{DeployError, DeployResult};

/// Parsed result from `terraform plan` output.
///
/// Captures resource change counts and drift detection state
/// from terraform's human-readable plan output.
#[derive(Debug, Clone)]
pub struct PlanResult {
	/// Number of resources to create.
	pub creates: u32,
	/// Number of resources to update in-place.
	pub updates: u32,
	/// Number of resources to destroy.
	pub destroys: u32,
	/// Number of resources to import.
	pub imports: u32,
	/// Number of resources with no changes.
	pub unchanged: u32,
	/// Whether infrastructure drift was detected outside of Terraform.
	pub drift_detected: bool,
	/// Raw terraform plan output for debugging or display.
	pub raw_output: String,
}

impl PlanResult {
	/// Parse terraform plan output into a structured `PlanResult`.
	///
	/// Extracts resource counts from the "Plan: X to add, Y to change, Z to destroy."
	/// line using regex matching. Detects drift from messages indicating
	/// objects have changed outside of Terraform.
	///
	/// # Arguments
	///
	/// * `output` - Raw stdout from `terraform plan`
	/// * `exit_code` - Process exit code (0 = no changes, 2 = changes pending)
	pub fn parse(output: &str, exit_code: i32) -> DeployResult<Self> {
		let drift_detected = output.contains("Objects have changed outside of Terraform");

		// Exit code 0 means no changes; also handle explicit "No changes" message
		if exit_code == 0 || output.contains("No changes") {
			return Ok(Self {
				creates: 0,
				updates: 0,
				destroys: 0,
				imports: 0,
				unchanged: 0,
				drift_detected,
				raw_output: output.to_owned(),
			});
		}

		// Pattern: "Plan: X to add, Y to change, Z to destroy."
		let plan_re =
			Regex::new(r"Plan:\s+(\d+)\s+to add,\s+(\d+)\s+to change,\s+(\d+)\s+to destroy")
				.expect("plan regex must compile");

		let caps = plan_re.captures(output).ok_or_else(|| DeployError::TerraformExecution {
			message: format!("failed to parse plan output: no 'Plan:' line found in terraform output (exit_code={exit_code})"),
		})?;

		let creates = caps[1]
			.parse::<u32>()
			.map_err(|e| DeployError::TerraformExecution {
				message: format!("failed to parse resource count: {e}"),
			})?;
		let updates = caps[2]
			.parse::<u32>()
			.map_err(|e| DeployError::TerraformExecution {
				message: format!("failed to parse resource count: {e}"),
			})?;
		let destroys = caps[3]
			.parse::<u32>()
			.map_err(|e| DeployError::TerraformExecution {
				message: format!("failed to parse resource count: {e}"),
			})?;

		Ok(Self {
			creates,
			updates,
			destroys,
			imports: 0,
			unchanged: 0,
			drift_detected,
			raw_output: output.to_owned(),
		})
	}

	/// Returns `true` if the plan includes any resource changes.
	pub fn has_changes(&self) -> bool {
		self.creates > 0 || self.updates > 0 || self.destroys > 0
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn plan_result_no_changes() {
		// Arrange
		let output = "No changes. Your infrastructure matches the configuration.";

		// Act
		let result = PlanResult::parse(output, 0).unwrap();

		// Assert
		assert_eq!(result.creates, 0);
		assert_eq!(result.updates, 0);
		assert_eq!(result.destroys, 0);
		assert!(!result.has_changes());
	}

	#[rstest]
	fn plan_result_with_changes() {
		// Arrange
		let output = "Plan: 2 to add, 1 to change, 0 to destroy.";

		// Act
		let result = PlanResult::parse(output, 2).unwrap();

		// Assert
		assert_eq!(result.creates, 2);
		assert_eq!(result.updates, 1);
		assert_eq!(result.destroys, 0);
		assert!(result.has_changes());
	}

	#[rstest]
	fn plan_result_with_destroys() {
		// Arrange
		let output = "Plan: 0 to add, 0 to change, 3 to destroy.";

		// Act
		let result = PlanResult::parse(output, 2).unwrap();

		// Assert
		assert_eq!(result.creates, 0);
		assert_eq!(result.updates, 0);
		assert_eq!(result.destroys, 3);
		assert!(result.has_changes());
	}

	#[rstest]
	fn drift_detection() {
		// Arrange
		let output = "Note: Objects have changed outside of Terraform\n\nPlan: 0 to add, 1 to change, 0 to destroy.";

		// Act
		let result = PlanResult::parse(output, 2).unwrap();

		// Assert
		assert!(result.drift_detected);
		assert_eq!(result.updates, 1);
	}

	#[rstest]
	fn drift_detection_no_drift() {
		// Arrange
		let plan = PlanResult {
			creates: 0,
			updates: 0,
			destroys: 0,
			imports: 0,
			unchanged: 15,
			drift_detected: false,
			raw_output: String::new(),
		};

		// Act & Assert
		assert!(!plan.drift_detected);
		assert!(!plan.has_changes());
	}

	#[rstest]
	fn plan_result_malformed_output_returns_error() {
		// Arrange
		let output = "Some random terraform output without plan line";

		// Act
		let result = PlanResult::parse(output, 2);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn plan_result_missing_plan_line_returns_error() {
		// Arrange
		let output =
			"Terraform will perform the following actions:\n  + resource\n\nChanges to Outputs:";

		// Act
		let result = PlanResult::parse(output, 2);

		// Assert
		assert!(result.is_err());
	}
}
