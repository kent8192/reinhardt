//! Deployment report generation.
//!
//! Provides structured report types and formatters for deployment results.
//! Supports human-readable terminal output, JSON serialization, and Markdown
//! output suitable for PR comments.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::DeployResult;

/// Report from a single pre-flight check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReport {
	pub name: String,
	pub passed: bool,
	pub message: String,
}

/// Pre-flight checks summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightReport {
	pub passed: bool,
	pub checks: Vec<CheckReport>,
}

/// Individual Terraform resource change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChange {
	pub action: String,
	pub resource_type: String,
	pub resource_name: String,
}

/// Terraform plan summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformReport {
	pub creates: u32,
	pub updates: u32,
	pub destroys: u32,
	pub unchanged: u32,
	pub drift: bool,
	pub changes: Vec<ResourceChange>,
}

/// Complete deployment report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployReport {
	pub version: String,
	pub timestamp: String,
	pub commit: String,
	pub environment: String,
	pub provider: String,
	pub dry_run: bool,
	pub preflight: PreflightReport,
	pub terraform: TerraformReport,
	pub exit_code: i32,
}

impl DeployReport {
	/// Format the report as human-readable terminal output.
	pub fn format_human(&self) -> String {
		let mut output = String::new();

		if self.dry_run {
			output.push_str("=== DRY-RUN REPORT ===\n");
		} else {
			output.push_str("=== DEPLOY REPORT ===\n");
		}

		output.push_str(&format!("Environment: {}\n", self.environment));
		output.push_str(&format!("Provider:    {}\n", self.provider));
		output.push_str(&format!("Commit:      {}\n", self.commit));
		output.push_str(&format!("Timestamp:   {}\n", self.timestamp));
		output.push('\n');

		// Pre-flight section
		output.push_str("Pre-flight Checks: ");
		if self.preflight.passed {
			output.push_str("PASSED\n");
		} else {
			output.push_str("FAILED\n");
		}
		for check in &self.preflight.checks {
			let status = if check.passed { "OK" } else { "FAIL" };
			output.push_str(&format!(
				"  [{}] {} - {}\n",
				status, check.name, check.message
			));
		}
		output.push('\n');

		// Terraform section
		output.push_str("Terraform Changes:\n");
		output.push_str(&format!("  Create:    {}\n", self.terraform.creates));
		output.push_str(&format!("  Update:    {}\n", self.terraform.updates));
		output.push_str(&format!("  Destroy:   {}\n", self.terraform.destroys));
		output.push_str(&format!("  Unchanged: {}\n", self.terraform.unchanged));

		if self.terraform.drift {
			output.push_str("  WARNING: Drift detected!\n");
		}

		output
	}

	/// Format the report as JSON.
	pub fn format_json(&self) -> DeployResult<String> {
		serde_json::to_string_pretty(self).map_err(|e| crate::error::DeployError::Template {
			message: format!("failed to serialize deploy report: {e}"),
		})
	}

	/// Format the report as Markdown for PR comments.
	pub fn format_markdown(&self) -> String {
		let mut output = String::new();

		let title = if self.dry_run {
			"Dry-Run Report"
		} else {
			"Deploy Report"
		};
		output.push_str(&format!("## {}\n\n", title));

		output.push_str("| Field | Value |\n");
		output.push_str("|-------|-------|\n");
		output.push_str(&format!("| Environment | {} |\n", self.environment));
		output.push_str(&format!("| Provider | {} |\n", self.provider));
		output.push_str(&format!("| Commit | `{}` |\n", self.commit));
		output.push_str(&format!("| Timestamp | {} |\n", self.timestamp));
		output.push('\n');

		// Terraform summary table
		output.push_str("### Terraform Changes\n\n");
		output.push_str("| Action | Count |\n");
		output.push_str("|--------|-------|\n");
		output.push_str(&format!("| Create | {} |\n", self.terraform.creates));
		output.push_str(&format!("| Update | {} |\n", self.terraform.updates));
		output.push_str(&format!("| Destroy | {} |\n", self.terraform.destroys));
		output.push_str(&format!("| Unchanged | {} |\n", self.terraform.unchanged));

		if self.terraform.drift {
			output.push_str("\n> **Warning**: Infrastructure drift detected!\n");
		}

		output
	}
}

/// Captured plan state for comparison between two deployment plans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSnapshot {
	pub plan_hash: String,
	pub config_hash: String,
	pub state_hash: String,
	pub timestamp: String,
	pub terraform: TerraformReport,
}

/// Result of comparing two plan snapshots.
#[derive(Debug, Clone)]
pub struct DiffResult {
	pub identical: bool,
	pub differences: Vec<DiffEntry>,
}

/// Individual difference between two plan snapshots.
#[derive(Debug, Clone)]
pub struct DiffEntry {
	pub field: String,
	pub left: String,
	pub right: String,
}

/// Load a plan snapshot from a JSON file.
pub fn load_plan_snapshot(path: &Path) -> DeployResult<PlanSnapshot> {
	let content = std::fs::read_to_string(path)?;
	let snapshot: PlanSnapshot =
		serde_json::from_str(&content).map_err(|e| crate::error::DeployError::ConfigParse {
			message: format!("failed to parse plan snapshot: {e}"),
		})?;
	Ok(snapshot)
}

/// Compare two plan snapshots and return a diff result.
///
/// Compares plan_hash, config_hash, state_hash, and terraform summary
/// (creates, updates, destroys). Returns `DiffResult` with `identical=true`
/// if all fields match, otherwise lists each difference.
pub fn compare_plans(left: &PlanSnapshot, right: &PlanSnapshot) -> DiffResult {
	let mut differences = Vec::new();

	if left.plan_hash != right.plan_hash {
		differences.push(DiffEntry {
			field: "plan_hash".to_string(),
			left: left.plan_hash.clone(),
			right: right.plan_hash.clone(),
		});
	}

	if left.config_hash != right.config_hash {
		differences.push(DiffEntry {
			field: "config_hash".to_string(),
			left: left.config_hash.clone(),
			right: right.config_hash.clone(),
		});
	}

	if left.state_hash != right.state_hash {
		differences.push(DiffEntry {
			field: "state_hash".to_string(),
			left: left.state_hash.clone(),
			right: right.state_hash.clone(),
		});
	}

	if left.terraform.creates != right.terraform.creates {
		differences.push(DiffEntry {
			field: "terraform.creates".to_string(),
			left: left.terraform.creates.to_string(),
			right: right.terraform.creates.to_string(),
		});
	}

	if left.terraform.updates != right.terraform.updates {
		differences.push(DiffEntry {
			field: "terraform.updates".to_string(),
			left: left.terraform.updates.to_string(),
			right: right.terraform.updates.to_string(),
		});
	}

	if left.terraform.destroys != right.terraform.destroys {
		differences.push(DiffEntry {
			field: "terraform.destroys".to_string(),
			left: left.terraform.destroys.to_string(),
			right: right.terraform.destroys.to_string(),
		});
	}

	DiffResult {
		identical: differences.is_empty(),
		differences,
	}
}

/// Format a diff result as human-readable output.
///
/// Returns "Plans are IDENTICAL" when no differences are found, or
/// "Plans DIFFER:" followed by each difference on its own line.
pub fn format_diff(diff: &DiffResult) -> String {
	if diff.identical {
		return "Plans are IDENTICAL".to_string();
	}

	let mut output = String::from("Plans DIFFER:\n");
	for entry in &diff.differences {
		output.push_str(&format!(
			"  {}: {} -> {}\n",
			entry.field, entry.left, entry.right
		));
	}
	output
}

/// Return an exit code based on the diff result.
///
/// Returns 0 if plans are identical, 1 if plans differ.
/// Intended for use with the `--exit-code` CI flag.
pub fn diff_exit_code(diff: &DiffResult) -> i32 {
	if diff.identical { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	fn sample_report(dry_run: bool) -> DeployReport {
		DeployReport {
			version: "1.0".to_string(),
			timestamp: "2026-02-14T10:30:00Z".to_string(),
			commit: "abc1234".to_string(),
			environment: "production".to_string(),
			provider: "docker".to_string(),
			dry_run,
			preflight: PreflightReport {
				passed: true,
				checks: vec![CheckReport {
					name: "terraform".to_string(),
					passed: true,
					message: "v1.11.2".to_string(),
				}],
			},
			terraform: TerraformReport {
				creates: 3,
				updates: 2,
				destroys: 0,
				unchanged: 15,
				drift: false,
				changes: vec![],
			},
			exit_code: 2,
		}
	}

	#[rstest]
	fn report_json_serialization() {
		// Arrange
		let report = sample_report(true);

		// Act
		let json = report.format_json().unwrap();

		// Assert
		assert!(json.contains("\"version\": \"1.0\""));
		assert!(json.contains("\"dry_run\": true"));
		assert!(json.contains("\"exit_code\": 2"));
	}

	#[rstest]
	fn report_human_readable_dry_run() {
		// Arrange
		let report = sample_report(true);

		// Act
		let output = report.format_human();

		// Assert
		assert!(output.contains("DRY-RUN REPORT"));
		assert!(output.contains("abc1234"));
		assert!(output.contains("production"));
	}

	#[rstest]
	fn report_human_readable_real_deploy() {
		// Arrange
		let report = sample_report(false);

		// Act
		let output = report.format_human();

		// Assert
		assert!(output.contains("DEPLOY REPORT"));
		assert!(!output.contains("DRY-RUN"));
	}

	#[rstest]
	fn report_markdown_format() {
		// Arrange
		let report = sample_report(true);

		// Act
		let md = report.format_markdown();

		// Assert
		assert!(md.contains("## Dry-Run Report"));
		assert!(md.contains("| Create | 3 |"));
		assert!(md.contains("| Update | 2 |"));
		assert!(md.contains("| `abc1234` |"));
	}

	#[rstest]
	fn report_markdown_deploy() {
		// Arrange
		let report = sample_report(false);

		// Act
		let md = report.format_markdown();

		// Assert
		assert!(md.contains("## Deploy Report"));
	}

	#[rstest]
	fn report_human_shows_preflight_checks() {
		// Arrange
		let report = sample_report(true);

		// Act
		let output = report.format_human();

		// Assert
		assert!(output.contains("Pre-flight Checks: PASSED"));
		assert!(output.contains("[OK] terraform"));
	}

	#[rstest]
	fn report_human_shows_failed_preflight() {
		// Arrange
		let mut report = sample_report(false);
		report.preflight.passed = false;
		report.preflight.checks.push(CheckReport {
			name: "docker".to_string(),
			passed: false,
			message: "not running".to_string(),
		});

		// Act
		let output = report.format_human();

		// Assert
		assert!(output.contains("Pre-flight Checks: FAILED"));
		assert!(output.contains("[FAIL] docker"));
	}

	#[rstest]
	fn report_human_shows_drift_warning() {
		// Arrange
		let mut report = sample_report(false);
		report.terraform.drift = true;

		// Act
		let output = report.format_human();

		// Assert
		assert!(output.contains("WARNING: Drift detected!"));
	}

	#[rstest]
	fn report_markdown_shows_drift_warning() {
		// Arrange
		let mut report = sample_report(false);
		report.terraform.drift = true;

		// Act
		let md = report.format_markdown();

		// Assert
		assert!(md.contains("Infrastructure drift detected"));
	}

	#[rstest]
	fn report_json_round_trip() {
		// Arrange
		let report = sample_report(false);

		// Act
		let json = report.format_json().unwrap();
		let deserialized: DeployReport = serde_json::from_str(&json).unwrap();

		// Assert
		assert_eq!(deserialized.version, "1.0");
		assert_eq!(deserialized.commit, "abc1234");
		assert_eq!(deserialized.terraform.creates, 3);
		assert!(!deserialized.dry_run);
	}

	fn sample_plan_snapshot() -> PlanSnapshot {
		PlanSnapshot {
			plan_hash: "abc123".to_string(),
			config_hash: "def456".to_string(),
			state_hash: "ghi789".to_string(),
			timestamp: "2026-02-14T10:30:00Z".to_string(),
			terraform: TerraformReport {
				creates: 3,
				updates: 2,
				destroys: 0,
				unchanged: 15,
				drift: false,
				changes: vec![],
			},
		}
	}

	#[rstest]
	fn load_plan_snapshot_from_json_file() {
		// Arrange
		let snapshot = sample_plan_snapshot();
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("plan.json");
		let json = serde_json::to_string_pretty(&snapshot).unwrap();
		std::fs::write(&path, json).unwrap();

		// Act
		let loaded = load_plan_snapshot(&path).unwrap();

		// Assert
		assert_eq!(loaded.plan_hash, "abc123");
		assert_eq!(loaded.config_hash, "def456");
		assert_eq!(loaded.state_hash, "ghi789");
		assert_eq!(loaded.terraform.creates, 3);
	}

	#[rstest]
	fn load_plan_snapshot_returns_error_for_missing_file() {
		// Arrange
		let path = std::path::Path::new("/tmp/nonexistent_plan_snapshot.json");

		// Act
		let result = load_plan_snapshot(path);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn compare_identical_plans_returns_identical() {
		// Arrange
		let left = sample_plan_snapshot();
		let right = sample_plan_snapshot();

		// Act
		let diff = compare_plans(&left, &right);

		// Assert
		assert!(diff.identical);
		assert!(diff.differences.is_empty());
	}

	#[rstest]
	fn compare_plans_with_different_plan_hash() {
		// Arrange
		let left = sample_plan_snapshot();
		let mut right = sample_plan_snapshot();
		right.plan_hash = "xyz999".to_string();

		// Act
		let diff = compare_plans(&left, &right);

		// Assert
		assert!(!diff.identical);
		assert_eq!(diff.differences.len(), 1);
		assert_eq!(diff.differences[0].field, "plan_hash");
		assert_eq!(diff.differences[0].left, "abc123");
		assert_eq!(diff.differences[0].right, "xyz999");
	}

	#[rstest]
	fn compare_plans_with_different_config_hash() {
		// Arrange
		let left = sample_plan_snapshot();
		let mut right = sample_plan_snapshot();
		right.config_hash = "changed".to_string();

		// Act
		let diff = compare_plans(&left, &right);

		// Assert
		assert!(!diff.identical);
		assert_eq!(diff.differences.len(), 1);
		assert_eq!(diff.differences[0].field, "config_hash");
		assert_eq!(diff.differences[0].left, "def456");
		assert_eq!(diff.differences[0].right, "changed");
	}

	#[rstest]
	fn compare_plans_with_different_state_hash() {
		// Arrange
		let left = sample_plan_snapshot();
		let mut right = sample_plan_snapshot();
		right.state_hash = "new_state".to_string();

		// Act
		let diff = compare_plans(&left, &right);

		// Assert
		assert!(!diff.identical);
		assert_eq!(diff.differences.len(), 1);
		assert_eq!(diff.differences[0].field, "state_hash");
		assert_eq!(diff.differences[0].left, "ghi789");
		assert_eq!(diff.differences[0].right, "new_state");
	}

	#[rstest]
	fn compare_plans_with_different_terraform_creates() {
		// Arrange
		let left = sample_plan_snapshot();
		let mut right = sample_plan_snapshot();
		right.terraform.creates = 10;

		// Act
		let diff = compare_plans(&left, &right);

		// Assert
		assert!(!diff.identical);
		assert_eq!(diff.differences.len(), 1);
		assert_eq!(diff.differences[0].field, "terraform.creates");
		assert_eq!(diff.differences[0].left, "3");
		assert_eq!(diff.differences[0].right, "10");
	}

	#[rstest]
	fn compare_plans_with_multiple_differences() {
		// Arrange
		let left = sample_plan_snapshot();
		let mut right = sample_plan_snapshot();
		right.plan_hash = "different_plan".to_string();
		right.terraform.updates = 99;
		right.terraform.destroys = 5;

		// Act
		let diff = compare_plans(&left, &right);

		// Assert
		assert!(!diff.identical);
		assert_eq!(diff.differences.len(), 3);
		let fields: Vec<&str> = diff.differences.iter().map(|d| d.field.as_str()).collect();
		assert!(fields.contains(&"plan_hash"));
		assert!(fields.contains(&"terraform.updates"));
		assert!(fields.contains(&"terraform.destroys"));
	}

	#[rstest]
	fn format_diff_for_identical_plans() {
		// Arrange
		let diff = DiffResult {
			identical: true,
			differences: vec![],
		};

		// Act
		let output = format_diff(&diff);

		// Assert
		assert_eq!(output, "Plans are IDENTICAL");
	}

	#[rstest]
	fn format_diff_for_different_plans_shows_field_details() {
		// Arrange
		let diff = DiffResult {
			identical: false,
			differences: vec![
				DiffEntry {
					field: "plan_hash".to_string(),
					left: "abc".to_string(),
					right: "xyz".to_string(),
				},
				DiffEntry {
					field: "terraform.creates".to_string(),
					left: "3".to_string(),
					right: "5".to_string(),
				},
			],
		};

		// Act
		let output = format_diff(&diff);

		// Assert
		assert!(output.contains("Plans DIFFER:"));
		assert!(output.contains("plan_hash: abc -> xyz"));
		assert!(output.contains("terraform.creates: 3 -> 5"));
	}

	#[rstest]
	fn diff_exit_code_zero_for_identical() {
		// Arrange
		let diff = DiffResult {
			identical: true,
			differences: vec![],
		};

		// Act
		let code = diff_exit_code(&diff);

		// Assert
		assert_eq!(code, 0);
	}

	#[rstest]
	fn diff_exit_code_one_for_different() {
		// Arrange
		let diff = DiffResult {
			identical: false,
			differences: vec![DiffEntry {
				field: "plan_hash".to_string(),
				left: "a".to_string(),
				right: "b".to_string(),
			}],
		};

		// Act
		let code = diff_exit_code(&diff);

		// Assert
		assert_eq!(code, 1);
	}
}
