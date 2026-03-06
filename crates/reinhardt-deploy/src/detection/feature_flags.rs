use crate::config::{DatabaseEngine, NoSqlEngine};

/// Result of analyzing Cargo feature flags to detect infrastructure requirements.
#[derive(Debug, Clone, Default)]
pub struct FeatureDetectionResult {
	pub database: bool,
	pub database_engine: Option<DatabaseEngine>,
	pub nosql: bool,
	pub nosql_engines: Vec<NoSqlEngine>,
	pub cache: bool,
	pub websockets: bool,
	pub frontend: bool,
	pub static_files: bool,
	pub media: bool,
	pub background_tasks: bool,
	pub mail: bool,
	pub wasm: bool,
	pub ambiguous: bool,
	pub model_count: usize,
	pub confidence: f64,
}

/// Analyze Cargo feature flags to detect infrastructure requirements.
///
/// Maps known Reinhardt feature flags to their corresponding infrastructure
/// needs. Specific features yield high confidence (1.0), while the "full"
/// meta-feature yields low confidence (0.3) because it enables everything
/// without indicating what is actually used.
pub fn analyze_feature_flags(features: &[String]) -> FeatureDetectionResult {
	let mut result = FeatureDetectionResult {
		database: false,
		database_engine: None,
		nosql: false,
		nosql_engines: Vec::new(),
		cache: false,
		websockets: false,
		frontend: false,
		static_files: false,
		media: false,
		background_tasks: false,
		mail: false,
		wasm: false,
		ambiguous: false,
		model_count: 0,
		confidence: 1.0,
	};

	for feature in features {
		match feature.as_str() {
			"db-postgres" => {
				result.database = true;
				result.database_engine = Some(DatabaseEngine::PostgreSql);
			}
			"db-mysql" => {
				result.database = true;
				result.database_engine = Some(DatabaseEngine::MySql);
			}
			"nosql-mongodb" => {
				result.nosql = true;
				result.nosql_engines.push(NoSqlEngine::MongoDb);
			}
			"nosql-dynamodb" => {
				result.nosql = true;
				result.nosql_engines.push(NoSqlEngine::DynamoDb);
			}
			"nosql-firestore" => {
				result.nosql = true;
				result.nosql_engines.push(NoSqlEngine::Firestore);
			}
			"websockets" => {
				result.websockets = true;
			}
			"cache" => {
				result.cache = true;
			}
			"pages" => {
				result.frontend = true;
			}
			"static-files" => {
				result.static_files = true;
			}
			"media" => {
				result.media = true;
			}
			"tasks" => {
				result.background_tasks = true;
			}
			"mail" => {
				result.mail = true;
			}
			"wasm" | "wasm-frontend" => {
				result.wasm = true;
			}
			"full" => {
				result.ambiguous = true;
				result.confidence = 0.3;
			}
			_ => {}
		}
	}

	result
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn detect_postgres_from_features() {
		// Arrange
		let features = vec!["db-postgres".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.database);
		assert_eq!(result.database_engine, Some(DatabaseEngine::PostgreSql));
		assert_eq!(result.confidence, 1.0);
	}

	#[rstest]
	fn detect_mysql_from_features() {
		// Arrange
		let features = vec!["db-mysql".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.database);
		assert_eq!(result.database_engine, Some(DatabaseEngine::MySql));
		assert_eq!(result.confidence, 1.0);
	}

	#[rstest]
	fn detect_full_returns_ambiguous() {
		// Arrange
		let features = vec!["full".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.ambiguous);
		assert_eq!(result.confidence, 0.3);
	}

	#[rstest]
	fn detect_multiple_features() {
		// Arrange
		let features = vec![
			"db-postgres".to_string(),
			"websockets".to_string(),
			"cache".to_string(),
			"pages".to_string(),
		];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.database);
		assert!(result.websockets);
		assert!(result.cache);
		assert!(result.frontend);
		assert!(!result.ambiguous);
		assert_eq!(result.confidence, 1.0);
	}

	#[rstest]
	fn detect_nosql_mongodb() {
		// Arrange
		let features = vec!["nosql-mongodb".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.nosql);
		assert_eq!(result.nosql_engines, vec![NoSqlEngine::MongoDb]);
	}

	#[rstest]
	fn detect_nosql_dynamodb() {
		// Arrange
		let features = vec!["nosql-dynamodb".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.nosql);
		assert_eq!(result.nosql_engines, vec![NoSqlEngine::DynamoDb]);
	}

	#[rstest]
	fn detect_nosql_firestore() {
		// Arrange
		let features = vec!["nosql-firestore".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.nosql);
		assert_eq!(result.nosql_engines, vec![NoSqlEngine::Firestore]);
	}

	#[rstest]
	fn detect_static_files() {
		// Arrange
		let features = vec!["static-files".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.static_files);
	}

	#[rstest]
	fn detect_media() {
		// Arrange
		let features = vec!["media".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.media);
	}

	#[rstest]
	fn detect_background_tasks() {
		// Arrange
		let features = vec!["tasks".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.background_tasks);
	}

	#[rstest]
	fn detect_mail() {
		// Arrange
		let features = vec!["mail".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.mail);
	}

	#[rstest]
	fn detect_wasm() {
		// Arrange
		let features = vec!["wasm".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.wasm);
	}

	#[rstest]
	fn detect_wasm_frontend() {
		// Arrange
		let features = vec!["wasm-frontend".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.wasm);
	}

	#[rstest]
	fn empty_features_returns_defaults() {
		// Arrange
		let features: Vec<String> = vec![];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(!result.database);
		assert!(result.database_engine.is_none());
		assert!(!result.nosql);
		assert!(result.nosql_engines.is_empty());
		assert!(!result.cache);
		assert!(!result.websockets);
		assert!(!result.frontend);
		assert!(!result.static_files);
		assert!(!result.media);
		assert!(!result.background_tasks);
		assert!(!result.mail);
		assert!(!result.wasm);
		assert!(!result.ambiguous);
		assert_eq!(result.model_count, 0);
		assert_eq!(result.confidence, 1.0);
	}

	#[rstest]
	fn unknown_features_are_ignored() {
		// Arrange
		let features = vec!["unknown-feature".to_string(), "another-one".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(!result.database);
		assert!(!result.nosql);
		assert!(!result.cache);
		assert!(!result.websockets);
		assert!(!result.frontend);
		assert_eq!(result.confidence, 1.0);
	}

	#[rstest]
	fn full_with_specific_features_stays_ambiguous() {
		// Arrange
		let features = vec!["full".to_string(), "db-postgres".to_string()];

		// Act
		let result = analyze_feature_flags(&features);

		// Assert
		assert!(result.ambiguous);
		assert!(result.database);
		assert_eq!(result.database_engine, Some(DatabaseEngine::PostgreSql));
		assert_eq!(result.confidence, 0.3);
	}
}
