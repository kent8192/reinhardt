//! Interactive Reinhardt dependency configuration for generated and existing projects.

use crate::{BaseCommand, CommandContext, CommandError, CommandResult};
use async_trait::async_trait;
use crates_io_api::AsyncClient;
use dialoguer::{Input, MultiSelect, Select};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value};

const CRATE_NAME: &str = "reinhardt-web";
const DEPENDENCY_NAME: &str = "reinhardt";
const DEFAULT_USER_AGENT: &str = "reinhardt-admin (https://github.com/kent8192/reinhardt-web)";

const PRESETS: &[&str] = &[
	"standard",
	"minimal",
	"full",
	"api-only",
	"graphql-server",
	"websocket-server",
	"cli-tools",
];

const ADDITIVE_FEATURES: &[&str] = &[
	"admin",
	"pages",
	"database",
	"db-postgres",
	"db-sqlite",
	"db-mysql",
	"auth",
	"sessions",
	"static-files",
	"openapi",
	"openapi-swagger-ui",
	"browsable-api",
	"websockets",
	"cache",
	"i18n",
	"mail",
	"tasks",
	"commands-autoreload",
	"middleware-compression",
	"image-validation",
];

/// Resolved dependency selection for the `reinhardt` facade crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReinhardtDependencySelection {
	/// Version requirement written to Cargo.toml.
	pub version: String,
	/// Whether Cargo default features should remain enabled.
	pub default_features: bool,
	/// Explicit feature list written to Cargo.toml.
	pub features: Vec<String>,
}

impl ReinhardtDependencySelection {
	/// Returns a TOML array literal suitable for embedded Tera templates.
	pub fn features_toml(&self) -> String {
		let quoted = self
			.features
			.iter()
			.map(|feature| format!("\"{}\"", feature))
			.collect::<Vec<_>>();
		format!("[{}]", quoted.join(", "))
	}
}

/// Resolve Reinhardt dependency options from explicit flags, prompts, or defaults.
pub async fn resolve_dependency_selection(
	ctx: &CommandContext,
	required_features: &[&str],
) -> CommandResult<ReinhardtDependencySelection> {
	let current_version = env!("CARGO_PKG_VERSION").to_string();
	let explicit_version = ctx.option("reinhardt-version").cloned();
	let explicit_features = explicit_feature_values(ctx)?;
	let explicit_default_features = ctx
		.option("default-features")
		.map(|value| parse_bool_option("default-features", value))
		.transpose()?;

	let interactive = should_prompt(ctx);
	let version_candidates = if interactive {
		fetch_versions()
			.await
			.unwrap_or_else(|_| vec![current_version.clone()])
	} else {
		Vec::new()
	};

	let version = if let Some(version) = explicit_version {
		version
	} else if interactive {
		prompt_version(&version_candidates, &current_version)?
	} else {
		default_version(&version_candidates, &current_version)
	};

	let default_features = explicit_default_features.unwrap_or(false);
	let mut features = if explicit_features.is_empty() {
		if interactive {
			prompt_features()?
		} else {
			default_noninteractive_features(required_features)
		}
	} else {
		explicit_features
	};

	for feature in required_features {
		if should_add_required_feature(&features, feature) {
			features.push((*feature).to_string());
		}
	}

	Ok(ReinhardtDependencySelection {
		version,
		default_features,
		features: normalize_features(features),
	})
}

fn default_noninteractive_features(required_features: &[&str]) -> Vec<String> {
	if required_features.contains(&"pages") {
		return required_features
			.iter()
			.map(|feature| (*feature).to_string())
			.collect();
	}
	vec!["standard".to_string()]
}

fn should_prompt(ctx: &CommandContext) -> bool {
	if cfg!(test)
		|| ctx.has_option("no-interactive")
		|| std::env::var("REINHARDT_TEST_MODE").is_ok()
	{
		return false;
	}
	std::io::stdin().is_terminal()
}

fn default_version(candidates: &[String], current_version: &str) -> String {
	if candidates.iter().any(|version| version == current_version) {
		current_version.to_string()
	} else {
		candidates
			.first()
			.cloned()
			.unwrap_or_else(|| current_version.to_string())
	}
}

async fn fetch_versions() -> CommandResult<Vec<String>> {
	let client =
		AsyncClient::new(DEFAULT_USER_AGENT, Duration::from_millis(1000)).map_err(|e| {
			CommandError::ExecutionError(format!("Failed to create crates.io client: {e}"))
		})?;
	let response = client.get_crate(CRATE_NAME).await.map_err(|e| {
		CommandError::ExecutionError(format!("Failed to fetch '{CRATE_NAME}' versions: {e}"))
	})?;
	let versions = response
		.versions
		.into_iter()
		.filter(|version| !version.yanked)
		.map(|version| version.num)
		.take(20)
		.collect::<Vec<_>>();
	if versions.is_empty() {
		return Err(CommandError::ExecutionError(format!(
			"crates.io returned no usable versions for '{CRATE_NAME}'"
		)));
	}
	Ok(versions)
}

fn prompt_version(candidates: &[String], current_version: &str) -> CommandResult<String> {
	let default = default_version(candidates, current_version);
	let mut choices = candidates.to_vec();
	choices.push("Custom version".to_string());
	let default_index = choices
		.iter()
		.position(|version| version == &default)
		.unwrap_or(0);
	let selected = Select::new()
		.with_prompt("Select Reinhardt version")
		.items(&choices)
		.default(default_index)
		.interact()
		.map_err(dialoguer_error)?;

	if choices[selected] == "Custom version" {
		Input::<String>::new()
			.with_prompt("Reinhardt version")
			.default(default)
			.interact()
			.map_err(dialoguer_error)
	} else {
		Ok(choices[selected].clone())
	}
}

fn prompt_features() -> CommandResult<Vec<String>> {
	let preset_index = Select::new()
		.with_prompt("Select Reinhardt feature preset")
		.items(PRESETS)
		.default(0)
		.interact()
		.map_err(dialoguer_error)?;
	let mut features = vec![PRESETS[preset_index].to_string()];

	let selected = MultiSelect::new()
		.with_prompt("Select additional Reinhardt features")
		.items(ADDITIVE_FEATURES)
		.interact()
		.map_err(dialoguer_error)?;
	for index in selected {
		features.push(ADDITIVE_FEATURES[index].to_string());
	}

	Ok(features)
}

fn dialoguer_error(error: dialoguer::Error) -> CommandError {
	CommandError::ExecutionError(format!("Prompt failed: {error}"))
}

fn explicit_feature_values(ctx: &CommandContext) -> CommandResult<Vec<String>> {
	let mut features = Vec::new();
	for key in ["feature", "features"] {
		if let Some(values) = ctx.option_values(key) {
			for value in values {
				for feature in value.split(',') {
					let trimmed = feature.trim();
					if !trimmed.is_empty() {
						features.push(trimmed.to_string());
					}
				}
			}
		}
	}
	if features
		.iter()
		.any(|feature| feature.contains(char::is_whitespace))
	{
		return Err(CommandError::InvalidArguments(
			"Feature names must not contain whitespace.".to_string(),
		));
	}
	Ok(normalize_features(features))
}

fn normalize_features(features: Vec<String>) -> Vec<String> {
	let mut normalized = Vec::new();
	for feature in features {
		let feature = normalize_feature_name(&feature).to_string();
		if !normalized.iter().any(|existing| existing == &feature) {
			normalized.push(feature);
		}
	}
	normalized
}

fn normalize_feature_name(feature: &str) -> &str {
	match feature {
		"server-fn" => "pages",
		_ => feature,
	}
}

fn should_add_required_feature(features: &[String], feature: &str) -> bool {
	if features.iter().any(|existing| existing == feature) {
		return false;
	}
	if feature == "minimal" && has_minimal_preset_feature(features) {
		return false;
	}
	if feature == "db-postgres" && has_database_backend_feature(features) {
		return false;
	}
	true
}

fn has_minimal_preset_feature(features: &[String]) -> bool {
	features.iter().any(|feature| {
		matches!(
			feature.as_str(),
			"standard" | "full" | "api-only" | "graphql-server"
		)
	})
}

fn has_database_backend_feature(features: &[String]) -> bool {
	features.iter().any(|feature| {
		matches!(
			feature.as_str(),
			"db-postgres" | "db-sqlite" | "db-mysql" | "db-cockroachdb"
		)
	})
}

fn parse_bool_option(name: &str, value: &str) -> CommandResult<bool> {
	match value {
		"true" | "1" | "yes" => Ok(true),
		"false" | "0" | "no" => Ok(false),
		_ => Err(CommandError::InvalidArguments(format!(
			"--{name} must be true or false, got '{value}'"
		))),
	}
}

/// Configure the `reinhardt` dependency in an existing Cargo project.
pub struct ConfigureCommand;

#[async_trait]
impl BaseCommand for ConfigureCommand {
	fn name(&self) -> &str {
		"configure"
	}

	fn description(&self) -> &str {
		"Configures the Reinhardt dependency version and feature flags for an existing project."
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let root = ctx
			.arg(0)
			.map(PathBuf::from)
			.unwrap_or_else(|| PathBuf::from("."));
		let selection = resolve_dependency_selection(ctx, &[]).await?;
		update_reinhardt_dependency(&root, &selection)?;
		ctx.success(&format!(
			"Configured Reinhardt {} with features {} in {}",
			selection.version,
			selection.features_toml(),
			root.join("Cargo.toml").display()
		));
		Ok(())
	}
}

/// Update or insert the `reinhardt` dependency in `Cargo.toml`.
pub fn update_reinhardt_dependency(
	project_root: &Path,
	selection: &ReinhardtDependencySelection,
) -> CommandResult<()> {
	let cargo_path = project_root.join("Cargo.toml");
	let content = std::fs::read_to_string(&cargo_path)?;
	let updated = update_reinhardt_dependency_content(&content, selection)?;
	std::fs::write(&cargo_path, updated)?;
	Ok(())
}

/// Update or insert the `reinhardt` dependency in TOML content.
pub fn update_reinhardt_dependency_content(
	content: &str,
	selection: &ReinhardtDependencySelection,
) -> CommandResult<String> {
	let mut doc: DocumentMut = content
		.parse()
		.map_err(|e| CommandError::ParseError(format!("Failed to parse Cargo.toml: {e}")))?;
	let deps = doc
		.entry("dependencies")
		.or_insert(Item::Table(Table::new()))
		.as_table_mut()
		.ok_or_else(|| {
			CommandError::ParseError("[dependencies] must be a TOML table".to_string())
		})?;

	deps.insert(DEPENDENCY_NAME, dependency_item(selection));
	Ok(doc.to_string())
}

fn dependency_item(selection: &ReinhardtDependencySelection) -> Item {
	let mut table = InlineTable::new();
	table.insert("version", Value::from(selection.version.as_str()));
	table.insert("package", Value::from(CRATE_NAME));
	table.insert("default-features", Value::from(selection.default_features));
	let mut features = Array::default();
	for feature in &selection.features {
		features.push(feature.as_str());
	}
	table.insert("features", Value::Array(features));
	Item::Value(Value::InlineTable(table))
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	fn selection() -> ReinhardtDependencySelection {
		ReinhardtDependencySelection {
			version: "0.2.0-rc.4".to_string(),
			default_features: false,
			features: vec!["minimal".to_string(), "db-sqlite".to_string()],
		}
	}

	fn selection_with(default_features: bool, features: &[&str]) -> ReinhardtDependencySelection {
		ReinhardtDependencySelection {
			version: "0.2.0-rc.4".to_string(),
			default_features,
			features: features
				.iter()
				.map(|feature| (*feature).to_string())
				.collect(),
		}
	}

	#[rstest]
	#[case(&[], "0.2.0", "0.2.0")]
	#[case(&["0.1.0".to_string(), "0.2.0".to_string()], "0.2.0", "0.2.0")]
	#[case(&["0.3.0".to_string(), "0.1.0".to_string()], "0.2.0", "0.3.0")]
	fn default_version_prefers_current_then_first_candidate(
		#[case] candidates: &[String],
		#[case] current: &str,
		#[case] expected: &str,
	) {
		// Act
		let version = default_version(candidates, current);

		// Assert
		assert_eq!(version, expected);
	}

	#[rstest]
	#[case(
		vec!["server-fn", "pages", "minimal", "minimal"],
		vec!["pages", "minimal"]
	)]
	#[case(vec!["db-sqlite", "db-postgres"], vec!["db-sqlite", "db-postgres"])]
	fn normalize_features_deduplicates_and_preserves_first_feature_order(
		#[case] input: Vec<&str>,
		#[case] expected: Vec<&str>,
	) {
		// Act
		let features = normalize_features(input.into_iter().map(str::to_string).collect());

		// Assert
		assert_eq!(
			features,
			expected.into_iter().map(str::to_string).collect::<Vec<_>>()
		);
	}

	#[rstest]
	#[case(&["standard"], "minimal", false)]
	#[case(&["pages"], "minimal", true)]
	#[case(&["db-sqlite"], "db-postgres", false)]
	#[case(&["db-mysql"], "db-postgres", false)]
	#[case(&["minimal"], "db-postgres", true)]
	fn required_feature_selection_respects_presets_and_database_backends(
		#[case] existing: &[&str],
		#[case] required: &str,
		#[case] expected: bool,
	) {
		// Arrange
		let features = existing
			.iter()
			.map(|feature| (*feature).to_string())
			.collect::<Vec<_>>();

		// Act
		let should_add = should_add_required_feature(&features, required);

		// Assert
		assert_eq!(should_add, expected);
	}

	#[rstest]
	#[case("true", true)]
	#[case("1", true)]
	#[case("yes", true)]
	#[case("false", false)]
	#[case("0", false)]
	#[case("no", false)]
	fn parse_bool_option_accepts_all_documented_spellings(
		#[case] value: &str,
		#[case] expected: bool,
	) {
		// Act
		let parsed = parse_bool_option("default-features", value).unwrap();

		// Assert
		assert_eq!(parsed, expected);
	}

	#[test]
	fn parse_bool_option_rejects_unknown_spelling_with_flag_context() {
		// Act
		let error = parse_bool_option("default-features", "enabled").unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Invalid arguments: --default-features must be true or false, got 'enabled'"
		);
	}

	#[test]
	fn explicit_feature_values_normalizes_aliases_and_comma_separated_values() {
		// Arrange
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi(
			"feature".to_string(),
			vec!["minimal, server-fn".to_string(), "pages".to_string()],
		);
		ctx.set_option_multi(
			"features".to_string(),
			vec!["db-sqlite, minimal".to_string()],
		);

		// Act
		let features = explicit_feature_values(&ctx).unwrap();

		// Assert
		assert_eq!(features, vec!["minimal", "pages", "db-sqlite"]);
	}

	#[test]
	fn explicit_feature_values_rejects_whitespace_inside_feature_name() {
		// Arrange
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("feature".to_string(), vec!["db sqlite".to_string()]);

		// Act
		let error = explicit_feature_values(&ctx).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Invalid arguments: Feature names must not contain whitespace."
		);
	}

	#[test]
	fn features_toml_formats_array_literal() {
		assert_eq!(selection().features_toml(), "[\"minimal\", \"db-sqlite\"]");
	}

	#[test]
	fn noninteractive_pages_default_uses_required_features() {
		assert_eq!(
			default_noninteractive_features(&[
				"minimal",
				"pages",
				"client-router",
				"db-sqlite",
				"forms",
				"auth-session",
			]),
			vec![
				"minimal",
				"pages",
				"client-router",
				"db-sqlite",
				"forms",
				"auth-session",
			]
		);
	}

	#[test]
	fn noninteractive_non_pages_default_keeps_standard_preset() {
		assert_eq!(
			default_noninteractive_features(&["conf", "commands", "db-postgres", "api"]),
			vec!["standard"]
		);
	}

	#[rstest::rstest]
	fn normalize_features_maps_stale_server_fn_feature_to_pages() {
		let normalized = normalize_features(vec![
			"minimal".to_string(),
			"server-fn".to_string(),
			"pages".to_string(),
		]);

		assert_eq!(normalized, vec!["minimal".to_string(), "pages".to_string()]);
	}

	#[rstest::rstest]
	fn required_postgres_backend_does_not_override_explicit_backend() {
		let features = vec!["minimal".to_string(), "db-sqlite".to_string()];

		assert!(!should_add_required_feature(&features, "db-postgres"));
		assert!(should_add_required_feature(&features, "commands"));
	}

	#[rstest::rstest]
	fn required_minimal_runtime_respects_presets_but_repairs_explicit_pages_list() {
		let default_pages_features = vec!["standard".to_string()];
		assert!(!should_add_required_feature(
			&default_pages_features,
			"minimal"
		));

		let explicit_pages_features = vec![
			"pages".to_string(),
			"admin".to_string(),
			"db-sqlite".to_string(),
		];
		assert!(should_add_required_feature(
			&explicit_pages_features,
			"minimal"
		));
	}

	#[test]
	fn update_dependency_inserts_missing_dependency() {
		let updated = update_reinhardt_dependency_content(
			"[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
			&selection(),
		)
		.unwrap();

		assert!(updated.contains("[dependencies]"));
		assert!(updated.contains("reinhardt = { version = \"0.2.0-rc.4\""));
		assert!(updated.contains("package = \"reinhardt-web\""));
		assert!(updated.contains("features = [\"minimal\", \"db-sqlite\"]"));
	}

	#[test]
	fn update_dependency_replaces_existing_dependency() {
		let updated = update_reinhardt_dependency_content(
			"[dependencies]\nreinhardt = { version = \"0.1.0\", package = \"reinhardt-web\", features = [\"full\"] }\nserde = \"1\"\n",
			&selection(),
		)
		.unwrap();

		assert!(updated.contains("serde = \"1\""));
		assert!(updated.contains("version = \"0.2.0-rc.4\""));
		assert!(updated.contains("features = [\"minimal\", \"db-sqlite\"]"));
		assert!(!updated.contains("\"full\""));
	}

	#[rstest]
	#[case("reinhardt = \"0.1.0\"\n")]
	#[case("reinhardt = { version = \"0.1.0\", features = [\"full\"] }\n")]
	fn update_dependency_replaces_string_and_inline_table_dependencies(#[case] existing: &str) {
		// Act
		let updated = update_reinhardt_dependency_content(
			&format!("[dependencies]\n{existing}serde = \"1\"\n"),
			&selection_with(false, &["pages"]),
		)
		.unwrap();
		let doc = updated.parse::<DocumentMut>().unwrap();
		let dependency = doc["dependencies"][DEPENDENCY_NAME]
			.as_inline_table()
			.unwrap();

		// Assert
		assert_eq!(
			dependency.get("version").and_then(Value::as_str),
			Some("0.2.0-rc.4")
		);
		assert_eq!(
			dependency.get("package").and_then(Value::as_str),
			Some(CRATE_NAME)
		);
		assert_eq!(
			dependency.get("default-features").and_then(Value::as_bool),
			Some(false)
		);
		assert_eq!(
			dependency
				.get("features")
				.and_then(Value::as_array)
				.unwrap()
				.len(),
			1
		);
		assert_eq!(doc["dependencies"]["serde"].as_str(), Some("1"));
	}

	#[rstest]
	#[case(false, &["pages"])]
	#[case(true, &[])]
	fn update_dependency_preserves_feature_and_default_feature_semantics(
		#[case] default_features: bool,
		#[case] features: &[&str],
	) {
		// Act
		let updated = update_reinhardt_dependency_content(
			"[dependencies]\n# Keep this comment\nserde = \"1\"\n",
			&selection_with(default_features, features),
		)
		.unwrap();
		let doc = updated.parse::<DocumentMut>().unwrap();
		let dependency = doc["dependencies"][DEPENDENCY_NAME]
			.as_inline_table()
			.unwrap();

		// Assert
		assert!(updated.contains("# Keep this comment"));
		assert_eq!(
			dependency.get("default-features").and_then(Value::as_bool),
			Some(default_features)
		);
		assert_eq!(
			dependency
				.get("features")
				.and_then(Value::as_array)
				.unwrap()
				.iter()
				.filter_map(Value::as_str)
				.collect::<Vec<_>>(),
			features
		);
		assert_eq!(doc["dependencies"]["serde"].as_str(), Some("1"));
	}

	#[test]
	fn update_dependency_inserts_a_semantic_table_when_dependencies_are_missing() {
		// Act
		let updated = update_reinhardt_dependency_content(
			"[package]\nname = \"demo\"\n",
			&selection_with(false, &[]),
		)
		.unwrap();
		let doc = updated.parse::<DocumentMut>().unwrap();

		// Assert
		assert!(doc["dependencies"].is_table());
		assert!(doc["dependencies"][DEPENDENCY_NAME].is_value());
	}

	#[rstest]
	#[case("[dependencies\n", "Parse error: Failed to parse Cargo.toml:")]
	#[case(
		"dependencies = []\n",
		"Parse error: [dependencies] must be a TOML table"
	)]
	fn update_dependency_rejects_invalid_and_non_table_dependency_sections(
		#[case] content: &str,
		#[case] expected_prefix: &str,
	) {
		// Act
		let error = update_reinhardt_dependency_content(content, &selection()).unwrap_err();

		// Assert
		assert!(error.to_string().starts_with(expected_prefix));
	}
}
