//! Shared SQL planning for migration execution and inspection.

use super::{
	DatabaseMigrationExecutor, Migration, MigrationError, Operation, ProjectState, Result,
	SchemaEditor,
	operations::{PlannedOperationOutput, SqlDialect, SqliteTableRecreation},
};
use crate::backends::{DatabaseConnection, types::DatabaseType};
#[cfg(feature = "sqlite")]
use std::collections::HashMap;

/// Direction in which a migration is planned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationDirection {
	/// Apply the migration.
	Forward,
	/// Unapply the migration.
	Backward,
}

/// A complete SQL plan for one migration.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrationSqlPlan {
	/// Whether the migration requests atomic execution.
	pub atomic: bool,
	/// Statements in execution order.
	pub statements: Vec<PlannedStatement>,
	pub(crate) planned_operations: Vec<Option<Operation>>,
	pub(crate) sqlite_recreation_groups: Vec<Option<usize>>,
	pub(crate) direction: MigrationDirection,
}

/// One item in a migration SQL plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedStatement {
	/// Executable SQL.
	Sql(String),
	/// Informational output that must not be sent to the database.
	Comment(String),
}

fn migration_sql_dialect(connection: &DatabaseConnection) -> SqlDialect {
	if connection.is_cockroachdb() {
		return SqlDialect::Cockroachdb;
	}

	match connection.database_type() {
		DatabaseType::Postgres => SqlDialect::Postgres,
		DatabaseType::Mysql => SqlDialect::Mysql,
		DatabaseType::Sqlite => SqlDialect::Sqlite,
	}
}

/// Splits SQL into payloads accepted by database prepared-statement protocols.
pub(crate) fn split_sql_statements(sql: &str) -> Vec<String> {
	let mut statements = Vec::new();
	let mut current = String::new();
	let mut chars = sql.chars().peekable();

	#[derive(Debug, PartialEq)]
	enum State {
		Normal,
		SingleQuote,
		DoubleQuote,
		LineComment,
		BlockComment,
		DollarQuote(String),
	}

	let mut state = State::Normal;

	while let Some(ch) = chars.next() {
		match state {
			State::Normal => {
				if ch == '\'' {
					current.push(ch);
					state = State::SingleQuote;
				} else if ch == '"' {
					current.push(ch);
					state = State::DoubleQuote;
				} else if ch == '-' && chars.peek() == Some(&'-') {
					current.push(ch);
					current.push(chars.next().expect("peeked SQL comment marker"));
					state = State::LineComment;
				} else if ch == '/' && chars.peek() == Some(&'*') {
					current.push(ch);
					current.push(chars.next().expect("peeked SQL comment marker"));
					state = State::BlockComment;
				} else if ch == '$' {
					let mut tag = String::from("$");
					current.push(ch);
					while let Some(&next_ch) = chars.peek() {
						if next_ch == '$' {
							tag.push(chars.next().expect("peeked dollar quote"));
							current.push('$');
							state = State::DollarQuote(tag);
							break;
						} else if next_ch.is_alphanumeric() || next_ch == '_' {
							tag.push(chars.next().expect("peeked dollar quote tag"));
							current.push(next_ch);
						} else {
							break;
						}
					}
				} else if ch == ';' {
					let trimmed = current.trim();
					if !trimmed.is_empty() {
						statements.push(trimmed.to_string());
					}
					current.clear();
				} else {
					current.push(ch);
				}
			}
			State::SingleQuote => {
				current.push(ch);
				if ch == '\'' {
					if chars.peek() == Some(&'\'') {
						current.push(chars.next().expect("peeked escaped quote"));
					} else {
						state = State::Normal;
					}
				} else if ch == '\\' && chars.peek().is_some() {
					current.push(chars.next().expect("peeked escaped character"));
				}
			}
			State::DoubleQuote => {
				current.push(ch);
				if ch == '"' {
					state = State::Normal;
				} else if ch == '\\' && chars.peek().is_some() {
					current.push(chars.next().expect("peeked escaped character"));
				}
			}
			State::LineComment => {
				current.push(ch);
				if ch == '\n' {
					state = State::Normal;
				}
			}
			State::BlockComment => {
				current.push(ch);
				if ch == '*' && chars.peek() == Some(&'/') {
					current.push(chars.next().expect("peeked block comment terminator"));
					state = State::Normal;
				}
			}
			State::DollarQuote(ref tag) => {
				current.push(ch);
				if ch == '$' {
					let mut potential_close = String::from("$");
					let mut suffix = Vec::new();
					while let Some(&next_ch) = chars.peek() {
						if next_ch == '$' {
							potential_close.push(chars.next().expect("peeked dollar quote"));
							suffix.push('$');
							break;
						} else if potential_close.len() < tag.len()
							&& (next_ch.is_alphanumeric() || next_ch == '_')
						{
							potential_close
								.push(chars.next().expect("peeked dollar quote terminator"));
							suffix.push(next_ch);
						} else {
							break;
						}
					}
					current.extend(suffix);
					if potential_close == *tag {
						state = State::Normal;
					}
				}
			}
		}
	}

	let trimmed = current.trim();
	if !trimmed.is_empty() {
		statements.push(trimmed.to_string());
	}
	statements
}

fn append_sql(statements: &mut Vec<PlannedStatement>, sql: &str) {
	statements.extend(split_sql_statements(sql).into_iter().map(|statement| {
		if statement.trim_start().starts_with("--") && !statement.contains('\n') {
			PlannedStatement::Comment(statement.trim_start_matches('-').trim_start().to_string())
		} else {
			PlannedStatement::Sql(statement)
		}
	}));
}

#[cfg(feature = "sqlite")]
type SqlitePlanningMetadata = (
	Vec<super::ColumnDefinition>,
	Vec<(String, String)>,
	Vec<super::Constraint>,
	Vec<super::operations::SqliteRecreatedConstraint>,
	Vec<super::operations::SqliteRecreatedIndex>,
	Vec<String>,
	bool,
	bool,
);

#[cfg(feature = "sqlite")]
async fn sqlite_planning_metadata(
	editor: &mut SchemaEditor,
	table: &str,
	previous: Option<SqliteTableRecreation>,
	prior_operations: &[Operation],
) -> Result<SqlitePlanningMetadata> {
	let mut metadata = if let Some(previous) = previous {
		let mut raw_constraints = previous.raw_constraints;
		raw_constraints.extend(previous.raw_constraint_sqls.into_iter().map(|sql| {
			super::operations::SqliteRecreatedConstraint {
				name: None,
				physical_name: None,
				columns: Vec::new(),
				sql,
			}
		}));
		(
			previous.new_columns,
			previous.column_collations,
			previous.constraints,
			raw_constraints,
			previous.indexes,
			previous.triggers,
			previous.without_rowid,
			previous.strict,
		)
	} else {
		DatabaseMigrationExecutor::read_sqlite_table_via_editor(editor, table).await?
	};

	for operation in prior_operations {
		match operation {
			Operation::CreateTable {
				columns,
				constraints,
				without_rowid,
				..
			} => {
				metadata.0.clone_from(columns);
				metadata.1.clear();
				metadata.2.clone_from(constraints);
				metadata.3.clear();
				metadata.4.clear();
				metadata.5.clear();
				metadata.6 = without_rowid.unwrap_or(false);
				metadata.7 = false;
			}
			Operation::AddColumn { column, .. } => {
				metadata.0.push(column.clone());
			}
			_ => {}
		}
	}

	Ok(metadata)
}

#[cfg(feature = "sqlite")]
async fn sqlite_recreation_statements(
	editor: &mut SchemaEditor,
	operation: &Operation,
	previous: Option<SqliteTableRecreation>,
	prior_operations: &[Operation],
) -> Result<(Vec<String>, SqliteTableRecreation)> {
	operation.validate_for_dialect(&SqlDialect::Sqlite)?;
	let recreation = match operation {
		Operation::AddColumn { table, column, .. } => {
			let metadata =
				sqlite_planning_metadata(editor, table, previous, prior_operations).await?;
			SqliteTableRecreation::for_add_column(table, metadata.0, column.clone(), metadata.2)
				.with_column_collations(metadata.1)
				.with_raw_constraints(metadata.3)
				.with_indexes(metadata.4)
				.with_triggers(metadata.5)
				.with_without_rowid(metadata.6)
				.with_strict(metadata.7)
		}
		Operation::DropColumn { table, column, .. } => {
			let metadata =
				sqlite_planning_metadata(editor, table, previous, prior_operations).await?;
			SqliteTableRecreation::for_drop_column(table, metadata.0, column, metadata.2)
				.with_column_collations(metadata.1)
				.with_raw_constraints(metadata.3)
				.with_indexes(metadata.4)
				.with_triggers(metadata.5)
				.with_without_rowid(metadata.6)
				.with_strict(metadata.7)
				.without_raw_constraints_referencing(column)
				.without_indexes_referencing(column)
		}
		Operation::AlterColumn {
			table,
			column,
			new_definition,
			..
		} => {
			let metadata =
				sqlite_planning_metadata(editor, table, previous, prior_operations).await?;
			SqliteTableRecreation::for_alter_column(
				table,
				metadata.0,
				column,
				new_definition.clone(),
				metadata.2,
			)
			.with_column_collations(metadata.1)
			.with_raw_constraints(metadata.3)
			.with_indexes(metadata.4)
			.with_triggers(metadata.5)
			.with_without_rowid(metadata.6)
			.with_strict(metadata.7)
		}
		Operation::AddConstraint {
			table,
			constraint_sql,
		}
		| Operation::AddConstraintRepair {
			table,
			constraint_sql,
		} => {
			let metadata =
				sqlite_planning_metadata(editor, table, previous, prior_operations).await?;
			SqliteTableRecreation::for_add_constraint(
				table,
				metadata.0,
				metadata.2,
				constraint_sql.clone(),
			)
			.with_column_collations(metadata.1)
			.with_raw_constraints(metadata.3)
			.with_indexes(metadata.4)
			.with_triggers(metadata.5)
			.with_without_rowid(metadata.6)
			.with_strict(metadata.7)
		}
		Operation::AddConstraintDefinition { table, constraint } => {
			let metadata =
				sqlite_planning_metadata(editor, table, previous, prior_operations).await?;
			SqliteTableRecreation::for_add_constraint_definition(
				table,
				metadata.0,
				metadata.2,
				constraint.clone(),
			)
			.with_column_collations(metadata.1)
			.with_raw_constraints(metadata.3)
			.with_indexes(metadata.4)
			.with_triggers(metadata.5)
			.with_without_rowid(metadata.6)
			.with_strict(metadata.7)
		}
		Operation::DropConstraint {
			table,
			constraint_name,
		} => {
			let metadata =
				sqlite_planning_metadata(editor, table, previous, prior_operations).await?;
			SqliteTableRecreation::for_drop_constraint(
				table,
				metadata.0,
				metadata.2,
				constraint_name,
			)
			.with_column_collations(metadata.1)
			.with_raw_constraints(metadata.3)
			.without_raw_constraint_named(constraint_name)
			.with_indexes(metadata.4)
			.with_triggers(metadata.5)
			.with_without_rowid(metadata.6)
			.with_strict(metadata.7)
		}
		Operation::DropConstraintDefinition { table, constraint } => {
			let metadata =
				sqlite_planning_metadata(editor, table, previous, prior_operations).await?;
			SqliteTableRecreation::for_drop_constraint(
				table,
				metadata.0,
				metadata.2,
				constraint.name(),
			)
			.with_column_collations(metadata.1)
			.with_raw_constraints(metadata.3)
			.without_raw_constraint_named(constraint.name())
			.with_indexes(metadata.4)
			.with_triggers(metadata.5)
			.with_without_rowid(metadata.6)
			.with_strict(metadata.7)
		}
		_ => {
			return Err(MigrationError::InvalidMigration(format!(
				"SQLite recreation is not implemented for {:?}",
				std::mem::discriminant(operation)
			)));
		}
	};
	let statements = recreation.try_to_sql_statements()?;
	Ok((statements, recreation))
}

#[cfg(feature = "sqlite")]
fn sqlite_recreation_table(operation: &Operation) -> Option<&str> {
	match operation {
		Operation::AddColumn { table, .. }
		| Operation::DropColumn { table, .. }
		| Operation::AlterColumn { table, .. }
		| Operation::AddConstraint { table, .. }
		| Operation::AddConstraintRepair { table, .. }
		| Operation::AddConstraintDefinition { table, .. }
		| Operation::DropConstraint { table, .. }
		| Operation::DropConstraintDefinition { table, .. } => Some(table),
		_ => None,
	}
}

#[cfg(feature = "sqlite")]
fn sqlite_virtual_from_metadata(
	table: &str,
	metadata: SqlitePlanningMetadata,
) -> SqliteTableRecreation {
	let columns_to_copy = metadata
		.0
		.iter()
		.filter(|column| column.generated.is_none())
		.map(|column| column.name.clone())
		.collect();
	SqliteTableRecreation {
		table_name: table.to_string(),
		new_columns: metadata.0,
		columns_to_copy,
		constraints: metadata.2,
		raw_constraint_sqls: Vec::new(),
		raw_constraints: metadata.3,
		column_collations: metadata.1,
		indexes: metadata.4,
		triggers: metadata.5,
		without_rowid: metadata.6,
		strict: metadata.7,
	}
}

#[cfg(feature = "sqlite")]
async fn sqlite_load_virtual_schema(
	editor: &mut SchemaEditor,
	table: &str,
	schemas: &mut HashMap<String, Option<SqliteTableRecreation>>,
) -> Result<Option<SqliteTableRecreation>> {
	if let Some(schema) = schemas.get(table) {
		return Ok(schema.clone());
	}
	let schema = if editor.table_exists(table).await? {
		let metadata =
			DatabaseMigrationExecutor::read_sqlite_table_via_editor(editor, table).await?;
		Some(sqlite_virtual_from_metadata(table, metadata))
	} else {
		None
	};
	schemas.insert(table.to_string(), schema.clone());
	Ok(schema)
}

#[cfg(feature = "sqlite")]
fn sqlite_rename_typed_constraint_column(
	constraint: &mut super::Constraint,
	old_name: &str,
	new_name: &str,
) -> Result<()> {
	let rename = |column: &mut String| {
		if column == old_name {
			new_name.clone_into(column);
		}
	};
	match constraint {
		super::Constraint::PrimaryKey { columns, .. }
		| super::Constraint::ForeignKey { columns, .. }
		| super::Constraint::Unique { columns, .. } => columns.iter_mut().for_each(rename),
		super::Constraint::EnumDomain { column, .. }
		| super::Constraint::OneToOne { column, .. } => rename(column),
		super::Constraint::ManyToMany {
			source_column,
			target_column,
			..
		} => {
			rename(source_column);
			rename(target_column);
		}
		super::Constraint::Exclude { elements, .. } => {
			for (column, _) in elements {
				rename(column);
			}
		}
		super::Constraint::Check { expression, .. } => {
			if expression.contains(old_name) {
				return Err(MigrationError::InvalidMigration(format!(
					"cannot safely plan SQLite column rename for CHECK expression referencing '{old_name}'"
				)));
			}
		}
	}
	Ok(())
}

#[cfg(feature = "sqlite")]
async fn sqlite_advance_virtual_schema(
	editor: &mut SchemaEditor,
	operation: &Operation,
	schemas: &mut HashMap<String, Option<SqliteTableRecreation>>,
) -> Result<()> {
	match operation {
		Operation::CreateTable {
			name,
			columns,
			constraints,
			without_rowid,
			..
		} => {
			if sqlite_load_virtual_schema(editor, name, schemas)
				.await?
				.is_none()
			{
				let columns_to_copy = columns
					.iter()
					.filter(|column| column.generated.is_none())
					.map(|column| column.name.clone())
					.collect();
				schemas.insert(
					name.clone(),
					Some(SqliteTableRecreation {
						table_name: name.clone(),
						new_columns: columns.clone(),
						columns_to_copy,
						constraints: constraints.clone(),
						raw_constraint_sqls: Vec::new(),
						raw_constraints: Vec::new(),
						column_collations: Vec::new(),
						indexes: Vec::new(),
						triggers: Vec::new(),
						without_rowid: without_rowid.unwrap_or(false),
						strict: false,
					}),
				);
			}
		}
		Operation::DropTable { name } => {
			schemas.insert(name.clone(), None);
		}
		Operation::AddColumn { table, column, .. } => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite AddColumn for missing table '{table}'"
					))
				})?;
			schema.new_columns.push(column.clone());
			if column.generated.is_none() {
				schema.columns_to_copy.push(column.name.clone());
			}
			schemas.insert(table.clone(), Some(schema));
		}
		Operation::RenameTable { old_name, new_name } => {
			let mut schema = sqlite_load_virtual_schema(editor, old_name, schemas)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite RenameTable for missing table '{old_name}'"
					))
				})?;
			if !schema.raw_constraints.is_empty() || !schema.triggers.is_empty() {
				return Err(MigrationError::InvalidMigration(format!(
					"cannot safely plan SQLite table rename for '{old_name}' with raw constraints or triggers"
				)));
			}
			schema.table_name.clone_from(new_name);
			schemas.insert(old_name.clone(), None);
			schemas.insert(new_name.clone(), Some(schema));
		}
		Operation::RenameColumn {
			table,
			old_name,
			new_name,
		} => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite RenameColumn for missing table '{table}'"
					))
				})?;
			if !schema.raw_constraints.is_empty() || !schema.triggers.is_empty() {
				return Err(MigrationError::InvalidMigration(format!(
					"cannot safely plan SQLite column rename for '{table}.{old_name}' with raw constraints or triggers"
				)));
			}
			for column in &mut schema.new_columns {
				if column.name == *old_name {
					column.name.clone_from(new_name);
				}
			}
			for column in &mut schema.columns_to_copy {
				if column == old_name {
					column.clone_from(new_name);
				}
			}
			for (column, _) in &mut schema.column_collations {
				if column == old_name {
					column.clone_from(new_name);
				}
			}
			for constraint in &mut schema.constraints {
				sqlite_rename_typed_constraint_column(constraint, old_name, new_name)?;
			}
			for index in &mut schema.indexes {
				for column in &mut index.columns {
					if column == old_name {
						column.clone_from(new_name);
					}
				}
				if !index.columns.is_empty() {
					index.sql = None;
				}
			}
			schemas.insert(table.clone(), Some(schema));
		}
		Operation::CreateIndex {
			table,
			columns,
			unique,
			expressions,
			..
		} => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite CreateIndex for missing table '{table}'"
					))
				})?;
			let suffix = if expressions.as_ref().is_some_and(|value| !value.is_empty()) {
				"expr".to_string()
			} else {
				columns.join("_")
			};
			schema
				.indexes
				.push(super::operations::SqliteRecreatedIndex {
					name: format!("idx_{table}_{suffix}"),
					columns: columns.clone(),
					unique: *unique,
					sql: Some(operation.try_to_sql(&SqlDialect::Sqlite)?),
				});
			schemas.insert(table.clone(), Some(schema));
		}
		#[cfg(feature = "pgvector")]
		Operation::CreateNamedIndex {
			table,
			name,
			columns,
			unique,
			..
		} => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite CreateNamedIndex for missing table '{table}'"
					))
				})?;
			schema
				.indexes
				.push(super::operations::SqliteRecreatedIndex {
					name: name.clone(),
					columns: columns.clone(),
					unique: *unique,
					sql: Some(operation.try_to_sql(&SqlDialect::Sqlite)?),
				});
			schemas.insert(table.clone(), Some(schema));
		}
		Operation::DropIndex { table, columns } => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite DropIndex for missing table '{table}'"
					))
				})?;
			let name = format!("idx_{table}_{}", columns.join("_"));
			schema.indexes.retain(|index| index.name != name);
			schemas.insert(table.clone(), Some(schema));
		}
		#[cfg(feature = "pgvector")]
		Operation::DropNamedIndex { table, name, .. } => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite DropNamedIndex for missing table '{table}'"
					))
				})?;
			schema.indexes.retain(|index| index.name != *name);
			schemas.insert(table.clone(), Some(schema));
		}
		_ => {}
	}
	Ok(())
}

#[cfg(feature = "sqlite")]
fn sqlite_operation_is_opaque(operation: &Operation) -> bool {
	matches!(
		operation,
		Operation::RunSQL { .. }
			| Operation::BulkLoad { .. }
			| Operation::CreateSchema { .. }
			| Operation::DropSchema { .. }
			| Operation::CreateExtension { .. }
	)
}

/// Builds the SQL plan consumed by both migration execution and SQL inspection.
pub async fn plan_migration_sql(
	connection: &DatabaseConnection,
	migration: &Migration,
	state: &ProjectState,
	direction: MigrationDirection,
) -> Result<MigrationSqlPlan> {
	#[cfg(feature = "sqlite")]
	if migration_requires_sqlite_recreation(connection, migration, direction) {
		let mut editor = SchemaEditor::new_for_migration(
			connection.clone(),
			false,
			connection.database_type(),
			true,
		)
		.await?;
		let plan = plan_migration_sql_with_irreversible_policy(
			connection,
			migration,
			state,
			direction,
			true,
			Some(&mut editor),
		)
		.await?;
		editor.finish().await?;
		return Ok(plan);
	}
	plan_migration_sql_with_irreversible_policy(connection, migration, state, direction, true, None)
		.await
}

pub(crate) async fn plan_migration_sql_for_execution(
	connection: &DatabaseConnection,
	migration: &Migration,
	state: &ProjectState,
	direction: MigrationDirection,
	editor: &mut SchemaEditor,
) -> Result<MigrationSqlPlan> {
	plan_migration_sql_with_irreversible_policy(
		connection,
		migration,
		state,
		direction,
		false,
		Some(editor),
	)
	.await
}

pub(crate) fn migration_requires_sqlite_recreation(
	connection: &DatabaseConnection,
	migration: &Migration,
	direction: MigrationDirection,
) -> bool {
	matches!(migration_sql_dialect(connection), SqlDialect::Sqlite)
		&& migration
			.operations
			.iter()
			.any(|operation| match direction {
				MigrationDirection::Forward => operation.requires_sqlite_recreation(),
				MigrationDirection::Backward => operation.reverse_requires_sqlite_recreation(),
			})
}

async fn plan_migration_sql_with_irreversible_policy(
	connection: &DatabaseConnection,
	migration: &Migration,
	state: &ProjectState,
	direction: MigrationDirection,
	strict_irreversible: bool,
	#[cfg_attr(not(feature = "sqlite"), allow(unused_variables))] mut sqlite_editor: Option<
		&mut SchemaEditor,
	>,
) -> Result<MigrationSqlPlan> {
	if migration.state_only {
		return Ok(MigrationSqlPlan {
			atomic: migration.atomic,
			statements: Vec::new(),
			planned_operations: Vec::new(),
			sqlite_recreation_groups: Vec::new(),
			direction,
		});
	}

	let dialect = migration_sql_dialect(connection);
	let mut statements = Vec::new();
	let mut planned_operations = Vec::new();
	let mut sqlite_recreation_groups = Vec::new();
	let mut next_recreation_group = 0;
	#[cfg(feature = "sqlite")]
	let needs_sqlite_editor = migration_requires_sqlite_recreation(connection, migration, direction);
	#[cfg(feature = "sqlite")]
	let mut sqlite_virtual_schemas = HashMap::<String, Option<SqliteTableRecreation>>::new();
	#[cfg(feature = "sqlite")]
	let mut sqlite_opaque_operation_seen = false;

	let operations: Vec<&Operation> = match direction {
		MigrationDirection::Forward => migration.operations.iter().collect(),
		MigrationDirection::Backward => migration.operations.iter().rev().collect(),
	};

	for operation in operations {
		let first_statement = statements.len();
		operation.validate_for_dialect(&dialect)?;
		let planned_operation = match direction {
			MigrationDirection::Forward => Some(operation.clone()),
			MigrationDirection::Backward => operation.to_reverse_operation(state)?,
		};
		#[cfg(feature = "sqlite")]
		if matches!(dialect, SqlDialect::Sqlite)
			&& planned_operation
				.as_ref()
				.is_some_and(sqlite_operation_is_opaque)
		{
			sqlite_opaque_operation_seen = true;
		}

		#[cfg(feature = "sqlite")]
		let requires_recreation = matches!(dialect, SqlDialect::Sqlite)
			&& match direction {
				MigrationDirection::Forward => operation.requires_sqlite_recreation(),
				MigrationDirection::Backward => operation.reverse_requires_sqlite_recreation(),
			};
		#[cfg(feature = "sqlite")]
		if requires_recreation {
			if sqlite_opaque_operation_seen {
				return Err(MigrationError::InvalidMigration(format!(
					"cannot safely plan SQLite recreation after opaque RunSQL in {}",
					migration.id()
				)));
			}
			let reverse_missing =
				matches!(direction, MigrationDirection::Backward) && planned_operation.is_none();
			if reverse_missing {
				if strict_irreversible {
					return Err(MigrationError::IrreversibleError(format!(
						"{} contains an irreversible operation",
						migration.id()
					)));
				}
				statements.push(PlannedStatement::Comment(format!(
					"No reverse SQL available for an operation in {}",
					migration.id()
				)));
				planned_operations.push(None);
				sqlite_recreation_groups.push(None);
				continue;
			}
			let operation = planned_operation
				.as_ref()
				.expect("checked reverse operation");
			let editor = sqlite_editor.as_mut().expect("SQLite planner editor");
			let (recreation_statements, recreation) = sqlite_recreation_statements(
				editor,
				operation,
				sqlite_recreation_table(operation)
					.and_then(|table| sqlite_virtual_schemas.remove(table))
					.flatten(),
				&[],
			)
			.await?;
			statements.extend(recreation_statements.into_iter().map(PlannedStatement::Sql));
			sqlite_virtual_schemas.insert(recreation.table_name.clone(), Some(recreation));
			planned_operations
				.extend((first_statement..statements.len()).map(|_| planned_operation.clone()));
			sqlite_recreation_groups
				.extend((first_statement..statements.len()).map(|_| Some(next_recreation_group)));
			next_recreation_group += 1;
			continue;
		}

		match direction {
			MigrationDirection::Forward => match operation.to_planned_forward_output(&dialect)? {
				PlannedOperationOutput::Sql(sql) => append_sql(&mut statements, &sql),
				PlannedOperationOutput::Comment(comment) => {
					statements.push(PlannedStatement::Comment(comment));
				}
			},
			MigrationDirection::Backward => {
				let reverse = operation.to_reverse_sql(&dialect, state)?;
				let Some(reverse) = reverse else {
					let operation_name = match operation {
						Operation::RunSQL { .. } => "RunSQL",
						Operation::RunRust { .. } => "RunRust",
						_ => "operation",
					};
					if strict_irreversible {
						return Err(MigrationError::IrreversibleError(format!(
							"{} contains an irreversible {} operation",
							migration.id(),
							operation_name
						)));
					}
					statements.push(PlannedStatement::Comment(format!(
						"No reverse SQL available for {operation_name} in {}",
						migration.id()
					)));
					planned_operations.push(planned_operation.clone());
					sqlite_recreation_groups.push(None);
					continue;
				};
				if matches!(operation, Operation::RunRust { .. }) {
					for comment in reverse {
						statements.push(PlannedStatement::Comment(
							comment.trim_start_matches('-').trim_start().to_string(),
						));
					}
				} else {
					for sql in reverse {
						append_sql(&mut statements, &sql);
					}
				}
			}
		}
		planned_operations
			.extend((first_statement..statements.len()).map(|_| planned_operation.clone()));
		sqlite_recreation_groups.extend((first_statement..statements.len()).map(|_| None));
		#[cfg(feature = "sqlite")]
		if needs_sqlite_editor && let Some(operation) = planned_operation.as_ref() {
			let editor = sqlite_editor.as_mut().expect("SQLite planner editor");
			sqlite_advance_virtual_schema(editor, operation, &mut sqlite_virtual_schemas).await?;
		}
	}

	Ok(MigrationSqlPlan {
		atomic: migration.atomic,
		statements,
		planned_operations,
		sqlite_recreation_groups,
		direction,
	})
}
