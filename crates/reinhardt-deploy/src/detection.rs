pub mod code_analysis;
pub mod feature_flags;
pub mod wizard;

use std::path::Path;

use crate::error::{DeployError, DeployResult};

pub use code_analysis::analyze_code;
pub use feature_flags::{FeatureDetectionResult, analyze_feature_flags};

/// Extract Reinhardt dependency features from the project's `Cargo.toml`.
///
/// Parses `Cargo.toml` as a generic [`toml::Value`] and navigates to
/// `dependencies.reinhardt.features` or `dependencies.reinhardt-web.features`.
/// Returns the list of feature strings.
///
/// Returns an empty `Vec` if:
/// - `Cargo.toml` does not exist
/// - The reinhardt dependency is not found
/// - The dependency has no `features` array
pub fn extract_reinhardt_features(project_root: &Path) -> DeployResult<Vec<String>> {
	let cargo_toml_path = project_root.join("Cargo.toml");

	if !cargo_toml_path.exists() {
		return Ok(Vec::new());
	}

	let content =
		std::fs::read_to_string(&cargo_toml_path).map_err(|e| DeployError::Detection {
			message: format!("failed to read Cargo.toml: {e}"),
		})?;

	let value: toml::Value = toml::from_str(&content).map_err(|e| DeployError::Detection {
		message: format!("failed to parse Cargo.toml: {e}"),
	})?;

	let deps = match value.get("dependencies") {
		Some(deps) => deps,
		None => return Ok(Vec::new()),
	};

	let reinhardt_dep = deps.get("reinhardt").or_else(|| deps.get("reinhardt-web"));

	let reinhardt_dep = match reinhardt_dep {
		Some(dep) => dep,
		None => return Ok(Vec::new()),
	};

	let features = match reinhardt_dep.get("features") {
		Some(toml::Value::Array(arr)) => arr
			.iter()
			.filter_map(|v| v.as_str().map(str::to_owned))
			.collect(),
		_ => Vec::new(),
	};

	Ok(features)
}

/// Unified feature detection pipeline that orchestrates all detection layers.
///
/// Combines three detection layers in order:
/// 1. **Feature flags** (Layer 1): Parse `Cargo.toml` feature flags for high-confidence detection.
/// 2. **Code analysis** (Layer 2): Scan source files for `use reinhardt::*` imports and
///    `#[model(` attributes. Only invoked when Layer 1 is ambiguous or mostly empty.
/// 3. **Interactive wizard** (Layer 3): Present results for user confirmation when
///    `interactive` is `true` and the result is still ambiguous.
///
/// Results from multiple layers are OR-merged: if any layer detects a feature,
/// it is considered detected. Confidence is set to the maximum value from all
/// contributing layers.
pub fn detect_features(
	project_root: &Path,
	interactive: bool,
) -> DeployResult<FeatureDetectionResult> {
	// Layer 1: Feature flags from Cargo.toml
	let features = extract_reinhardt_features(project_root)?;
	let layer1 = analyze_feature_flags(&features);

	let needs_code_analysis = layer1.ambiguous || is_mostly_empty(&layer1);

	let mut result = if needs_code_analysis {
		// Layer 2: Code analysis fallback
		let layer2 = analyze_code(project_root)?;
		merge_results(layer1, layer2)
	} else {
		layer1
	};

	// Layer 3: Interactive wizard (only in interactive mode)
	if interactive && result.ambiguous {
		wizard::present_detection_results(&mut result)?;
	}

	Ok(result)
}

/// Check whether a detection result has no specific feature detections.
fn is_mostly_empty(result: &FeatureDetectionResult) -> bool {
	!result.database
		&& !result.nosql
		&& !result.cache
		&& !result.websockets
		&& !result.frontend
		&& !result.static_files
		&& !result.media
		&& !result.background_tasks
		&& !result.mail
		&& !result.wasm
}

/// Merge detection results from two layers using OR semantics.
///
/// Boolean feature flags are OR-merged (if either layer detects a feature, it is
/// detected). Optional engine fields prefer Layer 1's value if present. Model
/// count takes the maximum from either layer. Confidence is set based on which
/// layers contributed: if Layer 1 had no detections, Layer 2's confidence is
/// used; otherwise the maximum of both.
fn merge_results(
	layer1: FeatureDetectionResult,
	layer2: FeatureDetectionResult,
) -> FeatureDetectionResult {
	let layer1_empty = is_mostly_empty(&layer1);

	FeatureDetectionResult {
		database: layer1.database || layer2.database,
		database_engine: layer1.database_engine.or(layer2.database_engine),
		nosql: layer1.nosql || layer2.nosql,
		nosql_engines: {
			let mut engines = layer1.nosql_engines;
			for engine in layer2.nosql_engines {
				if !engines.contains(&engine) {
					engines.push(engine);
				}
			}
			engines
		},
		cache: layer1.cache || layer2.cache,
		websockets: layer1.websockets || layer2.websockets,
		frontend: layer1.frontend || layer2.frontend,
		static_files: layer1.static_files || layer2.static_files,
		media: layer1.media || layer2.media,
		background_tasks: layer1.background_tasks || layer2.background_tasks,
		mail: layer1.mail || layer2.mail,
		wasm: layer1.wasm || layer2.wasm,
		ambiguous: layer1.ambiguous,
		model_count: layer1.model_count.max(layer2.model_count),
		confidence: if layer1_empty {
			layer2.confidence
		} else {
			f64::max(layer1.confidence, layer2.confidence)
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn unified_detection_with_specific_features() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let cargo_toml = tmp.path().join("Cargo.toml");
		std::fs::write(
			&cargo_toml,
			r#"
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
reinhardt = { version = "0.1", features = ["db-postgres", "pages"] }
"#,
		)
		.unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

		// Act
		let result = detect_features(tmp.path(), false).unwrap();

		// Assert
		assert!(result.database);
		assert!(result.frontend);
		assert!(!result.ambiguous);
	}

	#[rstest]
	fn extract_features_from_cargo_toml() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let cargo_toml = tmp.path().join("Cargo.toml");
		std::fs::write(
			&cargo_toml,
			r#"
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
reinhardt = { version = "0.1", features = ["db-postgres", "websockets"] }
"#,
		)
		.unwrap();

		// Act
		let features = extract_reinhardt_features(tmp.path()).unwrap();

		// Assert
		assert!(features.contains(&"db-postgres".to_string()));
		assert!(features.contains(&"websockets".to_string()));
	}

	#[rstest]
	fn extract_features_missing_cargo_toml() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();

		// Act
		let features = extract_reinhardt_features(tmp.path()).unwrap();

		// Assert
		assert!(features.is_empty());
	}

	#[rstest]
	fn extract_features_no_reinhardt_dependency() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let cargo_toml = tmp.path().join("Cargo.toml");
		std::fs::write(
			&cargo_toml,
			r#"
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
serde = "1.0"
"#,
		)
		.unwrap();

		// Act
		let features = extract_reinhardt_features(tmp.path()).unwrap();

		// Assert
		assert!(features.is_empty());
	}

	#[rstest]
	fn detect_features_with_code_analysis_fallback() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let cargo_toml = tmp.path().join("Cargo.toml");
		std::fs::write(
			&cargo_toml,
			r#"
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
reinhardt = { version = "0.1", features = ["full"] }
"#,
		)
		.unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			r#"
use reinhardt::db::models::Model;
use reinhardt::cache::CacheBackend;

fn main() {}
"#,
		)
		.unwrap();

		// Act
		let result = detect_features(tmp.path(), false).unwrap();

		// Assert
		assert!(result.database);
		assert!(result.cache);
		// "full" triggers ambiguous from Layer 1, but Layer 2 provides specific info
	}

	#[rstest]
	fn detect_features_reinhardt_web_dependency() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let cargo_toml = tmp.path().join("Cargo.toml");
		std::fs::write(
			&cargo_toml,
			r#"
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
reinhardt-web = { version = "0.1", features = ["db-postgres"] }
"#,
		)
		.unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

		// Act
		let result = detect_features(tmp.path(), false).unwrap();

		// Assert
		assert!(result.database);
	}
}
