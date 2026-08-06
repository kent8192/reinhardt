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

impl MigrationSqlPlan {
	/// Render a complete, uncolored SQL script for a database dialect.
	///
	/// Transaction wrappers are emitted only when the migration is atomic and
	/// the selected backend supports transactional DDL.
	pub fn render(&self, dialect: SqlDialect) -> String {
		let transactional_ddl = !matches!(dialect, SqlDialect::Mysql);
		let has_concurrent_index =
			matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb)
				&& self
					.planned_operations
					.iter()
					.flatten()
					.any(Operation::creates_index_concurrently);
		let wrapped = self.atomic && transactional_ddl && !has_concurrent_index;
		let sqlite_recreation = matches!(dialect, SqlDialect::Sqlite)
			&& self.sqlite_recreation_groups.iter().any(Option::is_some);
		let mut rendered = String::new();

		if sqlite_recreation {
			rendered.push_str("PRAGMA foreign_keys = OFF;\n");
		}
		if wrapped {
			rendered.push_str("BEGIN;\n");
		}
		for statement in &self.statements {
			match statement {
				PlannedStatement::Sql(sql) => {
					rendered.push_str(&render_sql_statement(sql, dialect));
				}
				PlannedStatement::Comment(comment) => {
					rendered.push_str("-- ");
					rendered.push_str(comment.trim());
					rendered.push('\n');
				}
			}
		}
		if wrapped {
			rendered.push_str("COMMIT;\n");
		}
		if sqlite_recreation {
			rendered.push_str("PRAGMA foreign_key_check;\n");
			rendered.push_str("PRAGMA foreign_keys = ON;\n");
		}

		rendered
	}
}

fn render_sql_statement(sql: &str, dialect: SqlDialect) -> String {
	let sql = sql.trim().trim_end_matches(';').trim_end();
	if let Some(comment_start) = trailing_line_comment_start(sql, dialect) {
		let (statement, comment) = sql.split_at(comment_start);
		return format!("{}; {}\n", statement.trim_end(), comment.trim_start());
	}
	format!("{sql};\n")
}

fn quote_uses_backslash_escapes(dialect: SqlDialect, delimiter: char, prefix: &str) -> bool {
	if matches!(dialect, SqlDialect::Mysql) {
		return true;
	}

	if !matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) || delimiter != '\'' {
		return false;
	}

	let Some((escape_prefix_index, escape_prefix)) = prefix.char_indices().last() else {
		return false;
	};
	if !matches!(escape_prefix, 'E' | 'e') {
		return false;
	}
	prefix[..escape_prefix_index]
		.chars()
		.last()
		.is_none_or(|character| !(character.is_ascii_alphanumeric() || character == '_'))
}

fn trailing_line_comment_start(sql: &str, dialect: SqlDialect) -> Option<usize> {
	let mut quote = None;
	let mut block_comment_depth = 0usize;
	let mut line_comment_start = None;
	let mut chars = sql.char_indices().peekable();
	while let Some((index, character)) = chars.next() {
		if line_comment_start.is_some() {
			if character == '\n' {
				line_comment_start = None;
			}
			continue;
		}
		if block_comment_depth > 0 {
			if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb)
				&& character == '/'
				&& chars.peek().is_some_and(|(_, next)| *next == '*')
			{
				chars.next();
				block_comment_depth += 1;
			} else if character == '*' && chars.peek().is_some_and(|(_, next)| *next == '/') {
				chars.next();
				block_comment_depth -= 1;
			}
			continue;
		}
		if let Some((delimiter, backslash_escapes)) = quote {
			if backslash_escapes && character == '\\' && chars.peek().is_some() {
				chars.next();
				continue;
			}
			if character == delimiter {
				if chars.peek().is_some_and(|(_, next)| *next == delimiter) {
					chars.next();
				} else {
					quote = None;
				}
			}
			continue;
		}
		match character {
			'\'' | '"' | '`' => {
				quote = Some((
					character,
					quote_uses_backslash_escapes(dialect, character, &sql[..index]),
				));
			}
			'/' if chars.peek().is_some_and(|(_, next)| *next == '*') => {
				chars.next();
				block_comment_depth = 1;
			}
			'-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
				let mut lookahead = chars.clone();
				lookahead.next();
				let mysql_comment = !matches!(dialect, SqlDialect::Mysql)
					|| lookahead
						.next()
						.is_none_or(|(_, next)| next.is_ascii_whitespace() || next.is_control());
				if mysql_comment {
					chars.next();
					line_comment_start = Some(index);
				}
			}
			'#' if matches!(dialect, SqlDialect::Mysql) => line_comment_start = Some(index),
			_ => {}
		}
	}
	line_comment_start
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
// The executor reuses the PostgreSQL-default splitter for raw SQL execution.
#[allow(dead_code)]
pub(crate) fn split_sql_statements(sql: &str) -> Vec<String> {
	split_sql_statements_for_dialect(sql, SqlDialect::Postgres)
}

fn split_sql_statements_for_dialect(sql: &str, dialect: SqlDialect) -> Vec<String> {
	let mut statements = Vec::new();
	let mut current = String::new();
	let mut chars = sql.chars().peekable();

	#[derive(Debug, PartialEq)]
	enum State {
		Normal,
		SingleQuote { backslash_escapes: bool },
		DoubleQuote { backslash_escapes: bool },
		BacktickQuote,
		LineComment,
		BlockComment { depth: usize },
		DollarQuote(String),
	}

	let mut state = State::Normal;
	let mut sqlite_token = String::new();
	let mut sqlite_create_seen = false;
	let mut sqlite_trigger_seen = false;
	let mut sqlite_trigger_depth = 0usize;

	while let Some(ch) = chars.next() {
		match state {
			State::Normal => {
				if matches!(dialect, SqlDialect::Sqlite)
					&& (ch.is_ascii_alphanumeric() || ch == '_')
				{
					current.push(ch);
					sqlite_token.push(ch.to_ascii_uppercase());
					continue;
				}
				if matches!(dialect, SqlDialect::Sqlite) && !sqlite_token.is_empty() {
					match sqlite_token.as_str() {
						"CREATE" => sqlite_create_seen = true,
						"TRIGGER" if sqlite_create_seen => sqlite_trigger_seen = true,
						"BEGIN" | "CASE" if sqlite_trigger_seen => {
							sqlite_trigger_depth += 1;
						}
						"END" if sqlite_trigger_seen => {
							sqlite_trigger_depth = sqlite_trigger_depth.saturating_sub(1);
						}
						_ => {}
					}
					sqlite_token.clear();
				}
				if ch == '\'' {
					let backslash_escapes = quote_uses_backslash_escapes(dialect, ch, &current);
					current.push(ch);
					state = State::SingleQuote { backslash_escapes };
				} else if ch == '"' {
					let backslash_escapes = quote_uses_backslash_escapes(dialect, ch, &current);
					current.push(ch);
					state = State::DoubleQuote { backslash_escapes };
				} else if ch == '`' {
					current.push(ch);
					state = State::BacktickQuote;
				} else if ch == '-' && chars.peek() == Some(&'-') {
					let mut lookahead = chars.clone();
					lookahead.next();
					let mysql_comment = !matches!(dialect, SqlDialect::Mysql)
						|| lookahead
							.next()
							.is_none_or(|next| next.is_ascii_whitespace() || next.is_control());
					current.push(ch);
					if mysql_comment {
						current.push(chars.next().expect("peeked SQL comment marker"));
						state = State::LineComment;
					}
				} else if ch == '#' && matches!(dialect, SqlDialect::Mysql) {
					current.push(ch);
					state = State::LineComment;
				} else if ch == '/' && chars.peek() == Some(&'*') {
					current.push(ch);
					current.push(chars.next().expect("peeked SQL comment marker"));
					state = State::BlockComment { depth: 1 };
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
					if sqlite_trigger_seen && sqlite_trigger_depth > 0 {
						current.push(ch);
						continue;
					}
					let trimmed = current.trim();
					if !trimmed.is_empty() {
						statements.push(trimmed.to_string());
					}
					current.clear();
					sqlite_token.clear();
					sqlite_create_seen = false;
					sqlite_trigger_seen = false;
					sqlite_trigger_depth = 0;
				} else {
					current.push(ch);
				}
			}
			State::SingleQuote { backslash_escapes } => {
				current.push(ch);
				if ch == '\'' {
					if chars.peek() == Some(&'\'') {
						current.push(chars.next().expect("peeked escaped quote"));
					} else {
						state = State::Normal;
					}
				} else if backslash_escapes && ch == '\\' && chars.peek().is_some() {
					current.push(chars.next().expect("peeked escaped character"));
				}
			}
			State::DoubleQuote { backslash_escapes } => {
				current.push(ch);
				if ch == '"' {
					state = State::Normal;
				} else if backslash_escapes && ch == '\\' && chars.peek().is_some() {
					current.push(chars.next().expect("peeked escaped character"));
				}
			}
			State::BacktickQuote => {
				current.push(ch);
				if ch == '`' {
					if chars.peek() == Some(&'`') {
						current.push(chars.next().expect("peeked escaped backtick"));
					} else {
						state = State::Normal;
					}
				}
			}
			State::LineComment => {
				current.push(ch);
				if ch == '\n' {
					state = State::Normal;
				}
			}
			State::BlockComment { mut depth } => {
				current.push(ch);
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb)
					&& ch == '/' && chars.peek() == Some(&'*')
				{
					current.push(chars.next().expect("peeked nested block comment marker"));
					depth += 1;
					state = State::BlockComment { depth };
				} else if ch == '*' && chars.peek() == Some(&'/') {
					current.push(chars.next().expect("peeked block comment terminator"));
					depth -= 1;
					state = if depth == 0 {
						State::Normal
					} else {
						State::BlockComment { depth }
					};
				} else {
					state = State::BlockComment { depth };
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

fn append_sql(statements: &mut Vec<PlannedStatement>, sql: &str, dialect: SqlDialect) {
	statements.extend(
		split_sql_statements_for_dialect(sql, dialect)
			.into_iter()
			.map(|statement| {
				if statement.trim_start().starts_with("--") && !statement.contains('\n') {
					PlannedStatement::Comment(
						statement.trim_start_matches('-').trim_start().to_string(),
					)
				} else {
					PlannedStatement::Sql(statement)
				}
			}),
	);
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
fn sqlite_virtual_from_project_state(
	state: &ProjectState,
	table: &str,
) -> Option<SqliteTableRecreation> {
	let model = state
		.models
		.values()
		.find(|model| model.table_name == table)?;
	let new_columns = model
		.fields
		.iter()
		.map(|(name, field)| super::ColumnDefinition::from_field_state(name, field))
		.collect::<Vec<_>>();
	let columns_to_copy = new_columns
		.iter()
		.filter(|column| column.generated.is_none())
		.map(|column| column.name.clone())
		.collect();
	let constraints = model
		.constraints
		.iter()
		.map(super::ConstraintDefinition::to_constraint)
		.collect();
	let indexes = model
		.indexes
		.iter()
		.map(|index| super::operations::SqliteRecreatedIndex {
			name: index.name.clone(),
			columns: index.fields.clone(),
			unique: index.unique,
			sql: None,
		})
		.collect();
	Some(SqliteTableRecreation {
		table_name: table.to_string(),
		new_columns,
		columns_to_copy,
		constraints,
		raw_constraint_sqls: Vec::new(),
		raw_constraints: Vec::new(),
		column_collations: Vec::new(),
		indexes,
		triggers: Vec::new(),
		without_rowid: model
			.options
			.get("without_rowid")
			.is_some_and(|value| value == "true"),
		strict: false,
	})
}

#[cfg(feature = "sqlite")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum SqliteRenameTransform {
	Table {
		old_name: String,
		new_name: String,
	},
	Column {
		table: String,
		old_name: String,
		new_name: String,
	},
}

#[cfg(feature = "sqlite")]
fn sqlite_sql_mentions_identifier(sql: &str, identifier: &str) -> bool {
	let mut chars = sql.char_indices().peekable();
	while let Some((start, ch)) = chars.next() {
		match ch {
			'\'' => {
				let mut quoted_token = String::new();
				while let Some((_, quoted)) = chars.next() {
					if quoted == '\'' {
						if chars.peek().is_some_and(|(_, escaped)| *escaped == '\'') {
							chars.next();
							quoted_token.push('\'');
						} else {
							break;
						}
					} else {
						quoted_token.push(quoted);
					}
				}
				if quoted_token.eq_ignore_ascii_case(identifier) {
					return true;
				}
			}
			'-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
				chars.next();
				for (_, comment) in chars.by_ref() {
					if comment == '\n' {
						break;
					}
				}
			}
			'/' if chars.peek().is_some_and(|(_, next)| *next == '*') => {
				chars.next();
				while let Some((_, comment)) = chars.next() {
					if comment == '*' && chars.peek().is_some_and(|(_, next)| *next == '/') {
						chars.next();
						break;
					}
				}
			}
			'"' | '`' => {
				let quote = ch;
				let mut quoted_identifier = String::new();
				while let Some((_, quoted)) = chars.next() {
					if quoted == quote {
						if chars.peek().is_some_and(|(_, escaped)| *escaped == quote) {
							chars.next();
							quoted_identifier.push(quote);
						} else {
							break;
						}
					} else {
						quoted_identifier.push(quoted);
					}
				}
				if quoted_identifier.eq_ignore_ascii_case(identifier) {
					return true;
				}
			}
			'[' => {
				let mut quoted_identifier = String::new();
				while let Some((_, quoted)) = chars.next() {
					if quoted == ']' {
						if chars.peek().is_some_and(|(_, escaped)| *escaped == ']') {
							chars.next();
							quoted_identifier.push(']');
						} else {
							break;
						}
					} else {
						quoted_identifier.push(quoted);
					}
				}
				if quoted_identifier.eq_ignore_ascii_case(identifier) {
					return true;
				}
			}
			ch if ch == '_' || ch.is_alphabetic() || !ch.is_ascii() => {
				let mut end = start + ch.len_utf8();
				while let Some(&(_, next)) = chars.peek() {
					if next == '_' || next == '$' || next.is_alphanumeric() || !next.is_ascii() {
						end += next.len_utf8();
						chars.next();
					} else {
						break;
					}
				}
				if sql[start..end].eq_ignore_ascii_case(identifier) {
					return true;
				}
			}
			_ => {}
		}
	}
	false
}

#[cfg(feature = "sqlite")]
fn sqlite_raw_sql_references_transform(sql: &str, transform: &SqliteRenameTransform) -> bool {
	match transform {
		SqliteRenameTransform::Table { old_name, .. } => {
			sqlite_sql_mentions_identifier(sql, old_name)
		}
		SqliteRenameTransform::Column {
			table, old_name, ..
		} => {
			sqlite_sql_mentions_identifier(sql, table)
				&& sqlite_sql_mentions_identifier(sql, old_name)
		}
	}
}

#[cfg(feature = "sqlite")]
fn sqlite_reject_raw_metadata_for_transform(
	schema: &SqliteTableRecreation,
	transform: &SqliteRenameTransform,
) -> Result<()> {
	let renamed_identifier = match transform {
		SqliteRenameTransform::Table { old_name, .. } => old_name.clone(),
		SqliteRenameTransform::Column {
			table, old_name, ..
		} => format!("{table}.{old_name}"),
	};
	let raw_constraint_references_transform = schema
		.raw_constraint_sqls
		.iter()
		.any(|sql| sqlite_raw_sql_references_transform(sql, transform))
		|| schema
			.raw_constraints
			.iter()
			.any(|constraint| sqlite_raw_sql_references_transform(&constraint.sql, transform));
	if raw_constraint_references_transform {
		return Err(MigrationError::InvalidMigration(format!(
			"raw constraint references renamed SQLite identifier '{renamed_identifier}'"
		)));
	}
	if schema
		.triggers
		.iter()
		.any(|trigger| sqlite_raw_sql_references_transform(trigger, transform))
	{
		return Err(MigrationError::InvalidMigration(format!(
			"raw trigger references renamed SQLite identifier '{renamed_identifier}'"
		)));
	}
	Ok(())
}

#[cfg(feature = "sqlite")]
fn sqlite_apply_rename_transform(
	schema: &mut SqliteTableRecreation,
	transform: &SqliteRenameTransform,
) -> Result<()> {
	sqlite_reject_raw_metadata_for_transform(schema, transform)?;
	for constraint in &mut schema.constraints {
		match (constraint, transform) {
			(
				super::Constraint::ForeignKey {
					referenced_table, ..
				}
				| super::Constraint::OneToOne {
					referenced_table, ..
				},
				SqliteRenameTransform::Table { old_name, new_name },
			) if referenced_table == old_name => referenced_table.clone_from(new_name),
			(
				super::Constraint::ManyToMany {
					through_table,
					target_table,
					..
				},
				SqliteRenameTransform::Table { old_name, new_name },
			) => {
				if through_table == old_name {
					through_table.clone_from(new_name);
				}
				if target_table == old_name {
					target_table.clone_from(new_name);
				}
			}
			(
				super::Constraint::ForeignKey {
					referenced_table,
					referenced_columns,
					..
				},
				SqliteRenameTransform::Column {
					table,
					old_name,
					new_name,
				},
			) if referenced_table == table => {
				for column in referenced_columns {
					if column == old_name {
						column.clone_from(new_name);
					}
				}
			}
			(
				super::Constraint::OneToOne {
					referenced_table,
					referenced_column,
					..
				},
				SqliteRenameTransform::Column {
					table,
					old_name,
					new_name,
				},
			) if referenced_table == table && referenced_column == old_name => {
				referenced_column.clone_from(new_name);
			}
			(
				super::Constraint::ManyToMany {
					target_table,
					target_column,
					..
				},
				SqliteRenameTransform::Column {
					table,
					old_name,
					new_name,
				},
			) if target_table == table && target_column == old_name => {
				target_column.clone_from(new_name);
			}
			_ => {}
		}
	}
	Ok(())
}

#[cfg(feature = "sqlite")]
fn sqlite_record_rename_transform(
	schemas: &mut HashMap<String, Option<SqliteTableRecreation>>,
	transforms: &mut Vec<SqliteRenameTransform>,
	transform: SqliteRenameTransform,
) -> Result<()> {
	for schema in schemas.values_mut().filter_map(Option::as_mut) {
		sqlite_apply_rename_transform(schema, &transform)?;
	}
	transforms.push(transform);
	Ok(())
}

#[cfg(feature = "sqlite")]
async fn sqlite_load_virtual_schema(
	editor: &mut SchemaEditor,
	table: &str,
	schemas: &mut HashMap<String, Option<SqliteTableRecreation>>,
	transforms: &[SqliteRenameTransform],
) -> Result<Option<SqliteTableRecreation>> {
	if let Some(schema) = schemas.get(table) {
		return Ok(schema.clone());
	}
	let schema = if editor.table_exists(table).await? {
		let metadata =
			DatabaseMigrationExecutor::read_sqlite_table_via_editor(editor, table).await?;
		let mut schema = sqlite_virtual_from_metadata(table, metadata);
		for transform in transforms {
			sqlite_apply_rename_transform(&mut schema, transform)?;
		}
		Some(schema)
	} else {
		None
	};
	schemas.insert(table.to_string(), schema.clone());
	Ok(schema)
}

#[cfg(feature = "sqlite")]
fn sqlite_rename_typed_constraint_column(
	constraint: &mut super::Constraint,
	table: &str,
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
		| super::Constraint::Unique { columns, .. } => columns.iter_mut().for_each(rename),
		super::Constraint::ForeignKey {
			columns,
			referenced_table,
			referenced_columns,
			..
		} => {
			columns.iter_mut().for_each(rename);
			if referenced_table == table {
				referenced_columns.iter_mut().for_each(rename);
			}
		}
		super::Constraint::EnumDomain { column, .. } => rename(column),
		super::Constraint::OneToOne {
			column,
			referenced_table,
			referenced_column,
			..
		} => {
			rename(column);
			if referenced_table == table {
				rename(referenced_column);
			}
		}
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
fn sqlite_quote_identifier(identifier: &str) -> String {
	format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(feature = "sqlite")]
fn sqlite_index_requires_raw_sql(
	index: &super::operations::SqliteRecreatedIndex,
	table: &str,
) -> bool {
	let Some(sql) = index.sql.as_deref() else {
		return false;
	};
	if index.columns.is_empty() {
		return true;
	}
	let unique = if index.unique { "UNIQUE " } else { "" };
	let columns = index
		.columns
		.iter()
		.map(|column| sqlite_quote_identifier(column))
		.collect::<Vec<_>>()
		.join(", ");
	let canonical = format!(
		"CREATE {unique}INDEX {} ON {} ({columns})",
		sqlite_quote_identifier(&index.name),
		sqlite_quote_identifier(table),
	);
	sql.trim().trim_end_matches(';') != canonical
}

#[cfg(feature = "sqlite")]
fn sqlite_typed_index_requires_raw_sql(
	index_type: Option<super::IndexType>,
	where_clause: Option<&str>,
	concurrently: bool,
	expressions: Option<&[String]>,
	operator_class: Option<&str>,
) -> bool {
	index_type.is_some_and(|index_type| !matches!(index_type, super::IndexType::BTree))
		|| where_clause.is_some()
		|| concurrently
		|| expressions.is_some_and(|expressions| !expressions.is_empty())
		|| operator_class.is_some()
}

#[cfg(feature = "sqlite")]
fn sqlite_reject_unrewritable_rename_metadata(
	schema: &SqliteTableRecreation,
	target: &str,
) -> Result<()> {
	if !schema.raw_constraints.is_empty() || !schema.triggers.is_empty() {
		return Err(MigrationError::InvalidMigration(format!(
			"cannot safely plan SQLite rename for '{target}' with raw constraints or triggers"
		)));
	}
	if schema
		.new_columns
		.iter()
		.any(|column| column.generated.is_some())
	{
		return Err(MigrationError::InvalidMigration(format!(
			"cannot safely plan SQLite rename for '{target}' with generated column expressions"
		)));
	}
	if schema
		.indexes
		.iter()
		.any(|index| sqlite_index_requires_raw_sql(index, &schema.table_name))
	{
		return Err(MigrationError::InvalidMigration(format!(
			"cannot safely plan SQLite rename for '{target}' with index metadata requiring raw SQL (partial or expression index)"
		)));
	}
	Ok(())
}

#[cfg(feature = "sqlite")]
async fn sqlite_advance_virtual_schema(
	editor: &mut SchemaEditor,
	operation: &Operation,
	schemas: &mut HashMap<String, Option<SqliteTableRecreation>>,
	transforms: &mut Vec<SqliteRenameTransform>,
) -> Result<()> {
	match operation {
		Operation::CreateTable {
			name,
			columns,
			constraints,
			without_rowid,
			..
		} => {
			if sqlite_load_virtual_schema(editor, name, schemas, transforms)
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
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas, transforms)
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
			let mut schema = sqlite_load_virtual_schema(editor, old_name, schemas, transforms)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite RenameTable for missing table '{old_name}'"
					))
				})?;
			sqlite_reject_unrewritable_rename_metadata(&schema, old_name)?;
			let transform = SqliteRenameTransform::Table {
				old_name: old_name.clone(),
				new_name: new_name.clone(),
			};
			sqlite_apply_rename_transform(&mut schema, &transform)?;
			sqlite_record_rename_transform(schemas, transforms, transform)?;
			for index in &mut schema.indexes {
				index.sql = None;
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
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas, transforms)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite RenameColumn for missing table '{table}'"
					))
				})?;
			sqlite_reject_unrewritable_rename_metadata(&schema, &format!("{table}.{old_name}"))?;
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
				sqlite_rename_typed_constraint_column(constraint, table, old_name, new_name)?;
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
			sqlite_record_rename_transform(
				schemas,
				transforms,
				SqliteRenameTransform::Column {
					table: table.clone(),
					old_name: old_name.clone(),
					new_name: new_name.clone(),
				},
			)?;
			schemas.insert(table.clone(), Some(schema));
		}
		Operation::AddDiscriminatorColumn {
			table,
			column_name,
			default_value,
		} => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas, transforms)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite AddDiscriminatorColumn for missing table '{table}'"
					))
				})?;
			let mut column =
				super::ColumnDefinition::new(column_name, super::FieldType::VarChar(50));
			column.default = Some(format!("'{default_value}'"));
			schema.new_columns.push(column);
			schema.columns_to_copy.push(column_name.clone());
			schemas.insert(table.clone(), Some(schema));
		}
		Operation::CreateInheritedTable {
			name,
			columns,
			base_table,
			join_column,
		} => {
			if sqlite_load_virtual_schema(editor, name, schemas, transforms)
				.await?
				.is_none()
			{
				let mut inherited_columns = Vec::with_capacity(columns.len() + 1);
				inherited_columns.push(super::ColumnDefinition::new(
					join_column,
					super::FieldType::Integer,
				));
				inherited_columns.extend(columns.iter().cloned());
				let columns_to_copy = inherited_columns
					.iter()
					.filter(|column| column.generated.is_none())
					.map(|column| column.name.clone())
					.collect();
				schemas.insert(
					name.clone(),
					Some(SqliteTableRecreation {
						table_name: name.clone(),
						new_columns: inherited_columns,
						columns_to_copy,
						constraints: vec![super::Constraint::ForeignKey {
							name: format!("{name}_{join_column}_fk"),
							columns: vec![join_column.clone()],
							referenced_table: base_table.clone(),
							referenced_columns: vec!["id".to_string()],
							on_delete: super::ForeignKeyAction::NoAction,
							on_update: super::ForeignKeyAction::NoAction,
							deferrable: None,
						}],
						raw_constraint_sqls: Vec::new(),
						raw_constraints: Vec::new(),
						column_collations: Vec::new(),
						indexes: Vec::new(),
						triggers: Vec::new(),
						without_rowid: false,
						strict: false,
					}),
				);
			}
		}
		Operation::MoveModel {
			rename_table,
			old_table_name,
			new_table_name,
			..
		} if *rename_table => {
			let (Some(old_name), Some(new_name)) = (old_table_name, new_table_name) else {
				return Err(MigrationError::InvalidMigration(
					"SQLite MoveModel table rename requires both table names".to_string(),
				));
			};
			let rename = Operation::RenameTable {
				old_name: old_name.clone(),
				new_name: new_name.clone(),
			};
			Box::pin(sqlite_advance_virtual_schema(
				editor, &rename, schemas, transforms,
			))
			.await?;
		}
		Operation::CreateIndex {
			table,
			columns,
			unique,
			index_type,
			where_clause,
			concurrently,
			expressions,
			operator_class,
			..
		} => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas, transforms)
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
			let sql = sqlite_typed_index_requires_raw_sql(
				*index_type,
				where_clause.as_deref(),
				*concurrently,
				expressions.as_deref(),
				operator_class.as_deref(),
			)
			.then(|| operation.try_to_sql(&SqlDialect::Sqlite))
			.transpose()?;
			schema
				.indexes
				.push(super::operations::SqliteRecreatedIndex {
					name: format!("idx_{table}_{suffix}"),
					columns: columns.clone(),
					unique: *unique,
					sql,
				});
			schemas.insert(table.clone(), Some(schema));
		}
		Operation::CreateIndexRepair {
			table,
			name,
			columns,
			unique,
			index_type,
			where_clause,
			concurrently,
			expressions,
			mysql_options,
			operator_class,
		}
		| Operation::RestoreIndexOnRollback {
			table,
			name,
			columns,
			unique,
			index_type,
			where_clause,
			concurrently,
			expressions,
			mysql_options,
			operator_class,
		} => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas, transforms)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite generated index for missing table '{table}'"
					))
				})?;
			let suffix = if columns.is_empty() {
				"expr".to_string()
			} else {
				columns.join("_")
			};
			let executable = Operation::CreateIndexRepair {
				table: table.clone(),
				name: name.clone(),
				columns: columns.clone(),
				unique: *unique,
				index_type: *index_type,
				where_clause: where_clause.clone(),
				concurrently: *concurrently,
				expressions: expressions.clone(),
				mysql_options: *mysql_options,
				operator_class: operator_class.clone(),
			};
			let sql = sqlite_typed_index_requires_raw_sql(
				*index_type,
				where_clause.as_deref(),
				*concurrently,
				expressions.as_deref(),
				operator_class.as_deref(),
			)
			.then(|| executable.try_to_sql(&SqlDialect::Sqlite))
			.transpose()?;
			schema
				.indexes
				.push(super::operations::SqliteRecreatedIndex {
					name: name
						.clone()
						.unwrap_or_else(|| format!("idx_{table}_{suffix}")),
					columns: columns.clone(),
					unique: *unique,
					sql,
				});
			schemas.insert(table.clone(), Some(schema));
		}
		#[cfg(feature = "pgvector")]
		Operation::CreateNamedIndex {
			table,
			name,
			columns,
			unique,
			index_type,
			where_clause,
			concurrently,
			expressions,
			operator_class,
			..
		} => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas, transforms)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite CreateNamedIndex for missing table '{table}'"
					))
				})?;
			let sql = sqlite_typed_index_requires_raw_sql(
				*index_type,
				where_clause.as_deref(),
				*concurrently,
				expressions.as_deref(),
				operator_class.as_deref(),
			)
			.then(|| operation.try_to_sql(&SqlDialect::Sqlite))
			.transpose()?;
			schema
				.indexes
				.push(super::operations::SqliteRecreatedIndex {
					name: name.clone(),
					columns: columns.clone(),
					unique: *unique,
					sql,
				});
			schemas.insert(table.clone(), Some(schema));
		}
		Operation::DropIndex { table, columns } => {
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas, transforms)
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
			let mut schema = sqlite_load_virtual_schema(editor, table, schemas, transforms)
				.await?
				.ok_or_else(|| {
					MigrationError::InvalidMigration(format!(
						"cannot plan SQLite DropNamedIndex for missing table '{table}'"
					))
				})?;
			schema.indexes.retain(|index| index.name != *name);
			schemas.insert(table.clone(), Some(schema));
		}
		Operation::MoveModel { .. }
		| Operation::AddConstraint { .. }
		| Operation::AddConstraintDefinition { .. }
		| Operation::AddConstraintRepair { .. }
		| Operation::RestoreConstraintOnRollback { .. }
		| Operation::DropConstraint { .. }
		| Operation::DropConstraintDefinition { .. }
		| Operation::DropColumn { .. }
		| Operation::AlterColumn { .. }
		| Operation::RunSQL { .. }
		| Operation::RunRust { .. }
		| Operation::AlterTableComment { .. }
		| Operation::AlterUniqueTogether { .. }
		| Operation::AlterModelOptions { .. }
		| Operation::CreateSchema { .. }
		| Operation::DropSchema { .. }
		| Operation::CreateExtension { .. }
		| Operation::BulkLoad { .. }
		| Operation::SetAutoIncrementValue { .. }
		| Operation::CreateCompositePrimaryKey { .. } => {
			return Err(MigrationError::InvalidMigration(format!(
				"SQLite virtual schema received an unclassified operation: {:?}",
				std::mem::discriminant(operation)
			)));
		}
	}
	Ok(())
}

#[cfg(feature = "sqlite")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqliteVirtualEffect {
	Simulate,
	SchemaNeutral,
	Opaque(&'static str),
}

#[cfg(feature = "sqlite")]
fn sqlite_forward_virtual_effect(operation: &Operation) -> SqliteVirtualEffect {
	match operation {
		Operation::CreateTable { .. }
		| Operation::DropTable { .. }
		| Operation::AddColumn { .. }
		| Operation::DropColumn { .. }
		| Operation::AlterColumn { .. }
		| Operation::RenameTable { .. }
		| Operation::RenameColumn { .. }
		| Operation::AddConstraint { .. }
		| Operation::AddConstraintDefinition { .. }
		| Operation::AddConstraintRepair { .. }
		| Operation::DropConstraint { .. }
		| Operation::DropConstraintDefinition { .. }
		| Operation::CreateIndex { .. }
		| Operation::CreateIndexRepair { .. }
		| Operation::DropIndex { .. }
		| Operation::CreateInheritedTable { .. }
		| Operation::AddDiscriminatorColumn { .. } => SqliteVirtualEffect::Simulate,
		#[cfg(feature = "pgvector")]
		Operation::CreateNamedIndex { .. } | Operation::DropNamedIndex { .. } => {
			SqliteVirtualEffect::Simulate
		}
		Operation::MoveModel { rename_table, .. } if *rename_table => SqliteVirtualEffect::Simulate,
		Operation::RunSQL { .. } => SqliteVirtualEffect::Opaque("RunSQL"),
		Operation::AlterUniqueTogether { .. } => SqliteVirtualEffect::Opaque("AlterUniqueTogether"),
		Operation::CreateSchema { .. } => SqliteVirtualEffect::Opaque("CreateSchema"),
		Operation::DropSchema { .. } => SqliteVirtualEffect::Opaque("DropSchema"),
		Operation::CreateExtension { .. } => SqliteVirtualEffect::Opaque("CreateExtension"),
		Operation::CreateCompositePrimaryKey { .. } => {
			SqliteVirtualEffect::Opaque("CreateCompositePrimaryKey")
		}
		Operation::RestoreConstraintOnRollback { .. }
		| Operation::RestoreIndexOnRollback { .. }
		| Operation::RunRust { .. }
		| Operation::AlterTableComment { .. }
		| Operation::AlterModelOptions { .. }
		| Operation::MoveModel { .. }
		| Operation::BulkLoad { .. }
		| Operation::SetAutoIncrementValue { .. } => SqliteVirtualEffect::SchemaNeutral,
	}
}

#[cfg(feature = "sqlite")]
fn sqlite_virtual_effect(
	operation: &Operation,
	planned_operation: Option<&Operation>,
	direction: MigrationDirection,
) -> SqliteVirtualEffect {
	match direction {
		MigrationDirection::Forward => sqlite_forward_virtual_effect(operation),
		MigrationDirection::Backward => match operation {
			Operation::RunSQL { reverse_sql, .. } => {
				if reverse_sql.is_some() {
					SqliteVirtualEffect::Opaque("RunSQL")
				} else {
					SqliteVirtualEffect::SchemaNeutral
				}
			}
			Operation::RestoreIndexOnRollback { .. } => SqliteVirtualEffect::Simulate,
			Operation::CreateTable { .. }
			| Operation::DropTable { .. }
			| Operation::AddColumn { .. }
			| Operation::DropColumn { .. }
			| Operation::AlterColumn { .. }
			| Operation::RenameTable { .. }
			| Operation::RenameColumn { .. }
			| Operation::AddConstraint { .. }
			| Operation::AddConstraintDefinition { .. }
			| Operation::AddConstraintRepair { .. }
			| Operation::RestoreConstraintOnRollback { .. }
			| Operation::DropConstraint { .. }
			| Operation::DropConstraintDefinition { .. }
			| Operation::CreateIndex { .. }
			| Operation::CreateIndexRepair { .. }
			| Operation::DropIndex { .. }
			| Operation::RunRust { .. }
			| Operation::AlterTableComment { .. }
			| Operation::AlterUniqueTogether { .. }
			| Operation::AlterModelOptions { .. }
			| Operation::CreateInheritedTable { .. }
			| Operation::AddDiscriminatorColumn { .. }
			| Operation::MoveModel { .. }
			| Operation::CreateSchema { .. }
			| Operation::DropSchema { .. }
			| Operation::CreateExtension { .. }
			| Operation::BulkLoad { .. }
			| Operation::SetAutoIncrementValue { .. }
			| Operation::CreateCompositePrimaryKey { .. } => planned_operation
				.map(sqlite_forward_virtual_effect)
				.unwrap_or(SqliteVirtualEffect::SchemaNeutral),
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex { .. } | Operation::DropNamedIndex { .. } => planned_operation
				.map(sqlite_forward_virtual_effect)
				.unwrap_or(SqliteVirtualEffect::SchemaNeutral),
		},
	}
}

/// Builds the SQL plan consumed by both migration execution and SQL inspection.
pub async fn plan_migration_sql(
	connection: &DatabaseConnection,
	migration: &Migration,
	state: &ProjectState,
	direction: MigrationDirection,
) -> Result<MigrationSqlPlan> {
	plan_migration_sql_for_inspection(connection, migration, state, direction, None, false).await
}

/// Builds a SQL plan from explicit states on both sides of a migration.
///
/// Forward planning uses `state_before`. Backward planning starts from
/// `state_after`, while replaying the migration forward from `state_before`
/// captures the pre-operation snapshots required to reverse destructive
/// operations such as legacy `DropColumn` and `DropTable`.
pub async fn plan_migration_sql_with_states(
	connection: &DatabaseConnection,
	migration: &Migration,
	state_before: &ProjectState,
	state_after: &ProjectState,
	direction: MigrationDirection,
) -> Result<MigrationSqlPlan> {
	let (operation_states, replayed_after) =
		migration_operation_pre_states(migration, state_before)?;
	let expected_after = if migration.database_only {
		state_before
	} else {
		&replayed_after
	};
	if expected_after != state_after {
		return Err(MigrationError::InvalidMigration(format!(
			"{} state_after does not match state_before replay",
			migration.id()
		)));
	}
	let backward_operation_states =
		matches!(direction, MigrationDirection::Backward).then_some(operation_states);
	let state = match direction {
		MigrationDirection::Forward => state_before,
		MigrationDirection::Backward => state_after,
	};
	plan_migration_sql_for_inspection(
		connection,
		migration,
		state,
		direction,
		backward_operation_states.as_deref(),
		true,
	)
	.await
}

fn migration_operation_pre_states(
	migration: &Migration,
	state_before: &ProjectState,
) -> Result<(Vec<ProjectState>, ProjectState)> {
	let mut current = state_before.clone();
	let mut states = Vec::with_capacity(migration.operations.len());
	for operation in &migration.operations {
		operation.validate_for_partial_state(&current)?;
		states.push(current.clone());
		operation.state_forwards(&migration.app_label, &mut current);
	}
	Ok((states, current))
}

fn irreversible_planning_error(migration: &Migration, operation: &Operation) -> MigrationError {
	let requires_pre_operation_state = matches!(
		operation,
		Operation::DropTable { .. }
			| Operation::DropColumn {
				old_definition: None,
				..
			} | Operation::AlterColumn {
			old_definition: None,
			..
		} | Operation::DropConstraint { .. }
	);
	if requires_pre_operation_state {
		MigrationError::IrreversibleError(format!(
			"{} requires pre-operation project state; use plan_migration_sql_with_states",
			migration.id()
		))
	} else {
		let operation_name = match operation {
			Operation::RunSQL { .. } => "RunSQL",
			Operation::RunRust { .. } => "RunRust",
			_ => "operation",
		};
		MigrationError::IrreversibleError(format!(
			"{} contains an irreversible {operation_name} operation",
			migration.id()
		))
	}
}

async fn plan_migration_sql_for_inspection(
	connection: &DatabaseConnection,
	migration: &Migration,
	state: &ProjectState,
	direction: MigrationDirection,
	backward_operation_states: Option<&[ProjectState]>,
	historical_state_only: bool,
) -> Result<MigrationSqlPlan> {
	if historical_state_only
		&& matches!(direction, MigrationDirection::Backward)
		&& state.has_opaque_schema_operations
	{
		return Err(MigrationError::InvalidMigration(format!(
			"cannot safely plan {} from historical state containing opaque schema operations",
			migration.id()
		)));
	}
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
			MigrationSqlPlanningOptions {
				strict_irreversible: true,
				sqlite_editor: Some(&mut editor),
				backward_operation_states,
				historical_state_only,
			},
		)
		.await?;
		editor.finish().await?;
		return Ok(plan);
	}
	plan_migration_sql_with_irreversible_policy(
		connection,
		migration,
		state,
		direction,
		MigrationSqlPlanningOptions {
			strict_irreversible: true,
			sqlite_editor: None,
			backward_operation_states,
			historical_state_only,
		},
	)
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
		MigrationSqlPlanningOptions {
			strict_irreversible: false,
			sqlite_editor: Some(editor),
			backward_operation_states: None,
			historical_state_only: false,
		},
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

struct MigrationSqlPlanningOptions<'a> {
	strict_irreversible: bool,
	sqlite_editor: Option<&'a mut SchemaEditor>,
	backward_operation_states: Option<&'a [ProjectState]>,
	historical_state_only: bool,
}

async fn plan_migration_sql_with_irreversible_policy(
	connection: &DatabaseConnection,
	migration: &Migration,
	state: &ProjectState,
	direction: MigrationDirection,
	options: MigrationSqlPlanningOptions<'_>,
) -> Result<MigrationSqlPlan> {
	let strict_irreversible = options.strict_irreversible;
	let backward_operation_states = options.backward_operation_states;
	let historical_state_only = options.historical_state_only;
	#[cfg_attr(not(feature = "sqlite"), allow(unused_variables))]
	let mut sqlite_editor = options.sqlite_editor;

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
	if historical_state_only
		&& matches!(dialect, SqlDialect::Sqlite)
		&& needs_sqlite_editor
		&& state.has_opaque_schema_operations
	{
		return Err(MigrationError::InvalidMigration(
			"cannot safely plan SQLite table recreation from historical state containing opaque SQL"
				.to_string(),
		));
	}
	#[cfg(feature = "sqlite")]
	let mut sqlite_virtual_schemas = HashMap::<String, Option<SqliteTableRecreation>>::new();
	#[cfg(feature = "sqlite")]
	if historical_state_only && matches!(dialect, SqlDialect::Sqlite) {
		for model in state.models.values() {
			let table = model.table_name.clone();
			let schema = sqlite_virtual_from_project_state(state, &table);
			sqlite_virtual_schemas.insert(table, schema);
		}
		for operation in &migration.operations {
			if let Some(table) = sqlite_recreation_table(operation) {
				sqlite_virtual_schemas
					.entry(table.to_string())
					.or_insert(None);
			}
		}
	}
	#[cfg(feature = "sqlite")]
	let mut sqlite_rename_transforms = Vec::<SqliteRenameTransform>::new();
	#[cfg(feature = "sqlite")]
	let mut sqlite_opaque_operation_seen = None;

	let operations: Vec<(usize, &Operation)> = match direction {
		MigrationDirection::Forward => migration.operations.iter().enumerate().collect(),
		MigrationDirection::Backward => migration.operations.iter().enumerate().rev().collect(),
	};

	for (operation_index, operation) in operations {
		let first_statement = statements.len();
		operation.validate_for_dialect(&dialect)?;
		if matches!(direction, MigrationDirection::Backward)
			&& matches!(operation, Operation::BulkLoad { .. })
		{
			return Err(MigrationError::IrreversibleError(format!(
				"{} contains an irreversible BulkLoad operation",
				migration.id()
			)));
		}
		let operation_state = backward_operation_states
			.and_then(|states| states.get(operation_index))
			.unwrap_or(state);
		let planned_operation = match direction {
			MigrationDirection::Forward => Some(operation.clone()),
			MigrationDirection::Backward => operation.to_reverse_operation(operation_state)?,
		};
		#[cfg(feature = "sqlite")]
		let sqlite_effect = if matches!(dialect, SqlDialect::Sqlite) {
			sqlite_virtual_effect(operation, planned_operation.as_ref(), direction)
		} else {
			SqliteVirtualEffect::SchemaNeutral
		};
		#[cfg(feature = "sqlite")]
		if let SqliteVirtualEffect::Opaque(operation_name) = sqlite_effect {
			sqlite_opaque_operation_seen = Some(operation_name);
		}

		#[cfg(feature = "sqlite")]
		let requires_recreation = matches!(dialect, SqlDialect::Sqlite)
			&& match direction {
				MigrationDirection::Forward => operation.requires_sqlite_recreation(),
				MigrationDirection::Backward => operation.reverse_requires_sqlite_recreation(),
			};
		#[cfg(feature = "sqlite")]
		if requires_recreation {
			if let Some(operation_name) = sqlite_opaque_operation_seen {
				return Err(MigrationError::InvalidMigration(format!(
					"cannot safely plan SQLite recreation after opaque {operation_name} in {}",
					migration.id(),
				)));
			}
			let reverse_missing =
				matches!(direction, MigrationDirection::Backward) && planned_operation.is_none();
			if reverse_missing {
				if strict_irreversible {
					return Err(irreversible_planning_error(migration, operation));
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
			let table = sqlite_recreation_table(operation).ok_or_else(|| {
				MigrationError::InvalidMigration(format!(
					"SQLite recreation operation has no table: {:?}",
					std::mem::discriminant(operation)
				))
			})?;
			sqlite_load_virtual_schema(
				editor,
				table,
				&mut sqlite_virtual_schemas,
				&sqlite_rename_transforms,
			)
			.await?;
			let previous = sqlite_virtual_schemas.remove(table).flatten();
			if historical_state_only && previous.is_none() {
				return Err(MigrationError::InvalidMigration(format!(
					"cannot plan SQLite recreation for '{table}' because the historical project state does not define it"
				)));
			}
			let (recreation_statements, recreation) =
				sqlite_recreation_statements(editor, operation, previous, &[]).await?;
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
				PlannedOperationOutput::Sql(sql) => append_sql(&mut statements, &sql, dialect),
				PlannedOperationOutput::Comment(comment) => {
					statements.push(PlannedStatement::Comment(comment));
				}
			},
			MigrationDirection::Backward => {
				let reverse = operation.to_reverse_sql(&dialect, operation_state)?;
				let Some(reverse) = reverse else {
					if strict_irreversible {
						return Err(irreversible_planning_error(migration, operation));
					}
					let operation_name = match operation {
						Operation::RunSQL { .. } => "RunSQL",
						Operation::RunRust { .. } => "RunRust",
						_ => "operation",
					};
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
						append_sql(&mut statements, &sql, dialect);
					}
				}
			}
		}
		planned_operations
			.extend((first_statement..statements.len()).map(|_| planned_operation.clone()));
		sqlite_recreation_groups.extend((first_statement..statements.len()).map(|_| None));
		#[cfg(feature = "sqlite")]
		if needs_sqlite_editor && matches!(sqlite_effect, SqliteVirtualEffect::Simulate) {
			let operation = planned_operation.as_ref().unwrap_or(operation);
			let editor = sqlite_editor.as_mut().expect("SQLite planner editor");
			sqlite_advance_virtual_schema(
				editor,
				operation,
				&mut sqlite_virtual_schemas,
				&mut sqlite_rename_transforms,
			)
			.await?;
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

#[cfg(all(test, feature = "sqlite"))]
mod tests {
	use super::*;
	use crate::migrations::{
		BulkLoadFormat, BulkLoadOptions, BulkLoadSource, ColumnDefinition, FieldType,
	};
	use rstest::rstest;

	#[rstest]
	#[case(
		SqlDialect::Postgres,
		r"SELECT 'C:\\'; SELECT 2;",
		&[r"SELECT 'C:\\'", "SELECT 2"]
	)]
	#[case(
		SqlDialect::Sqlite,
		r"SELECT 'C:\\'; SELECT 2;",
		&[r"SELECT 'C:\\'", "SELECT 2"]
	)]
	#[case(
		SqlDialect::Mysql,
		r"SELECT 'it\'s; still'; SELECT 2;",
		&[r"SELECT 'it\'s; still'", "SELECT 2"]
	)]
	#[case(
		SqlDialect::Postgres,
		r"SELECT E'it\'s; still'; SELECT 2;",
		&[r"SELECT E'it\'s; still'", "SELECT 2"]
	)]
	fn split_sql_statements_uses_dialect_specific_backslash_rules(
		#[case] dialect: SqlDialect,
		#[case] sql: &str,
		#[case] expected: &[&str],
	) {
		let statements = split_sql_statements_for_dialect(sql, dialect);

		assert_eq!(
			statements.iter().map(String::as_str).collect::<Vec<_>>(),
			expected
		);
	}

	#[rstest]
	fn trailing_line_comment_detection_uses_standard_postgres_quote_rules() {
		let sql = r"SELECT 'C:\\'; -- comment";

		assert_eq!(
			trailing_line_comment_start(sql, SqlDialect::Postgres),
			sql.find("--")
		);
	}

	#[rstest]
	fn mysql_dash_comment_requires_following_whitespace() {
		let sql = "SELECT 5--2";

		assert_eq!(trailing_line_comment_start(sql, SqlDialect::Mysql), None);
		assert_eq!(
			split_sql_statements_for_dialect("SELECT 5--2; SELECT 3;", SqlDialect::Mysql),
			vec!["SELECT 5--2".to_string(), "SELECT 3".to_string()]
		);
	}

	#[rstest]
	fn trailing_comment_detection_ignores_markers_inside_block_comments() {
		let sql = "SELECT 1 /* -- note */";

		assert_eq!(trailing_line_comment_start(sql, SqlDialect::Postgres), None);
		assert_eq!(
			render_sql_statement(sql, SqlDialect::Postgres),
			"SELECT 1 /* -- note */;\n"
		);
	}

	#[rstest]
	fn postgres_splitter_tracks_nested_block_comments() {
		let statements = split_sql_statements_for_dialect(
			"SELECT 1 /* outer /* inner */ still outer; */; SELECT 2;",
			SqlDialect::Postgres,
		);

		assert_eq!(
			statements,
			vec![
				"SELECT 1 /* outer /* inner */ still outer; */".to_string(),
				"SELECT 2".to_string(),
			]
		);
	}

	#[rstest]
	fn sqlite_splitter_keeps_trigger_bodies_together() {
		let statements = split_sql_statements_for_dialect(
			"CREATE TRIGGER audit AFTER INSERT ON users BEGIN INSERT INTO log VALUES (NEW.id); END; SELECT 2;",
			SqlDialect::Sqlite,
		);

		assert_eq!(
			statements,
			vec![
				"CREATE TRIGGER audit AFTER INSERT ON users BEGIN INSERT INTO log VALUES (NEW.id); END"
					.to_string(),
				"SELECT 2".to_string(),
			]
		);
	}

	#[rstest]
	fn sqlite_splitter_tracks_nested_case_end_in_trigger_bodies() {
		let statements = split_sql_statements_for_dialect(
			"CREATE TRIGGER audit AFTER UPDATE ON users BEGIN UPDATE log SET value = CASE WHEN NEW.value > 0 THEN 1 ELSE 0 END; INSERT INTO log VALUES (NEW.id); END; SELECT 2;",
			SqlDialect::Sqlite,
		);

		assert_eq!(
			statements,
			vec![
				"CREATE TRIGGER audit AFTER UPDATE ON users BEGIN UPDATE log SET value = CASE WHEN NEW.value > 0 THEN 1 ELSE 0 END; INSERT INTO log VALUES (NEW.id); END"
					.to_string(),
				"SELECT 2".to_string(),
			]
		);
	}

	#[test]
	fn mysql_hash_comment_does_not_split_on_its_semicolon() {
		let statements = split_sql_statements_for_dialect(
			"SELECT 1 # explanation; still comment\n; SELECT 2;",
			SqlDialect::Mysql,
		);

		assert_eq!(
			statements,
			vec![
				"SELECT 1 # explanation; still comment".to_string(),
				"SELECT 2".to_string(),
			]
		);
	}

	#[test]
	fn render_omits_transaction_for_all_concurrent_index_variants() {
		let plan = MigrationSqlPlan {
			atomic: true,
			statements: vec![PlannedStatement::Sql(
				"CREATE INDEX CONCURRENTLY idx ON books (id)".to_string(),
			)],
			planned_operations: vec![Some(Operation::CreateIndexRepair {
				table: "books".to_string(),
				name: Some("idx".to_string()),
				columns: vec!["id".to_string()],
				unique: false,
				index_type: None,
				where_clause: None,
				concurrently: true,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			})],
			sqlite_recreation_groups: vec![None],
			direction: MigrationDirection::Forward,
		};

		let rendered = plan.render(SqlDialect::Postgres);

		assert!(!rendered.contains("BEGIN;"), "{rendered}");
		assert!(!rendered.contains("COMMIT;"), "{rendered}");
	}

	#[test]
	fn render_keeps_terminator_outside_trailing_line_comment() {
		let plan = MigrationSqlPlan {
			atomic: false,
			statements: vec![PlannedStatement::Sql("SELECT 1 -- explanation".to_string())],
			planned_operations: vec![None],
			sqlite_recreation_groups: vec![None],
			direction: MigrationDirection::Forward,
		};

		assert_eq!(
			plan.render(SqlDialect::Postgres),
			"SELECT 1; -- explanation\n"
		);
	}

	#[test]
	fn sqlite_virtual_effect_classifies_special_schema_operations() {
		let discriminator = Operation::AddDiscriminatorColumn {
			table: "animals".to_string(),
			column_name: "kind".to_string(),
			default_value: "animal".to_string(),
		};
		let inherited = Operation::CreateInheritedTable {
			name: "employees".to_string(),
			columns: vec![ColumnDefinition::new("name", FieldType::Text)],
			base_table: "people".to_string(),
			join_column: "person_id".to_string(),
		};
		let moved = Operation::MoveModel {
			model_name: "Book".to_string(),
			from_app: "old".to_string(),
			to_app: "new".to_string(),
			rename_table: true,
			old_table_name: Some("old_books".to_string()),
			new_table_name: Some("new_books".to_string()),
		};
		let state_only_move = Operation::MoveModel {
			model_name: "Book".to_string(),
			from_app: "old".to_string(),
			to_app: "new".to_string(),
			rename_table: false,
			old_table_name: None,
			new_table_name: None,
		};
		let create_repair = Operation::CreateIndexRepair {
			table: "books".to_string(),
			name: Some("idx_books_title".to_string()),
			columns: vec!["title".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let restore = Operation::RestoreIndexOnRollback {
			table: "books".to_string(),
			name: Some("idx_books_title".to_string()),
			columns: vec!["title".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		assert_eq!(
			sqlite_forward_virtual_effect(&discriminator),
			SqliteVirtualEffect::Simulate
		);
		assert_eq!(
			sqlite_forward_virtual_effect(&inherited),
			SqliteVirtualEffect::Simulate
		);
		assert_eq!(
			sqlite_forward_virtual_effect(&moved),
			SqliteVirtualEffect::Simulate
		);
		assert_eq!(
			sqlite_forward_virtual_effect(&state_only_move),
			SqliteVirtualEffect::SchemaNeutral
		);
		assert_eq!(
			sqlite_forward_virtual_effect(&create_repair),
			SqliteVirtualEffect::Simulate
		);
		assert_eq!(
			sqlite_forward_virtual_effect(&restore),
			SqliteVirtualEffect::SchemaNeutral
		);
		assert_eq!(
			sqlite_virtual_effect(&restore, None, MigrationDirection::Backward),
			SqliteVirtualEffect::Simulate
		);
	}

	#[test]
	fn sqlite_virtual_effect_treats_bulk_load_as_schema_neutral() {
		let bulk_load = Operation::BulkLoad {
			table: "books".to_string(),
			source: BulkLoadSource::Stdin,
			format: BulkLoadFormat::Csv,
			options: BulkLoadOptions::default(),
		};

		assert_eq!(
			sqlite_forward_virtual_effect(&bulk_load),
			SqliteVirtualEffect::SchemaNeutral
		);
	}

	#[test]
	fn sqlite_virtual_effect_classifies_reverse_run_sql_from_original_operation() {
		let operation = Operation::RunSQL {
			sql: "ALTER TABLE books ADD COLUMN opaque TEXT".to_string(),
			reverse_sql: Some("ALTER TABLE books DROP COLUMN opaque".to_string()),
		};

		assert_eq!(
			sqlite_virtual_effect(&operation, None, MigrationDirection::Backward),
			SqliteVirtualEffect::Opaque("RunSQL")
		);
	}

	#[test]
	fn sqlite_simple_index_allow_list_requires_canonical_structural_sql() {
		let canonical = super::super::operations::SqliteRecreatedIndex {
			name: "idx_books_title".to_string(),
			columns: vec!["title".to_string()],
			unique: false,
			sql: Some("CREATE INDEX \"idx_books_title\" ON \"books\" (\"title\")".to_string()),
		};
		let mut multiline_partial = canonical.clone();
		multiline_partial.sql = Some(
			"CREATE INDEX \"idx_books_title\" ON \"books\" (\"title\")\nWHERE\t\"title\" IS NOT NULL"
				.to_string(),
		);
		let mut collated = canonical.clone();
		collated.sql = Some(
			"CREATE INDEX \"idx_books_title\" ON \"books\" (\"title\" COLLATE NOCASE)".to_string(),
		);
		let mut unquoted = canonical.clone();
		unquoted.sql = Some("CREATE INDEX idx_books_title ON books (title)".to_string());
		let unique = super::super::operations::SqliteRecreatedIndex {
			name: "idx_books_title_unique".to_string(),
			columns: vec!["title".to_string()],
			unique: true,
			sql: Some(
				"CREATE UNIQUE INDEX \"idx_books_title_unique\" ON \"books\" (\"title\")"
					.to_string(),
			),
		};
		let mut descending = canonical.clone();
		descending.sql =
			Some("CREATE INDEX \"idx_books_title\" ON \"books\" (\"title\" DESC)".to_string());

		assert!(!sqlite_index_requires_raw_sql(&canonical, "books"));
		assert!(!sqlite_index_requires_raw_sql(&unique, "books"));
		assert!(sqlite_index_requires_raw_sql(&multiline_partial, "books"));
		assert!(sqlite_index_requires_raw_sql(&collated, "books"));
		assert!(sqlite_index_requires_raw_sql(&descending, "books"));
		assert!(sqlite_index_requires_raw_sql(&unquoted, "books"));
		assert!(sqlite_typed_index_requires_raw_sql(
			Some(super::super::IndexType::Hash),
			None,
			false,
			None,
			None,
		));
		assert!(sqlite_typed_index_requires_raw_sql(
			None,
			None,
			false,
			None,
			Some("custom_ops"),
		));
	}

	#[test]
	fn sqlite_stored_column_rename_rejects_raw_constraint_reference() {
		let mut schema = SqliteTableRecreation::for_drop_column(
			"children",
			vec![
				ColumnDefinition::new("id", FieldType::Integer),
				ColumnDefinition::new("obsolete", FieldType::Text),
			],
			"obsolete",
			Vec::new(),
		)
		.with_raw_constraints(vec![
			super::super::operations::SqliteRecreatedConstraint {
				name: Some("children_parent_fk".to_string()),
				physical_name: None,
				columns: vec!["parent_code".to_string()],
				sql: "CONSTRAINT \"children_parent_fk\" FOREIGN KEY (\"parent_code\") REFERENCES \"parents\" (\"code\")".to_string(),
			},
		]);
		let transform = SqliteRenameTransform::Column {
			table: "parents".to_string(),
			old_name: "code".to_string(),
			new_name: "slug".to_string(),
		};

		let error = sqlite_apply_rename_transform(&mut schema, &transform).unwrap_err();

		assert_eq!(
			error.to_string(),
			"Invalid migration: raw constraint references renamed SQLite identifier 'parents.code'"
		);
	}
}
