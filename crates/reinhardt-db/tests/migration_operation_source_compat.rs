#[cfg(feature = "migrations")]
#[test]
fn migration_operation_source_compat() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/ui/drop_constraint_legacy.rs");
	tests.pass("tests/ui/create_index_legacy.rs");
	tests.pass("tests/ui/migration_renderer_supported_variants.rs");
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

	let error = repository
		.render(
			&migration,
			MigrationRenderOptions {
				include_header: true,
			},
		)
		.unwrap_err();

	assert!(matches!(
		error,
		reinhardt_db::migrations::MigrationError::UnsupportedMigrationRendering {
			operation
		} if operation == "RunRust"
	));
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
			"CreateTable.Constraint::Exclude",
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
			"AddConstraintDefinition.Constraint::Exclude",
			Operation::AddConstraintDefinition {
				table: "accounts".to_string(),
				constraint: exclude(),
			},
		),
		(
			"DropConstraintDefinition.Constraint::Exclude",
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
	]);
	#[cfg(feature = "pgvector")]
	operations.push(Operation::DropNamedIndex {
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
	assert_eq!(expected_variants, 31);
	for (index, operation) in operations.into_iter().enumerate() {
		let (kind, supported) = match &operation {
			Operation::CreateTable { .. } => ("CreateTable", true),
			Operation::DropTable { .. } => ("DropTable", true),
			Operation::AddColumn { .. } => ("AddColumn", true),
			Operation::DropColumn { .. } => ("DropColumn", true),
			Operation::AlterColumn { .. } => ("AlterColumn", false),
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
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex { .. } => ("DropNamedIndex", true),
			Operation::RunSQL { .. } => ("RunSQL", true),
			Operation::RunRust { .. } => ("RunRust", false),
			Operation::AlterTableComment { .. } => ("AlterTableComment", false),
			Operation::AlterUniqueTogether { .. } => ("AlterUniqueTogether", false),
			Operation::AlterModelOptions { .. } => ("AlterModelOptions", false),
			Operation::CreateInheritedTable { .. } => ("CreateInheritedTable", false),
			Operation::AddDiscriminatorColumn { .. } => ("AddDiscriminatorColumn", false),
			Operation::MoveModel { .. } => ("MoveModel", false),
			Operation::CreateSchema { .. } => ("CreateSchema", false),
			Operation::DropSchema { .. } => ("DropSchema", false),
			Operation::CreateExtension { .. } => ("CreateExtension", true),
			Operation::BulkLoad { .. } => ("BulkLoad", false),
			Operation::SetAutoIncrementValue { .. } => ("SetAutoIncrementValue", false),
			Operation::CreateCompositePrimaryKey { .. } => ("CreateCompositePrimaryKey", false),
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
					if operation == kind
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
