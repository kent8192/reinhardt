//! Fixture loading functionality.
//!
//! This module handles loading fixture data into the database.

use std::collections::HashMap;
use std::path::Path;

use super::{FixtureData, FixtureParser, ModelRegistry};
use crate::error::{SeedingError, SeedingResult};

/// Options for fixture loading.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
	/// Filter fixtures to only these app labels.
	pub app_labels: Vec<String>,

	/// Continue loading even if a model is not found in the registry.
	pub ignore_missing: bool,

	/// Wrap the entire load operation in a transaction.
	pub use_transaction: bool,

	/// Verbosity level (0 = silent, 1 = summary, 2 = detailed).
	pub verbosity: u8,
}

impl LoadOptions {
	/// Creates new default load options.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets app labels to filter.
	pub fn with_app_labels(mut self, labels: Vec<String>) -> Self {
		self.app_labels = labels;
		self
	}

	/// Sets whether to ignore missing models.
	pub fn with_ignore_missing(mut self, ignore: bool) -> Self {
		self.ignore_missing = ignore;
		self
	}

	/// Sets whether to use a transaction.
	pub fn with_transaction(mut self, use_tx: bool) -> Self {
		self.use_transaction = use_tx;
		self
	}

	/// Sets the verbosity level.
	pub fn with_verbosity(mut self, level: u8) -> Self {
		self.verbosity = level;
		self
	}
}

/// Result of a fixture load operation.
#[derive(Debug, Clone, Default)]
pub struct LoadResult {
	/// Total number of records successfully loaded.
	pub records_loaded: usize,

	/// Number of records loaded per model.
	pub models_loaded: HashMap<String, usize>,

	/// Models that were skipped due to missing loaders.
	pub skipped_models: Vec<String>,

	/// Errors encountered during loading (if ignore_missing is true).
	pub errors: Vec<String>,
}

impl LoadResult {
	/// Creates a new empty load result.
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds a loaded record to the result.
	pub fn add_loaded(&mut self, model_id: &str) {
		self.records_loaded += 1;
		*self.models_loaded.entry(model_id.to_string()).or_insert(0) += 1;
	}

	/// Adds a skipped model to the result.
	pub fn add_skipped(&mut self, model_id: &str) {
		if !self.skipped_models.contains(&model_id.to_string()) {
			self.skipped_models.push(model_id.to_string());
		}
	}

	/// Adds an error to the result.
	pub fn add_error(&mut self, error: String) {
		self.errors.push(error);
	}

	/// Returns true if any errors occurred.
	pub fn has_errors(&self) -> bool {
		!self.errors.is_empty()
	}

	/// Merges another result into this one.
	pub fn merge(&mut self, other: LoadResult) {
		self.records_loaded += other.records_loaded;
		for (model, count) in other.models_loaded {
			*self.models_loaded.entry(model).or_insert(0) += count;
		}
		self.skipped_models.extend(other.skipped_models);
		self.errors.extend(other.errors);
	}
}

/// Fixture loader for importing data into the database.
#[derive(Debug)]
pub struct FixtureLoader {
	/// Parser for fixture files.
	parser: FixtureParser,

	/// Model registry for loader lookup.
	registry: ModelRegistry,

	/// Loading options.
	options: LoadOptions,
}

impl FixtureLoader {
	/// Creates a new fixture loader with default options.
	pub fn new() -> Self {
		Self {
			parser: FixtureParser::new(),
			registry: ModelRegistry::new(),
			options: LoadOptions::default(),
		}
	}

	/// Creates a new fixture loader with the specified options.
	pub fn with_options(options: LoadOptions) -> Self {
		Self {
			parser: FixtureParser::new(),
			registry: ModelRegistry::new(),
			options,
		}
	}

	/// Sets the loading options.
	pub fn set_options(&mut self, options: LoadOptions) {
		self.options = options;
	}

	/// Loads fixtures from the specified file paths.
	///
	/// # Arguments
	///
	/// * `paths` - Paths to fixture files
	///
	/// # Returns
	///
	/// Returns the load result with statistics.
	pub async fn load_from_paths(&self, paths: &[&Path]) -> SeedingResult<LoadResult> {
		let data = self.parser.parse_files(paths)?;
		self.load_data(&data).await
	}

	/// Loads fixtures from a single file path.
	///
	/// # Arguments
	///
	/// * `path` - Path to the fixture file
	///
	/// # Returns
	///
	/// Returns the load result with statistics.
	pub async fn load_from_path(&self, path: &Path) -> SeedingResult<LoadResult> {
		let data = self.parser.parse_file(path)?;
		self.load_data(&data).await
	}

	/// Loads parsed fixture data into the database.
	///
	/// # Arguments
	///
	/// * `data` - Parsed fixture data
	///
	/// # Returns
	///
	/// Returns the load result with statistics.
	pub async fn load_data(&self, data: &FixtureData) -> SeedingResult<LoadResult> {
		let mut result = LoadResult::new();

		// Group records by model for potential batch optimization
		let groups = data.group_by_model();

		for (model_id, records) in groups {
			// Filter by app label if specified
			if !self.options.app_labels.is_empty() {
				let app_label = model_id.split('.').next().unwrap_or("");
				if !self.options.app_labels.iter().any(|l| l == app_label) {
					continue;
				}
			}

			// Check if we have a loader for this model
			if !self.registry.has_loader(model_id) {
				if self.options.ignore_missing {
					result.add_skipped(model_id);
					if self.options.verbosity > 0 {
						result.add_error(format!("Model loader not found: {}", model_id));
					}
					continue;
				} else {
					return Err(SeedingError::ModelNotFound(model_id.to_string()));
				}
			}

			// Load records
			for record in records {
				match self.registry.load_record(record).await {
					Ok(_) => {
						result.add_loaded(model_id);
					}
					Err(e) => {
						if self.options.ignore_missing {
							result.add_error(format!(
								"Failed to load record for {}: {}",
								model_id, e
							));
						} else {
							return Err(e);
						}
					}
				}
			}
		}

		Ok(result)
	}

	/// Loads fixtures from a JSON string.
	pub async fn load_from_json(&self, json: &str) -> SeedingResult<LoadResult> {
		let data = self.parser.parse_string(json, super::FixtureFormat::Json)?;
		self.load_data(&data).await
	}

	/// Loads fixtures from a YAML string.
	#[cfg(feature = "yaml")]
	pub async fn load_from_yaml(&self, yaml: &str) -> SeedingResult<LoadResult> {
		let data = self.parser.parse_string(yaml, super::FixtureFormat::Yaml)?;
		self.load_data(&data).await
	}
}

impl Default for FixtureLoader {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fixtures::{FixtureRecord, ModelLoader, register_model_loader};
	use async_trait::async_trait;
	use rstest::rstest;
	use serde_json::json;
	use std::io::Write;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};
	use tempfile::NamedTempFile;

	struct CountingLoader {
		model_id: String,
		count: Arc<AtomicUsize>,
	}

	impl CountingLoader {
		fn new(model_id: &str) -> Self {
			Self {
				model_id: model_id.to_string(),
				count: Arc::new(AtomicUsize::new(0)),
			}
		}
	}

	#[async_trait]
	impl ModelLoader for CountingLoader {
		fn model_id(&self) -> &str {
			&self.model_id
		}

		async fn load_record(&self, record: &FixtureRecord) -> SeedingResult<serde_json::Value> {
			self.count.fetch_add(1, Ordering::SeqCst);
			Ok(record.pk.clone().unwrap_or(json!(1)))
		}
	}

	#[rstest]
	fn test_load_options_builder() {
		let options = LoadOptions::new()
			.with_app_labels(vec!["auth".to_string()])
			.with_ignore_missing(true)
			.with_transaction(true)
			.with_verbosity(2);

		assert_eq!(options.app_labels, vec!["auth".to_string()]);
		assert!(options.ignore_missing);
		assert!(options.use_transaction);
		assert_eq!(options.verbosity, 2);
	}

	#[rstest]
	fn test_load_result_operations() {
		let mut result = LoadResult::new();
		result.add_loaded("auth.User");
		result.add_loaded("auth.User");
		result.add_loaded("blog.Post");
		result.add_skipped("unknown.Model");
		result.add_error("Test error".to_string());

		assert_eq!(result.records_loaded, 3);
		assert_eq!(result.models_loaded.len(), 2);
		assert_eq!(result.models_loaded["auth.User"], 2);
		assert_eq!(result.models_loaded["blog.Post"], 1);
		assert_eq!(result.skipped_models.len(), 1);
		assert!(result.has_errors());
	}

	#[rstest]
	fn test_load_result_merge() {
		let mut result1 = LoadResult::new();
		result1.add_loaded("auth.User");

		let mut result2 = LoadResult::new();
		result2.add_loaded("auth.User");
		result2.add_loaded("blog.Post");

		result1.merge(result2);

		assert_eq!(result1.records_loaded, 3);
		assert_eq!(result1.models_loaded["auth.User"], 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_from_json_string() {
		let registry = ModelRegistry::new();
		registry.clear();

		register_model_loader(CountingLoader::new("test.User"));

		let loader = FixtureLoader::new();
		let json = r#"[
            {"model": "test.User", "pk": 1, "fields": {"name": "alice"}},
            {"model": "test.User", "pk": 2, "fields": {"name": "bob"}}
        ]"#;

		let result = loader.load_from_json(json).await.unwrap();
		assert_eq!(result.records_loaded, 2);
		assert_eq!(result.models_loaded["test.User"], 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_missing_model_error() {
		let registry = ModelRegistry::new();
		registry.clear();

		let loader = FixtureLoader::new();
		let json = r#"[{"model": "missing.Model", "fields": {}}]"#;

		let result = loader.load_from_json(json).await;
		assert!(matches!(result, Err(SeedingError::ModelNotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_ignore_missing() {
		let registry = ModelRegistry::new();
		registry.clear();

		let options = LoadOptions::new().with_ignore_missing(true);
		let loader = FixtureLoader::with_options(options);
		let json = r#"[{"model": "missing.Model", "fields": {}}]"#;

		let result = loader.load_from_json(json).await.unwrap();
		assert_eq!(result.records_loaded, 0);
		assert_eq!(result.skipped_models.len(), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_filter_by_app() {
		let registry = ModelRegistry::new();
		registry.clear();

		register_model_loader(CountingLoader::new("auth.User"));
		register_model_loader(CountingLoader::new("blog.Post"));

		let options = LoadOptions::new().with_app_labels(vec!["auth".to_string()]);
		let loader = FixtureLoader::with_options(options);
		let json = r#"[
            {"model": "auth.User", "fields": {}},
            {"model": "blog.Post", "fields": {}}
        ]"#;

		let result = loader.load_from_json(json).await.unwrap();
		assert_eq!(result.records_loaded, 1);
		assert_eq!(result.models_loaded.get("auth.User"), Some(&1));
		assert!(result.models_loaded.get("blog.Post").is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_from_file() {
		let registry = ModelRegistry::new();
		registry.clear();

		register_model_loader(CountingLoader::new("file.Test"));

		let mut file = NamedTempFile::with_suffix(".json").unwrap();
		writeln!(file, r#"[{{"model": "file.Test", "fields": {{}}}}]"#).unwrap();

		let loader = FixtureLoader::new();
		let result = loader.load_from_path(file.path()).await.unwrap();
		assert_eq!(result.records_loaded, 1);
	}
}
