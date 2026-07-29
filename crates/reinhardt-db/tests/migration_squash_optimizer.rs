use reinhardt_db::migrations::dependency::{
	DependencyCondition, OptionalDependency, SwappableDependency,
};
use reinhardt_db::migrations::squash::MigrationSquasher;
use reinhardt_db::migrations::{
	BulkLoadFormat, BulkLoadOptions, BulkLoadSource, ColumnDefinition, FieldType, Migration,
	Operation, SquashRange,
};

fn column(name: &str, field_type: FieldType) -> ColumnDefinition {
	ColumnDefinition::new(name, field_type)
}

fn create_table(name: &str) -> Operation {
	Operation::CreateTable {
		name: name.to_string(),
		columns: vec![column("id", FieldType::Integer)],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	}
}

fn drop_table(name: &str) -> Operation {
	Operation::DropTable {
		name: name.to_string(),
	}
}

fn add_column(table: &str, name: &str, field_type: FieldType) -> Operation {
	Operation::AddColumn {
		table: table.to_string(),
		column: column(name, field_type),
		mysql_options: None,
	}
}

fn drop_column(table: &str, name: &str) -> Operation {
	Operation::DropColumn {
		table: table.to_string(),
		column: name.to_string(),
		old_definition: None,
	}
}

fn alter_column(table: &str, name: &str, old_type: FieldType, new_type: FieldType) -> Operation {
	Operation::AlterColumn {
		table: table.to_string(),
		column: name.to_string(),
		old_definition: Some(column(name, old_type)),
		new_definition: column(name, new_type),
		mysql_options: None,
	}
}

fn migration(name: &str, operations: Vec<Operation>) -> Migration {
	let mut migration = Migration::new(name, "accounts");
	migration.operations = operations;
	migration
}

fn range(migrations: Vec<Migration>) -> SquashRange {
	SquashRange {
		migrations,
		external_dependencies: vec![
			("auth".to_string(), "0001_initial".to_string()),
			("audit".to_string(), "0002_events".to_string()),
		],
	}
}

#[test]
fn optimizer_applies_only_proven_schema_reductions() {
	struct Case {
		name: &'static str,
		operations: Vec<Operation>,
		expected: Vec<Operation>,
	}

	let retained_other_table = add_column("profiles", "display_name", FieldType::VarChar(120));
	let first_alter = alter_column(
		"accounts",
		"handle",
		FieldType::VarChar(40),
		FieldType::VarChar(80),
	);
	let second_alter = alter_column(
		"accounts",
		"handle",
		FieldType::VarChar(80),
		FieldType::VarChar(160),
	);
	let expected_alter = alter_column(
		"accounts",
		"handle",
		FieldType::VarChar(40),
		FieldType::VarChar(160),
	);
	let cases = vec![
		Case {
			name: "create then drop removes the transient table lifecycle",
			operations: vec![
				create_table("temporary"),
				add_column("temporary", "note", FieldType::Text),
				retained_other_table.clone(),
				drop_table("temporary"),
			],
			expected: vec![retained_other_table.clone()],
		},
		Case {
			name: "add then drop removes the transient column lifecycle",
			operations: vec![
				add_column("accounts", "temporary", FieldType::Integer),
				alter_column(
					"accounts",
					"temporary",
					FieldType::Integer,
					FieldType::BigInteger,
				),
				retained_other_table.clone(),
				drop_column("accounts", "temporary"),
			],
			expected: vec![retained_other_table.clone()],
		},
		Case {
			name: "adjacent alters retain the first old and final new definitions",
			operations: vec![first_alter, second_alter],
			expected: vec![expected_alter],
		},
		Case {
			name: "independent tables reduce without reordering retained operations",
			operations: vec![
				add_column("accounts", "temporary", FieldType::Integer),
				add_column("profiles", "temporary", FieldType::Integer),
				retained_other_table.clone(),
				drop_column("accounts", "temporary"),
				drop_column("profiles", "temporary"),
			],
			expected: vec![retained_other_table],
		},
	];

	let squasher = MigrationSquasher::new();
	for case in cases {
		assert_eq!(
			squasher.optimize_operations(case.operations),
			case.expected,
			"{}",
			case.name
		);
	}
}

#[test]
fn optimizer_uses_original_adjacency_for_alter_reductions() {
	let first_alter = alter_column(
		"accounts",
		"handle",
		FieldType::VarChar(40),
		FieldType::VarChar(80),
	);
	let second_alter = alter_column(
		"accounts",
		"handle",
		FieldType::VarChar(80),
		FieldType::VarChar(160),
	);
	let operations = vec![
		first_alter.clone(),
		add_column("profiles", "temporary", FieldType::Integer),
		drop_column("profiles", "temporary"),
		second_alter.clone(),
	];

	let optimized = MigrationSquasher::new().optimize_operations(operations);

	assert_eq!(optimized, vec![first_alter, second_alter]);
}

#[test]
fn optimizer_coalesces_only_alters_adjacent_in_the_original_segment() {
	let first_alter = alter_column(
		"accounts",
		"handle",
		FieldType::VarChar(40),
		FieldType::VarChar(80),
	);
	let second_alter = alter_column(
		"accounts",
		"handle",
		FieldType::VarChar(80),
		FieldType::VarChar(160),
	);
	let expected_alter = alter_column(
		"accounts",
		"handle",
		FieldType::VarChar(40),
		FieldType::VarChar(160),
	);

	let adjacent = MigrationSquasher::new()
		.optimize_operations(vec![first_alter.clone(), second_alter.clone()]);
	let separated_by_barrier = MigrationSquasher::new().optimize_operations(vec![
		first_alter.clone(),
		Operation::RunSQL {
			sql: "SELECT handle FROM accounts".to_string(),
			reverse_sql: None,
		},
		second_alter.clone(),
	]);

	assert_eq!(adjacent, vec![expected_alter]);
	assert_eq!(
		separated_by_barrier,
		vec![
			first_alter,
			Operation::RunSQL {
				sql: "SELECT handle FROM accounts".to_string(),
				reverse_sql: None,
			},
			second_alter,
		]
	);
}

#[test]
fn optimizer_does_not_reduce_across_barriers() {
	struct Case {
		name: &'static str,
		barrier: Operation,
	}

	let cases = vec![
		Case {
			name: "RunSQL",
			barrier: Operation::RunSQL {
				sql: "UPDATE accounts SET temporary = 1".to_string(),
				reverse_sql: None,
			},
		},
		Case {
			name: "RunRust",
			barrier: Operation::RunRust {
				code: "backfill_accounts".to_string(),
				reverse_code: None,
			},
		},
		Case {
			name: "BulkLoad",
			barrier: Operation::BulkLoad {
				table: "accounts".to_string(),
				source: BulkLoadSource::Stdin,
				format: BulkLoadFormat::Csv,
				options: BulkLoadOptions::default(),
			},
		},
		Case {
			name: "custom schema operation",
			barrier: Operation::AddConstraint {
				table: "accounts".to_string(),
				constraint_sql: "CHECK (temporary >= 0)".to_string(),
			},
		},
		Case {
			name: "otherwise unknown operation",
			barrier: Operation::RenameColumn {
				table: "profiles".to_string(),
				old_name: "label".to_string(),
				new_name: "display_name".to_string(),
			},
		},
	];

	let squasher = MigrationSquasher::new();
	for case in cases {
		let operations = vec![
			add_column("accounts", "temporary", FieldType::Integer),
			case.barrier,
			drop_column("accounts", "temporary"),
		];

		assert_eq!(
			squasher.optimize_operations(operations.clone()),
			operations,
			"{} must split optimization segments",
			case.name
		);
	}
}

#[test]
fn squash_range_preserves_order_metadata_replacements_and_flags() {
	let swappable = SwappableDependency::new("AUTH_USER_MODEL", "auth", "User", "0001_initial");
	let optional = OptionalDependency::new(
		"audit",
		"0002_events",
		DependencyCondition::FeatureEnabled("audit".to_string()),
	);
	let first_operation = add_column("accounts", "temporary", FieldType::Integer);
	let barrier = Operation::RunSQL {
		sql: "UPDATE accounts SET temporary = 1".to_string(),
		reverse_sql: None,
	};
	let last_operation = drop_column("accounts", "temporary");

	let mut first = migration("0003_previous_squash", vec![first_operation.clone()]);
	first.replaces = vec![
		("accounts".to_string(), "0001_initial".to_string()),
		("accounts".to_string(), "0002_profile".to_string()),
	];
	first.atomic = true;
	first.initial = Some(true);
	first.state_only = true;
	first.swappable_dependencies = vec![swappable.clone()];
	first.optional_dependencies = vec![optional.clone()];

	let mut second = migration(
		"0004_backfill",
		vec![barrier.clone(), last_operation.clone()],
	);
	second.atomic = false;
	second.state_only = true;
	second.swappable_dependencies = vec![swappable.clone()];
	second.optional_dependencies = vec![optional.clone()];

	let result = MigrationSquasher::new()
		.squash_range(&range(vec![first, second]), "0001_squashed_0004", true)
		.expect("compatible migration flags should squash");

	assert_eq!(result.original_operation_count, 3);
	assert_eq!(result.optimized_operation_count, 3);
	assert_eq!(
		result.migration.operations,
		vec![first_operation, barrier, last_operation]
	);
	assert_eq!(
		result.migration.dependencies,
		vec![
			("auth".to_string(), "0001_initial".to_string()),
			("audit".to_string(), "0002_events".to_string()),
		]
	);
	assert_eq!(
		result.migration.replaces,
		vec![
			("accounts".to_string(), "0001_initial".to_string()),
			("accounts".to_string(), "0002_profile".to_string()),
			("accounts".to_string(), "0003_previous_squash".to_string()),
			("accounts".to_string(), "0004_backfill".to_string()),
		]
	);
	assert!(!result.migration.atomic);
	assert_eq!(result.migration.initial, Some(true));
	assert!(result.migration.state_only);
	assert!(!result.migration.database_only);
	assert_eq!(result.migration.swappable_dependencies, vec![swappable]);
	assert_eq!(result.migration.optional_dependencies, vec![optional]);
}

#[test]
fn squash_range_rejects_mixed_whole_migration_modes() {
	for (name, first_state_only, second_state_only, first_database_only, second_database_only) in [
		("state_only", false, true, false, false),
		("database_only", false, false, false, true),
	] {
		let mut first = migration("0001_initial", vec![]);
		first.state_only = first_state_only;
		first.database_only = first_database_only;
		let mut second = migration("0002_change", vec![]);
		second.state_only = second_state_only;
		second.database_only = second_database_only;

		let error = MigrationSquasher::new()
			.squash_range(&range(vec![first, second]), "0001_squashed_0002", true)
			.expect_err("mixed whole-migration execution modes must be rejected");

		assert_eq!(
			error.to_string(),
			format!("Invalid migration: Cannot squash migrations with mixed {name} flags")
		);
	}
}

#[test]
fn squash_range_rejects_publicly_constructed_cross_app_ranges() {
	let first = migration(
		"0001_initial",
		vec![add_column("accounts", "handle", FieldType::VarChar(80))],
	);
	let mut second = migration(
		"0002_profile",
		vec![add_column("profiles", "bio", FieldType::Text)],
	);
	second.app_label = "profiles".to_string();

	let error = MigrationSquasher::new()
		.squash_range(&range(vec![first, second]), "0001_squashed_0002", true)
		.expect_err("public squash ranges must not combine apps");

	assert_eq!(
		error.to_string(),
		"Invalid migration: All migrations in squash range must belong to the same app"
	);
}

#[test]
fn squash_range_without_optimization_preserves_exact_source_order() {
	let operations = vec![
		add_column("accounts", "temporary", FieldType::Integer),
		Operation::RunRust {
			code: "observe_temporary".to_string(),
			reverse_code: None,
		},
		drop_column("accounts", "temporary"),
		create_table("scratch"),
		drop_table("scratch"),
	];
	let selected = range(vec![
		migration("0001_initial", operations[..2].to_vec()),
		migration("0002_cleanup", operations[2..].to_vec()),
	]);

	let result = MigrationSquasher::new()
		.squash_range(&selected, "0001_squashed_0002", false)
		.expect("valid range should squash without optimization");

	assert_eq!(result.original_operation_count, operations.len());
	assert_eq!(result.optimized_operation_count, operations.len());
	assert_eq!(result.migration.operations, operations);
}
