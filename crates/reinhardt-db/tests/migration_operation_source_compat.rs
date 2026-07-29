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

	let source = repository
		.render(
			&migration,
			MigrationRenderOptions {
				include_header: true,
			},
		)
		.unwrap();

	syn::parse_file(&source).unwrap();
	assert!(source.contains("Operation::RunRust"));
	assert!(source.contains("\"seed_accounts\".to_string()"));
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
#[test]
fn migration_renderer_compatibility_matrix_covers_every_operation_variant() {
	use reinhardt_db::migrations::{
		BulkLoadFormat, BulkLoadOptions, BulkLoadSource, ColumnDefinition, Constraint, FieldType,
		FilesystemRepository, Migration, MigrationRenderOptions, Operation,
	};
	use std::collections::HashMap;

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
	let mut migration = Migration::new("0003_matrix", "accounts");
	migration.operations = operations;

	let source = FilesystemRepository::new("/unused")
		.render(
			&migration,
			MigrationRenderOptions {
				include_header: false,
			},
		)
		.unwrap();

	syn::parse_file(&source).unwrap();
	assert_eq!(source.matches("Operation::").count(), expected_variants);
}
