use comfy_table::{Cell, Table};
use console::style;
use inquire::{Confirm, InquireError, MultiSelect, Select};

use crate::config::{DatabaseEngine, NoSqlEngine};
use crate::detection::feature_flags::FeatureDetectionResult;
use crate::error::{DeployError, DeployResult};

/// Feature labels displayed in the interactive checklist.
///
/// Each label maps positionally to the boolean fields of
/// `FeatureDetectionResult` in the order: database, nosql, cache,
/// websockets, frontend, static_files, media, background_tasks, mail.
const FEATURE_LABELS: [&str; 9] = [
	"Database (SQL)",
	"NoSQL Database",
	"Cache (Redis)",
	"WebSockets",
	"Frontend / Templates",
	"Static Files",
	"Media Storage",
	"Background Tasks",
	"Email / Mail",
];

/// NoSQL engine labels for the interactive engine selection prompt.
const NOSQL_ENGINE_LABELS: [&str; 3] = ["MongoDB", "DynamoDB", "Firestore"];

/// Present detection results to the user for interactive confirmation.
///
/// When the unified detection pipeline produces ambiguous results (e.g., the
/// "full" feature flag is used and code analysis cannot determine exact usage),
/// this wizard allows the user to confirm or override each detected feature.
///
/// The wizard runs four phases in a loop until the user confirms:
/// 1. Display a header summarizing the current detection state
/// 2. Present a feature toggle checklist via `MultiSelect`
/// 3. Prompt for engine selection when database/nosql features are enabled
/// 4. Show a summary table and ask for final confirmation
pub fn present_detection_results(result: &mut FeatureDetectionResult) -> DeployResult<()> {
	loop {
		// Phase 1: Header
		print_detection_header(result);

		// Phase 2: Feature toggle
		let defaults = get_feature_defaults(result);
		let selected = MultiSelect::new(
			"Select the features your project uses:",
			FEATURE_LABELS.to_vec(),
		)
		.with_default(&defaults)
		.prompt()
		.map_err(map_inquire_error)?;

		let selected_indices: Vec<usize> = selected
			.iter()
			.filter_map(|s| FEATURE_LABELS.iter().position(|l| l == s))
			.collect();
		apply_feature_selections(result, &selected_indices);

		// Phase 3: Engine selection (conditional)
		if result.database {
			let db_options = vec!["PostgreSQL", "MySQL"];
			let db_default = match result.database_engine {
				Some(DatabaseEngine::MySql) => 1,
				_ => 0,
			};
			let db_choice = Select::new("Select your database engine:", db_options)
				.with_starting_cursor(db_default)
				.prompt()
				.map_err(map_inquire_error)?;
			result.database_engine = match db_choice {
				"MySQL" => Some(DatabaseEngine::MySql),
				_ => Some(DatabaseEngine::PostgreSql),
			};
		} else {
			result.database_engine = None;
		}

		if result.nosql {
			let nosql_default = get_nosql_default(&result.nosql_engines);
			let nosql_choice =
				Select::new("Select your NoSQL engine:", NOSQL_ENGINE_LABELS.to_vec())
					.with_starting_cursor(nosql_default)
					.prompt()
					.map_err(map_inquire_error)?;

			result.nosql_engines = match nosql_choice {
				"DynamoDB" => vec![NoSqlEngine::DynamoDb],
				"Firestore" => vec![NoSqlEngine::Firestore],
				_ => vec![NoSqlEngine::MongoDb],
			};
		} else {
			result.nosql_engines = Vec::new();
		}

		// Phase 4: Confirmation
		let table = build_summary_table(result);
		println!("\n{table}");

		let confirmed = Confirm::new("Accept this configuration?")
			.with_default(true)
			.prompt()
			.map_err(map_inquire_error)?;

		if confirmed {
			result.ambiguous = false;
			result.confidence = 1.0;
			return Ok(());
		}
		// Loop back to Phase 2 if rejected
	}
}

/// Display a styled header summarizing the current detection state.
pub fn print_detection_header(result: &FeatureDetectionResult) {
	println!(
		"\n{}",
		style("── Reinhardt Deploy: Feature Detection Wizard ──").bold()
	);
	println!("  Confidence : {:.0}%", result.confidence * 100.0);
	println!("  Models     : {}", result.model_count);
	if result.ambiguous {
		println!(
			"  Status     : {}",
			style("ambiguous — please confirm features").yellow()
		);
	} else {
		println!("  Status     : {}", style("resolved").green());
	}
	println!();
}

/// Extract default toggle states from a `FeatureDetectionResult`.
///
/// Returns a vector of indices that should be pre-selected in the
/// `MultiSelect` prompt. Each index corresponds to a `FEATURE_LABELS` entry.
pub fn get_feature_defaults(result: &FeatureDetectionResult) -> Vec<usize> {
	let flags = [
		result.database,
		result.nosql,
		result.cache,
		result.websockets,
		result.frontend,
		result.static_files,
		result.media,
		result.background_tasks,
		result.mail,
	];
	flags
		.iter()
		.enumerate()
		.filter_map(|(i, &enabled)| if enabled { Some(i) } else { None })
		.collect()
}

/// Apply the user's feature selections back to the detection result.
///
/// `selected_indices` contains the indices of features the user enabled.
/// All features not in `selected_indices` are set to `false`.
pub fn apply_feature_selections(result: &mut FeatureDetectionResult, selected_indices: &[usize]) {
	result.database = selected_indices.contains(&0);
	result.nosql = selected_indices.contains(&1);
	result.cache = selected_indices.contains(&2);
	result.websockets = selected_indices.contains(&3);
	result.frontend = selected_indices.contains(&4);
	result.static_files = selected_indices.contains(&5);
	result.media = selected_indices.contains(&6);
	result.background_tasks = selected_indices.contains(&7);
	result.mail = selected_indices.contains(&8);
}

/// Get the default cursor position for NoSQL engine selection.
///
/// Returns the index into `NOSQL_ENGINE_LABELS` matching the current engine,
/// or 0 (MongoDB) if no engine is set.
pub fn get_nosql_default(engines: &[NoSqlEngine]) -> usize {
	match engines.first() {
		Some(NoSqlEngine::DynamoDb) => 1,
		Some(NoSqlEngine::Firestore) => 2,
		_ => 0,
	}
}

/// Apply the user's NoSQL engine selection.
///
/// Converts a `NOSQL_ENGINE_LABELS` index into the corresponding
/// `NoSqlEngine` variant and stores it in the result.
pub fn apply_nosql_selection(engine: &mut Option<NoSqlEngine>, selected_index: usize) {
	*engine = match selected_index {
		1 => Some(NoSqlEngine::DynamoDb),
		2 => Some(NoSqlEngine::Firestore),
		_ => Some(NoSqlEngine::MongoDb),
	};
}

/// Build a summary table of the current feature configuration.
///
/// Renders a `comfy_table::Table` showing each feature and its enabled/disabled
/// status, plus engine details where applicable.
pub fn build_summary_table(result: &FeatureDetectionResult) -> Table {
	let mut table = Table::new();
	table.set_header(vec![
		Cell::new("Feature"),
		Cell::new("Enabled"),
		Cell::new("Details"),
	]);

	let db_detail = result
		.database_engine
		.as_ref()
		.map(|e| match e {
			DatabaseEngine::PostgreSql => "PostgreSQL",
			DatabaseEngine::MySql => "MySQL",
		})
		.unwrap_or("-");

	let nosql_detail = result
		.nosql_engines
		.first()
		.map(|e| match e {
			NoSqlEngine::MongoDb => "MongoDB",
			NoSqlEngine::DynamoDb => "DynamoDB",
			NoSqlEngine::Firestore => "Firestore",
		})
		.unwrap_or("-");

	let rows: Vec<(&str, bool, &str)> = vec![
		("Database (SQL)", result.database, db_detail),
		("NoSQL Database", result.nosql, nosql_detail),
		("Cache (Redis)", result.cache, "-"),
		("WebSockets", result.websockets, "-"),
		("Frontend / Templates", result.frontend, "-"),
		("Static Files", result.static_files, "-"),
		("Media Storage", result.media, "-"),
		("Background Tasks", result.background_tasks, "-"),
		("Email / Mail", result.mail, "-"),
	];

	for (feature, enabled, detail) in rows {
		let status = if enabled { "Yes" } else { "No" };
		table.add_row(vec![
			Cell::new(feature),
			Cell::new(status),
			Cell::new(detail),
		]);
	}

	table
}

/// Map `InquireError` to `DeployError::Detection`.
fn map_inquire_error(err: InquireError) -> DeployError {
	DeployError::Detection {
		message: format!("interactive prompt failed: {err}"),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn get_feature_defaults_all_false() {
		// Arrange
		let result = FeatureDetectionResult::default();

		// Act
		let defaults = get_feature_defaults(&result);

		// Assert
		assert!(defaults.is_empty());
	}

	#[rstest]
	fn get_feature_defaults_mixed() {
		// Arrange
		let result = FeatureDetectionResult {
			database: true,
			cache: true,
			frontend: true,
			..Default::default()
		};

		// Act
		let defaults = get_feature_defaults(&result);

		// Assert
		assert_eq!(defaults, vec![0, 2, 4]);
	}

	#[rstest]
	fn apply_feature_selections_empty() {
		// Arrange
		let mut result = FeatureDetectionResult {
			database: true,
			cache: true,
			..Default::default()
		};

		// Act
		apply_feature_selections(&mut result, &[]);

		// Assert
		assert!(!result.database);
		assert!(!result.nosql);
		assert!(!result.cache);
		assert!(!result.websockets);
		assert!(!result.frontend);
		assert!(!result.static_files);
		assert!(!result.media);
		assert!(!result.background_tasks);
		assert!(!result.mail);
	}

	#[rstest]
	fn apply_feature_selections_all() {
		// Arrange
		let mut result = FeatureDetectionResult::default();

		// Act
		apply_feature_selections(&mut result, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);

		// Assert
		assert!(result.database);
		assert!(result.nosql);
		assert!(result.cache);
		assert!(result.websockets);
		assert!(result.frontend);
		assert!(result.static_files);
		assert!(result.media);
		assert!(result.background_tasks);
		assert!(result.mail);
	}

	#[rstest]
	fn apply_feature_selections_subset() {
		// Arrange
		let mut result = FeatureDetectionResult::default();

		// Act
		apply_feature_selections(&mut result, &[0, 3, 7]);

		// Assert
		assert!(result.database);
		assert!(!result.nosql);
		assert!(!result.cache);
		assert!(result.websockets);
		assert!(!result.frontend);
		assert!(!result.static_files);
		assert!(!result.media);
		assert!(result.background_tasks);
		assert!(!result.mail);
	}

	#[rstest]
	fn get_nosql_default_none() {
		// Arrange
		let engines: Vec<NoSqlEngine> = Vec::new();

		// Act
		let default = get_nosql_default(&engines);

		// Assert
		assert_eq!(default, 0);
	}

	#[rstest]
	fn get_nosql_default_with_engine() {
		// Arrange
		let engines = vec![NoSqlEngine::Firestore];

		// Act
		let default = get_nosql_default(&engines);

		// Assert
		assert_eq!(default, 2);
	}

	#[rstest]
	fn apply_nosql_selection_mongodb() {
		// Arrange
		let mut engine: Option<NoSqlEngine> = None;

		// Act
		apply_nosql_selection(&mut engine, 0);

		// Assert
		assert_eq!(engine, Some(NoSqlEngine::MongoDb));
	}

	#[rstest]
	fn apply_nosql_selection_dynamodb() {
		// Arrange
		let mut engine: Option<NoSqlEngine> = None;

		// Act
		apply_nosql_selection(&mut engine, 1);

		// Assert
		assert_eq!(engine, Some(NoSqlEngine::DynamoDb));
	}

	#[rstest]
	fn apply_nosql_selection_firestore() {
		// Arrange
		let mut engine: Option<NoSqlEngine> = None;

		// Act
		apply_nosql_selection(&mut engine, 2);

		// Assert
		assert_eq!(engine, Some(NoSqlEngine::Firestore));
	}

	#[rstest]
	fn build_summary_table_renders_enabled_features() {
		// Arrange
		let result = FeatureDetectionResult {
			database: true,
			database_engine: Some(DatabaseEngine::PostgreSql),
			nosql: true,
			nosql_engines: vec![NoSqlEngine::MongoDb],
			cache: false,
			websockets: true,
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

		// Act
		let table = build_summary_table(&result);
		let rendered = table.to_string();

		// Assert
		assert!(rendered.contains("Database (SQL)"));
		assert!(rendered.contains("PostgreSQL"));
		assert!(rendered.contains("MongoDB"));
		assert!(rendered.contains("Yes"));
		assert!(rendered.contains("No"));
	}
}
