//! Deploy pipeline orchestration.
//!
//! Defines the pipeline structure, stage ordering, and result tracking
//! for the deployment process.

/// Options controlling pipeline behavior.
pub struct PipelineOptions {
	pub dry_run: bool,
	pub preview: bool,
	pub environment: String,
}

impl Default for PipelineOptions {
	fn default() -> Self {
		Self {
			dry_run: false,
			preview: false,
			environment: "production".to_string(),
		}
	}
}

/// Stages of the deployment pipeline in execution order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineStage {
	ConfigParse,
	FeatureDetection,
	PreflightChecks,
	Build,
	TerraformGenerate,
	TerraformInit,
	TerraformPlan,
}

impl PipelineStage {
	/// Return all stages in their execution order.
	pub fn ordered() -> Vec<PipelineStage> {
		vec![
			PipelineStage::ConfigParse,
			PipelineStage::FeatureDetection,
			PipelineStage::PreflightChecks,
			PipelineStage::Build,
			PipelineStage::TerraformGenerate,
			PipelineStage::TerraformInit,
			PipelineStage::TerraformPlan,
		]
	}
}

/// Result of a stage execution.
pub struct StageResult {
	pub stage: PipelineStage,
	pub success: bool,
	pub message: String,
	pub duration_ms: u64,
}

/// Result of the entire pipeline execution.
pub struct PipelineResult {
	pub stages: Vec<StageResult>,
	pub success: bool,
	pub dry_run: bool,
}

impl PipelineResult {
	/// Create a new pipeline result with no completed stages.
	pub fn new(dry_run: bool) -> Self {
		Self {
			stages: Vec::new(),
			success: true,
			dry_run,
		}
	}

	/// Record a stage result, updating overall success status.
	pub fn add_stage(&mut self, result: StageResult) {
		if !result.success {
			self.success = false;
		}
		self.stages.push(result);
	}

	/// Return the first failed stage, if any.
	pub fn failed_stage(&self) -> Option<&StageResult> {
		self.stages.iter().find(|s| !s.success)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn pipeline_options_default() {
		// Arrange & Act
		let opts = PipelineOptions::default();

		// Assert
		assert!(!opts.dry_run);
		assert!(!opts.preview);
		assert_eq!(opts.environment, "production");
	}

	#[rstest]
	fn pipeline_options_dry_run() {
		// Arrange & Act
		let opts = PipelineOptions {
			dry_run: true,
			..PipelineOptions::default()
		};

		// Assert
		assert!(opts.dry_run);
	}

	#[rstest]
	fn pipeline_stage_ordering() {
		// Arrange
		let stages = PipelineStage::ordered();

		// Act & Assert
		assert_eq!(stages[0], PipelineStage::ConfigParse);
		assert_eq!(stages[1], PipelineStage::FeatureDetection);
		assert_eq!(stages[2], PipelineStage::PreflightChecks);
		assert_eq!(stages[3], PipelineStage::Build);
		assert_eq!(stages[4], PipelineStage::TerraformGenerate);
		assert_eq!(stages[5], PipelineStage::TerraformInit);
		assert_eq!(stages[6], PipelineStage::TerraformPlan);
	}

	#[rstest]
	fn pipeline_result_new_starts_successful() {
		// Arrange & Act
		let result = PipelineResult::new(false);

		// Assert
		assert!(result.success);
		assert!(!result.dry_run);
		assert!(result.stages.is_empty());
	}

	#[rstest]
	fn pipeline_result_add_failed_stage_sets_failure() {
		// Arrange
		let mut result = PipelineResult::new(false);
		let stage_result = StageResult {
			stage: PipelineStage::PreflightChecks,
			success: false,
			message: "Docker not found".to_string(),
			duration_ms: 50,
		};

		// Act
		result.add_stage(stage_result);

		// Assert
		assert!(!result.success);
		assert_eq!(result.stages.len(), 1);
	}

	#[rstest]
	fn pipeline_result_failed_stage_returns_first_failure() {
		// Arrange
		let mut result = PipelineResult::new(false);
		result.add_stage(StageResult {
			stage: PipelineStage::ConfigParse,
			success: true,
			message: "OK".to_string(),
			duration_ms: 10,
		});
		result.add_stage(StageResult {
			stage: PipelineStage::PreflightChecks,
			success: false,
			message: "Docker not found".to_string(),
			duration_ms: 50,
		});

		// Act
		let failed = result.failed_stage();

		// Assert
		assert!(failed.is_some());
		assert_eq!(failed.unwrap().stage, PipelineStage::PreflightChecks);
	}

	#[rstest]
	fn pipeline_result_no_failure() {
		// Arrange
		let mut result = PipelineResult::new(true);
		result.add_stage(StageResult {
			stage: PipelineStage::ConfigParse,
			success: true,
			message: "OK".to_string(),
			duration_ms: 10,
		});

		// Act
		let failed = result.failed_stage();

		// Assert
		assert!(failed.is_none());
		assert!(result.success);
		assert!(result.dry_run);
	}
}
