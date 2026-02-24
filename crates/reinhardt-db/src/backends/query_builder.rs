//! Query builder with dialect support

use std::sync::Arc;

use reinhardt_query::prelude::{
	Alias, ColumnRef, Expr, ExprTrait, Query, QueryBuilder as RqQueryBuilder, SelectStatement,
	Value,
};

use super::{
	backend::DatabaseBackend,
	error::Result,
	types::{QueryResult, QueryValue, Row},
};

/// Convert QueryValue to reinhardt-query Value
fn query_value_to_sea_value(qv: &QueryValue) -> Value {
	match qv {
		// BigInt(None) is used for generic NULL values across all dialects
		// (consistent with PostgreSQL, MySQL, SQLite backend implementations)
		QueryValue::Null => Value::BigInt(None),
		QueryValue::Bool(b) => Value::Bool(Some(*b)),
		QueryValue::Int(i) => Value::BigInt(Some(*i)),
		QueryValue::Float(f) => Value::Double(Some(*f)),
		QueryValue::String(s) => Value::String(Some(Box::new(s.clone()))),
		QueryValue::Bytes(b) => Value::Bytes(Some(Box::new(b.clone()))),
		QueryValue::Timestamp(dt) => Value::ChronoDateTimeUtc(Some(Box::new(*dt))),
		QueryValue::Uuid(u) => Value::Uuid(Some(Box::new(*u))),
		// NOW() is handled specially in build() methods, should not reach here
		QueryValue::Now => {
			panic!("QueryValue::Now should be handled in build() method, not converted to Value")
		}
	}
}

/// Conflict target specifying which constraint or columns trigger the conflict
#[derive(Debug, Clone)]
pub enum ConflictTarget {
	/// Specify conflict columns (e.g., `ON CONFLICT (email, tenant_id)`)
	Columns(Vec<String>),
	/// Specify a named constraint (e.g., `ON CONFLICT ON CONSTRAINT users_email_key`)
	/// Note: Only supported by PostgreSQL
	Constraint(String),
}

/// ON CONFLICT action for INSERT statements
#[derive(Debug, Clone)]
pub enum OnConflictAction {
	/// Do nothing on conflict (PostgreSQL: ON CONFLICT DO NOTHING, MySQL: INSERT IGNORE, SQLite: INSERT OR IGNORE)
	DoNothing {
		/// Conflict columns (PostgreSQL only)
		conflict_columns: Option<Vec<String>>,
	},
	/// Update on conflict (PostgreSQL: ON CONFLICT DO UPDATE, MySQL: ON DUPLICATE KEY UPDATE)
	DoUpdate {
		/// Conflict columns (PostgreSQL only)
		conflict_columns: Option<Vec<String>>,
		/// Columns to update on conflict
		update_columns: Vec<String>,
	},
}

/// Fluent builder for ON CONFLICT clause with advanced options
///
/// This builder provides a more fluent API for constructing ON CONFLICT clauses,
/// with support for:
/// - Column-based and constraint-based conflict targets
/// - Conditional updates with WHERE clauses
/// - Explicit column assignments using EXCLUDED values
///
/// # Example
///
/// ```rust,ignore
/// // Basic upsert on email column
/// builder.on_conflict(OnConflictClause::columns(vec!["email"])
///     .do_update(vec!["name", "updated_at"]))
///
/// // Upsert with conditional WHERE clause
/// builder.on_conflict(OnConflictClause::columns(vec!["email"])
///     .do_update(vec!["name", "updated_at"])
///     .where_clause("users.updated_at < EXCLUDED.updated_at"))
///
/// // Upsert on named constraint (PostgreSQL only)
/// builder.on_conflict(OnConflictClause::constraint("users_email_key")
///     .do_update(vec!["name"]))
/// ```
#[derive(Debug, Clone)]
pub struct OnConflictClause {
	/// The conflict target (columns or constraint)
	target: Option<ConflictTarget>,
	/// The action to take on conflict
	action: OnConflictClauseAction,
	/// Optional WHERE clause for conditional updates (PostgreSQL/SQLite only)
	where_condition: Option<String>,
}

/// Action to take when a conflict occurs
#[derive(Debug, Clone)]
pub enum OnConflictClauseAction {
	/// Do nothing on conflict
	DoNothing,
	/// Update specified columns on conflict
	DoUpdate {
		/// Columns to update on conflict
		update_columns: Vec<String>,
	},
}

impl OnConflictClause {
	/// Create a new ON CONFLICT clause targeting specific columns
	///
	/// # Arguments
	///
	/// * `columns` - Columns that form the conflict target
	///
	/// # Example
	///
	/// ```rust,ignore
	/// OnConflictClause::columns(vec!["email", "tenant_id"])
	///     .do_update(vec!["name"])
	/// ```
	pub fn columns(columns: Vec<impl Into<String>>) -> Self {
		Self {
			target: Some(ConflictTarget::Columns(
				columns.into_iter().map(Into::into).collect(),
			)),
			action: OnConflictClauseAction::DoNothing,
			where_condition: None,
		}
	}

	/// Create a new ON CONFLICT clause targeting a named constraint
	///
	/// Note: This is only supported by PostgreSQL
	///
	/// # Arguments
	///
	/// * `constraint_name` - Name of the constraint to target
	///
	/// # Example
	///
	/// ```rust,ignore
	/// OnConflictClause::constraint("users_email_key")
	///     .do_update(vec!["name"])
	/// ```
	pub fn constraint(constraint_name: impl Into<String>) -> Self {
		Self {
			target: Some(ConflictTarget::Constraint(constraint_name.into())),
			action: OnConflictClauseAction::DoNothing,
			where_condition: None,
		}
	}

	/// Create a new ON CONFLICT clause with no specific target
	///
	/// This matches any unique constraint violation. Note that for SQLite
	/// with DO UPDATE, a target is required.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// OnConflictClause::any()
	///     .do_nothing()
	/// ```
	pub fn any() -> Self {
		Self {
			target: None,
			action: OnConflictClauseAction::DoNothing,
			where_condition: None,
		}
	}

	/// Set the action to DO NOTHING on conflict
	///
	/// # Example
	///
	/// ```rust,ignore
	/// OnConflictClause::columns(vec!["email"])
	///     .do_nothing()
	/// ```
	pub fn do_nothing(mut self) -> Self {
		self.action = OnConflictClauseAction::DoNothing;
		self
	}

	/// Set the action to DO UPDATE with specified columns
	///
	/// The updated values will be taken from the EXCLUDED pseudo-table
	/// (or VALUES() function for MySQL).
	///
	/// # Arguments
	///
	/// * `columns` - Columns to update when conflict occurs
	///
	/// # Example
	///
	/// ```rust,ignore
	/// OnConflictClause::columns(vec!["email"])
	///     .do_update(vec!["name", "updated_at"])
	/// ```
	pub fn do_update(mut self, columns: Vec<impl Into<String>>) -> Self {
		self.action = OnConflictClauseAction::DoUpdate {
			update_columns: columns.into_iter().map(Into::into).collect(),
		};
		self
	}

	/// Add a WHERE clause for conditional updates
	///
	/// The WHERE clause is evaluated before the update is performed.
	/// Only rows matching the condition will be updated.
	///
	/// Note: Only supported by PostgreSQL and SQLite. MySQL does not support
	/// conditional updates in ON DUPLICATE KEY UPDATE.
	///
	/// # Arguments
	///
	/// * `condition` - SQL condition expression
	///
	/// # Safety
	///
	/// This method embeds the `condition` string directly into the generated SQL
	/// without any escaping or parameterization. The caller **must** ensure that
	/// the input is trusted and not derived from user-controlled data.
	///
	/// # SQL Injection Risk
	///
	/// Passing unsanitized user input to this method will result in a SQL injection
	/// vulnerability. Always use hardcoded or application-controlled expressions.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// // Only update if the new data is newer
	/// OnConflictClause::columns(vec!["email"])
	///     .do_update(vec!["name", "updated_at"])
	///     .where_clause("users.updated_at < EXCLUDED.updated_at")
	///
	/// // Only update if version is greater
	/// OnConflictClause::columns(vec!["id"])
	///     .do_update(vec!["data", "version"])
	///     .where_clause("users.version < EXCLUDED.version")
	/// ```
	pub fn where_clause(mut self, condition: impl Into<String>) -> Self {
		self.where_condition = Some(condition.into());
		self
	}
}

/// INSERT query builder
pub struct InsertBuilder {
	backend: Arc<dyn DatabaseBackend>,
	table: String,
	columns: Vec<String>,
	values: Vec<QueryValue>,
	returning: Option<Vec<String>>,
	on_conflict: Option<OnConflictAction>,
	on_conflict_clause: Option<OnConflictClause>,
}

impl InsertBuilder {
	pub fn new(backend: Arc<dyn DatabaseBackend>, table: impl Into<String>) -> Self {
		Self {
			backend,
			table: table.into(),
			columns: Vec::new(),
			values: Vec::new(),
			returning: None,
			on_conflict: None,
			on_conflict_clause: None,
		}
	}

	pub fn value(mut self, column: impl Into<String>, value: impl Into<QueryValue>) -> Self {
		self.columns.push(column.into());
		self.values.push(value.into());
		self
	}

	pub fn returning(mut self, columns: Vec<&str>) -> Self {
		if self.backend.supports_returning() {
			self.returning = Some(columns.iter().map(|s| (*s).to_owned()).collect());
		}
		self
	}

	/// Set ON CONFLICT DO NOTHING behavior
	///
	/// # Arguments
	///
	/// * `conflict_columns` - Columns to check for conflict (PostgreSQL only, None for all unique constraints)
	///
	/// # Example
	///
	/// ```rust,ignore
	/// builder.on_conflict_do_nothing(Some(vec!["email".to_string()]))
	/// ```
	pub fn on_conflict_do_nothing(mut self, conflict_columns: Option<Vec<String>>) -> Self {
		self.on_conflict = Some(OnConflictAction::DoNothing { conflict_columns });
		self
	}

	/// Set ON CONFLICT DO UPDATE behavior
	///
	/// # Arguments
	///
	/// * `conflict_columns` - Columns to check for conflict (PostgreSQL only)
	/// * `update_columns` - Columns to update on conflict
	///
	/// # Example
	///
	/// ```rust,ignore
	/// builder.on_conflict_do_update(
	///     Some(vec!["email".to_string()]),
	///     vec!["name".to_string(), "updated_at".to_string()],
	/// )
	/// ```
	pub fn on_conflict_do_update(
		mut self,
		conflict_columns: Option<Vec<String>>,
		update_columns: Vec<String>,
	) -> Self {
		self.on_conflict = Some(OnConflictAction::DoUpdate {
			conflict_columns,
			update_columns,
		});
		self
	}

	/// Set ON CONFLICT behavior using the fluent `OnConflictClause` builder
	///
	/// This method provides a more flexible API for specifying conflict handling,
	/// including support for:
	/// - Column-based conflict targets
	/// - Constraint-based conflict targets (PostgreSQL only)
	/// - Conditional updates with WHERE clauses
	///
	/// # Arguments
	///
	/// * `clause` - The ON CONFLICT clause configuration
	///
	/// # Example
	///
	/// ```rust,ignore
	/// // Basic upsert on email column
	/// builder.on_conflict(OnConflictClause::columns(vec!["email"])
	///     .do_update(vec!["name", "updated_at"]))
	///
	/// // Upsert with conditional WHERE clause (only update if newer)
	/// builder.on_conflict(OnConflictClause::columns(vec!["email"])
	///     .do_update(vec!["name", "updated_at"])
	///     .where_clause("users.updated_at < EXCLUDED.updated_at"))
	///
	/// // Upsert on named constraint (PostgreSQL only)
	/// builder.on_conflict(OnConflictClause::constraint("users_email_key")
	///     .do_update(vec!["name"]))
	///
	/// // Do nothing on any conflict
	/// builder.on_conflict(OnConflictClause::any()
	///     .do_nothing())
	/// ```
	pub fn on_conflict(mut self, clause: OnConflictClause) -> Self {
		self.on_conflict_clause = Some(clause);
		self
	}

	pub fn build(&self) -> Result<(String, Vec<QueryValue>)> {
		use super::types::DatabaseType;
		use reinhardt_query::prelude::{
			MySqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder,
		};

		let mut stmt = Query::insert()
			.into_table(Alias::new(&self.table))
			.to_owned();

		// Add columns
		let column_refs: Vec<Alias> = self.columns.iter().map(Alias::new).collect();
		stmt.columns(column_refs);

		// Add values
		if !self.values.is_empty() {
			let sea_values: Vec<Value> = self.values.iter().map(query_value_to_sea_value).collect();
			stmt.values(sea_values).unwrap();
		}

		// Add RETURNING clause if supported
		if let Some(ref cols) = self.returning {
			let col_refs: Vec<Alias> = cols.iter().map(Alias::new).collect();
			stmt.returning(col_refs);
		}

		// Build SQL based on database type
		let mut sql = match self.backend.database_type() {
			DatabaseType::Postgres => PostgresQueryBuilder.build_insert(&stmt).0,
			DatabaseType::Mysql => MySqlQueryBuilder.build_insert(&stmt).0,
			DatabaseType::Sqlite => SqliteQueryBuilder.build_insert(&stmt).0,
		};

		// Add ON CONFLICT clause if specified
		// Prefer the new OnConflictClause over the legacy OnConflictAction
		if let Some(ref clause) = self.on_conflict_clause {
			sql = self.apply_new_on_conflict_clause(sql, clause)?;
		} else if let Some(ref on_conflict) = self.on_conflict {
			sql = self.apply_on_conflict_clause(sql, on_conflict)?;
		}

		Ok((sql, self.values.clone()))
	}

	/// Apply ON CONFLICT clause to SQL string based on database type
	fn apply_on_conflict_clause(
		&self,
		mut sql: String,
		action: &OnConflictAction,
	) -> Result<String> {
		use super::types::DatabaseType;

		match self.backend.database_type() {
			DatabaseType::Postgres => match action {
				OnConflictAction::DoNothing { conflict_columns } => {
					if let Some(cols) = conflict_columns {
						let cols_str = cols.join(", ");
						sql.push_str(&format!(" ON CONFLICT ({}) DO NOTHING", cols_str));
					} else {
						sql.push_str(" ON CONFLICT DO NOTHING");
					}
				}
				OnConflictAction::DoUpdate {
					conflict_columns,
					update_columns,
				} => {
					let conflict_str = if let Some(cols) = conflict_columns {
						format!("({})", cols.join(", "))
					} else {
						String::new()
					};

					let update_str = update_columns
						.iter()
						.map(|col| format!("{} = EXCLUDED.{}", col, col))
						.collect::<Vec<_>>()
						.join(", ");

					sql.push_str(&format!(
						" ON CONFLICT {} DO UPDATE SET {}",
						conflict_str, update_str
					));
				}
			},
			DatabaseType::Mysql => {
				match action {
					OnConflictAction::DoNothing { .. } => {
						// MySQL: INSERT IGNORE
						sql = sql.replacen("INSERT", "INSERT IGNORE", 1);
					}
					OnConflictAction::DoUpdate {
						conflict_columns: _,
						update_columns,
					} => {
						// MySQL: ON DUPLICATE KEY UPDATE
						let update_str = update_columns
							.iter()
							.map(|col| format!("{} = VALUES({})", col, col))
							.collect::<Vec<_>>()
							.join(", ");

						sql.push_str(&format!(" ON DUPLICATE KEY UPDATE {}", update_str));
					}
				}
			}
			DatabaseType::Sqlite => {
				match action {
					OnConflictAction::DoNothing { .. } => {
						// SQLite: INSERT OR IGNORE
						sql = sql.replacen("INSERT", "INSERT OR IGNORE", 1);
					}
					OnConflictAction::DoUpdate {
						conflict_columns,
						update_columns,
					} => {
						// SQLite: ON CONFLICT DO UPDATE (SQLite 3.24.0+)
						let conflict_str = if let Some(cols) = conflict_columns {
							if cols.is_empty() {
								return Err(super::error::DatabaseError::SyntaxError(
									"SQLite ON CONFLICT requires non-empty conflict_columns for DO UPDATE".to_string(),
								));
							}
							format!("({})", cols.join(", "))
						} else {
							// SQLite requires conflict target - skip ON CONFLICT clause
							return Ok(sql);
						};

						if update_columns.is_empty() {
							return Err(super::error::DatabaseError::SyntaxError(
								"update_columns cannot be empty for OnConflictAction::DoUpdate"
									.to_string(),
							));
						}

						let update_str = update_columns
							.iter()
							.map(|col| format!("{} = excluded.{}", col, col)) // lowercase 'excluded'
							.collect::<Vec<_>>()
							.join(", ");

						sql.push_str(&format!(
							" ON CONFLICT {} DO UPDATE SET {}",
							conflict_str, update_str
						));
					}
				}
			}
		}

		Ok(sql)
	}

	/// Apply new OnConflictClause to SQL string based on database type
	///
	/// This method handles the enhanced ON CONFLICT clause with support for:
	/// - Column-based conflict targets
	/// - Constraint-based conflict targets (PostgreSQL only)
	/// - WHERE clauses for conditional updates
	fn apply_new_on_conflict_clause(
		&self,
		mut sql: String,
		clause: &OnConflictClause,
	) -> Result<String> {
		use super::types::DatabaseType;

		match self.backend.database_type() {
			DatabaseType::Postgres => {
				// Build conflict target
				let target_str = match &clause.target {
					Some(ConflictTarget::Columns(cols)) => {
						format!("({})", cols.join(", "))
					}
					Some(ConflictTarget::Constraint(name)) => {
						format!("ON CONSTRAINT {}", name)
					}
					None => String::new(),
				};

				match &clause.action {
					OnConflictClauseAction::DoNothing => {
						if target_str.is_empty() {
							sql.push_str(" ON CONFLICT DO NOTHING");
						} else {
							sql.push_str(&format!(" ON CONFLICT {} DO NOTHING", target_str));
						}
					}
					OnConflictClauseAction::DoUpdate { update_columns } => {
						let update_str = update_columns
							.iter()
							.map(|col| format!("{} = EXCLUDED.{}", col, col))
							.collect::<Vec<_>>()
							.join(", ");

						let mut clause_str =
							format!(" ON CONFLICT {} DO UPDATE SET {}", target_str, update_str);

						// Add WHERE clause if specified
						if let Some(ref where_cond) = clause.where_condition {
							clause_str.push_str(&format!(" WHERE {}", where_cond));
						}

						sql.push_str(&clause_str);
					}
				}
			}
			DatabaseType::Mysql => {
				// MySQL does not support conflict targets or WHERE clauses
				// Warn if constraint-based target is used
				if let Some(ConflictTarget::Constraint(_)) = &clause.target {
					// MySQL doesn't support ON CONFLICT ON CONSTRAINT
					// Fall back to standard MySQL behavior
				}

				match &clause.action {
					OnConflictClauseAction::DoNothing => {
						sql = sql.replacen("INSERT", "INSERT IGNORE", 1);
					}
					OnConflictClauseAction::DoUpdate { update_columns } => {
						let update_str = update_columns
							.iter()
							.map(|col| format!("{} = VALUES({})", col, col))
							.collect::<Vec<_>>()
							.join(", ");

						sql.push_str(&format!(" ON DUPLICATE KEY UPDATE {}", update_str));
						// Note: MySQL does not support WHERE clause in ON DUPLICATE KEY UPDATE
					}
				}
			}
			DatabaseType::Sqlite => {
				// SQLite uses similar syntax to PostgreSQL but with lowercase 'excluded'
				match &clause.action {
					OnConflictClauseAction::DoNothing => {
						sql = sql.replacen("INSERT", "INSERT OR IGNORE", 1);
					}
					OnConflictClauseAction::DoUpdate { update_columns } => {
						// SQLite requires conflict columns for DO UPDATE
						let conflict_str = match &clause.target {
							Some(ConflictTarget::Columns(cols)) => {
								if cols.is_empty() {
									return Err(super::error::DatabaseError::SyntaxError(
										"SQLite ON CONFLICT requires non-empty conflict_columns for DO UPDATE".to_string(),
									));
								}
								format!("({})", cols.join(", "))
							}
							Some(ConflictTarget::Constraint(_)) => {
								// SQLite doesn't support ON CONSTRAINT syntax
								return Err(super::error::DatabaseError::NotSupported(
									"SQLite does not support ON CONFLICT ON CONSTRAINT syntax"
										.to_string(),
								));
							}
							None => {
								// SQLite requires conflict target for DO UPDATE
								return Ok(sql);
							}
						};

						if update_columns.is_empty() {
							return Err(super::error::DatabaseError::SyntaxError(
								"update_columns cannot be empty for OnConflictClauseAction::DoUpdate".to_string(),
							));
						}

						let update_str = update_columns
							.iter()
							.map(|col| format!("{} = excluded.{}", col, col)) // lowercase 'excluded'
							.collect::<Vec<_>>()
							.join(", ");

						let mut clause_str =
							format!(" ON CONFLICT {} DO UPDATE SET {}", conflict_str, update_str);

						// Add WHERE clause if specified
						if let Some(ref where_cond) = clause.where_condition {
							clause_str.push_str(&format!(" WHERE {}", where_cond));
						}

						sql.push_str(&clause_str);
					}
				}
			}
		}

		Ok(sql)
	}

	pub async fn execute(&self) -> Result<QueryResult> {
		let (sql, params) = self.build()?;
		self.backend.execute(&sql, params).await
	}

	pub async fn fetch_one(&self) -> Result<Row> {
		let (sql, params) = self.build()?;
		self.backend.fetch_one(&sql, params).await
	}

	/// Convert to INSERT FROM SELECT builder
	///
	/// This method is mutually exclusive with `value()`. When `from_select()` is
	/// called, all previously added values are discarded and the SELECT statement
	/// is used as the source of data.
	///
	/// # Arguments
	///
	/// * `columns` - Columns to insert into
	/// * `select_stmt` - The SELECT statement to use as data source
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let select = Query::select()
	///     .columns([Alias::new("id"), Alias::new("name")])
	///     .from(Alias::new("source_table"))
	///     .to_owned();
	///
	/// builder.from_select(vec!["id", "name"], select)
	/// ```
	pub fn from_select(
		self,
		columns: Vec<&str>,
		select_stmt: SelectStatement,
	) -> InsertFromSelectBuilder {
		InsertFromSelectBuilder::new(self.backend, &self.table, columns, select_stmt)
			.with_returning(self.returning)
			.with_on_conflict(self.on_conflict)
	}
}

/// INSERT FROM SELECT query builder
///
/// Builds INSERT INTO ... SELECT statements for inserting data from a subquery.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_query::prelude::{QueryStatementBuilder, Alias, Query};
///
/// let select = Query::select()
///     .columns([Alias::new("id"), Alias::new("name")])
///     .from(Alias::new("source_table"))
///     .to_owned();
///
/// let builder = InsertFromSelectBuilder::new(
///     backend,
///     "target_table",
///     vec!["id", "name"],
///     select,
/// );
///
/// let (sql, _) = builder.build();
/// // Generates: INSERT INTO "target_table" ("id", "name") SELECT "id", "name" FROM "source_table"
/// ```
pub struct InsertFromSelectBuilder {
	backend: Arc<dyn DatabaseBackend>,
	table: String,
	columns: Vec<String>,
	select_stmt: SelectStatement,
	returning: Option<Vec<String>>,
	on_conflict: Option<OnConflictAction>,
}

impl InsertFromSelectBuilder {
	pub fn new(
		backend: Arc<dyn DatabaseBackend>,
		table: impl Into<String>,
		columns: Vec<&str>,
		select_stmt: SelectStatement,
	) -> Self {
		Self {
			backend,
			table: table.into(),
			columns: columns.iter().map(|s| (*s).to_owned()).collect(),
			select_stmt,
			returning: None,
			on_conflict: None,
		}
	}

	fn with_returning(mut self, returning: Option<Vec<String>>) -> Self {
		self.returning = returning;
		self
	}

	fn with_on_conflict(mut self, on_conflict: Option<OnConflictAction>) -> Self {
		self.on_conflict = on_conflict;
		self
	}

	pub fn returning(mut self, columns: Vec<&str>) -> Self {
		if self.backend.supports_returning() {
			self.returning = Some(columns.iter().map(|s| (*s).to_owned()).collect());
		}
		self
	}

	pub fn on_conflict_do_nothing(mut self, conflict_columns: Option<Vec<String>>) -> Self {
		self.on_conflict = Some(OnConflictAction::DoNothing { conflict_columns });
		self
	}

	pub fn on_conflict_do_update(
		mut self,
		conflict_columns: Option<Vec<String>>,
		update_columns: Vec<String>,
	) -> Self {
		self.on_conflict = Some(OnConflictAction::DoUpdate {
			conflict_columns,
			update_columns,
		});
		self
	}

	pub fn build(&self) -> (String, Vec<QueryValue>) {
		use super::types::DatabaseType;
		use reinhardt_query::prelude::{
			MySqlQueryBuilder, PostgresQueryBuilder, QueryStatementBuilder, SqliteQueryBuilder,
		};

		// Build INSERT ... SELECT manually since InsertStatement doesn't support select_from
		let quote = match self.backend.database_type() {
			DatabaseType::Postgres | DatabaseType::Sqlite => '"',
			DatabaseType::Mysql => '`',
		};

		let cols_str = self
			.columns
			.iter()
			.map(|c| format!("{}{}{}", quote, c, quote))
			.collect::<Vec<_>>()
			.join(", ");

		let select_sql = match self.backend.database_type() {
			DatabaseType::Postgres => self.select_stmt.to_string(PostgresQueryBuilder),
			DatabaseType::Mysql => self.select_stmt.to_string(MySqlQueryBuilder),
			DatabaseType::Sqlite => self.select_stmt.to_string(SqliteQueryBuilder),
		};

		let mut sql = format!(
			"INSERT INTO {}{}{} ({}) {}",
			quote, self.table, quote, cols_str, select_sql
		);

		// Add RETURNING clause if supported
		if let Some(ref cols) = self.returning {
			let returning_str = cols
				.iter()
				.map(|c| format!("{}{}{}", quote, c, quote))
				.collect::<Vec<_>>()
				.join(", ");
			sql.push_str(&format!(" RETURNING {}", returning_str));
		}

		if let Some(ref on_conflict) = self.on_conflict {
			sql = self.apply_on_conflict_clause(sql, on_conflict);
		}

		(sql, Vec::new())
	}

	fn apply_on_conflict_clause(&self, mut sql: String, action: &OnConflictAction) -> String {
		use super::types::DatabaseType;

		match self.backend.database_type() {
			DatabaseType::Postgres => match action {
				OnConflictAction::DoNothing { conflict_columns } => {
					if let Some(cols) = conflict_columns {
						sql.push_str(&format!(" ON CONFLICT ({}) DO NOTHING", cols.join(", ")));
					} else {
						sql.push_str(" ON CONFLICT DO NOTHING");
					}
				}
				OnConflictAction::DoUpdate {
					conflict_columns,
					update_columns,
				} => {
					let conflict_str = conflict_columns
						.as_ref()
						.map(|cols| format!("({})", cols.join(", ")))
						.unwrap_or_default();
					let update_str = update_columns
						.iter()
						.map(|col| format!("{} = EXCLUDED.{}", col, col))
						.collect::<Vec<_>>()
						.join(", ");
					sql.push_str(&format!(
						" ON CONFLICT {} DO UPDATE SET {}",
						conflict_str, update_str
					));
				}
			},
			DatabaseType::Mysql => match action {
				OnConflictAction::DoNothing { .. } => {
					sql = sql.replacen("INSERT", "INSERT IGNORE", 1);
				}
				OnConflictAction::DoUpdate { update_columns, .. } => {
					let update_str = update_columns
						.iter()
						.map(|col| format!("{} = VALUES({})", col, col))
						.collect::<Vec<_>>()
						.join(", ");
					sql.push_str(&format!(" ON DUPLICATE KEY UPDATE {}", update_str));
				}
			},
			DatabaseType::Sqlite => match action {
				OnConflictAction::DoNothing { .. } => {
					sql = sql.replacen("INSERT", "INSERT OR IGNORE", 1);
				}
				OnConflictAction::DoUpdate {
					conflict_columns,
					update_columns,
				} => {
					let conflict_str = match conflict_columns {
						Some(cols) if !cols.is_empty() => format!("({})", cols.join(", ")),
						_ => return sql,
					};
					let update_str = update_columns
						.iter()
						.map(|col| format!("{} = excluded.{}", col, col))
						.collect::<Vec<_>>()
						.join(", ");
					sql.push_str(&format!(
						" ON CONFLICT {} DO UPDATE SET {}",
						conflict_str, update_str
					));
				}
			},
		}
		sql
	}

	pub async fn execute(&self) -> Result<QueryResult> {
		let (sql, params) = self.build();
		self.backend.execute(&sql, params).await
	}

	pub async fn fetch_one(&self) -> Result<Row> {
		let (sql, params) = self.build();
		self.backend.fetch_one(&sql, params).await
	}
}

/// UPDATE query builder
pub struct UpdateBuilder {
	backend: Arc<dyn DatabaseBackend>,
	table: String,
	sets: Vec<(String, QueryValue)>,
	wheres: Vec<(String, String, QueryValue)>,
}

impl UpdateBuilder {
	pub fn new(backend: Arc<dyn DatabaseBackend>, table: impl Into<String>) -> Self {
		Self {
			backend,
			table: table.into(),
			sets: Vec::new(),
			wheres: Vec::new(),
		}
	}

	pub fn set(mut self, column: impl Into<String>, value: impl Into<QueryValue>) -> Self {
		self.sets.push((column.into(), value.into()));
		self
	}

	pub fn set_now(mut self, column: impl Into<String>) -> Self {
		self.sets.push((column.into(), QueryValue::Now));
		self
	}

	pub fn where_eq(mut self, column: impl Into<String>, value: impl Into<QueryValue>) -> Self {
		self.wheres
			.push((column.into(), "=".to_string(), value.into()));
		self
	}

	pub fn build(&self) -> (String, Vec<QueryValue>) {
		use super::types::DatabaseType;
		use reinhardt_query::prelude::{
			MySqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder,
		};

		// Sentinel placeholder for NOW() values (replaced in final SQL)
		const NOW_PLACEHOLDER: &str = "__REINHARDT_NOW__";

		let mut stmt = Query::update().table(Alias::new(&self.table)).to_owned();

		// Add SET clauses
		for (col, val) in &self.sets {
			if matches!(val, QueryValue::Now) {
				// Use a sentinel string that will be replaced with NOW() in the output
				stmt.value(Alias::new(col), NOW_PLACEHOLDER);
				continue;
			}
			stmt.value(Alias::new(col), query_value_to_sea_value(val));
		}

		// Add WHERE clauses
		for (col, op, val) in &self.wheres {
			if op == "=" {
				stmt.and_where(
					Expr::col(Alias::new(col)).eq(Expr::val(query_value_to_sea_value(val))),
				);
			}
		}

		// Build SQL based on database type
		let sql = match self.backend.database_type() {
			DatabaseType::Postgres => PostgresQueryBuilder.build_update(&stmt).0,
			DatabaseType::Mysql => MySqlQueryBuilder.build_update(&stmt).0,
			DatabaseType::Sqlite => SqliteQueryBuilder.build_update(&stmt).0,
		};

		// Replace NOW() placeholder sentinel with actual function call
		let sql = sql.replace(&format!("'{}'", NOW_PLACEHOLDER), "NOW()");

		// Preserve parameter order: first SET values, then WHERE values
		let mut params = Vec::new();
		for (_, val) in &self.sets {
			if !matches!(val, QueryValue::Now) {
				params.push(val.clone());
			}
		}
		for (_, _, val) in &self.wheres {
			params.push(val.clone());
		}

		(sql, params)
	}

	pub async fn execute(&self) -> Result<QueryResult> {
		let (sql, params) = self.build();
		self.backend.execute(&sql, params).await
	}
}

/// SELECT query builder
pub struct SelectBuilder {
	backend: Arc<dyn DatabaseBackend>,
	columns: Vec<String>,
	table: String,
	wheres: Vec<(String, String, QueryValue)>,
	limit: Option<i64>,
}

impl SelectBuilder {
	pub fn new(backend: Arc<dyn DatabaseBackend>) -> Self {
		Self {
			backend,
			columns: vec!["*".to_string()],
			table: String::new(),
			wheres: Vec::new(),
			limit: None,
		}
	}

	pub fn columns(mut self, columns: Vec<&str>) -> Self {
		self.columns = columns.iter().map(|s| s.to_string()).collect();
		self
	}

	pub fn from(mut self, table: impl Into<String>) -> Self {
		self.table = table.into();
		self
	}

	pub fn where_eq(mut self, column: impl Into<String>, value: impl Into<QueryValue>) -> Self {
		self.wheres
			.push((column.into(), "=".to_string(), value.into()));
		self
	}

	pub fn limit(mut self, limit: i64) -> Self {
		self.limit = Some(limit);
		self
	}

	pub fn build(&self) -> (String, Vec<QueryValue>) {
		use super::types::DatabaseType;
		use reinhardt_query::prelude::{
			MySqlQueryBuilder, PostgresQueryBuilder, QueryStatementBuilder, SqliteQueryBuilder,
		};

		let mut stmt = Query::select().from(Alias::new(&self.table)).to_owned();

		// Add columns
		if self.columns == vec!["*".to_string()] {
			stmt.column(ColumnRef::asterisk());
		} else {
			for col in &self.columns {
				stmt.column(Alias::new(col));
			}
		}

		// Add WHERE clauses
		for (col, op, val) in &self.wheres {
			if op == "=" {
				stmt.and_where(
					Expr::col(Alias::new(col)).eq(Expr::val(query_value_to_sea_value(val))),
				);
			}
		}

		// Add LIMIT (only apply non-negative values)
		if let Some(limit) = self.limit
			&& let Ok(limit_u64) = u64::try_from(limit)
		{
			stmt.limit(limit_u64);
		}

		// Build SQL with inline values
		let sql = match self.backend.database_type() {
			DatabaseType::Postgres => stmt.to_string(PostgresQueryBuilder),
			DatabaseType::Mysql => stmt.to_string(MySqlQueryBuilder),
			DatabaseType::Sqlite => stmt.to_string(SqliteQueryBuilder),
		};

		// Collect parameters
		let params: Vec<QueryValue> = self.wheres.iter().map(|(_, _, val)| val.clone()).collect();

		(sql, params)
	}

	pub async fn fetch_all(&self) -> Result<Vec<Row>> {
		let (sql, params) = self.build();
		self.backend.fetch_all(&sql, params).await
	}

	pub async fn fetch_one(&self) -> Result<Row> {
		let (sql, params) = self.build();
		self.backend.fetch_one(&sql, params).await
	}
}

/// DELETE query builder
pub struct DeleteBuilder {
	backend: Arc<dyn DatabaseBackend>,
	table: String,
	wheres: Vec<(String, String, QueryValue)>,
}

impl DeleteBuilder {
	pub fn new(backend: Arc<dyn DatabaseBackend>, table: impl Into<String>) -> Self {
		Self {
			backend,
			table: table.into(),
			wheres: Vec::new(),
		}
	}

	pub fn where_eq(mut self, column: impl Into<String>, value: impl Into<QueryValue>) -> Self {
		self.wheres
			.push((column.into(), "=".to_string(), value.into()));
		self
	}

	pub fn where_in(mut self, column: impl Into<String> + Clone, values: Vec<QueryValue>) -> Self {
		for value in values {
			self.wheres
				.push((column.clone().into(), "IN".to_string(), value));
		}
		self
	}

	pub fn build(&self) -> (String, Vec<QueryValue>) {
		use super::types::DatabaseType;
		use reinhardt_query::prelude::{
			MySqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder,
		};

		let mut stmt = Query::delete()
			.from_table(Alias::new(&self.table))
			.to_owned();

		// Add WHERE clauses
		for (col, op, val) in &self.wheres {
			match op.as_str() {
				"=" => {
					stmt.and_where(
						Expr::col(Alias::new(col)).eq(Expr::val(query_value_to_sea_value(val))),
					);
				}
				"IN" => {
					stmt.and_where(
						Expr::col(Alias::new(col))
							.is_in([Expr::val(query_value_to_sea_value(val))]),
					);
				}
				_ => {}
			}
		}

		// Build SQL
		let sql = match self.backend.database_type() {
			DatabaseType::Postgres => PostgresQueryBuilder.build_delete(&stmt).0,
			DatabaseType::Mysql => MySqlQueryBuilder.build_delete(&stmt).0,
			DatabaseType::Sqlite => SqliteQueryBuilder.build_delete(&stmt).0,
		};

		// Collect parameters
		let params: Vec<QueryValue> = self.wheres.iter().map(|(_, _, val)| val.clone()).collect();

		(sql, params)
	}

	pub async fn execute(&self) -> Result<QueryResult> {
		let (sql, params) = self.build();
		self.backend.execute(&sql, params).await
	}
}

/// ANALYZE statement builder for updating database statistics
///
/// The ANALYZE statement updates table statistics used by the query planner
/// to optimize query execution plans.
///
/// # Database Support
///
/// | Database | Syntax | Notes |
/// |----------|--------|-------|
/// | PostgreSQL | `ANALYZE [VERBOSE] [table [(columns...)]]` | Supports verbose mode and column-level analysis |
/// | MySQL | `ANALYZE TABLE table [, ...]` | Supports multiple tables |
/// | SQLite | `ANALYZE [table_or_index]` | Analyzes entire database if no target specified |
/// | CockroachDB | `ANALYZE table` | PostgreSQL-compatible syntax |
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_db::backends::AnalyzeBuilder;
///
/// // Analyze all tables
/// let builder = AnalyzeBuilder::new(backend.clone());
/// builder.execute().await?;
///
/// // Analyze specific table
/// let builder = AnalyzeBuilder::new(backend.clone())
///     .table("users");
/// builder.execute().await?;
///
/// // Analyze specific columns (PostgreSQL only)
/// let builder = AnalyzeBuilder::new(backend.clone())
///     .table("users")
///     .columns(vec!["email", "created_at"])
///     .verbose(true);
/// builder.execute().await?;
/// ```
pub struct AnalyzeBuilder {
	backend: Arc<dyn DatabaseBackend>,
	table: Option<String>,
	columns: Vec<String>,
	verbose: bool,
}

impl AnalyzeBuilder {
	/// Create a new ANALYZE builder
	///
	/// Without specifying a table, this will analyze all tables in the database.
	pub fn new(backend: Arc<dyn DatabaseBackend>) -> Self {
		Self {
			backend,
			table: None,
			columns: Vec::new(),
			verbose: false,
		}
	}

	/// Set the table to analyze
	///
	/// # Arguments
	///
	/// * `table` - The name of the table to analyze
	pub fn table(mut self, table: impl Into<String>) -> Self {
		self.table = Some(table.into());
		self
	}

	/// Set specific columns to analyze (PostgreSQL only)
	///
	/// This option is ignored on MySQL and SQLite as they don't support
	/// column-level ANALYZE.
	///
	/// # Arguments
	///
	/// * `columns` - List of column names to analyze
	pub fn columns(mut self, columns: Vec<&str>) -> Self {
		self.columns = columns.iter().map(|s| (*s).to_owned()).collect();
		self
	}

	/// Enable verbose output (PostgreSQL only)
	///
	/// When enabled, PostgreSQL will print progress messages as it analyzes.
	/// This option is ignored on MySQL and SQLite.
	pub fn verbose(mut self, verbose: bool) -> Self {
		self.verbose = verbose;
		self
	}

	/// Build the ANALYZE SQL statement
	///
	/// Returns the SQL string appropriate for the database backend.
	pub fn build(&self) -> String {
		use super::types::DatabaseType;

		match self.backend.database_type() {
			DatabaseType::Postgres => self.build_postgres(),
			DatabaseType::Mysql => self.build_mysql(),
			DatabaseType::Sqlite => self.build_sqlite(),
		}
	}

	fn build_postgres(&self) -> String {
		let mut sql = String::from("ANALYZE");

		if self.verbose {
			sql.push_str(" VERBOSE");
		}

		if let Some(ref table) = self.table {
			sql.push_str(&format!(" \"{}\"", table));

			if !self.columns.is_empty() {
				let cols = self
					.columns
					.iter()
					.map(|c| format!("\"{}\"", c))
					.collect::<Vec<_>>()
					.join(", ");
				sql.push_str(&format!(" ({})", cols));
			}
		}

		sql
	}

	fn build_mysql(&self) -> String {
		if let Some(ref table) = self.table {
			format!("ANALYZE TABLE `{}`", table)
		} else {
			// MySQL requires at least one table; analyze all tables requires
			// querying information_schema first. Return empty for database-wide.
			// Users should call with specific tables.
			String::from("ANALYZE TABLE")
		}
	}

	fn build_sqlite(&self) -> String {
		if let Some(ref table) = self.table {
			format!("ANALYZE \"{}\"", table)
		} else {
			// SQLite: ANALYZE without arguments analyzes the entire database
			String::from("ANALYZE")
		}
	}

	/// Execute the ANALYZE statement
	pub async fn execute(&self) -> Result<QueryResult> {
		let sql = self.build();
		self.backend.execute(&sql, Vec::new()).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::backends::backend::DatabaseBackend;
	use crate::backends::error::DatabaseError;
	use crate::backends::types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor};
	use rstest::rstest;

	// Mock transaction executor for testing
	struct MockTransactionExecutor;

	#[async_trait::async_trait]
	impl TransactionExecutor for MockTransactionExecutor {
		async fn execute(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			Ok(QueryResult { rows_affected: 0 })
		}

		async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			Ok(Row::new())
		}

		async fn fetch_all(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			Ok(Vec::new())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			Ok(None)
		}

		async fn commit(self: Box<Self>) -> Result<()> {
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> Result<()> {
			Ok(())
		}
	}

	struct MockBackend;

	#[async_trait::async_trait]
	impl DatabaseBackend for MockBackend {
		fn database_type(&self) -> DatabaseType {
			DatabaseType::Postgres
		}

		fn placeholder(&self, index: usize) -> String {
			format!("${}", index)
		}

		fn supports_returning(&self) -> bool {
			true
		}

		fn supports_on_conflict(&self) -> bool {
			true
		}

		async fn execute(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			Ok(QueryResult { rows_affected: 1 })
		}

		async fn fetch_one(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			Ok(Row::new())
		}

		async fn fetch_all(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			Ok(Vec::new())
		}

		async fn fetch_optional(
			&self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			Ok(None)
		}

		fn as_any(&self) -> &dyn std::any::Any {
			self
		}

		async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
			Ok(Box::new(MockTransactionExecutor))
		}
	}

	#[test]
	fn test_delete_builder_basic() {
		let backend = Arc::new(MockBackend);
		let builder = DeleteBuilder::new(backend, "users");
		let (sql, params) = builder.build();

		// reinhardt-query uses quotes for identifiers
		assert_eq!(sql, "DELETE FROM \"users\"");
		assert!(params.is_empty());
	}

	#[test]
	fn test_delete_builder_where_eq() {
		let backend = Arc::new(MockBackend);
		let builder = DeleteBuilder::new(backend, "users").where_eq("id", QueryValue::Int(1));
		let (sql, params) = builder.build();

		// reinhardt-query uses parameterized queries with placeholders
		assert_eq!(sql, "DELETE FROM \"users\" WHERE \"id\" = $1");
		assert_eq!(params.len(), 1);
		assert!(matches!(params[0], QueryValue::Int(1)));
	}

	#[test]
	fn test_delete_builder_where_in() {
		let backend = Arc::new(MockBackend);
		let builder = DeleteBuilder::new(backend, "users")
			.where_in("id", vec![QueryValue::Int(1), QueryValue::Int(2)]);
		let (sql, params) = builder.build();

		// reinhardt-query uses parameterized queries with placeholders
		assert_eq!(
			sql,
			"DELETE FROM \"users\" WHERE \"id\" IN ($1) AND \"id\" IN ($2)"
		);
		assert_eq!(params.len(), 2);
		assert!(matches!(params[0], QueryValue::Int(1)));
		assert!(matches!(params[1], QueryValue::Int(2)));
	}

	#[test]
	fn test_delete_builder_multiple_conditions() {
		let backend = Arc::new(MockBackend);
		let builder = DeleteBuilder::new(backend, "users")
			.where_eq("status", QueryValue::String("inactive".to_string()))
			.where_eq("age", QueryValue::Int(18));
		let (sql, params) = builder.build();

		// reinhardt-query uses parameterized queries with placeholders
		assert_eq!(
			sql,
			"DELETE FROM \"users\" WHERE \"status\" = $1 AND \"age\" = $2"
		);
		assert_eq!(params.len(), 2);
	}

	// Mock backends for different database types
	struct MockMysqlBackend;

	#[async_trait::async_trait]
	impl DatabaseBackend for MockMysqlBackend {
		fn database_type(&self) -> DatabaseType {
			DatabaseType::Mysql
		}
		fn placeholder(&self, index: usize) -> String {
			format!("?{}", index)
		}
		fn supports_returning(&self) -> bool {
			false
		}
		fn supports_on_conflict(&self) -> bool {
			false
		}
		async fn execute(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			Ok(QueryResult { rows_affected: 1 })
		}
		async fn fetch_one(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			Ok(Row::new())
		}
		async fn fetch_all(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			Ok(Vec::new())
		}
		async fn fetch_optional(
			&self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			Ok(None)
		}
		fn as_any(&self) -> &dyn std::any::Any {
			self
		}
		async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
			Ok(Box::new(MockTransactionExecutor))
		}
	}

	struct MockSqliteBackend;

	#[async_trait::async_trait]
	impl DatabaseBackend for MockSqliteBackend {
		fn database_type(&self) -> DatabaseType {
			DatabaseType::Sqlite
		}
		fn placeholder(&self, index: usize) -> String {
			format!("?{}", index)
		}
		fn supports_returning(&self) -> bool {
			true
		}
		fn supports_on_conflict(&self) -> bool {
			true
		}
		async fn execute(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			Ok(QueryResult { rows_affected: 1 })
		}
		async fn fetch_one(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			Ok(Row::new())
		}
		async fn fetch_all(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			Ok(Vec::new())
		}
		async fn fetch_optional(
			&self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			Ok(None)
		}
		fn as_any(&self) -> &dyn std::any::Any {
			self
		}
		async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
			Ok(Box::new(MockTransactionExecutor))
		}
	}

	// Tests for OnConflictClause (new fluent API)

	// ==========================================
	// PostgreSQL Tests - Exact SQL Verification
	// ==========================================

	#[test]
	fn test_on_conflict_clause_columns_do_nothing_postgres_exact_sql() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_nothing());
		let (sql, params) = builder.build().unwrap();

		// Assert - verify exact SQL structure (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\") VALUES ($1) ON CONFLICT (email) DO NOTHING"
		);
		assert_eq!(params.len(), 1);
		assert!(matches!(&params[0], QueryValue::String(s) if s == "test@example.com"));
	}

	#[test]
	fn test_on_conflict_clause_columns_do_update_postgres_exact_sql() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.value("name", QueryValue::String("Test User".to_string()))
			.on_conflict(
				OnConflictClause::columns(vec!["email"]).do_update(vec!["name", "updated_at"]),
			);
		let (sql, params) = builder.build().unwrap();

		// Assert - verify exact SQL structure with EXCLUDED references (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\", \"name\") VALUES ($1, $2) ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name, updated_at = EXCLUDED.updated_at"
		);
		assert_eq!(params.len(), 2);
	}

	#[test]
	fn test_on_conflict_clause_with_where_postgres_exact_sql() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.value("version", QueryValue::Int(2))
			.on_conflict(
				OnConflictClause::columns(vec!["email"])
					.do_update(vec!["version"])
					.where_clause("users.version < EXCLUDED.version"),
			);
		let (sql, params) = builder.build().unwrap();

		// Assert - verify WHERE clause is appended correctly (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\", \"version\") VALUES ($1, $2) ON CONFLICT (email) DO UPDATE SET version = EXCLUDED.version WHERE users.version < EXCLUDED.version"
		);
		assert_eq!(params.len(), 2);
	}

	#[test]
	fn test_on_conflict_clause_constraint_postgres_exact_sql() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::constraint("users_email_key").do_update(vec!["name"]));
		let (sql, _) = builder.build().unwrap();

		// Assert - verify ON CONSTRAINT syntax (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\") VALUES ($1) ON CONFLICT ON CONSTRAINT users_email_key DO UPDATE SET name = EXCLUDED.name"
		);
	}

	#[test]
	fn test_on_conflict_clause_any_do_nothing_postgres_exact_sql() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::any().do_nothing());
		let (sql, _) = builder.build().unwrap();

		// Assert - verify no conflict target specified (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\") VALUES ($1) ON CONFLICT DO NOTHING"
		);
	}

	#[test]
	fn test_on_conflict_clause_multiple_columns_postgres_exact_sql() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("tenant_id", QueryValue::Int(1))
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(
				OnConflictClause::columns(vec!["tenant_id", "email"]).do_update(vec!["name"]),
			);
		let (sql, _) = builder.build().unwrap();

		// Assert - verify multiple conflict columns (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"tenant_id\", \"email\") VALUES ($1, $2) ON CONFLICT (tenant_id, email) DO UPDATE SET name = EXCLUDED.name"
		);
	}

	#[test]
	fn test_on_conflict_clause_multiple_update_columns_postgres() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("id", QueryValue::Int(1))
			.on_conflict(OnConflictClause::columns(vec!["id"]).do_update(vec![
				"name",
				"email",
				"updated_at",
				"version",
			]));
		let (sql, _) = builder.build().unwrap();

		// Assert - verify all update columns are included (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"id\") VALUES ($1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, email = EXCLUDED.email, updated_at = EXCLUDED.updated_at, version = EXCLUDED.version"
		);
	}

	// ==========================================
	// MySQL Tests - Exact SQL Verification
	// ==========================================

	#[test]
	fn test_on_conflict_clause_do_nothing_mysql_exact_sql() {
		// Arrange
		let backend = Arc::new(MockMysqlBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_nothing());
		let (sql, _) = builder.build().unwrap();

		// Assert - MySQL uses INSERT IGNORE syntax (reinhardt-query uses parameterized queries)
		assert_eq!(sql, "INSERT IGNORE INTO `users` (`email`) VALUES (?)");
	}

	#[test]
	fn test_on_conflict_clause_do_update_mysql_exact_sql() {
		// Arrange
		let backend = Arc::new(MockMysqlBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_update(vec!["name"]));
		let (sql, _) = builder.build().unwrap();

		// Assert - MySQL uses ON DUPLICATE KEY UPDATE with VALUES() function (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO `users` (`email`) VALUES (?) ON DUPLICATE KEY UPDATE name = VALUES(name)"
		);
	}

	#[test]
	fn test_on_conflict_clause_do_update_multiple_columns_mysql() {
		// Arrange
		let backend = Arc::new(MockMysqlBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("id", QueryValue::Int(1))
			.on_conflict(OnConflictClause::columns(vec!["id"]).do_update(vec![
				"name",
				"email",
				"updated_at",
			]));
		let (sql, _) = builder.build().unwrap();

		// Assert - verify multiple update columns with VALUES() syntax (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO `users` (`id`) VALUES (?) ON DUPLICATE KEY UPDATE name = VALUES(name), email = VALUES(email), updated_at = VALUES(updated_at)"
		);
	}

	#[test]
	fn test_on_conflict_clause_where_ignored_mysql() {
		// Arrange
		let backend = Arc::new(MockMysqlBackend);

		// Act - MySQL does not support WHERE clause, but should not error
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(
				OnConflictClause::columns(vec!["email"])
					.do_update(vec!["name"])
					.where_clause("users.version < VALUES(version)"),
			);
		let (sql, _) = builder.build().unwrap();

		// Assert - WHERE clause is ignored for MySQL (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO `users` (`email`) VALUES (?) ON DUPLICATE KEY UPDATE name = VALUES(name)"
		);
		assert!(!sql.contains("WHERE"));
	}

	// ==========================================
	// SQLite Tests - Exact SQL Verification
	// ==========================================

	#[test]
	fn test_on_conflict_clause_do_nothing_sqlite_exact_sql() {
		// Arrange
		let backend = Arc::new(MockSqliteBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_nothing());
		let (sql, _) = builder.build().unwrap();

		// Assert - SQLite uses INSERT OR IGNORE syntax (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT OR IGNORE INTO \"users\" (\"email\") VALUES (?)"
		);
	}

	#[test]
	fn test_on_conflict_clause_do_update_sqlite_exact_sql() {
		// Arrange
		let backend = Arc::new(MockSqliteBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_update(vec!["name"]));
		let (sql, _) = builder.build().unwrap();

		// Assert - SQLite uses lowercase 'excluded' pseudo-table (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\") VALUES (?) ON CONFLICT (email) DO UPDATE SET name = excluded.name"
		);
	}

	#[test]
	fn test_on_conflict_clause_with_where_sqlite_exact_sql() {
		// Arrange
		let backend = Arc::new(MockSqliteBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(
				OnConflictClause::columns(vec!["email"])
					.do_update(vec!["version"])
					.where_clause("users.version < excluded.version"),
			);
		let (sql, _) = builder.build().unwrap();

		// Assert - SQLite supports WHERE clause (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\") VALUES (?) ON CONFLICT (email) DO UPDATE SET version = excluded.version WHERE users.version < excluded.version"
		);
	}

	#[test]
	fn test_on_conflict_clause_multiple_columns_sqlite() {
		// Arrange
		let backend = Arc::new(MockSqliteBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("tenant_id", QueryValue::Int(1))
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(
				OnConflictClause::columns(vec!["tenant_id", "email"]).do_update(vec!["name"]),
			);
		let (sql, _) = builder.build().unwrap();

		// Assert - verify multiple conflict columns (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"tenant_id\", \"email\") VALUES (?, ?) ON CONFLICT (tenant_id, email) DO UPDATE SET name = excluded.name"
		);
	}

	#[test]
	fn test_on_conflict_clause_any_do_nothing_sqlite() {
		// Arrange
		let backend = Arc::new(MockSqliteBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::any().do_nothing());
		let (sql, _) = builder.build().unwrap();

		// Assert - SQLite uses INSERT OR IGNORE even with any() target (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT OR IGNORE INTO \"users\" (\"email\") VALUES (?)"
		);
	}

	// ==========================================
	// Legacy API Tests - Backwards Compatibility
	// ==========================================

	#[test]
	fn test_legacy_on_conflict_do_nothing_still_works() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act - using legacy API
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict_do_nothing(Some(vec!["email".to_string()]));
		let (sql, _) = builder.build().unwrap();

		// Assert - legacy API should still work (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\") VALUES ($1) ON CONFLICT (email) DO NOTHING"
		);
	}

	#[test]
	fn test_legacy_on_conflict_do_update_still_works() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act - using legacy API
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict_do_update(
				Some(vec!["email".to_string()]),
				vec!["name".to_string(), "updated_at".to_string()],
			);
		let (sql, _) = builder.build().unwrap();

		// Assert - legacy API should still work (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"email\") VALUES ($1) ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name, updated_at = EXCLUDED.updated_at"
		);
	}

	#[test]
	fn test_new_api_takes_precedence_over_legacy() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act - both APIs used, new should take precedence
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict_do_nothing(Some(vec!["email".to_string()])) // legacy
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_update(vec!["name"])); // new
		let (sql, _) = builder.build().unwrap();

		// Assert - new API should be used (DO UPDATE, not DO NOTHING)
		assert!(sql.contains("DO UPDATE SET"));
		assert!(!sql.contains("DO NOTHING"));
	}

	// ==========================================
	// Edge Cases and Error Conditions
	// ==========================================

	#[test]
	fn test_on_conflict_clause_single_column_single_update() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("id", QueryValue::Int(1))
			.on_conflict(OnConflictClause::columns(vec!["id"]).do_update(vec!["name"]));
		let (sql, _) = builder.build().unwrap();

		// Assert (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"id\") VALUES ($1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name"
		);
	}

	#[test]
	fn test_on_conflict_clause_with_returning() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.returning(vec!["id", "created_at"])
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_update(vec!["name"]));
		let (sql, _) = builder.build().unwrap();

		// Assert - RETURNING should come before ON CONFLICT in reinhardt-query output
		assert!(sql.contains("RETURNING"));
		assert!(sql.contains("ON CONFLICT"));
	}

	#[test]
	fn test_on_conflict_clause_with_null_value() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.value("name", QueryValue::Null)
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_update(vec!["name"]));
		let (sql, params) = builder.build().unwrap();

		// Assert
		assert!(sql.contains("ON CONFLICT (email) DO UPDATE SET"));
		assert_eq!(params.len(), 2);
		assert!(matches!(params[1], QueryValue::Null));
	}

	#[test]
	fn test_on_conflict_clause_with_integer_values() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "counters")
			.value("key", QueryValue::String("visits".to_string()))
			.value("count", QueryValue::Int(1))
			.on_conflict(OnConflictClause::columns(vec!["key"]).do_update(vec!["count"]));
		let (sql, params) = builder.build().unwrap();

		// Assert (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"counters\" (\"key\", \"count\") VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET count = EXCLUDED.count"
		);
		assert_eq!(params.len(), 2);
		assert!(matches!(params[1], QueryValue::Int(1)));
	}

	#[test]
	fn test_on_conflict_clause_complex_where_condition() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "documents")
			.value("id", QueryValue::Int(1))
			.on_conflict(
				OnConflictClause::columns(vec!["id"])
					.do_update(vec!["content", "version"])
					.where_clause(
						"documents.version < EXCLUDED.version AND documents.locked = false",
					),
			);
		let (sql, _) = builder.build().unwrap();

		// Assert - complex WHERE with AND condition (reinhardt-query uses parameterized queries)
		assert_eq!(
			sql,
			"INSERT INTO \"documents\" (\"id\") VALUES ($1) ON CONFLICT (id) DO UPDATE SET content = EXCLUDED.content, version = EXCLUDED.version WHERE documents.version < EXCLUDED.version AND documents.locked = false"
		);
	}

	// ==========================================
	// Fluent API Chain Tests
	// ==========================================

	#[test]
	fn test_fluent_api_do_nothing_then_do_update_uses_last() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act - chain do_nothing then do_update
		let clause = OnConflictClause::columns(vec!["email"])
			.do_nothing()
			.do_update(vec!["name"]);
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(clause);
		let (sql, _) = builder.build().unwrap();

		// Assert - last action (do_update) should be used
		assert!(sql.contains("DO UPDATE SET"));
		assert!(!sql.contains("DO NOTHING"));
	}

	#[test]
	fn test_fluent_api_do_update_then_do_nothing_uses_last() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act - chain do_update then do_nothing
		let clause = OnConflictClause::columns(vec!["email"])
			.do_update(vec!["name"])
			.do_nothing();
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(clause);
		let (sql, _) = builder.build().unwrap();

		// Assert - last action (do_nothing) should be used
		assert!(sql.contains("DO NOTHING"));
		assert!(!sql.contains("DO UPDATE"));
	}

	#[test]
	fn test_conflict_target_types() {
		// Act - test ConflictTarget::Columns
		let columns_clause = OnConflictClause::columns(vec!["a", "b"]);
		assert!(matches!(
			columns_clause.target,
			Some(ConflictTarget::Columns(ref cols)) if cols == &vec!["a".to_string(), "b".to_string()]
		));

		// Act - test ConflictTarget::Constraint
		let constraint_clause = OnConflictClause::constraint("my_constraint");
		assert!(matches!(
			constraint_clause.target,
			Some(ConflictTarget::Constraint(ref name)) if name == "my_constraint"
		));

		// Act - test no target (any)
		let any_clause = OnConflictClause::any();
		assert!(any_clause.target.is_none());

		// Verify they all build correctly with separate backends
		let backend1: Arc<dyn DatabaseBackend> = Arc::new(MockBackend);
		let builder1 = InsertBuilder::new(backend1, "t")
			.value("x", QueryValue::Int(1))
			.on_conflict(columns_clause.do_nothing());
		let (sql1, _) = builder1.build().unwrap();
		assert!(sql1.contains("ON CONFLICT (a, b) DO NOTHING"));

		let backend2: Arc<dyn DatabaseBackend> = Arc::new(MockBackend);
		let builder2 = InsertBuilder::new(backend2, "t")
			.value("x", QueryValue::Int(1))
			.on_conflict(constraint_clause.do_nothing());
		let (sql2, _) = builder2.build().unwrap();
		assert!(sql2.contains("ON CONFLICT ON CONSTRAINT my_constraint DO NOTHING"));

		let backend3: Arc<dyn DatabaseBackend> = Arc::new(MockBackend);
		let builder3 = InsertBuilder::new(backend3, "t")
			.value("x", QueryValue::Int(1))
			.on_conflict(any_clause.do_nothing());
		let (sql3, _) = builder3.build().unwrap();
		assert!(sql3.contains("ON CONFLICT DO NOTHING"));
	}

	// ==========================================
	// Parameter Preservation Tests
	// ==========================================

	#[test]
	fn test_parameters_preserved_with_on_conflict() {
		// Arrange
		let backend = Arc::new(MockBackend);

		// Act
		let builder = InsertBuilder::new(backend, "users")
			.value("id", QueryValue::Int(42))
			.value("name", QueryValue::String("John".to_string()))
			.value("active", QueryValue::Bool(true))
			.on_conflict(OnConflictClause::columns(vec!["id"]).do_update(vec!["name", "active"]));
		let (_, params) = builder.build().unwrap();

		// Assert - all parameters should be preserved in order
		assert_eq!(params.len(), 3);
		assert!(matches!(params[0], QueryValue::Int(42)));
		assert!(matches!(&params[1], QueryValue::String(s) if s == "John"));
		assert!(matches!(params[2], QueryValue::Bool(true)));
	}

	// ==========================================
	// ANALYZE Builder Tests
	// ==========================================

	#[test]
	fn test_analyze_builder_postgres_database_wide() {
		let backend = Arc::new(MockBackend);
		let builder = AnalyzeBuilder::new(backend);
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE");
	}

	#[test]
	fn test_analyze_builder_postgres_specific_table() {
		let backend = Arc::new(MockBackend);
		let builder = AnalyzeBuilder::new(backend).table("users");
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE \"users\"");
	}

	#[test]
	fn test_analyze_builder_postgres_verbose() {
		let backend = Arc::new(MockBackend);
		let builder = AnalyzeBuilder::new(backend).table("users").verbose(true);
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE VERBOSE \"users\"");
	}

	#[test]
	fn test_analyze_builder_postgres_with_columns() {
		let backend = Arc::new(MockBackend);
		let builder = AnalyzeBuilder::new(backend)
			.table("users")
			.columns(vec!["email", "created_at"]);
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE \"users\" (\"email\", \"created_at\")");
	}

	#[test]
	fn test_analyze_builder_postgres_verbose_with_columns() {
		let backend = Arc::new(MockBackend);
		let builder = AnalyzeBuilder::new(backend)
			.table("users")
			.columns(vec!["email"])
			.verbose(true);
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE VERBOSE \"users\" (\"email\")");
	}

	#[test]
	fn test_analyze_builder_mysql_specific_table() {
		let backend = Arc::new(MockMysqlBackend);
		let builder = AnalyzeBuilder::new(backend).table("users");
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE TABLE `users`");
	}

	#[test]
	fn test_analyze_builder_mysql_database_wide() {
		let backend = Arc::new(MockMysqlBackend);
		let builder = AnalyzeBuilder::new(backend);
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE TABLE");
	}

	#[test]
	fn test_analyze_builder_sqlite_database_wide() {
		let backend = Arc::new(MockSqliteBackend);
		let builder = AnalyzeBuilder::new(backend);
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE");
	}

	#[test]
	fn test_analyze_builder_sqlite_specific_table() {
		let backend = Arc::new(MockSqliteBackend);
		let builder = AnalyzeBuilder::new(backend).table("users");
		let sql = builder.build();
		assert_eq!(sql, "ANALYZE \"users\"");
	}

	// ==========================================
	// INSERT FROM SELECT Tests
	// ==========================================

	#[test]
	fn test_insert_from_select_basic_postgres() {
		// Arrange
		let backend = Arc::new(MockBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "target_table", vec!["id", "name"], select);
		let (sql, params) = builder.build();

		// Assert
		assert_eq!(
			sql,
			"INSERT INTO \"target_table\" (\"id\", \"name\") SELECT \"id\", \"name\" FROM \"source_table\""
		);
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_from_select_with_where_clause() {
		// Arrange
		let backend = Arc::new(MockBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("users"))
			.and_where(Expr::col(Alias::new("status")).eq("inactive"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "archived_users", vec!["id", "name"], select);
		let (sql, params) = builder.build();

		// Assert (inline values in SQL, no parameters)
		assert_eq!(
			sql,
			"INSERT INTO \"archived_users\" (\"id\", \"name\") SELECT \"id\", \"name\" FROM \"users\" WHERE \"status\" = 'inactive'"
		);
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_from_select_with_returning() {
		// Arrange
		let backend = Arc::new(MockBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "target_table", vec!["id", "name"], select)
				.returning(vec!["id"]);
		let (sql, params) = builder.build();

		// Assert
		assert!(sql.contains("RETURNING"));
		assert!(sql.contains("\"id\""));
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_from_select_on_conflict_do_nothing_postgres() {
		// Arrange
		let backend = Arc::new(MockBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "target_table", vec!["id", "name"], select)
				.on_conflict_do_nothing(Some(vec!["id".to_string()]));
		let (sql, params) = builder.build();

		// Assert
		assert!(sql.contains("ON CONFLICT (id) DO NOTHING"));
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_from_select_on_conflict_do_update_postgres() {
		// Arrange
		let backend = Arc::new(MockBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "target_table", vec!["id", "name"], select)
				.on_conflict_do_update(Some(vec!["id".to_string()]), vec!["name".to_string()]);
		let (sql, params) = builder.build();

		// Assert
		assert!(sql.contains("ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name"));
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_from_select_mysql() {
		// Arrange
		let backend = Arc::new(MockMysqlBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "target_table", vec!["id", "name"], select);
		let (sql, params) = builder.build();

		// Assert
		assert_eq!(
			sql,
			"INSERT INTO `target_table` (`id`, `name`) SELECT `id`, `name` FROM `source_table`"
		);
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_from_select_mysql_ignore() {
		// Arrange
		let backend = Arc::new(MockMysqlBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "target_table", vec!["id", "name"], select)
				.on_conflict_do_nothing(None);
		let (sql, params) = builder.build();

		// Assert
		assert!(sql.starts_with("INSERT IGNORE INTO"));
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_from_select_sqlite() {
		// Arrange
		let backend = Arc::new(MockSqliteBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "target_table", vec!["id", "name"], select);
		let (sql, params) = builder.build();

		// Assert
		assert_eq!(
			sql,
			"INSERT INTO \"target_table\" (\"id\", \"name\") SELECT \"id\", \"name\" FROM \"source_table\""
		);
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_from_select_sqlite_or_ignore() {
		// Arrange
		let backend = Arc::new(MockSqliteBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertFromSelectBuilder::new(backend, "target_table", vec!["id", "name"], select)
				.on_conflict_do_nothing(None);
		let (sql, params) = builder.build();

		// Assert
		assert!(sql.starts_with("INSERT OR IGNORE INTO"));
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_builder_from_select_conversion() {
		// Arrange
		let backend = Arc::new(MockBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder =
			InsertBuilder::new(backend, "target_table").from_select(vec!["id", "name"], select);
		let (sql, params) = builder.build();

		// Assert
		assert_eq!(
			sql,
			"INSERT INTO \"target_table\" (\"id\", \"name\") SELECT \"id\", \"name\" FROM \"source_table\""
		);
		assert!(params.is_empty());
	}

	#[test]
	fn test_insert_builder_from_select_preserves_returning() {
		// Arrange
		let backend = Arc::new(MockBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder = InsertBuilder::new(backend, "target_table")
			.returning(vec!["id"])
			.from_select(vec!["id", "name"], select);
		let (sql, _) = builder.build();

		// Assert
		assert!(sql.contains("RETURNING"));
	}

	#[test]
	fn test_insert_builder_from_select_preserves_on_conflict() {
		// Arrange
		let backend = Arc::new(MockBackend);
		let select = Query::select()
			.column(Alias::new("id"))
			.column(Alias::new("name"))
			.from(Alias::new("source_table"))
			.to_owned();

		// Act
		let builder = InsertBuilder::new(backend, "target_table")
			.on_conflict_do_nothing(Some(vec!["id".to_string()]))
			.from_select(vec!["id", "name"], select);
		let (sql, _) = builder.build();

		// Assert
		assert!(sql.contains("ON CONFLICT (id) DO NOTHING"));
	}

	// ==========================================
	// Error Handling Tests - Panics Replaced with Result Errors
	// ==========================================

	#[rstest]
	#[test]
	fn test_sqlite_empty_conflict_columns_returns_error() {
		// Arrange
		let backend: Arc<dyn DatabaseBackend> = Arc::new(MockSqliteBackend);
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict_do_update(
				Some(vec![]), // empty conflict columns
				vec!["name".to_string()],
			);

		// Act
		let result = builder.build();

		// Assert: Should return error instead of panicking
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, DatabaseError::SyntaxError(_)));
	}

	#[rstest]
	#[test]
	fn test_sqlite_empty_update_columns_returns_error() {
		// Arrange
		let backend: Arc<dyn DatabaseBackend> = Arc::new(MockSqliteBackend);
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict_do_update(
				Some(vec!["email".to_string()]),
				vec![], // empty update columns
			);

		// Act
		let result = builder.build();

		// Assert: Should return error instead of panicking
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, DatabaseError::SyntaxError(_)));
	}

	#[rstest]
	#[test]
	fn test_sqlite_constraint_target_returns_error() {
		// Arrange
		let backend: Arc<dyn DatabaseBackend> = Arc::new(MockSqliteBackend);
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::constraint("users_email_key").do_update(vec!["name"]));

		// Act
		let result = builder.build();

		// Assert: Should return NotSupported error instead of panicking
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, DatabaseError::NotSupported(_)));
	}

	#[rstest]
	#[test]
	fn test_sqlite_empty_conflict_columns_new_api_returns_error() {
		// Arrange
		let backend: Arc<dyn DatabaseBackend> = Arc::new(MockSqliteBackend);
		let empty_cols: Vec<String> = vec![];
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::columns(empty_cols).do_update(vec!["name"]));

		// Act
		let result = builder.build();

		// Assert: Should return error instead of panicking
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, DatabaseError::SyntaxError(_)));
	}

	#[rstest]
	#[test]
	fn test_sqlite_empty_update_columns_new_api_returns_error() {
		// Arrange
		let backend: Arc<dyn DatabaseBackend> = Arc::new(MockSqliteBackend);
		let empty_update: Vec<String> = vec![];
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_update(empty_update));

		// Act
		let result = builder.build();

		// Assert: Should return error instead of panicking
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, DatabaseError::SyntaxError(_)));
	}

	#[rstest]
	#[test]
	fn test_postgres_build_succeeds_with_valid_input() {
		// Arrange
		let backend: Arc<dyn DatabaseBackend> = Arc::new(MockBackend);
		let builder = InsertBuilder::new(backend, "users")
			.value("email", QueryValue::String("test@example.com".to_string()))
			.on_conflict(OnConflictClause::columns(vec!["email"]).do_update(vec!["name"]));

		// Act
		let result = builder.build();

		// Assert: Should succeed for PostgreSQL
		assert!(result.is_ok());
	}
}
