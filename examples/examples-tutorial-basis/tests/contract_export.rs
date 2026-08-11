#![cfg(server)]

use jsonschema::draft202012;
use serde_json::Value;
use std::process::{Command, Output};
use tempfile::TempDir;

const MANAGE: &str = env!("CARGO_BIN_EXE_manage");
const SCHEMA_URL: &str = "https://reinhardt-web.dev/schemas/application-contract/v0.json";

fn export_contract(cwd: &std::path::Path, database_url: Option<&str>) -> Output {
	let mut command = Command::new(MANAGE);
	command
		.current_dir(cwd)
		.env("REINHARDT_ENV", "local")
		.env_remove("DATABASE_URL")
		.args(["contract", "export", "--format", "json"]);
	if let Some(url) = database_url {
		command.args(["--database-url", url]);
	}
	command.output().expect("run tutorial manage binary")
}

fn schema_validator() -> jsonschema::Validator {
	let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(2)
		.expect("repository root");
	let schema: Value = serde_json::from_str(
		&std::fs::read_to_string(
			root.join("website/static/schemas/application-contract/v0.json"),
		)
		.expect("read v0 schema"),
	)
	.expect("parse v0 schema");
	draft202012::new(&schema).expect("compile v0 schema")
}

#[test]
fn export_is_deterministic_and_schema_valid() {
	let temp = TempDir::new().expect("create export directory");
	let database_path = temp.path().join("contract.sqlite3");
	let database_url = format!("sqlite://{}?mode=rwc", database_path.display());

	let first = export_contract(temp.path(), Some(&database_url));
	assert!(
		first.status.success(),
		"explicit contract export should succeed: {}",
		String::from_utf8_lossy(&first.stderr)
	);
	assert!(first.stderr.is_empty());
	let second = export_contract(temp.path(), Some(&database_url));
	assert!(second.status.success());
	assert!(second.stderr.is_empty());
	assert_eq!(first.stdout, second.stdout);
	assert!(first.stdout.ends_with(b"\n"));
	assert!(!first.stdout.ends_with(b"\n\n"));

	let document: Value = serde_json::from_slice(&first.stdout).expect("parse contract JSON");
	assert_eq!(document["$schema"], SCHEMA_URL);
	assert!(document["models"].as_array().is_some_and(|items| !items.is_empty()));
	assert!(document["migrations"].as_array().is_some_and(|items| !items.is_empty()));
	assert!(document["routes"].as_array().is_some_and(|items| !items.is_empty()));
	assert!(document["settings"].as_array().is_some_and(|items| !items.is_empty()));
	let choices = document["models"]
		.as_array()
		.expect("model array")
		.iter()
		.find(|model| model["table_name"] == "choices")
		.expect("tutorial choices model");
	let question_id = choices["fields"]
		.as_array()
		.expect("choices field array")
		.iter()
		.find(|field| field["name"] == "question_id")
		.expect("choices question_id field");
	assert_eq!(question_id["type"]["kind"], "big_integer");
	for migration in document["migrations"].as_array().expect("migration array") {
		assert_eq!(migration["applied"], false);
	}
	schema_validator()
		.validate(&document)
		.expect("tutorial export validates against v0 schema");
}

#[test]
fn implicit_database_failure_is_warning_with_null_applied_state() {
	let temp = TempDir::new().expect("create inaccessible database directory");
	std::fs::create_dir(temp.path().join("db.sqlite3")).expect("reserve sqlite path as directory");

	let output = export_contract(temp.path(), None);
	assert!(
		output.status.success(),
		"implicit database failure should not fail export: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let warning = String::from_utf8(output.stderr).expect("warning should be UTF-8");
	assert_eq!(warning.matches("Warning:").count(), 1);
	let document: Value = serde_json::from_slice(&output.stdout).expect("parse contract JSON");
	for migration in document["migrations"].as_array().expect("migration array") {
		assert!(migration["applied"].is_null());
	}
	assert!(!warning.contains("sqlite3"));
}

#[cfg(feature = "commands-shell")]
#[test]
fn shell_uses_plain_project_settings_factory() {
	let _: fn() -> examples_tutorial_basis::config::settings::ProjectSettings =
		examples_tutorial_basis::config::settings::get_shell_settings;
	assert_eq!(
		examples_tutorial_basis::config::shell::get_shell_config().settings_factory_path(),
		"examples_tutorial_basis::config::settings::get_shell_settings"
	);
}
