#[cfg(feature = "migrations")]
#[test]
fn migration_operation_source_compat() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/ui/drop_constraint_legacy.rs");
	tests.pass("tests/ui/create_index_legacy.rs");
}

#[cfg(feature = "migrations")]
#[test]
fn migration_renderer_round_trips_rust_callback_source() {
	use reinhardt_db::migrations::{
		FilesystemRepository, Migration, MigrationRenderOptions, Operation,
	};

	let mut migration = Migration::new("0001_run_rust", "accounts");
	migration.operations.push(Operation::RunRust {
		code: "seed_accounts".to_string(),
		reverse_code: None,
	});
	let repository = FilesystemRepository::new("/unused");

	let rendered = repository
		.render(
			&migration,
			MigrationRenderOptions {
				include_header: true,
			},
		)
		.unwrap();

	assert!(rendered.contains("Operation::RunRust"));
}

#[cfg(feature = "migrations")]
#[test]
fn migration_renderer_rejects_nested_exclude_constraints_without_losing_payload() {
	use reinhardt_db::migrations::{
		ColumnDefinition, Constraint, FieldType, FilesystemRepository, Migration, MigrationError,
		MigrationRenderOptions, Operation,
	};

	let exclude = || Constraint::Exclude {
		name: "accounts_active_excl".to_string(),
		elements: vec![("active".to_string(), "=".to_string())],
		using: Some("gist".to_string()),
		where_clause: Some("active".to_string()),
	};
	let cases = [
		(
			"operations[0].CreateTable.Constraint::Exclude",
			Operation::CreateTable {
				name: "accounts".to_string(),
				columns: vec![ColumnDefinition::new("active", FieldType::Boolean)],
				constraints: vec![exclude()],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		),
		(
			"operations[0].AddConstraintDefinition.Constraint::Exclude",
			Operation::AddConstraintDefinition {
				table: "accounts".to_string(),
				constraint: exclude(),
			},
		),
		(
			"operations[0].DropConstraintDefinition.Constraint::Exclude",
			Operation::DropConstraintDefinition {
				table: "accounts".to_string(),
				constraint: exclude(),
			},
		),
	];

	for (expected, operation) in cases {
		let mut migration = Migration::new("0001_exclude", "accounts");
		migration.operations.push(operation);
		let error = FilesystemRepository::new("/unused")
			.render(
				&migration,
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap_err();
		assert!(matches!(
			error,
			MigrationError::UnsupportedMigrationRendering { operation }
				if operation == expected
		));
	}
}

#[cfg(feature = "migrations")]
#[test]
fn rendered_data_bearing_source_compiles_with_its_own_imports() {
	use reinhardt_db::migrations::{
		AlterTableOptions, ColumnDefinition, Constraint, DeferrableOption, FieldType,
		FilesystemRepository, ForeignKeyAction, GeneratedColumnDefinition, GeneratedStorage,
		IndexType, InterleaveSpec, Migration, MigrationRenderOptions, MySqlAlgorithm, MySqlLock,
		Operation, PartitionDef, PartitionOptions, PartitionType, PartitionValues, SchemaExpr,
	};
	use std::{fs, process::Command};
	use tempfile::TempDir;

	let mut generated_column = ColumnDefinition::new("normalized_id", FieldType::Integer);
	generated_column.generated = Some(GeneratedColumnDefinition::typed(
		SchemaExpr::val(1_i32).cast(reinhardt_db::migrations::ColumnType::Integer),
		"SchemaExpr::val(1_i32).cast(ColumnType::Integer)",
		GeneratedStorage::Virtual,
	));
	let mut migration = Migration::new("0001_compile", "accounts");
	migration.operations = vec![
		Operation::CreateTable {
			name: "accounts".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Integer),
				generated_column,
			],
			constraints: vec![Constraint::ForeignKey {
				name: "accounts_owner_fk".to_string(),
				columns: vec!["owner_id".to_string()],
				referenced_table: "users".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::Restrict,
				deferrable: Some(DeferrableOption::Deferred),
			}],
			without_rowid: None,
			interleave_in_parent: Some(InterleaveSpec::new(
				"accounts_parent".to_string(),
				vec!["id".to_string()],
			)),
			partition: Some(PartitionOptions::new(
				PartitionType::Range,
				"id",
				vec![PartitionDef::new(
					"before_2026",
					PartitionValues::LessThan("2026-01-01".to_string()),
				)],
			)),
		},
		Operation::CreateIndex {
			table: "accounts".to_string(),
			columns: vec!["normalized_id".to_string()],
			unique: false,
			index_type: Some(IndexType::BTree),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: Some(
				AlterTableOptions::new()
					.with_algorithm(MySqlAlgorithm::Inplace)
					.with_lock(MySqlLock::Shared),
			),
			operator_class: None,
		},
	];
	let source = FilesystemRepository::new("/unused")
		.render(
			&migration,
			MigrationRenderOptions {
				include_header: false,
			},
		)
		.unwrap();
	let crate_dir = TempDir::new().unwrap();
	fs::create_dir(crate_dir.path().join("src")).unwrap();
	let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(std::path::Path::parent)
		.unwrap();
	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			"[package]\nname = \"rendered-migration-check\"\nversion = \"0.0.0\"\nedition = \
			 \"2024\"\n\n[dependencies]\nreinhardt = {{ package = \"reinhardt-web\", path = \
			 {:?}, default-features = false, features = [\"database\"] }}\n",
			workspace_root
		),
	)
	.unwrap();
	fs::write(crate_dir.path().join("src/migration.rs"), source).unwrap();
	fs::write(
		crate_dir.path().join("src/main.rs"),
		"mod migration;\nfn main() { let _ = migration::migration(); }\n",
	)
	.unwrap();

	let output = Command::new(env!("CARGO"))
		.args(["check", "--quiet"])
		.current_dir(crate_dir.path())
		.output()
		.unwrap();

	assert!(
		output.status.success(),
		"rendered migration did not compile:\n{}",
		String::from_utf8_lossy(&output.stderr)
	);
}

#[cfg(feature = "migrations")]
#[test]
fn migration_renderer_rejects_unsupported_generated_values_on_every_column_path() {
	use reinhardt_db::migrations::{
		ColumnDefinition, FieldType, FilesystemRepository, GeneratedColumnDefinition,
		GeneratedStorage, Migration, MigrationError, MigrationRenderOptions, Operation, SchemaExpr,
	};
	use reinhardt_query::Value;

	let generated_column = || {
		let mut column = ColumnDefinition::new("payload", FieldType::Binary);
		column.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::val(Value::Bytes(Some(Box::new(vec![1, 2, 3])))),
			"SchemaExpr::val(Value::Bytes(Some(Box::new(vec![1, 2, 3]))))",
			GeneratedStorage::Stored,
		));
		column
	};
	let cases = [
		(
			"operations[0].CreateTable.GeneratedColumnDefinition.SchemaExpr.Value::Bytes",
			Operation::CreateTable {
				name: "documents".to_string(),
				columns: vec![generated_column()],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		),
		(
			"operations[0].AddColumn.GeneratedColumnDefinition.SchemaExpr.Value::Bytes",
			Operation::AddColumn {
				table: "documents".to_string(),
				column: generated_column(),
				mysql_options: None,
			},
		),
		(
			"operations[0].AlterColumn.GeneratedColumnDefinition.SchemaExpr.Value::Bytes",
			Operation::AlterColumn {
				table: "documents".to_string(),
				column: "payload".to_string(),
				old_definition: None,
				new_definition: generated_column(),
				mysql_options: None,
			},
		),
	];

	for (expected, operation) in cases {
		let mut migration = Migration::new("0001_bytes", "documents");
		migration.operations.push(operation);
		let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			FilesystemRepository::new("/unused").render(
				&migration,
				MigrationRenderOptions {
					include_header: false,
				},
			)
		}));
		assert!(matches!(
			rendered,
			Ok(Err(MigrationError::UnsupportedMigrationRendering { operation }))
				if operation == expected
		));
	}
}

#[cfg(feature = "migrations")]
#[test]
fn migration_renderer_attributes_nested_failure_to_the_later_operation() {
	use reinhardt_db::migrations::{
		ColumnDefinition, FieldType, FilesystemRepository, GeneratedColumnDefinition,
		GeneratedStorage, Migration, MigrationError, MigrationRenderOptions, Operation, SchemaExpr,
	};
	use reinhardt_query::Value;

	let mut column = ColumnDefinition::new("payload", FieldType::Binary);
	column.generated = Some(GeneratedColumnDefinition::typed(
		SchemaExpr::val(Value::Bytes(Some(Box::new(vec![4, 5, 6])))),
		"SchemaExpr::val(Value::Bytes(Some(Box::new(vec![4, 5, 6]))))",
		GeneratedStorage::Stored,
	));
	let mut migration = Migration::new("0002_bad_second", "documents");
	migration.operations = vec![
		Operation::DropTable {
			name: "old_documents".to_string(),
		},
		Operation::CreateTable {
			name: "documents".to_string(),
			columns: vec![column],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		},
	];

	let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		FilesystemRepository::new("/unused").render(
			&migration,
			MigrationRenderOptions {
				include_header: false,
			},
		)
	}));

	assert!(matches!(
		rendered,
		Ok(Err(MigrationError::UnsupportedMigrationRendering { operation }))
			if operation
				== "operations[1].CreateTable.GeneratedColumnDefinition.SchemaExpr.Value::Bytes"
	));
}

#[cfg(feature = "migrations")]
#[test]
fn migration_renderer_round_trips_supported_operation_source() {
	use reinhardt_db::migrations::{
		FilesystemRepository, Migration, MigrationRenderOptions, Operation,
	};

	let operations = vec![
		Operation::CreateExtension {
			name: "pg_trgm".to_string(),
			if_not_exists: true,
			schema: Some("public".to_string()),
		},
		Operation::RenameTable {
			old_name: "account".to_string(),
			new_name: "accounts".to_string(),
		},
		Operation::RunSQL {
			sql: "UPDATE accounts SET active = TRUE".to_string(),
			reverse_sql: Some("UPDATE accounts SET active = FALSE".to_string()),
		},
	];
	let mut migration = Migration::new("0002_supported", "accounts");
	migration.operations = operations.clone();
	let repository = FilesystemRepository::new("/unused");

	let source = repository
		.render(
			&migration,
			MigrationRenderOptions {
				include_header: false,
			},
		)
		.unwrap();
	let syntax = syn::parse_file(&source).unwrap();

	assert!(!syntax.items.is_empty());
	assert!(source.contains("Operation::CreateExtension"));
	assert!(source.contains("Operation::RenameTable"));
	assert!(source.contains("Operation::RunSQL"));
}

#[cfg(feature = "migrations")]
#[tokio::test]
async fn migration_renderer_compatibility_matrix_covers_every_operation_variant() {
	use reinhardt_db::migrations::{
		BulkLoadFormat, BulkLoadOptions, BulkLoadSource, ColumnDefinition, Constraint, FieldType,
		FilesystemRepository, Migration, MigrationError, MigrationRenderOptions, MigrationSource,
		Operation,
	};
	use std::collections::HashMap;
	use tempfile::TempDir;

	let column = || ColumnDefinition::new("value", FieldType::Integer);
	let constraint = || Constraint::Check {
		name: "value_positive".to_string(),
		expression: "value > 0".to_string(),
	};
	let index_fields = || {
		(
			"accounts".to_string(),
			vec!["value".to_string()],
			false,
			None,
			None,
			false,
			None,
			None,
			None,
		)
	};
	let mut operations = vec![
		Operation::CreateTable {
			name: "accounts".to_string(),
			columns: vec![column()],
			constraints: vec![constraint()],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		},
		Operation::DropTable {
			name: "legacy".to_string(),
		},
		Operation::AddColumn {
			table: "accounts".to_string(),
			column: column(),
			mysql_options: None,
		},
		Operation::DropColumn {
			table: "accounts".to_string(),
			column: "legacy".to_string(),
			old_definition: Some(column()),
		},
		Operation::AlterColumn {
			table: "accounts".to_string(),
			column: "value".to_string(),
			old_definition: Some(column()),
			new_definition: column(),
			mysql_options: None,
		},
		Operation::RenameTable {
			old_name: "account".to_string(),
			new_name: "accounts".to_string(),
		},
		Operation::RenameColumn {
			table: "accounts".to_string(),
			old_name: "old".to_string(),
			new_name: "new".to_string(),
		},
		Operation::AddConstraint {
			table: "accounts".to_string(),
			constraint_sql: "CHECK (value > 0)".to_string(),
		},
		Operation::AddConstraintDefinition {
			table: "accounts".to_string(),
			constraint: constraint(),
		},
		Operation::AddConstraintRepair {
			table: "accounts".to_string(),
			constraint_sql: "CHECK (value > 0)".to_string(),
		},
		Operation::RestoreConstraintOnRollback {
			table: "accounts".to_string(),
			constraint_sql: "CHECK (value > 0)".to_string(),
		},
		Operation::DropConstraint {
			table: "accounts".to_string(),
			constraint_name: "value_positive".to_string(),
		},
		Operation::DropConstraintDefinition {
			table: "accounts".to_string(),
			constraint: constraint(),
		},
	];
	let (
		table,
		columns,
		unique,
		index_type,
		where_clause,
		concurrently,
		expressions,
		mysql_options,
		operator_class,
	) = index_fields();
	operations.push(Operation::CreateIndex {
		table,
		columns,
		unique,
		index_type,
		where_clause,
		concurrently,
		expressions,
		mysql_options,
		operator_class,
	});
	#[cfg(feature = "pgvector")]
	operations.push(Operation::CreateNamedIndex {
		table: "accounts".to_string(),
		name: "accounts_value_idx".to_string(),
		columns: vec!["value".to_string()],
		unique: false,
		index_type: None,
		where_clause: None,
		concurrently: false,
		expressions: None,
		mysql_options: None,
		operator_class: None,
	});
	operations.extend([
		Operation::CreateIndexRepair {
			table: "accounts".to_string(),
			name: Some("accounts_value_idx".to_string()),
			columns: vec!["value".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		},
		Operation::RestoreIndexOnRollback {
			table: "accounts".to_string(),
			name: Some("accounts_value_idx".to_string()),
			columns: vec!["value".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		},
		Operation::DropIndex {
			table: "accounts".to_string(),
			columns: vec!["value".to_string()],
		},
		Operation::DropNamedIndex {
			table: "accounts".to_string(),
			name: "accounts_value_idx".to_string(),
			columns: vec!["value".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		},
	]);
	operations.extend([
		Operation::RunSQL {
			sql: "SELECT 1".to_string(),
			reverse_sql: None,
		},
		Operation::RunRust {
			code: "forward".to_string(),
			reverse_code: Some("backward".to_string()),
		},
		Operation::AlterTableComment {
			table: "accounts".to_string(),
			comment: Some("Accounts".to_string()),
		},
		Operation::AlterUniqueTogether {
			table: "accounts".to_string(),
			unique_together: vec![vec!["value".to_string()]],
		},
		Operation::AlterModelOptions {
			table: "accounts".to_string(),
			options: HashMap::from([("ordering".to_string(), "value".to_string())]),
		},
		Operation::CreateInheritedTable {
			name: "staff_accounts".to_string(),
			columns: vec![column()],
			base_table: "accounts".to_string(),
			join_column: "account_id".to_string(),
		},
		Operation::AddDiscriminatorColumn {
			table: "accounts".to_string(),
			column_name: "kind".to_string(),
			default_value: "account".to_string(),
		},
		Operation::MoveModel {
			model_name: "Account".to_string(),
			from_app: "legacy".to_string(),
			to_app: "accounts".to_string(),
			rename_table: true,
			old_table_name: Some("legacy_account".to_string()),
			new_table_name: Some("accounts_account".to_string()),
		},
		Operation::CreateSchema {
			name: "tenant".to_string(),
			if_not_exists: true,
		},
		Operation::DropSchema {
			name: "tenant".to_string(),
			cascade: false,
			if_exists: true,
		},
		Operation::CreateExtension {
			name: "pg_trgm".to_string(),
			if_not_exists: true,
			schema: None,
		},
		Operation::BulkLoad {
			table: "accounts".to_string(),
			source: BulkLoadSource::Stdin,
			format: BulkLoadFormat::Csv,
			options: BulkLoadOptions::default(),
		},
		Operation::SetAutoIncrementValue {
			table: "accounts".to_string(),
			column: "id".to_string(),
			value: 42,
		},
		Operation::CreateCompositePrimaryKey {
			table: "accounts".to_string(),
			columns: vec!["tenant_id".to_string(), "id".to_string()],
			constraint_name: Some("accounts_pkey".to_string()),
		},
	]);
	let expected_variants = operations.len();
	#[cfg(feature = "pgvector")]
	assert_eq!(expected_variants, 33);
	#[cfg(not(feature = "pgvector"))]
	assert_eq!(expected_variants, 32);
	for (index, operation) in operations.into_iter().enumerate() {
		let (kind, supported) = match &operation {
			Operation::CreateTable { .. } => ("CreateTable", true),
			Operation::DropTable { .. } => ("DropTable", true),
			Operation::AddColumn { .. } => ("AddColumn", true),
			Operation::DropColumn { .. } => ("DropColumn", true),
			Operation::AlterColumn { .. } => ("AlterColumn", true),
			Operation::RenameTable { .. } => ("RenameTable", true),
			Operation::RenameColumn { .. } => ("RenameColumn", true),
			Operation::AddConstraint { .. } => ("AddConstraint", true),
			Operation::AddConstraintDefinition { .. } => ("AddConstraintDefinition", true),
			Operation::AddConstraintRepair { .. } => ("AddConstraintRepair", true),
			Operation::RestoreConstraintOnRollback { .. } => ("RestoreConstraintOnRollback", true),
			Operation::DropConstraint { .. } => ("DropConstraint", true),
			Operation::DropConstraintDefinition { .. } => ("DropConstraintDefinition", true),
			Operation::CreateIndex { .. } => ("CreateIndex", true),
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex { .. } => ("CreateNamedIndex", true),
			Operation::CreateIndexRepair { .. } => ("CreateIndexRepair", true),
			Operation::RestoreIndexOnRollback { .. } => ("RestoreIndexOnRollback", true),
			Operation::DropIndex { .. } => ("DropIndex", true),
			Operation::DropNamedIndex { .. } => ("DropNamedIndex", true),
			Operation::RunSQL { .. } => ("RunSQL", true),
			Operation::RunRust { .. } => ("RunRust", true),
			Operation::AlterTableComment { .. } => ("AlterTableComment", true),
			Operation::AlterUniqueTogether { .. } => ("AlterUniqueTogether", true),
			Operation::AlterModelOptions { .. } => ("AlterModelOptions", true),
			Operation::CreateInheritedTable { .. } => ("CreateInheritedTable", true),
			Operation::AddDiscriminatorColumn { .. } => ("AddDiscriminatorColumn", true),
			Operation::MoveModel { .. } => ("MoveModel", true),
			Operation::CreateSchema { .. } => ("CreateSchema", true),
			Operation::DropSchema { .. } => ("DropSchema", true),
			Operation::CreateExtension { .. } => ("CreateExtension", true),
			Operation::BulkLoad { .. } => ("BulkLoad", true),
			Operation::SetAutoIncrementValue { .. } => ("SetAutoIncrementValue", true),
			Operation::CreateCompositePrimaryKey { .. } => ("CreateCompositePrimaryKey", true),
		};
		let temp_dir = TempDir::new().unwrap();
		let repository = FilesystemRepository::new(temp_dir.path());
		let migration_name = format!("0001_matrix_{index}");
		let mut migration = Migration::new(&migration_name, "accounts");
		migration.operations.push(operation.clone());
		let rendered = repository.render(
			&migration,
			MigrationRenderOptions {
				include_header: false,
			},
		);
		if !supported {
			assert!(matches!(
				rendered,
				Err(MigrationError::UnsupportedMigrationRendering { operation })
					if operation == format!("operations[0].{kind}")
			));
			continue;
		}
		let source = rendered.unwrap_or_else(|error| panic!("{kind}: {error}"));
		repository
			.create_new_source("accounts", &migration_name, &source)
			.unwrap();
		let loaded = reinhardt_db::migrations::FilesystemSource::new(temp_dir.path())
			.all_migrations()
			.await
			.unwrap();
		assert_eq!(loaded[0].operations, vec![operation], "{kind}");
	}
}
