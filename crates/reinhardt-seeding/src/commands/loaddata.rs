//! loaddata command implementation.
//!
//! This command loads fixture data from files into the database.

use std::path::PathBuf;

use crate::error::{SeedingError, SeedingResult};
use crate::fixtures::{FixtureLoader, LoadOptions, LoadResult};

/// Arguments for the loaddata command.
#[derive(Debug, Clone, Default)]
pub struct LoadDataArgs {
	/// Fixture file paths to load.
	pub fixture_paths: Vec<PathBuf>,
}

/// Options for the loaddata command.
#[derive(Debug, Clone, Default)]
pub struct LoadDataOptions {
	/// Filter by app labels.
	pub app_labels: Vec<String>,

	/// Continue even if a model is not found.
	pub ignore_missing: bool,

	/// Wrap the load in a transaction.
	pub use_transaction: bool,

	/// Database alias to use.
	pub database: Option<String>,

	/// Verbosity level.
	pub verbosity: u8,
}

impl LoadDataOptions {
	/// Creates new default options.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets app labels filter.
	pub fn with_app_labels(mut self, labels: Vec<String>) -> Self {
		self.app_labels = labels;
		self
	}

	/// Sets ignore missing flag.
	pub fn with_ignore_missing(mut self, ignore: bool) -> Self {
		self.ignore_missing = ignore;
		self
	}

	/// Sets use transaction flag.
	pub fn with_transaction(mut self, use_tx: bool) -> Self {
		self.use_transaction = use_tx;
		self
	}

	/// Sets database alias.
	pub fn with_database(mut self, db: impl Into<String>) -> Self {
		self.database = Some(db.into());
		self
	}

	/// Sets verbosity level.
	pub fn with_verbosity(mut self, level: u8) -> Self {
		self.verbosity = level;
		self
	}
}

/// The loaddata command for loading fixtures into the database.
///
/// This command is equivalent to Django's `manage.py loaddata` command.
///
/// # Example
///
/// ```ignore
/// let command = LoadDataCommand::new();
/// let args = LoadDataArgs {
///     fixture_paths: vec![PathBuf::from("fixtures/users.json")],
/// };
/// let options = LoadDataOptions::new().with_verbosity(1);
/// let result = command.execute(args, options).await?;
/// println!("Loaded {} records", result.records_loaded);
/// ```
#[derive(Debug, Default)]
pub struct LoadDataCommand;

impl LoadDataCommand {
	/// Creates a new loaddata command.
	pub fn new() -> Self {
		Self
	}

	/// Returns the command name.
	pub fn name(&self) -> &str {
		"loaddata"
	}

	/// Returns the command description.
	pub fn description(&self) -> &str {
		"Installs the named fixture(s) in the database"
	}

	/// Returns the command help text.
	pub fn help(&self) -> &str {
		r#"
Usage: loaddata [options] fixture [fixture ...]

Installs the named fixture(s) in the database.

Arguments:
  fixture              One or more fixture files to load

Options:
  --app, -a LABEL      Only load fixtures for the specified app(s)
  --ignore-missing     Ignore missing models when loading fixtures
  --database DB        Database alias to load fixtures into
  --verbosity LEVEL    Verbosity level (0=minimal, 1=normal, 2=verbose)
"#
	}

	/// Executes the loaddata command.
	///
	/// # Arguments
	///
	/// * `args` - Command arguments (fixture paths)
	/// * `options` - Command options
	///
	/// # Returns
	///
	/// Returns the load result with statistics.
	pub async fn execute(
		&self,
		args: LoadDataArgs,
		options: LoadDataOptions,
	) -> SeedingResult<LoadResult> {
		if args.fixture_paths.is_empty() {
			return Err(SeedingError::ValidationError {
				field: "fixture_paths".to_string(),
				message: "At least one fixture file must be specified".to_string(),
			});
		}

		// Validate all paths exist
		for path in &args.fixture_paths {
			if !path.exists() {
				return Err(SeedingError::FileNotFound(path.display().to_string()));
			}
		}

		// Convert options to LoadOptions
		let load_options = LoadOptions {
			app_labels: options.app_labels,
			ignore_missing: options.ignore_missing,
			use_transaction: options.use_transaction,
			verbosity: options.verbosity,
		};

		// Create loader and load fixtures
		let loader = FixtureLoader::with_options(load_options);
		let paths: Vec<&std::path::Path> = args.fixture_paths.iter().map(|p| p.as_path()).collect();

		let result = loader.load_from_paths(&paths).await?;

		if options.verbosity > 0 {
			self.print_result(&result);
		}

		Ok(result)
	}

	/// Prints the load result summary.
	fn print_result(&self, result: &LoadResult) {
		println!("Installed {} object(s)", result.records_loaded);

		if !result.skipped_models.is_empty() {
			println!("Skipped models: {:?}", result.skipped_models);
		}

		if !result.errors.is_empty() {
			eprintln!("Errors:");
			for error in &result.errors {
				eprintln!("  - {}", error);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::io::Write;
	use tempfile::NamedTempFile;

	use crate::fixtures::{FixtureRecord, ModelLoader, ModelRegistry, register_model_loader};
	use async_trait::async_trait;
	use serde_json::json;

	struct TestLoader {
		model_id: String,
	}

	#[async_trait]
	impl ModelLoader for TestLoader {
		fn model_id(&self) -> &str {
			&self.model_id
		}

		async fn load_record(&self, record: &FixtureRecord) -> SeedingResult<serde_json::Value> {
			Ok(record.pk.clone().unwrap_or(json!(1)))
		}
	}

	#[rstest]
	fn test_command_metadata() {
		let cmd = LoadDataCommand::new();
		assert_eq!(cmd.name(), "loaddata");
		assert!(!cmd.description().is_empty());
		assert!(!cmd.help().is_empty());
	}

	#[rstest]
	fn test_options_builder() {
		let options = LoadDataOptions::new()
			.with_app_labels(vec!["auth".to_string()])
			.with_ignore_missing(true)
			.with_transaction(true)
			.with_database("secondary")
			.with_verbosity(2);

		assert_eq!(options.app_labels, vec!["auth".to_string()]);
		assert!(options.ignore_missing);
		assert!(options.use_transaction);
		assert_eq!(options.database, Some("secondary".to_string()));
		assert_eq!(options.verbosity, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_execute_empty_paths() {
		let cmd = LoadDataCommand::new();
		let args = LoadDataArgs {
			fixture_paths: vec![],
		};
		let options = LoadDataOptions::new();

		let result = cmd.execute(args, options).await;
		assert!(matches!(result, Err(SeedingError::ValidationError { .. })));
	}

	#[rstest]
	#[tokio::test]
	async fn test_execute_nonexistent_file() {
		let cmd = LoadDataCommand::new();
		let args = LoadDataArgs {
			fixture_paths: vec![PathBuf::from("/nonexistent/fixture.json")],
		};
		let options = LoadDataOptions::new();

		let result = cmd.execute(args, options).await;
		assert!(matches!(result, Err(SeedingError::FileNotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_execute_with_fixture() {
		let registry = ModelRegistry::new();
		registry.clear();

		register_model_loader(TestLoader {
			model_id: "loaddata.Test".to_string(),
		});

		let mut file = NamedTempFile::with_suffix(".json").unwrap();
		writeln!(file, r#"[{{"model": "loaddata.Test", "fields": {{}}}}]"#).unwrap();

		let cmd = LoadDataCommand::new();
		let args = LoadDataArgs {
			fixture_paths: vec![file.path().to_path_buf()],
		};
		let options = LoadDataOptions::new();

		let result = cmd.execute(args, options).await.unwrap();
		assert_eq!(result.records_loaded, 1);
	}
}
