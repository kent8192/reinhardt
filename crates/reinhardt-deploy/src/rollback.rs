//! Deployment history and rollback support.
//!
//! Tracks deployment history in `.reinhardt/deployments.toml` and provides
//! rollback target resolution.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::DeployResult;

/// Status of a deployment entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatus {
	Active,
	Inactive,
	RolledBack,
}

/// A single deployment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentEntry {
	pub version: u32,
	pub commit: String,
	pub image: String,
	pub timestamp: String,
	pub environment: String,
	pub status: DeploymentStatus,
}

/// Deployment history for a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeploymentHistory {
	#[serde(default)]
	pub deployments: Vec<DeploymentEntry>,
}

impl DeploymentHistory {
	/// Find the rollback target.
	///
	/// If `target_version` is None, returns the most recent inactive deployment.
	/// If `target_version` is Some, returns the deployment with that version.
	pub fn rollback_target(&self, target_version: Option<u32>) -> Option<&DeploymentEntry> {
		match target_version {
			Some(version) => self.deployments.iter().find(|d| d.version == version),
			None => {
				// Find the most recent inactive deployment
				self.deployments
					.iter()
					.rev()
					.find(|d| d.status == DeploymentStatus::Inactive)
			}
		}
	}

	/// Get the next version number.
	pub fn next_version(&self) -> u32 {
		self.deployments
			.iter()
			.map(|d| d.version)
			.max()
			.unwrap_or(0)
			+ 1
	}

	/// Get the currently active deployment.
	pub fn active_deployment(&self) -> Option<&DeploymentEntry> {
		self.deployments
			.iter()
			.rev()
			.find(|d| d.status == DeploymentStatus::Active)
	}
}

/// Load deployment history from the project directory.
///
/// Reads `.reinhardt/deployments.toml`. Returns an empty history if the
/// file does not exist.
pub fn load_history(project_root: &Path) -> DeployResult<DeploymentHistory> {
	let history_path = project_root.join(".reinhardt").join("deployments.toml");

	if !history_path.exists() {
		return Ok(DeploymentHistory::default());
	}

	let content = std::fs::read_to_string(&history_path)?;
	let history: DeploymentHistory =
		toml::from_str(&content).map_err(|e| crate::error::DeployError::ConfigParse {
			message: format!("failed to parse deployment history: {e}"),
		})?;

	Ok(history)
}

/// Record a deployment entry to the project's history file.
///
/// Creates the `.reinhardt/` directory if it does not exist.
/// Appends the entry to the existing history.
pub fn record_deployment(project_root: &Path, entry: &DeploymentEntry) -> DeployResult<()> {
	let reinhardt_dir = project_root.join(".reinhardt");
	if !reinhardt_dir.exists() {
		std::fs::create_dir_all(&reinhardt_dir)?;
	}

	let mut history = load_history(project_root)?;
	history.deployments.push(entry.clone());

	let content =
		toml::to_string_pretty(&history).map_err(|e| crate::error::DeployError::ConfigParse {
			message: format!("failed to serialize deployment history: {e}"),
		})?;

	let history_path = reinhardt_dir.join("deployments.toml");
	std::fs::write(&history_path, content)?;

	Ok(())
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	fn sample_entry(version: u32, status: DeploymentStatus) -> DeploymentEntry {
		DeploymentEntry {
			version,
			commit: format!("commit{version}"),
			image: format!("img:v{version}"),
			timestamp: format!("2026-02-14T{version}:00:00Z"),
			environment: "production".to_string(),
			status,
		}
	}

	#[rstest]
	fn record_and_load_deployment() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let entry = sample_entry(1, DeploymentStatus::Active);

		// Act
		record_deployment(tmp.path(), &entry).unwrap();
		let history = load_history(tmp.path()).unwrap();

		// Assert
		assert_eq!(history.deployments.len(), 1);
		assert_eq!(history.deployments[0].version, 1);
		assert_eq!(history.deployments[0].status, DeploymentStatus::Active);
	}

	#[rstest]
	fn rollback_target_previous() {
		// Arrange
		let history = DeploymentHistory {
			deployments: vec![
				sample_entry(1, DeploymentStatus::Inactive),
				sample_entry(2, DeploymentStatus::Active),
			],
		};

		// Act
		let target = history.rollback_target(None);

		// Assert
		assert!(target.is_some());
		assert_eq!(target.unwrap().version, 1);
	}

	#[rstest]
	fn rollback_target_specific_version() {
		// Arrange
		let history = DeploymentHistory {
			deployments: vec![
				sample_entry(1, DeploymentStatus::Inactive),
				sample_entry(2, DeploymentStatus::Inactive),
				sample_entry(3, DeploymentStatus::Active),
			],
		};

		// Act
		let target = history.rollback_target(Some(1));

		// Assert
		assert!(target.is_some());
		assert_eq!(target.unwrap().version, 1);
	}

	#[rstest]
	fn rollback_target_none_when_empty() {
		// Arrange
		let history = DeploymentHistory::default();

		// Act
		let target = history.rollback_target(None);

		// Assert
		assert!(target.is_none());
	}

	#[rstest]
	fn next_version_from_empty() {
		// Arrange
		let history = DeploymentHistory::default();

		// Act
		let next = history.next_version();

		// Assert
		assert_eq!(next, 1);
	}

	#[rstest]
	fn next_version_increments() {
		// Arrange
		let history = DeploymentHistory {
			deployments: vec![
				sample_entry(1, DeploymentStatus::Inactive),
				sample_entry(2, DeploymentStatus::Active),
			],
		};

		// Act
		let next = history.next_version();

		// Assert
		assert_eq!(next, 3);
	}

	#[rstest]
	fn active_deployment_found() {
		// Arrange
		let history = DeploymentHistory {
			deployments: vec![
				sample_entry(1, DeploymentStatus::Inactive),
				sample_entry(2, DeploymentStatus::Active),
			],
		};

		// Act
		let active = history.active_deployment();

		// Assert
		assert!(active.is_some());
		assert_eq!(active.unwrap().version, 2);
	}

	#[rstest]
	fn active_deployment_none() {
		// Arrange
		let history = DeploymentHistory {
			deployments: vec![sample_entry(1, DeploymentStatus::Inactive)],
		};

		// Act
		let active = history.active_deployment();

		// Assert
		assert!(active.is_none());
	}

	#[rstest]
	fn load_history_no_file() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();

		// Act
		let history = load_history(tmp.path()).unwrap();

		// Assert
		assert!(history.deployments.is_empty());
	}

	#[rstest]
	fn record_multiple_deployments() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();

		// Act
		record_deployment(tmp.path(), &sample_entry(1, DeploymentStatus::Active)).unwrap();
		record_deployment(tmp.path(), &sample_entry(2, DeploymentStatus::Active)).unwrap();
		let history = load_history(tmp.path()).unwrap();

		// Assert
		assert_eq!(history.deployments.len(), 2);
	}

	#[rstest]
	fn load_history_invalid_toml() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let reinhardt_dir = tmp.path().join(".reinhardt");
		std::fs::create_dir_all(&reinhardt_dir).unwrap();
		std::fs::write(reinhardt_dir.join("deployments.toml"), "not valid [[[").unwrap();

		// Act
		let result = load_history(tmp.path());

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn rollback_target_nonexistent_version() {
		// Arrange
		let history = DeploymentHistory {
			deployments: vec![
				sample_entry(1, DeploymentStatus::Inactive),
				sample_entry(2, DeploymentStatus::Active),
			],
		};

		// Act
		let target = history.rollback_target(Some(99));

		// Assert
		assert!(target.is_none());
	}

	#[rstest]
	fn rollback_target_no_inactive_deployments() {
		// Arrange
		let history = DeploymentHistory {
			deployments: vec![
				sample_entry(1, DeploymentStatus::Active),
				sample_entry(2, DeploymentStatus::RolledBack),
			],
		};

		// Act
		let target = history.rollback_target(None);

		// Assert
		assert!(target.is_none());
	}
}
