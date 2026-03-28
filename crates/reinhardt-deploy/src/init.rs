use std::path::Path;

use crate::config::{DatabaseEngine, NoSqlEngine, ProviderType};
use crate::detection::FeatureDetectionResult;
use crate::error::DeployResult;

/// Generate deploy.toml content from detection results.
///
/// Creates a TOML configuration string based on detected features,
/// provider selection, and project name. Only includes sections for
/// features that were detected.
pub fn generate_deploy_toml(
	app_name: &str,
	provider: ProviderType,
	detection: &FeatureDetectionResult,
) -> String {
	let mut sections = Vec::new();

	// [project]
	sections.push(format!("[project]\nname = \"{}\"", app_name));

	// [provider]
	let provider_str = match provider {
		ProviderType::Docker => "docker",
		ProviderType::FlyIo => "fly",
		ProviderType::Aws => "aws",
		ProviderType::Gcp => "gcp",
	};
	sections.push(format!("[provider]\ntype = \"{}\"", provider_str));

	// [database] - only if detected
	if detection.database {
		let engine = match &detection.database_engine {
			Some(DatabaseEngine::PostgreSql) => "postgresql",
			Some(DatabaseEngine::MySql) => "mysql",
			None => "postgresql", // default
		};
		sections.push(format!("[database]\nengine = \"{}\"", engine));
	}

	// [nosql] - only if detected
	if detection.nosql {
		if detection.nosql_engines.is_empty() {
			sections.push("[[nosql]]\nengine = \"mongodb\"".to_string());
		} else {
			for engine in &detection.nosql_engines {
				let engine_str = match engine {
					NoSqlEngine::MongoDb => "mongodb",
					NoSqlEngine::DynamoDb => "dynamodb",
					NoSqlEngine::Firestore => "firestore",
				};
				sections.push(format!("[[nosql]]\nengine = \"{}\"", engine_str));
			}
		}
	}

	// [cache] - only if detected
	if detection.cache {
		sections.push("[cache]\nengine = \"redis\"".to_string());
	}

	// [websockets] - only if detected
	if detection.websockets {
		sections.push("[websockets]\nenabled = true".to_string());
	}

	// [frontend] - only if detected
	if detection.frontend {
		sections.push("[frontend]\nbuilder = \"npm\"".to_string());
	}

	// [tasks] - only if detected
	if detection.background_tasks {
		sections.push("[tasks]\nworker_count = 1".to_string());
	}

	// [mail] - only if detected
	if detection.mail {
		sections.push("[mail]\nprovider = \"smtp\"".to_string());
	}

	sections.join("\n\n") + "\n"
}

/// Write deploy.toml content to the given project directory.
pub fn write_deploy_toml(project_root: &Path, content: &str) -> DeployResult<()> {
	let path = project_root.join("deploy.toml");
	std::fs::write(&path, content)?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::{DatabaseEngine, NoSqlEngine, ProviderType};
	use crate::detection::FeatureDetectionResult;
	use rstest::rstest;

	#[rstest]
	fn generate_deploy_toml_minimal() {
		// Arrange
		let detection = FeatureDetectionResult {
			database: true,
			database_engine: Some(DatabaseEngine::PostgreSql),
			..Default::default()
		};

		// Act
		let toml_content = generate_deploy_toml("myapp", ProviderType::Docker, &detection);

		// Assert
		assert!(toml_content.contains("[project]"));
		assert!(toml_content.contains("name = \"myapp\""));
		assert!(toml_content.contains("[database]"));
		assert!(toml_content.contains("engine = \"postgresql\""));
	}

	#[rstest]
	fn generate_deploy_toml_with_nosql() {
		// Arrange
		let detection = FeatureDetectionResult {
			database: true,
			database_engine: Some(DatabaseEngine::PostgreSql),
			nosql: true,
			nosql_engines: vec![NoSqlEngine::MongoDb],
			..Default::default()
		};

		// Act
		let toml_content = generate_deploy_toml("myapp", ProviderType::Aws, &detection);

		// Assert
		assert!(toml_content.contains("[[nosql]]"));
		assert!(toml_content.contains("engine = \"mongodb\""));
	}

	#[rstest]
	fn write_deploy_toml_to_file() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let content = "[project]\nname = \"test\"\n";

		// Act
		write_deploy_toml(tmp.path(), content).unwrap();

		// Assert
		assert!(tmp.path().join("deploy.toml").exists());
		let read_content = std::fs::read_to_string(tmp.path().join("deploy.toml")).unwrap();
		assert_eq!(read_content, content);
	}

	#[rstest]
	fn generate_deploy_toml_all_features() {
		// Arrange
		let detection = FeatureDetectionResult {
			database: true,
			database_engine: Some(DatabaseEngine::MySql),
			nosql: true,
			nosql_engines: vec![NoSqlEngine::DynamoDb],
			cache: true,
			websockets: true,
			frontend: true,
			background_tasks: true,
			mail: true,
			..Default::default()
		};

		// Act
		let toml_content = generate_deploy_toml("fullapp", ProviderType::Gcp, &detection);

		// Assert
		assert!(toml_content.contains("[project]"));
		assert!(toml_content.contains("name = \"fullapp\""));
		assert!(toml_content.contains("[provider]"));
		assert!(toml_content.contains("type = \"gcp\""));
		assert!(toml_content.contains("[database]"));
		assert!(toml_content.contains("engine = \"mysql\""));
		assert!(toml_content.contains("[[nosql]]"));
		assert!(toml_content.contains("engine = \"dynamodb\""));
		assert!(toml_content.contains("[cache]"));
		assert!(toml_content.contains("[websockets]"));
		assert!(toml_content.contains("[frontend]"));
		assert!(toml_content.contains("[tasks]"));
		assert!(toml_content.contains("[mail]"));
	}

	#[rstest]
	fn generate_deploy_toml_no_features() {
		// Arrange
		let detection = FeatureDetectionResult::default();

		// Act
		let toml_content = generate_deploy_toml("basic", ProviderType::Docker, &detection);

		// Assert
		assert!(toml_content.contains("[project]"));
		assert!(toml_content.contains("name = \"basic\""));
		assert!(toml_content.contains("[provider]"));
		assert!(toml_content.contains("type = \"docker\""));
		// No optional sections
		assert!(!toml_content.contains("[database]"));
		assert!(!toml_content.contains("[[nosql]]"));
		assert!(!toml_content.contains("[cache]"));
	}

	#[rstest]
	fn generate_deploy_toml_fly_provider() {
		// Arrange
		let detection = FeatureDetectionResult::default();

		// Act
		let toml_content = generate_deploy_toml("flyapp", ProviderType::FlyIo, &detection);

		// Assert
		assert!(toml_content.contains("type = \"fly\""));
	}
}
