use async_trait::async_trait;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldState, FieldType, Migration, MigrationCatalog, MigrationError,
	MigrationKey, MigrationSource, ModelState, Operation, ProjectState, Result, SchemaCheckError,
	SchemaContractState, SchemaFinding, verify_schema_contract,
};
use std::collections::BTreeSet;

struct TestSource {
	migrations: Vec<Migration>,
}

#[async_trait]
impl MigrationSource for TestSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		Ok(self.migrations.clone())
	}
}

fn create_table(app: &str, name: &str, table: &str) -> Migration {
	Migration::new(name, app).add_operation(Operation::CreateTable {
		name: table.to_string(),
		columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
		constraints: Vec::new(),
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	})
}

async fn strict_catalog(migrations: Vec<Migration>) -> MigrationCatalog {
	MigrationCatalog::load_strict(&TestSource { migrations })
		.await
		.expect("test migrations must form a strict catalog")
}

#[tokio::test]
async fn resolved_project_state_replays_cross_application_dependencies() {
	let users = create_table("users", "0001_initial", "users");
	let posts =
		create_table("posts", "0001_initial", "posts").add_dependency("users", "0001_initial");
	let catalog = strict_catalog(vec![posts, users]).await;

	let mut expected_state = ProjectState::new();
	create_table("users", "0001_initial", "users").operations[0]
		.state_forwards("users", &mut expected_state);
	create_table("posts", "0001_initial", "posts").operations[0]
		.state_forwards("posts", &mut expected_state);

	let resolved = catalog
		.resolved_project_state()
		.expect("strict catalog replay should succeed");

	assert_eq!(resolved, expected_state);
}

#[tokio::test]
async fn resolved_project_state_skips_database_only_migrations() {
	let mut database_only = create_table("catalog", "0001_database_only", "books");
	database_only.database_only = true;
	let catalog = strict_catalog(vec![database_only]).await;

	let resolved = catalog
		.resolved_project_state()
		.expect("strict catalog replay should succeed");

	assert_eq!(resolved, ProjectState::new());
}

#[tokio::test]
async fn resolved_project_state_replays_state_only_migrations() {
	let mut state_only = create_table("catalog", "0001_state_only", "books");
	state_only.state_only = true;
	let catalog = strict_catalog(vec![state_only]).await;

	let mut expected_state = ProjectState::new();
	create_table("catalog", "0001_state_only", "books").operations[0]
		.state_forwards("catalog", &mut expected_state);
	let resolved = catalog
		.resolved_project_state()
		.expect("strict catalog replay should succeed");

	assert_eq!(resolved, expected_state);
}

#[tokio::test]
async fn resolved_project_state_preserves_opaque_schema_operations() {
	let opaque = Migration::new("0001_opaque", "catalog").add_operation(Operation::RunSQL {
		sql: "CREATE INDEX books_title_idx ON books (title)".to_string(),
		reverse_sql: None,
	});
	let catalog = strict_catalog(vec![opaque]).await;

	let resolved = catalog
		.resolved_project_state()
		.expect("strict catalog replay should succeed");

	assert!(resolved.has_opaque_schema_operations);
}

#[tokio::test]
async fn strict_catalog_rejects_malformed_graphs_before_state_replay() {
	let missing_dependency =
		create_table("catalog", "0001_initial", "books").add_dependency("missing", "0001_initial");
	let error = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![missing_dependency],
	})
	.await
	.expect_err("missing dependency must fail catalog construction");
	assert!(matches!(error, MigrationError::DependencyError(_)));

	let mut replacement = create_table("catalog", "0001_squashed", "books");
	replacement.replaces = vec![("catalog".to_string(), "0001_squashed".to_string())];
	let error = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![replacement],
	})
	.await
	.expect_err("self replacement must fail catalog construction");
	assert!(matches!(error, MigrationError::InvalidMigration(_)));
}

fn state_with_model(app: &str, name: &str, fields: Vec<FieldState>) -> ProjectState {
	let mut model = ModelState::new(app, name);
	for field in fields {
		model.add_field(field);
	}
	let mut state = ProjectState::new();
	state.add_model(model);
	state
}

fn schema_contract(
	model_state: ProjectState,
	migration_state: ProjectState,
) -> SchemaContractState {
	SchemaContractState {
		model_state,
		migration_state,
		known_migrations: Vec::new(),
		applied_migrations: None,
		replacement_edges: Vec::new(),
	}
}

#[test]
fn schema_contract_reports_no_drift_for_equal_states() {
	let state = state_with_model(
		"books",
		"Book",
		vec![FieldState::new("id", FieldType::Integer, false)],
	);

	let verification = verify_schema_contract(&schema_contract(state.clone(), state));

	assert_eq!(verification.findings, Vec::<SchemaFinding>::new());
	assert_eq!(verification.check_errors, Vec::<SchemaCheckError>::new());
}

#[test]
fn schema_contract_reports_each_generated_operation_as_missing_migration() {
	let model_state = state_with_model(
		"books",
		"Book",
		vec![FieldState::new("id", FieldType::Integer, false)],
	);

	let verification = verify_schema_contract(&schema_contract(model_state, ProjectState::new()));

	assert_eq!(
		verification.findings,
		vec![SchemaFinding::MissingMigration {
			app_label: "books".to_string(),
			name_fragment: "books_book".to_string(),
			description: "Create table books_book".to_string(),
		}]
	);
	assert_eq!(verification.check_errors, Vec::<SchemaCheckError>::new());
}

#[test]
fn schema_contract_reports_autodetector_ambiguity_without_guessing_drift() {
	let migration_state = state_with_model(
		"projects",
		"Project",
		vec![
			FieldState::new("old_code", FieldType::VarChar(255), false),
			FieldState::new("legacy_code", FieldType::VarChar(255), false),
		],
	);
	let model_state = state_with_model(
		"projects",
		"Project",
		vec![FieldState::new(
			"project_code",
			FieldType::VarChar(255),
			false,
		)],
	);

	let verification = verify_schema_contract(&schema_contract(model_state, migration_state));

	assert_eq!(verification.findings, Vec::<SchemaFinding>::new());
	assert_eq!(
		verification.check_errors,
		vec![SchemaCheckError::Autodetector { app_label: None }]
	);
}

#[test]
fn schema_contract_reports_opaque_state_without_skipping_unapplied_migrations() {
	let mut migration_state = ProjectState::new();
	migration_state.has_opaque_schema_operations = true;
	let missing = MigrationKey::new("books", "0001_initial");
	let state = SchemaContractState {
		model_state: ProjectState::new(),
		migration_state,
		known_migrations: vec![missing.clone()],
		applied_migrations: Some(BTreeSet::new()),
		replacement_edges: Vec::new(),
	};

	let verification = verify_schema_contract(&state);

	assert_eq!(
		verification.findings,
		vec![SchemaFinding::UnappliedMigration { migration: missing }]
	);
	assert_eq!(
		verification.check_errors,
		vec![SchemaCheckError::OpaqueMigrationState]
	);
}

#[test]
fn schema_contract_skips_applied_comparison_when_recorder_is_unavailable() {
	let state = SchemaContractState {
		model_state: ProjectState::new(),
		migration_state: ProjectState::new(),
		known_migrations: vec![MigrationKey::new("books", "0001_initial")],
		applied_migrations: None,
		replacement_edges: Vec::new(),
	};

	let verification = verify_schema_contract(&state);

	assert_eq!(verification.findings, Vec::<SchemaFinding>::new());
	assert_eq!(verification.check_errors, Vec::<SchemaCheckError>::new());
}

#[test]
fn schema_contract_marks_terminal_squash_and_replaced_ancestors_as_covered() {
	let initial = MigrationKey::new("books", "0001_initial");
	let first_squash = MigrationKey::new("books", "0001_squashed_0002");
	let terminal_squash = MigrationKey::new("books", "0001_squashed_0003");
	let state = SchemaContractState {
		model_state: ProjectState::new(),
		migration_state: ProjectState::new(),
		known_migrations: vec![
			initial.clone(),
			first_squash.clone(),
			terminal_squash.clone(),
		],
		applied_migrations: Some(BTreeSet::from([terminal_squash.clone()])),
		replacement_edges: vec![
			(first_squash.clone(), initial),
			(terminal_squash, first_squash.clone()),
		],
	};

	let verification = verify_schema_contract(&state);

	assert_eq!(verification.findings, Vec::<SchemaFinding>::new());
}

#[test]
fn schema_contract_marks_a_replacement_covered_when_all_inputs_are_applied() {
	let first = MigrationKey::new("books", "0001_initial");
	let second = MigrationKey::new("books", "0002_title");
	let squash = MigrationKey::new("books", "0001_squashed_0002");
	let state = SchemaContractState {
		model_state: ProjectState::new(),
		migration_state: ProjectState::new(),
		known_migrations: vec![first.clone(), second.clone(), squash.clone()],
		applied_migrations: Some(BTreeSet::from([first.clone(), second.clone()])),
		replacement_edges: vec![
			(squash, first),
			(MigrationKey::new("books", "0001_squashed_0002"), second),
		],
	};

	let verification = verify_schema_contract(&state);

	assert_eq!(verification.findings, Vec::<SchemaFinding>::new());
}
