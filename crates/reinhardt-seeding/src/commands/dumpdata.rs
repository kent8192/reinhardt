//! dumpdata command implementation.
//!
//! This command dumps database data to fixture files.

use std::path::PathBuf;

use crate::error::SeedingResult;
use crate::fixtures::{FixtureFormat, FixtureRecord, FixtureSerializer};

/// Arguments for the dumpdata command.
#[derive(Debug, Clone, Default)]
pub struct DumpDataArgs {
	/// Model specifications to dump (e.g., "auth.User", "blog.Post").
	/// If empty, dumps all models.
	pub models: Vec<String>,
}

/// Options for the dumpdata command.
#[derive(Debug, Clone)]
pub struct DumpDataOptions {
	/// Output file path. If None, outputs to stdout.
	pub output: Option<PathBuf>,

	/// Output format.
	pub format: FixtureFormat,

	/// Indentation level for pretty printing.
	pub indent: usize,

	/// Models to exclude from the dump.
	pub exclude: Vec<String>,

	/// Primary keys to include (filter by pk).
	pub pks: Vec<String>,

	/// Use natural keys instead of numeric pks.
	pub natural_keys: bool,

	/// Use natural foreign keys.
	pub natural_foreign: bool,

	/// Database alias to dump from.
	pub database: Option<String>,
}

impl Default for DumpDataOptions {
	fn default() -> Self {
		Self {
			output: None,
			format: FixtureFormat::Json,
			indent: 2,
			exclude: Vec::new(),
			pks: Vec::new(),
			natural_keys: false,
			natural_foreign: false,
			database: None,
		}
	}
}

impl DumpDataOptions {
	/// Creates new default options.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets output file path.
	pub fn with_output(mut self, path: impl Into<PathBuf>) -> Self {
		self.output = Some(path.into());
		self
	}

	/// Sets output format.
	pub fn with_format(mut self, format: FixtureFormat) -> Self {
		self.format = format;
		self
	}

	/// Sets indentation level.
	pub fn with_indent(mut self, indent: usize) -> Self {
		self.indent = indent;
		self
	}

	/// Sets models to exclude.
	pub fn with_exclude(mut self, exclude: Vec<String>) -> Self {
		self.exclude = exclude;
		self
	}

	/// Sets primary keys filter.
	pub fn with_pks(mut self, pks: Vec<String>) -> Self {
		self.pks = pks;
		self
	}

	/// Sets natural keys flag.
	pub fn with_natural_keys(mut self, natural: bool) -> Self {
		self.natural_keys = natural;
		self
	}

	/// Sets database alias.
	pub fn with_database(mut self, db: impl Into<String>) -> Self {
		self.database = Some(db.into());
		self
	}
}

/// Result of a dump operation.
#[derive(Debug, Clone)]
pub struct DumpResult {
	/// Number of records dumped.
	pub records_dumped: usize,

	/// Models that were dumped.
	pub models_dumped: Vec<String>,

	/// Output content (if not written to file).
	pub content: Option<String>,

	/// Output file path (if written to file).
	pub output_path: Option<PathBuf>,
}

/// The dumpdata command for exporting database data to fixtures.
///
/// This command is equivalent to Django's `manage.py dumpdata` command.
///
/// # Example
///
/// ```ignore
/// let command = DumpDataCommand::new();
/// let args = DumpDataArgs {
///     models: vec!["auth.User".to_string()],
/// };
/// let options = DumpDataOptions::new()
///     .with_output("fixtures/users.json")
///     .with_format(FixtureFormat::Json);
/// let result = command.execute(args, options).await?;
/// ```
#[derive(Debug, Default)]
pub struct DumpDataCommand;

impl DumpDataCommand {
	/// Creates a new dumpdata command.
	pub fn new() -> Self {
		Self
	}

	/// Returns the command name.
	pub fn name(&self) -> &str {
		"dumpdata"
	}

	/// Returns the command description.
	pub fn description(&self) -> &str {
		"Output the contents of the database as a fixture"
	}

	/// Returns the command help text.
	pub fn help(&self) -> &str {
		r#"
Usage: dumpdata [options] [app_label[.ModelName] ...]

Output the contents of the database as a fixture of the given format.

Arguments:
  app_label.ModelName  Specific model(s) to dump (optional)

Options:
  --output, -o FILE    Write output to FILE instead of stdout
  --format FORMAT      Output format (json, yaml). Default: json
  --indent N           Indentation level. Default: 2
  --exclude MODEL      Exclude MODEL from the dump
  --pks PK1,PK2        Only dump objects with given primary keys
  --natural-keys       Use natural keys for serialization
  --natural-foreign    Use natural foreign keys
  --database DB        Database alias to dump from
"#
	}

	/// Executes the dumpdata command.
	///
	/// Note: This is a placeholder implementation. Full implementation
	/// requires integration with the model registry to query the database.
	pub async fn execute(
		&self,
		_args: DumpDataArgs,
		options: DumpDataOptions,
	) -> SeedingResult<DumpResult> {
		// TODO: Implement actual database querying through model registry
		// For now, this is a placeholder that demonstrates the structure

		let records: Vec<FixtureRecord> = Vec::new();
		let serializer = FixtureSerializer::new()
			.with_format(options.format)
			.with_indent(options.indent);

		let content = serializer.serialize(&records)?;

		if let Some(output_path) = &options.output {
			std::fs::write(output_path, &content)?;
			Ok(DumpResult {
				records_dumped: records.len(),
				models_dumped: Vec::new(),
				content: None,
				output_path: Some(output_path.clone()),
			})
		} else {
			Ok(DumpResult {
				records_dumped: records.len(),
				models_dumped: Vec::new(),
				content: Some(content),
				output_path: None,
			})
		}
	}

	/// Dumps specific fixture records.
	///
	/// This method can be used when you already have the records to dump.
	pub fn dump_records(
		&self,
		records: &[FixtureRecord],
		options: &DumpDataOptions,
	) -> SeedingResult<String> {
		let serializer = FixtureSerializer::new()
			.with_format(options.format)
			.with_indent(options.indent);
		serializer.serialize(records)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;
	use tempfile::tempdir;

	#[rstest]
	fn test_command_metadata() {
		let cmd = DumpDataCommand::new();
		assert_eq!(cmd.name(), "dumpdata");
		assert!(!cmd.description().is_empty());
		assert!(!cmd.help().is_empty());
	}

	#[rstest]
	fn test_options_builder() {
		let options = DumpDataOptions::new()
			.with_output("output.json")
			.with_format(FixtureFormat::Json)
			.with_indent(4)
			.with_exclude(vec!["auth.Session".to_string()])
			.with_pks(vec!["1".to_string(), "2".to_string()])
			.with_natural_keys(true)
			.with_database("secondary");

		assert_eq!(options.output, Some(PathBuf::from("output.json")));
		assert_eq!(options.format, FixtureFormat::Json);
		assert_eq!(options.indent, 4);
		assert_eq!(options.exclude, vec!["auth.Session".to_string()]);
		assert_eq!(options.pks, vec!["1".to_string(), "2".to_string()]);
		assert!(options.natural_keys);
		assert_eq!(options.database, Some("secondary".to_string()));
	}

	#[rstest]
	#[tokio::test]
	async fn test_execute_to_stdout() {
		let cmd = DumpDataCommand::new();
		let args = DumpDataArgs::default();
		let options = DumpDataOptions::new();

		let result = cmd.execute(args, options).await.unwrap();
		assert!(result.content.is_some());
		assert!(result.output_path.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_execute_to_file() {
		let dir = tempdir().unwrap();
		let output_path = dir.path().join("output.json");

		let cmd = DumpDataCommand::new();
		let args = DumpDataArgs::default();
		let options = DumpDataOptions::new().with_output(&output_path);

		let result = cmd.execute(args, options).await.unwrap();
		assert!(result.content.is_none());
		assert_eq!(result.output_path, Some(output_path.clone()));
		assert!(output_path.exists());
	}

	#[rstest]
	fn test_dump_records() {
		let cmd = DumpDataCommand::new();
		let records = vec![FixtureRecord::with_pk(
			"auth.User",
			json!(1),
			json!({"username": "admin"}),
		)];
		let options = DumpDataOptions::new();

		let content = cmd.dump_records(&records, &options).unwrap();
		assert!(content.contains("auth.User"));
		assert!(content.contains("admin"));
	}

	#[rstest]
	fn test_dump_records_yaml() {
		let cmd = DumpDataCommand::new();
		let records = vec![FixtureRecord::new("test.Model", json!({"field": "value"}))];

		// JSON format
		let options = DumpDataOptions::new().with_format(FixtureFormat::Json);
		let json_content = cmd.dump_records(&records, &options).unwrap();
		assert!(json_content.contains("{"));

		// YAML format would require the yaml feature
	}
}
