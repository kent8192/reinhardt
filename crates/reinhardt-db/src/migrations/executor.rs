//! Migration executor
//!
//! Translated from Django's db/migrations/executor.py

// Allow unused_imports: ForeignKeyAction is used in database-specific code
// that may be conditionally compiled based on feature flags
#[allow(unused_imports)]
use super::{
	DatabaseMigrationRecorder, ForeignKeyAction, Migration, MigrationError, MigrationPlan,
	MigrationService, MigrationSqlPlan, Operation, PlannedStatement, ProjectState, Result,
	SchemaEditor,
	operations::SqlDialect,
	sql_plan::{
		MigrationDirection, migration_requires_sqlite_recreation, plan_migration_sql_for_execution,
		split_sql_statements,
	},
};
use crate::backends::{connection::DatabaseConnection, types::DatabaseType};
use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[cfg(feature = "sqlite")]
use super::introspection::SQLiteIntrospector;

fn planned_operation_context(
	operation: Option<&Operation>,
) -> Option<crate::backends::error::PgvectorOperationKind> {
	operation.and_then(Operation::pgvector_operation_kind)
}

#[cfg(feature = "sqlite")]
fn can_execute_sqlite_recreation_sequentially(error: &MigrationError) -> bool {
	matches!(
		error,
		MigrationError::InvalidMigration(message)
			if message.starts_with("cannot safely plan SQLite recreation after opaque ")
	)
}

fn validate_and_advance_migration_state(
	migration: &Migration,
	state: &mut super::ProjectState,
) -> Result<()> {
	for operation in &migration.operations {
		operation.validate_for_partial_state(state)?;
		operation.state_forwards(&migration.app_label, state);
	}
	Ok(())
}

fn migration_is_atomic(migration: &Migration, database_type: DatabaseType) -> bool {
	migration.atomic
		&& !(database_type == DatabaseType::Postgres
			&& migration
				.operations
				.iter()
				.any(Operation::creates_index_concurrently))
}

fn replacement_history_is_fully_covered(
	migration: &Migration,
	migrations: &[Migration],
	applied_records: &[(String, String)],
) -> bool {
	if migration.replaces.is_empty() {
		return false;
	}
	let mut covered: HashSet<(&str, &str)> = applied_records
		.iter()
		.map(|(app, name)| (app.as_str(), name.as_str()))
		.collect();
	loop {
		let covered_before = covered.len();
		for known in migrations {
			if covered.contains(&(known.app_label.as_str(), known.name.as_str())) {
				covered.extend(
					known
						.replaces
						.iter()
						.map(|(app, name)| (app.as_str(), name.as_str())),
				);
			}
		}
		for known in migrations {
			if !known.replaces.is_empty()
				&& known
					.replaces
					.iter()
					.all(|(app, name)| covered.contains(&(app.as_str(), name.as_str())))
			{
				covered.insert((known.app_label.as_str(), known.name.as_str()));
			}
		}
		if covered.len() == covered_before {
			break;
		}
	}
	migration
		.replaces
		.iter()
		.all(|(app, name)| covered.contains(&(app.as_str(), name.as_str())))
}

#[cfg(test)]
mod replacement_history_tests {
	use super::*;

	#[test]
	fn nested_replacement_is_covered_by_its_fully_applied_ancestor_history() {
		let original_one = Migration::new("0001_initial", "app");
		let original_two = Migration::new("0002_add_field", "app");
		let mut first_squash = Migration::new("0001_squashed_0002", "app");
		first_squash.replaces = vec![
			("app".to_string(), "0001_initial".to_string()),
			("app".to_string(), "0002_add_field".to_string()),
		];
		let mut second_squash = Migration::new("0001_squashed_0002_v2", "app");
		second_squash.replaces = vec![("app".to_string(), "0001_squashed_0002".to_string())];
		let applied_records = vec![
			("app".to_string(), "0001_initial".to_string()),
			("app".to_string(), "0002_add_field".to_string()),
		];

		assert!(replacement_history_is_fully_covered(
			&second_squash,
			&[
				original_one,
				original_two,
				first_squash,
				second_squash.clone()
			],
			&applied_records,
		));
	}
}

#[derive(Debug)]
/// Represents a execution result.
pub struct ExecutionResult {
	/// The applied.
	pub applied: Vec<String>,
	/// The failed.
	pub failed: Option<String>,
}

/// A replacement-aware subset of migrations selected for execution or display.
///
/// Replacement migrations supersede their replaced migrations on a fresh
/// database. When every replaced migration is already recorded, the
/// replacement is selected for recorder adoption instead. A partially applied
/// replacement set is invalid because neither history can be chosen safely.
#[derive(Debug)]
pub struct ReplacementMigrationSelection<'a> {
	migrations: Vec<&'a Migration>,
	replacements_to_adopt: Vec<&'a Migration>,
}

impl<'a> ReplacementMigrationSelection<'a> {
	/// Return migrations that remain after replacement selection.
	pub fn migrations(&self) -> &[&'a Migration] {
		&self.migrations
	}

	/// Return replacements whose complete original history must be adopted.
	pub fn replacements_to_adopt(&self) -> &[&'a Migration] {
		&self.replacements_to_adopt
	}
}

/// Select one side of each replacement set from the supplied migration list.
///
/// This function is shared by the executor and command previews so `--plan`,
/// `--fake`, and real execution report the same migration history.
pub fn select_replacement_migrations<'a>(
	migrations: &'a [Migration],
	applied: &HashSet<super::graph::MigrationKey>,
) -> Result<ReplacementMigrationSelection<'a>> {
	let mut excluded = HashSet::new();
	let mut replacements_to_adopt = Vec::new();
	let mut covered_by_applied_history = applied.clone();
	loop {
		let mut changed = false;
		for migration in migrations {
			let key = super::graph::MigrationKey::new(&migration.app_label, &migration.name);
			if covered_by_applied_history.contains(&key) {
				for (app, name) in &migration.replaces {
					changed |= covered_by_applied_history
						.insert(super::graph::MigrationKey::new(app, name));
				}
			}
		}
		if !changed {
			break;
		}
	}

	for replacement in migrations
		.iter()
		.filter(|migration| !migration.replaces.is_empty())
	{
		let replacement_key = super::graph::MigrationKey::new(
			replacement.app_label.clone(),
			replacement.name.clone(),
		);
		let replacement_is_applied = applied.contains(&replacement_key);
		let replaced_applied = replacement
			.replaces
			.iter()
			.filter(|(app, name)| {
				covered_by_applied_history.contains(&super::graph::MigrationKey::new(app, name))
			})
			.count();

		if replacement_is_applied || replaced_applied == 0 {
			excluded.extend(
				replacement
					.replaces
					.iter()
					.map(|(app, name)| super::graph::MigrationKey::new(app.clone(), name.clone())),
			);
		} else if replaced_applied == replacement.replaces.len() {
			replacements_to_adopt.push(replacement);
			excluded.extend(
				replacement
					.replaces
					.iter()
					.map(|(app, name)| super::graph::MigrationKey::new(app.clone(), name.clone())),
			);
			excluded.insert(replacement_key);
		} else {
			return Err(MigrationError::InvalidMigration(format!(
				"Cannot apply replacement {} with a partially applied replacement set",
				replacement.id()
			)));
		}
	}

	let migrations = migrations
		.iter()
		.filter(|migration| {
			!excluded.contains(&super::graph::MigrationKey::new(
				migration.app_label.clone(),
				migration.name.clone(),
			))
		})
		.collect();

	Ok(ReplacementMigrationSelection {
		migrations,
		replacements_to_adopt,
	})
}

#[async_trait]
trait SqlPlanner: Send + Sync {
	async fn plan(
		&self,
		connection: &DatabaseConnection,
		migration: &Migration,
		state: &ProjectState,
		direction: MigrationDirection,
		editor: &mut SchemaEditor,
	) -> Result<MigrationSqlPlan>;
}

struct DefaultSqlPlanner;

#[async_trait]
impl SqlPlanner for DefaultSqlPlanner {
	async fn plan(
		&self,
		connection: &DatabaseConnection,
		migration: &Migration,
		state: &ProjectState,
		direction: MigrationDirection,
		editor: &mut SchemaEditor,
	) -> Result<MigrationSqlPlan> {
		plan_migration_sql_for_execution(connection, migration, state, direction, editor).await
	}
}

/// Migration executor using DatabaseConnection (supports multiple database types)
pub struct DatabaseMigrationExecutor {
	connection: DatabaseConnection,
	recorder: DatabaseMigrationRecorder,
	db_type: DatabaseType,
	planner: Arc<dyn SqlPlanner>,
}

impl DatabaseMigrationExecutor {
	/// Create a new migration executor with DatabaseConnection
	///
	/// The database type is automatically detected from the connection.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
	/// // Example: connecting to a PostgreSQL database
	/// let db = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let executor = DatabaseMigrationExecutor::new(db.clone());
	/// // Database type is automatically detected as PostgreSQL
	/// # });
	/// ```
	pub fn new(connection: DatabaseConnection) -> Self {
		let db_type = connection.database_type();
		let recorder = DatabaseMigrationRecorder::new(connection.clone());
		Self {
			connection,
			recorder,
			db_type,
			planner: Arc::new(DefaultSqlPlanner),
		}
	}

	#[cfg(test)]
	fn new_with_planner(connection: DatabaseConnection, planner: Arc<dyn SqlPlanner>) -> Self {
		let db_type = connection.database_type();
		let recorder = DatabaseMigrationRecorder::new(connection.clone());
		Self {
			connection,
			recorder,
			db_type,
			planner,
		}
	}

	/// Get a reference to the database connection
	pub fn connection(&self) -> &DatabaseConnection {
		&self.connection
	}

	/// Get the database type
	pub fn database_type(&self) -> DatabaseType {
		self.db_type
	}

	/// Performs the apply migrations operation.
	pub async fn apply_migrations(&mut self, migrations: &[Migration]) -> Result<ExecutionResult> {
		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self
				.apply_migrations_with_cockroachdb_schema_lock(migrations)
				.await;
		}

		// Ensure the migration recorder table exists
		self.recorder.ensure_schema_table().await?;
		self.apply_migrations_after_schema_table(migrations).await
	}

	#[cfg(feature = "postgres")]
	async fn apply_migrations_with_cockroachdb_schema_lock(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		let _lock = self.recorder.acquire_cockroachdb_schema_lock().await?;
		self.recorder.ensure_schema_table_internal().await?;
		self.apply_migrations_after_schema_table(migrations).await
	}

	async fn apply_migrations_after_schema_table(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		let mut applied = Vec::new();
		let mut validation_state = super::ProjectState::new();
		let applied_records = self.recorder.get_applied_migrations().await?;
		let applied_record_keys: Vec<_> = applied_records
			.iter()
			.map(|record| (record.app.clone(), record.name.clone()))
			.collect();
		let applied_record_set: HashSet<_> = applied_record_keys
			.iter()
			.map(|(app, name)| (app.as_str(), name.as_str()))
			.collect();

		// A partly applied original history must continue on the original chain.
		// Once every original is recorded (or no original is recorded), select the
		// replacement so the normal reconciliation path can reduce it to one row.
		let partial_replacements: HashSet<_> = migrations
			.iter()
			.filter(|migration| {
				!migration.replaces.is_empty()
					&& !replacement_history_is_fully_covered(
						migration,
						migrations,
						&applied_record_keys,
					) && migration
					.replaces
					.iter()
					.any(|(app, name)| applied_record_set.contains(&(app.as_str(), name.as_str())))
			})
			.map(|migration| (migration.app_label.as_str(), migration.name.as_str()))
			.collect();
		let replaced_by_selected_replacements: HashSet<_> = migrations
			.iter()
			.filter(|migration| {
				!partial_replacements
					.contains(&(migration.app_label.as_str(), migration.name.as_str()))
			})
			.flat_map(|migration| {
				migration
					.replaces
					.iter()
					.map(|(app, name)| (app.as_str(), name.as_str()))
			})
			.collect();
		let selected_migrations: Vec<_> = migrations
			.iter()
			.filter(|migration| {
				!partial_replacements
					.contains(&(migration.app_label.as_str(), migration.name.as_str()))
					&& !replaced_by_selected_replacements
						.contains(&(migration.app_label.as_str(), migration.name.as_str()))
			})
			.collect();
		let partial_replacement_dependencies: HashMap<_, Vec<_>> = migrations
			.iter()
			.filter(|migration| {
				partial_replacements
					.contains(&(migration.app_label.as_str(), migration.name.as_str()))
			})
			.map(|migration| {
				(
					(migration.app_label.clone(), migration.name.clone()),
					migration
						.replaces
						.iter()
						.map(|(app, name)| {
							super::graph::MigrationKey::new(app.clone(), name.clone())
						})
						.collect(),
				)
			})
			.collect();

		// Build MigrationGraph for dependency resolution
		let mut graph = super::graph::MigrationGraph::new();

		for migration in &selected_migrations {
			let key = super::graph::MigrationKey::new(
				migration.app_label.clone(),
				migration.name.clone(),
			);
			let deps: Vec<super::graph::MigrationKey> = migration
				.dependencies
				.iter()
				.flat_map(|(app, name)| {
					partial_replacement_dependencies
						.get(&(app.clone(), name.clone()))
						.cloned()
						.unwrap_or_else(|| {
							vec![super::graph::MigrationKey::new(app.clone(), name.clone())]
						})
				})
				.collect();

			let replaces = migration
				.replaces
				.iter()
				.map(|(app, name)| super::graph::MigrationKey::new(app.clone(), name.clone()))
				.collect();
			graph.add_migration_with_replaces(key, deps, replaces);
		}

		// Resolve replacements before sorting so a squashed migration is selected
		// instead of reapplying every migration it supersedes.
		let sorted_keys = graph.resolve_execution_order_with_replaces()?;

		// Apply migrations in dependency-resolved order
		for key in sorted_keys {
			// Find the migration corresponding to this key
			let migration = selected_migrations
				.iter()
				.find(|m| m.app_label == key.app_label && m.name == key.name)
				.copied()
				.ok_or_else(|| {
					MigrationError::DependencyError(format!("Migration not found: {}", key.id()))
				})?;
			validate_and_advance_migration_state(migration, &mut validation_state)?;

			// A previous replacement reconciliation can have recorded the replacement
			// before removing the records it supersedes. Resume that cleanup here so a
			// retry cannot leave both histories applied indefinitely.
			if self
				.recorder
				.is_applied(&migration.app_label, &migration.name)
				.await?
			{
				if !migration.replaces.is_empty() {
					for (app_label, name) in &migration.replaces {
						if self.recorder.is_applied(app_label, name).await? {
							self.recorder.unapply(app_label, name).await?;
						}
					}
				}
				continue;
			}
			if !migration.replaces.is_empty() {
				let applied_records = self.recorder.get_applied_migrations().await?;
				let applied_records_set: HashSet<_> = applied_records
					.iter()
					.map(|record| (record.app.as_str(), record.name.as_str()))
					.collect();
				let replaced: HashSet<_> = migration
					.replaces
					.iter()
					.map(|(app, name)| (app.as_str(), name.as_str()))
					.collect();
				if replacement_history_is_fully_covered(
					migration,
					migrations,
					&applied_records
						.iter()
						.map(|record| (record.app.clone(), record.name.clone()))
						.collect::<Vec<_>>(),
				) {
					let historical_record = applied_records
						.iter()
						.find(|record| {
							replaced.contains(&(record.app.as_str(), record.name.as_str()))
						})
						.ok_or_else(|| {
							MigrationError::InvalidMigration(format!(
								"cannot apply replacement {} because a competing replacement already covers its history",
								migration.id()
							))
						})?;
					self.recorder
						.rename_applied(
							&historical_record.app,
							&historical_record.name,
							&migration.app_label,
							&migration.name,
						)
						.await?;
					for (app_label, name) in &migration.replaces {
						if applied_records_set.contains(&(app_label.as_str(), name.as_str()))
							&& (app_label != &historical_record.app
								|| name != &historical_record.name)
						{
							self.recorder.unapply(app_label, name).await?;
						}
					}
					applied.push(migration.id());
					continue;
				}
				if !replaced.is_disjoint(&applied_records_set) {
					return Err(MigrationError::InvalidMigration(format!(
						"cannot apply replacement {} because only some replaced migrations are recorded",
						migration.id()
					)));
				}
			}

			// Apply migration operations
			self.apply_migration(migration).await?;

			// Record migration as applied
			self.recorder
				.record_applied(&migration.app_label, &migration.name)
				.await?;

			applied.push(migration.id());
		}

		Ok(ExecutionResult {
			applied,
			failed: None,
		})
	}

	/// Rollback (unapply) a list of migrations
	///
	/// Migrations are rolled back in reverse order (newest first).
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::{Migration, executor::DatabaseMigrationExecutor};
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # async fn example() {
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let mut executor = DatabaseMigrationExecutor::new(connection);
	///
	/// let migrations = vec![Migration::new("0001_initial", "myapp")];
	/// let result = executor.rollback_migrations(&migrations).await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn rollback_migrations(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self
				.rollback_migrations_with_cockroachdb_schema_lock(migrations)
				.await;
		}

		// Ensure the migration recorder table exists
		self.recorder.ensure_schema_table().await?;
		self.rollback_migrations_after_schema_table(migrations)
			.await
	}

	#[cfg(feature = "postgres")]
	async fn rollback_migrations_with_cockroachdb_schema_lock(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		let _lock = self.recorder.acquire_cockroachdb_schema_lock().await?;
		self.recorder.ensure_schema_table_internal().await?;
		self.rollback_migrations_after_schema_table(migrations)
			.await
	}

	async fn rollback_migrations_after_schema_table(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		let mut rolledback = Vec::new();

		// Process migrations in reverse order (newest first)
		for migration in migrations.iter().rev() {
			// Check if migration is actually applied
			let is_applied = self
				.recorder
				.is_applied(&migration.app_label, &migration.name)
				.await?;

			if !is_applied {
				continue;
			}

			// Rollback the migration
			self.rollback_migration(migration).await?;

			// Remove from recorder
			self.recorder
				.unapply(&migration.app_label, &migration.name)
				.await?;

			rolledback.push(migration.id());
		}

		Ok(ExecutionResult {
			applied: rolledback,
			failed: None,
		})
	}

	/// Rollback a single migration
	async fn rollback_migration(&mut self, migration: &Migration) -> Result<()> {
		// Skip database operations if state_only flag is set
		if migration.state_only {
			tracing::debug!(
				"Skipping database operations for migration '{}' (state_only=true)",
				migration.id()
			);
			return Ok(());
		}

		let project_state = super::ProjectState::default();
		let requires_sqlite_recreation = migration_requires_sqlite_recreation(
			&self.connection,
			migration,
			MigrationDirection::Backward,
		);
		let mut editor = SchemaEditor::new_for_migration(
			self.connection.clone(),
			migration_is_atomic(migration, self.db_type),
			self.db_type,
			requires_sqlite_recreation,
		)
		.await?;
		let plan_result = self
			.planner
			.plan(
				&self.connection,
				migration,
				&project_state,
				MigrationDirection::Backward,
				&mut editor,
			)
			.await;
		match plan_result {
			Ok(plan) => self.execute_sql_plan(migration, plan, editor).await?,
			#[cfg(feature = "sqlite")]
			Err(error) if can_execute_sqlite_recreation_sequentially(&error) => {
				tracing::debug!(
					"Falling back to reverse sequential SQLite migration execution after opaque operation: {error}"
				);
				self.execute_sqlite_migration_sequentially(
					migration,
					editor,
					MigrationDirection::Backward,
				)
				.await?;
			}
			Err(error) => return Err(error),
		}

		Ok(())
	}

	/// Apply a single migration with atomic transaction support
	///
	/// If the migration's `atomic` flag is true and the database supports
	/// transactional DDL (PostgreSQL, SQLite), all operations are wrapped
	/// in a transaction that can be rolled back on failure. PostgreSQL migrations
	/// containing a concurrent index operation are always planned as non-atomic.
	///
	/// For databases that don't support transactional DDL (MySQL), operations
	/// are executed directly without transaction wrapping, and a warning is logged.
	async fn apply_migration(&self, migration: &Migration) -> Result<()> {
		// Skip database operations if state_only flag is set
		// (Django's SeparateDatabaseAndState equivalent with state_operations only)
		if migration.state_only {
			tracing::debug!(
				"Skipping database operations for migration '{}' (state_only=true)",
				migration.id()
			);
			return Ok(());
		}

		// Log if database_only flag is set
		if migration.database_only {
			tracing::debug!(
				"Skipping ProjectState updates for migration '{}' (database_only=true)",
				migration.id()
			);
		}

		tracing::debug!(
			"Planning migration '{}' (atomic={})",
			migration.id(),
			migration.atomic,
		);
		let requires_sqlite_recreation = migration_requires_sqlite_recreation(
			&self.connection,
			migration,
			MigrationDirection::Forward,
		);
		let mut editor = SchemaEditor::new_for_migration(
			self.connection.clone(),
			migration_is_atomic(migration, self.db_type),
			self.db_type,
			requires_sqlite_recreation,
		)
		.await?;
		let plan_result = self
			.planner
			.plan(
				&self.connection,
				migration,
				&ProjectState::default(),
				MigrationDirection::Forward,
				&mut editor,
			)
			.await;
		match plan_result {
			Ok(plan) => self.execute_sql_plan(migration, plan, editor).await?,
			#[cfg(feature = "sqlite")]
			Err(error) if can_execute_sqlite_recreation_sequentially(&error) => {
				tracing::debug!(
					"Falling back to sequential SQLite migration execution after opaque operation: {error}"
				);
				self.execute_sqlite_migration_sequentially(
					migration,
					editor,
					MigrationDirection::Forward,
				)
				.await?;
			}
			Err(error) => return Err(error),
		}

		tracing::debug!("Migration '{}' applied successfully", migration.id());

		Ok(())
	}

	async fn execute_sql_plan(
		&self,
		migration: &Migration,
		plan: MigrationSqlPlan,
		mut editor: SchemaEditor,
	) -> Result<()> {
		self.execute_sql_plan_statements(migration, &plan, &mut editor)
			.await?;
		editor.finish().await
	}

	async fn execute_sql_plan_statements(
		&self,
		migration: &Migration,
		plan: &MigrationSqlPlan,
		editor: &mut SchemaEditor,
	) -> Result<()> {
		tracing::debug!(
			"Executing migration '{}' (atomic={}, effective_atomic={})",
			migration.id(),
			plan.atomic,
			editor.is_atomic()
		);

		let mut index = 0;
		while index < plan.statements.len() {
			#[cfg(feature = "sqlite")]
			if let Some(group) = plan.sqlite_recreation_groups[index] {
				let mut sql = Vec::new();
				while index < plan.statements.len()
					&& plan.sqlite_recreation_groups[index] == Some(group)
				{
					if let PlannedStatement::Sql(statement) = &plan.statements[index] {
						sql.push(statement.clone());
					}
					index += 1;
				}
				editor
					.with_foreign_keys_disabled(move |editor| {
						Box::pin(async move {
							for statement in sql {
								editor.execute(&statement).await?;
							}
							let violations = editor.check_foreign_key_integrity().await?;
							if !violations.is_empty() {
								return Err(MigrationError::ForeignKeyViolation(format!(
									"Foreign key violations detected after table recreation: {}",
									violations.join("; ")
								)));
							}
							Ok(())
						})
					})
					.await?;
				continue;
			}

			let statement = &plan.statements[index];
			let operation = plan.planned_operations[index].as_ref();
			index += 1;
			match statement {
				PlannedStatement::Comment(comment) => {
					tracing::debug!("Migration plan comment: {comment}");
				}
				PlannedStatement::Sql(statement) => {
					if let Some(Operation::CreateTable { name, .. }) = operation
						&& editor.table_exists(name).await?
					{
						tracing::info!(
							"Table '{}' already exists, skipping CREATE TABLE operation",
							name
						);
						continue;
					}
					let context = planned_operation_context(operation);
					editor
						.execute_with_context(statement, context)
						.await
						.map_err(|error| {
							tracing::error!(
								"Migration operation failed: {}. SQL: {}",
								error,
								&statement[..statement.len().min(200)]
							);
							error
						})?;
				}
			}
		}

		Ok(())
	}

	#[cfg(feature = "sqlite")]
	async fn execute_sqlite_migration_sequentially(
		&self,
		migration: &Migration,
		mut editor: SchemaEditor,
		direction: MigrationDirection,
	) -> Result<()> {
		let mut operations = migration.operations.clone();
		if direction == MigrationDirection::Backward {
			operations.reverse();
		}
		for operation in operations {
			let mut single_operation = migration.clone();
			single_operation.operations = vec![operation];
			let plan = self
				.planner
				.plan(
					&self.connection,
					&single_operation,
					&ProjectState::default(),
					direction,
					&mut editor,
				)
				.await?;
			self.execute_sql_plan_statements(&single_operation, &plan, &mut editor)
				.await?;
		}
		editor.finish().await
	}

	/// Apply migrations from a MigrationPlan
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::{MigrationPlan, executor::DatabaseMigrationExecutor};
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
	/// // Example: connecting to a PostgreSQL database
	/// let db = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let mut executor = DatabaseMigrationExecutor::new(db);
	///
	/// let plan = MigrationPlan::new();
	/// let result = executor.apply(&plan).await.unwrap();
	/// # });
	/// ```
	pub async fn apply(&mut self, plan: &MigrationPlan) -> Result<ExecutionResult> {
		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self.apply_with_cockroachdb_schema_lock(plan).await;
		}

		// Ensure the migration recorder table exists
		self.recorder.ensure_schema_table().await?;
		self.apply_after_schema_table(plan).await
	}

	#[cfg(feature = "postgres")]
	async fn apply_with_cockroachdb_schema_lock(
		&mut self,
		plan: &MigrationPlan,
	) -> Result<ExecutionResult> {
		let _lock = self.recorder.acquire_cockroachdb_schema_lock().await?;
		self.recorder.ensure_schema_table_internal().await?;
		self.apply_after_schema_table(plan).await
	}

	async fn apply_after_schema_table(&mut self, plan: &MigrationPlan) -> Result<ExecutionResult> {
		let mut applied = Vec::new();
		let mut validation_state = super::ProjectState::new();

		for migration in &plan.migrations {
			validate_and_advance_migration_state(migration, &mut validation_state)?;

			// Check if already applied
			if self
				.recorder
				.is_applied(&migration.app_label, &migration.name)
				.await?
			{
				continue;
			}

			self.apply_migration(migration).await?;

			// Record migration as applied
			self.recorder
				.record_applied(&migration.app_label, &migration.name)
				.await?;

			applied.push(migration.id());
		}

		Ok(ExecutionResult {
			applied,
			failed: None,
		})
	}

	/// Build migration plan - returns list of migrations to apply
	///
	/// Returns (app_label, migration_name) tuples in dependency order
	// Allow dead_code: public API for migration CLI tooling to preview pending migrations
	#[allow(dead_code)]
	pub async fn build_plan(&self, service: &MigrationService) -> Result<Vec<(String, String)>> {
		let graph = service.build_dependency_graph().await?;
		let mut plan = Vec::new();

		for migration in graph {
			let is_applied = self
				.recorder
				.is_applied(&migration.app_label, &migration.name)
				.await?;

			if !is_applied {
				plan.push((migration.app_label.to_string(), migration.name.to_string()));
			}
		}

		Ok(plan)
	}

	/// Read recreation metadata for a SQLite table using the SchemaEditor's
	/// currently-open transaction (if any), falling back to the live pool when
	/// the editor is non-atomic.
	///
	/// This exists because `SQLiteIntrospector` issues every query through the
	/// underlying `sqlx::SqlitePool`, which transparently picks a *different*
	/// physical connection than the one holding the editor's open transaction.
	/// That second connection cannot see uncommitted DDL — so a recreation
	/// triggered later in the same migration would rebuild the table from a
	/// stale column list and silently discard the just-`ALTER`'d column.
	/// See reinhardt-web#4447.
	#[cfg(feature = "sqlite")]
	pub(crate) async fn read_sqlite_table_via_editor(
		editor: &mut SchemaEditor,
		table_name: &str,
	) -> Result<(
		Vec<super::ColumnDefinition>,
		Vec<(String, String)>,
		Vec<super::Constraint>,
		Vec<super::operations::SqliteRecreatedConstraint>,
		Vec<super::operations::SqliteRecreatedIndex>,
		Vec<String>,
		bool,
		bool,
	)> {
		// 1. PRAGMA table_xinfo(<table>) → columns. Identifier interpolation
		//    via the shared `sqlite_pragma` helper. See issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let table_info_sql = format!(
			"PRAGMA table_xinfo({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let info_rows = editor.fetch_all(&table_info_sql, vec![]).await?;

		// Collect rows into typed records first so we can detect AUTOINCREMENT.
		struct ColRow {
			name: String,
			type_str: String,
			notnull: i64,
			default: Option<String>,
			pk: i64,
			hidden: i64,
		}
		let mut col_rows: Vec<ColRow> = Vec::with_capacity(info_rows.len());
		for row in &info_rows {
			let name: String = row
				.get("name")
				.map_err(|e| MigrationError::IntrospectionError(format!("table_info name: {e}")))?;
			let type_str: String = row.get("type").unwrap_or_default();
			let notnull: i64 = row.get("notnull").unwrap_or(0);
			let default: Option<String> = row.get("dflt_value").ok();
			let pk: i64 = row.get("pk").unwrap_or(0);
			let hidden: i64 = row.get("hidden").unwrap_or(0);
			col_rows.push(ColRow {
				name,
				type_str,
				notnull,
				default,
				pk,
				hidden,
			});
		}

		// 2. CREATE TABLE SQL → detect AUTOINCREMENT and parse named
		//    constraint metadata (FK names + CHECK constraints).
		let create_sql_row = editor
			.fetch_optional(
				"SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
				vec![table_name.into()],
			)
			.await?;
		let create_sql: Option<String> = create_sql_row.and_then(|r| r.get("sql").ok());
		let column_collations = create_sql
			.as_deref()
			.map(parse_sqlite_column_collations)
			.unwrap_or_default();
		let has_autoincrement = create_sql
			.as_ref()
			.map(|sql| sql.to_uppercase().contains("AUTOINCREMENT"))
			.unwrap_or(false);
		// SQLite exposes table options as structured metadata. Reading them via
		// the editor keeps this lookup on the dedicated migration connection and
		// avoids parsing CREATE SQL that may contain comments or unusual spacing.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let table_list_sql = format!(
			"PRAGMA table_list({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let table_options = editor.fetch_optional(&table_list_sql, vec![]).await?;
		let without_rowid = table_options
			.as_ref()
			.and_then(|row| row.get::<i64>("wr").ok())
			.map(|value| value != 0)
			.unwrap_or_else(|| {
				// SQLite versions before table_list was introduced can still use
				// WITHOUT ROWID, so retain the CREATE SQL fallback for that option.
				create_sql.as_deref().is_some_and(|sql| {
					sql.rsplit_once(')').is_some_and(|(_, options)| {
						options.to_ascii_uppercase().contains("WITHOUT ROWID")
					})
				})
			});
		let strict = table_options
			.as_ref()
			.and_then(|row| row.get::<i64>("strict").ok())
			.unwrap_or(0)
			!= 0;

		// 3. Build ColumnDefinition list, mirroring the introspector's
		//    semantics (PK columns are implicitly NOT NULL; AUTOINCREMENT is
		//    only meaningful on PK columns).
		let mut composite_primary_key: Vec<(i64, String)> = col_rows
			.iter()
			.filter(|column| column.pk > 0)
			.map(|column| (column.pk, column.name.clone()))
			.collect();
		composite_primary_key.sort_by_key(|(ordinal, _)| *ordinal);
		let has_composite_primary_key = composite_primary_key.len() > 1;
		let mut columns: Vec<super::ColumnDefinition> = col_rows
			.iter()
			.map(|c| {
				let is_pk = c.pk > 0 && !has_composite_primary_key;
				let is_auto = is_pk && has_autoincrement;
				let nullable = if c.pk > 0 { false } else { c.notnull == 0 };
				// Preserve `dflt_value` verbatim as the raw SQL fragment
				// (e.g. `'pending'` including surrounding quotes). The
				// downstream `format!("DEFAULT {}", default)` paths in
				// `operations.rs` then emit valid DDL (`DEFAULT 'pending'`,
				// not the previously broken `DEFAULT pending`). See
				// `super::sqlite_pragma` and issue #4454.
				let default = c
					.default
					.as_ref()
					.map(|v| super::sqlite_pragma::normalize_default_value(v));
				super::ColumnDefinition {
					name: c.name.clone(),
					type_definition: SQLiteIntrospector::parse_sqlite_type(&c.type_str),
					not_null: !nullable,
					unique: false,
					primary_key: is_pk,
					auto_increment: is_auto,
					default,
					generated: parse_sqlite_generated_column(
						create_sql.as_deref(),
						&c.name,
						c.hidden,
					),
					domain: None,
				}
			})
			.collect();

		// Preserve the introspector's ordering: PK first, then by name. This
		// matters because `SqliteTableRecreation` uses column order to emit
		// the new CREATE TABLE and the INSERT SELECT.
		columns.sort_by(|a, b| {
			if a.primary_key && !b.primary_key {
				std::cmp::Ordering::Less
			} else if !a.primary_key && b.primary_key {
				std::cmp::Ordering::Greater
			} else {
				a.name.cmp(&b.name)
			}
		});

		// 4. Foreign keys via PRAGMA foreign_key_list(<table>), grouped by
		//    id. Identifier interpolation via the shared `sqlite_pragma`
		//    helper. See issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let fk_sql = format!(
			"PRAGMA foreign_key_list({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let fk_rows = editor.fetch_all(&fk_sql, vec![]).await?;

		struct FkRow {
			id: i64,
			seq: i64,
			table: String,
			from: String,
			to: String,
			on_update: String,
			on_delete: String,
		}
		let parsed_fks: Vec<FkRow> = fk_rows
			.iter()
			.map(|row| FkRow {
				id: row.get("id").unwrap_or(0),
				seq: row.get("seq").unwrap_or(0),
				table: row.get("table").unwrap_or_default(),
				from: row.get("from").unwrap_or_default(),
				to: row.get("to").unwrap_or_default(),
				on_update: row.get("on_update").unwrap_or_default(),
				on_delete: row.get("on_delete").unwrap_or_default(),
			})
			.collect();

		let fk_metadata = create_sql
			.as_deref()
			.map(parse_sqlite_fk_metadata)
			.unwrap_or_default();

		let mut fk_groups: std::collections::HashMap<i64, Vec<FkRow>> =
			std::collections::HashMap::new();
		for r in parsed_fks {
			fk_groups.entry(r.id).or_default().push(r);
		}

		fn fk_action(s: &str) -> super::ForeignKeyAction {
			match s {
				"CASCADE" => super::ForeignKeyAction::Cascade,
				"SET NULL" => super::ForeignKeyAction::SetNull,
				"SET DEFAULT" => super::ForeignKeyAction::SetDefault,
				"NO ACTION" => super::ForeignKeyAction::NoAction,
				_ => super::ForeignKeyAction::Restrict,
			}
		}

		let mut constraints: Vec<super::Constraint> = Vec::new();
		if has_composite_primary_key {
			constraints.push(super::Constraint::PrimaryKey {
				name: format!("{}_pkey", table_name),
				columns: composite_primary_key
					.into_iter()
					.map(|(_, column)| column)
					.collect(),
			});
		}
		for (fk_id, mut group) in fk_groups {
			group.sort_by_key(|r| r.seq);
			let referenced_table = group[0].table.clone();
			let columns_from: Vec<String> = group.iter().map(|r| r.from.clone()).collect();
			let mut columns_to: Vec<String> = group
				.iter()
				.map(|r| r.to.clone())
				.filter(|column| !column.is_empty())
				.collect();
			let signature = (
				columns_from.clone(),
				referenced_table.clone(),
				columns_to.clone(),
			);
			let metadata = fk_metadata.get(&signature);
			let name = metadata
				.and_then(|metadata| metadata.name.clone())
				.unwrap_or_else(|| format!("fk_{}_{}", table_name, fk_id));
			if columns_to.is_empty() {
				// SQLite leaves the target columns null when REFERENCES omits them.
				// Resolve the target primary key in its declared ordinal order so the
				// recreated table emits a valid explicit foreign key target list.
				let target_columns_sql = format!(
					"PRAGMA table_info({})",
					super::sqlite_pragma::quote_pragma_identifier(&referenced_table)
				);
				let target_column_rows = editor.fetch_all(&target_columns_sql, vec![]).await?;
				let mut target_primary_key: Vec<(i64, String)> = target_column_rows
					.iter()
					.filter_map(|row| {
						let ordinal = row.get::<i64>("pk").ok()?;
						let name = row.get::<String>("name").ok()?;
						(ordinal > 0).then_some((ordinal, name))
					})
					.collect();
				target_primary_key.sort_by_key(|(ordinal, _)| *ordinal);
				columns_to = target_primary_key
					.into_iter()
					.map(|(_, column)| column)
					.collect();
				if columns_to.is_empty() {
					return Err(MigrationError::InvalidMigration(format!(
						"cannot recreate SQLite table '{table_name}': foreign key '{name}' omits referenced columns, but referenced table '{referenced_table}' has no primary key"
					)));
				}
			}
			if columns_from.len() != columns_to.len() {
				let source_label = if columns_from.len() == 1 {
					"column"
				} else {
					"columns"
				};
				let referenced_label = if columns_to.len() == 1 {
					"column"
				} else {
					"columns"
				};
				return Err(MigrationError::InvalidMigration(format!(
					"cannot recreate SQLite table '{table_name}': foreign key '{name}' has {} source {source_label}, but referenced table '{referenced_table}' resolves to {} referenced {referenced_label}",
					columns_from.len(),
					columns_to.len()
				)));
			}
			let deferrable = metadata.and_then(|metadata| metadata.deferrable);
			constraints.push(super::Constraint::ForeignKey {
				name,
				columns: columns_from,
				referenced_table,
				referenced_columns: columns_to,
				on_delete: fk_action(&group[0].on_delete),
				on_update: fk_action(&group[0].on_update),
				deferrable,
			});
		}

		// 5. Unique constraints via PRAGMA index_list / index_info where
		//    origin = 'u' (i.e. declared with the UNIQUE keyword or as a
		//    named CONSTRAINT … UNIQUE). Identifier interpolation via the
		//    shared `sqlite_pragma` helper. See issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let idx_list_sql = format!(
			"PRAGMA index_list({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let idx_rows = editor.fetch_all(&idx_list_sql, vec![]).await?;
		let unique_constraint_metadata = create_sql
			.as_deref()
			.map(parse_sqlite_unique_constraint_metadata)
			.unwrap_or_default();
		let raw_constraint_metadata = unique_constraint_metadata
			.iter()
			.filter(|metadata| metadata.raw_sql.is_some())
			.collect::<Vec<_>>();
		let mut raw_constraints = raw_constraint_metadata
			.iter()
			.map(|metadata| super::operations::SqliteRecreatedConstraint {
				name: metadata.name.clone(),
				physical_name: None,
				columns: metadata.columns.clone(),
				sql: metadata.raw_sql.clone().unwrap_or_default(),
			})
			.collect::<Vec<_>>();
		let mut restored_unique_constraint_names = std::collections::HashSet::new();
		let mut indexes = Vec::new();
		for row in &idx_rows {
			let origin: String = row.get("origin").unwrap_or_default();
			let unique: i64 = row.get("unique").unwrap_or(0);
			if origin != "u" && origin != "c" {
				continue;
			}
			let idx_name: String = row.get("name").unwrap_or_default();
			let idx_sql = editor
				.fetch_optional(
					"SELECT sql FROM sqlite_master WHERE type='index' AND name=?",
					vec![idx_name.clone().into()],
				)
				.await?
				.and_then(|r| r.get("sql").ok());
			// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
			let info_sql = format!(
				"PRAGMA index_info({})",
				super::sqlite_pragma::quote_pragma_identifier(&idx_name)
			);
			let info_rows = editor.fetch_all(&info_sql, vec![]).await?;
			let cols: Vec<String> = info_rows
				.iter()
				.filter_map(|r| r.get::<String>("name").ok())
				.collect();
			if origin == "u" && unique == 1 {
				// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
				let xinfo_sql = format!(
					"PRAGMA index_xinfo({})",
					super::sqlite_pragma::quote_pragma_identifier(&idx_name)
				);
				let indexed_columns = editor
					.fetch_all(&xinfo_sql, vec![])
					.await?
					.into_iter()
					.filter(|row| row.get::<i64>("key").unwrap_or(1) == 1)
					.filter_map(|row| {
						Some(SqliteIndexedColumnMetadata {
							name: row.get::<String>("name").ok()?,
							collation: row.get::<String>("coll").ok(),
							descending: Some(row.get::<i64>("desc").unwrap_or(0) != 0),
						})
					})
					.collect::<Vec<_>>();
				let scored_raw_constraints = raw_constraint_metadata
					.iter()
					.enumerate()
					.filter(|(index, _)| raw_constraints[*index].physical_name.is_none())
					.filter_map(|(index, metadata)| {
						sqlite_unique_index_match_score(&metadata.indexed_columns, &indexed_columns)
							.map(|score| (index, score))
					})
					.collect::<Vec<_>>();
				if let Some(max_score) =
					scored_raw_constraints.iter().map(|(_, score)| *score).max()
				{
					for (index, _) in scored_raw_constraints
						.into_iter()
						.filter(|(_, score)| *score == max_score)
					{
						raw_constraints[index].physical_name = Some(idx_name.clone());
					}
				}
				let matches_columns = |metadata: &SqliteUniqueConstraintMetadata| {
					metadata.columns.len() == cols.len()
						&& metadata
							.columns
							.iter()
							.zip(&cols)
							.all(|(declared, actual)| declared.eq_ignore_ascii_case(actual))
				};
				if unique_constraint_metadata
					.iter()
					.filter(|metadata| metadata.raw_sql.is_some())
					.any(&matches_columns)
				{
					continue;
				}
				let declared_names: Vec<_> = unique_constraint_metadata
					.iter()
					.filter(|metadata| metadata.raw_sql.is_none())
					.filter(|metadata| matches_columns(metadata))
					.filter_map(|metadata| metadata.name.clone())
					.collect();
				if declared_names.is_empty() {
					constraints.push(super::Constraint::Unique {
						name: idx_name,
						columns: cols,
					});
				} else {
					for name in declared_names {
						if restored_unique_constraint_names.insert(name.clone()) {
							constraints.push(super::Constraint::Unique {
								name,
								columns: cols.clone(),
							});
						}
					}
				}
			} else if origin == "c" && (!cols.is_empty() || idx_sql.is_some()) {
				indexes.push(super::operations::SqliteRecreatedIndex {
					name: idx_name,
					columns: cols,
					unique: unique == 1,
					sql: idx_sql,
				});
			}
		}

		// 6. CHECK constraints parsed from CREATE TABLE SQL (SQLite has no
		//    PRAGMA for these).
		if let Some(ref sql) = create_sql {
			for (idx, check) in SQLiteIntrospector::parse_check_constraints(sql)?
				.into_iter()
				.enumerate()
			{
				constraints.push(super::Constraint::Check {
					name: check.name.unwrap_or_else(|| format!("check_{}", idx)),
					expression: check.expression,
				});
			}
		}

		// 7. Triggers must be recreated after the temporary table is renamed.
		//    Dropping the original table removes them from sqlite_master. Preserve
		//    sqlite_master row order so their creation order does not change.
		let trigger_rows = editor
			.fetch_all(
				"SELECT sql FROM sqlite_master WHERE type='trigger' AND tbl_name=? AND sql IS NOT NULL ORDER BY rowid",
				vec![table_name.into()],
			)
			.await?;
		let mut triggers = Vec::with_capacity(trigger_rows.len());
		for row in &trigger_rows {
			triggers.push(row.get::<String>("sql").map_err(|error| {
				MigrationError::IntrospectionError(format!("sqlite_master trigger SQL: {error}"))
			})?);
		}

		Ok((
			columns,
			column_collations,
			constraints,
			raw_constraints,
			indexes,
			triggers,
			without_rowid,
			strict,
		))
	}

	/// Record a migration as applied without actually running it
	pub async fn record_migration(&mut self, app_label: &str, migration_name: &str) -> Result<()> {
		self.recorder
			.record_applied(app_label, migration_name)
			.await?;
		Ok(())
	}

	/// Execute a migration by loading it from the service
	// Allow dead_code: public API for programmatic single-migration execution
	#[allow(dead_code)]
	pub async fn execute_migration(
		&mut self,
		app_label: &str,
		migration_name: &str,
		service: &MigrationService,
	) -> Result<()> {
		let migration = service.load_migration(app_label, migration_name).await?;
		let mut validation_state = super::ProjectState::new();
		for historical_migration in service.build_dependency_graph().await? {
			if historical_migration.app_label == migration.app_label
				&& historical_migration.name == migration.name
			{
				break;
			}
			validate_and_advance_migration_state(&historical_migration, &mut validation_state)?;
		}
		validate_and_advance_migration_state(&migration, &mut validation_state)?;

		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self
				.execute_migration_with_cockroachdb_schema_lock(&migration)
				.await;
		}

		self.recorder.ensure_schema_table().await?;
		self.execute_migration_after_schema_table(&migration).await
	}

	#[cfg(feature = "postgres")]
	async fn execute_migration_with_cockroachdb_schema_lock(
		&mut self,
		migration: &Migration,
	) -> Result<()> {
		let _lock = self.recorder.acquire_cockroachdb_schema_lock().await?;
		self.recorder.ensure_schema_table_internal().await?;
		self.execute_migration_after_schema_table(migration).await
	}

	async fn execute_migration_after_schema_table(&mut self, migration: &Migration) -> Result<()> {
		// Apply operations
		self.apply_migration(migration).await?;

		// Record as applied
		self.recorder
			.record_applied(&migration.app_label, &migration.name)
			.await?;

		Ok(())
	}
}

/// Operation optimizer for migration execution
///
/// Reorders and optimizes operations for better performance and safety.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::executor::OperationOptimizer;
/// use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
///
/// let ops = vec![
///     Operation::AddColumn {
///         table: "users".to_string(),
///         column: ColumnDefinition::new("name", FieldType::VarChar(100)),
///         mysql_options: None,
///     },
///     Operation::CreateTable {
///         name: "users".to_string(),
///         columns: vec![],
///         constraints: vec![],
///         without_rowid: None,
///         interleave_in_parent: None,
///         partition: None,
///     },
/// ];
///
/// let optimizer = OperationOptimizer::new();
/// let optimized = optimizer.optimize(ops);
/// // CreateTable should come before AddColumn
/// ```
pub struct OperationOptimizer {
	_private: (),
}

impl OperationOptimizer {
	/// Create a new operation optimizer
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::executor::OperationOptimizer;
	///
	/// let optimizer = OperationOptimizer::new();
	/// ```
	pub fn new() -> Self {
		Self { _private: () }
	}

	/// Optimize and reorder operations
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::executor::OperationOptimizer;
	/// use reinhardt_db::migrations::{Operation, ColumnDefinition};
	///
	/// let ops = vec![
	///     Operation::CreateTable {
	///         name: "users".to_string(),
	///         columns: vec![],
	///         constraints: vec![],
	///         without_rowid: None,
	///         interleave_in_parent: None,
	///         partition: None,
	///     },
	/// ];
	///
	/// let optimizer = OperationOptimizer::new();
	/// let optimized = optimizer.optimize(ops);
	/// assert_eq!(optimized.len(), 1);
	/// ```
	pub fn optimize(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut optimized = operations;

		// Step 1: Reorder operations by dependency
		optimized = self.reorder_by_dependency(optimized);

		// Step 2: Group similar operations
		optimized = self.group_similar_operations(optimized);

		// Step 3: Remove redundant operations
		optimized = self.remove_redundant_operations(optimized);

		optimized
	}

	/// Reorder operations to respect dependencies
	fn reorder_by_dependency(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut ordered = Vec::new();
		let mut remaining = operations;
		let mut created_tables = HashSet::new();

		// Priority order:
		// 1. CreateTable (sorted by foreign key dependencies)
		// 2. AddColumn
		// 3. AlterColumn
		// 4. CreateIndex
		// 5. AddConstraint
		// 6. RunSQL
		// 7. RenameColumn
		// 8. DropColumn
		// 9. DropTable

		// First pass: Create tables (respecting foreign key dependencies)
		// Extract all CreateTable operations
		let mut create_table_ops = Vec::new();
		let mut i = 0;
		while i < remaining.len() {
			if matches!(&remaining[i], Operation::CreateTable { .. }) {
				create_table_ops.push(remaining.remove(i));
			} else {
				i += 1;
			}
		}

		// Sort CreateTable operations by dependencies using topological sort
		while !create_table_ops.is_empty() {
			let mut found_independent = false;

			for i in 0..create_table_ops.len() {
				if let Operation::CreateTable {
					name, constraints, ..
				} = &create_table_ops[i]
				{
					// Extract foreign key references from constraints
					let mut depends_on_uncreated = false;
					for constraint in constraints {
						if let Some(referenced_table) =
							self.extract_foreign_key_reference(constraint)
						{
							// Check if the referenced table has been created
							if !created_tables.contains(&referenced_table)
								&& referenced_table != *name
							{
								depends_on_uncreated = true;
								break;
							}
						}
					}

					// If this table doesn't depend on any uncreated table, we can create it now
					if !depends_on_uncreated {
						// Clone the name before removing the operation
						let name_copy = name.clone();
						let op = create_table_ops.remove(i);
						created_tables.insert(name_copy);
						ordered.push(op);
						found_independent = true;
						break;
					}
				}
			}

			// If we couldn't find any independent table, just add the remaining tables
			// (this handles circular dependencies or malformed constraints)
			if !found_independent {
				for op in create_table_ops.drain(..) {
					if let Operation::CreateTable { ref name, .. } = op {
						created_tables.insert(name.clone());
					}
					ordered.push(op);
				}
				break;
			}
		}

		// Second pass: Add columns (for all tables)
		i = 0;
		while i < remaining.len() {
			if let Operation::AddColumn { .. } = &remaining[i] {
				ordered.push(remaining.remove(i));
			} else {
				i += 1;
			}
		}

		// Third pass: Other operations
		ordered.extend(remaining);

		ordered
	}

	/// Extract the referenced table name from a FOREIGN KEY constraint
	/// Returns the referenced table name if the constraint is a ForeignKey
	fn extract_foreign_key_reference(&self, constraint: &super::Constraint) -> Option<String> {
		match constraint {
			super::Constraint::ForeignKey {
				referenced_table, ..
			} => Some(referenced_table.clone()),
			_ => None,
		}
	}

	/// Extract constraint name from SQL definition
	fn extract_constraint_name(&self, constraint_sql: &str) -> Option<String> {
		let trimmed = constraint_sql.trim();

		// Check if starts with "CONSTRAINT"
		if !trimmed.starts_with("CONSTRAINT") {
			return None;
		}

		// Skip "CONSTRAINT" and whitespace
		let after_keyword = trimmed["CONSTRAINT".len()..].trim_start();

		// Extract identifier (alphanumeric + underscore)
		let name: String = after_keyword
			.chars()
			.take_while(|c| c.is_alphanumeric() || *c == '_')
			.collect();

		if name.is_empty() { None } else { Some(name) }
	}

	/// Group similar operations together
	fn group_similar_operations(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut by_table: IndexMap<String, Vec<Operation>> = IndexMap::new();
		let mut create_ops = Vec::new();
		let mut other_ops = Vec::new();

		for op in operations {
			match &op {
				Operation::CreateTable { .. } => {
					// CreateTable operations go first
					create_ops.push(op);
				}
				Operation::AddColumn { table, .. }
				| Operation::DropColumn { table, .. }
				| Operation::AlterColumn { table, .. } => {
					by_table.entry(table.to_string()).or_default().push(op);
				}
				_ => {
					other_ops.push(op);
				}
			}
		}

		let mut grouped = Vec::new();

		// Add create table operations first
		grouped.extend(create_ops);

		// Add table-specific operations grouped by table
		for (_, ops) in by_table {
			grouped.extend(ops);
		}

		// Add other operations
		grouped.extend(other_ops);

		grouped
	}

	/// Remove redundant operations by detecting cancellations and merging similar operations
	fn remove_redundant_operations(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut optimized = Vec::new();
		let mut removed_indices = HashSet::new();

		// Pass 1: Detect and remove operation cancellations
		for i in 0..operations.len() {
			if removed_indices.contains(&i) {
				continue;
			}

			let op = &operations[i];
			let mut found_cancellation = false;

			// Search forward for cancelling operations
			for (j, next_op) in operations.iter().enumerate().skip(i + 1) {
				if removed_indices.contains(&j) {
					continue;
				}

				// Check for cancellation patterns
				let cancels = match (op, next_op) {
					// CreateTable + DropTable
					(
						Operation::CreateTable { name: n1, .. },
						Operation::DropTable { name: n2 },
					) if n1 == n2 => true,
					// AddColumn + DropColumn
					(
						Operation::AddColumn {
							table: t1,
							column: col1,
							..
						},
						Operation::DropColumn {
							table: t2,
							column: col2,
							..
						},
					) if t1 == t2 && col1.name == *col2 => true,
					// CreateIndex + DropIndex
					(
						Operation::CreateIndex {
							table: t1,
							columns: c1,
							..
						},
						Operation::DropIndex {
							table: t2,
							columns: c2,
						},
					) if t1 == t2 && c1 == c2 => true,
					// CreateNamedIndex + DropNamedIndex
					#[cfg(feature = "pgvector")]
					(
						Operation::CreateNamedIndex {
							table: t1,
							name: n1,
							..
						},
						Operation::DropNamedIndex {
							table: t2,
							name: n2,
							..
						},
					) if t1 == t2 && n1 == n2 => true,
					// AddConstraint + DropConstraint
					(
						Operation::AddConstraint {
							table: t1,
							constraint_sql,
						},
						Operation::DropConstraint {
							table: t2,
							constraint_name,
							..
						},
					) if t1 == t2 => {
						// Try to extract constraint name from SQL for exact matching
						if let Some(extracted_name) = self.extract_constraint_name(constraint_sql) {
							// Perfect match: compare extracted name with drop target
							extracted_name == *constraint_name
						} else {
							// Fallback: approximate match by table only
							true
						}
					}
					_ => false,
				};

				if cancels {
					removed_indices.insert(i);
					removed_indices.insert(j);
					found_cancellation = true;
					break;
				}
			}

			if !found_cancellation {
				optimized.push(op.clone());
			}
		}

		// Pass 1.5: Remove duplicate CreateTable operations (keep last occurrence)
		let mut deduped = Vec::new();
		let mut create_table_map: IndexMap<String, Operation> = IndexMap::new();

		for operation in optimized {
			match &operation {
				Operation::CreateTable { name, .. } => {
					// Last CreateTable for same table wins
					create_table_map.insert(name.to_string(), operation.clone());
				}
				_ => {
					// Flush accumulated CreateTable operations before non-CreateTable operation
					for (_, create_op) in create_table_map.drain(..) {
						deduped.push(create_op);
					}
					deduped.push(operation);
				}
			}
		}

		// Flush remaining CreateTable operations
		for (_, create_op) in create_table_map {
			deduped.push(create_op);
		}

		// Pass 2: Merge consecutive AlterColumn operations on same column
		let mut merged = Vec::new();
		let mut alter_column_map: IndexMap<(String, String), Operation> = IndexMap::new();

		for operation in deduped {
			match &operation {
				Operation::AlterColumn {
					table,
					column,
					new_definition: _,
					..
				} => {
					let key = (table.to_string(), column.to_string());
					// Last AlterColumn wins (overwrites previous)
					alter_column_map.insert(key, operation.clone());
				}
				_ => {
					// Flush accumulated AlterColumn operations before non-AlterColumn operation
					for (_, alter_op) in alter_column_map.drain(..) {
						merged.push(alter_op);
					}
					merged.push(operation);
				}
			}
		}

		// Flush remaining AlterColumn operations
		for (_, alter_op) in alter_column_map {
			merged.push(alter_op);
		}

		// Pass 3: Chain consecutive RenameTable operations
		let mut chained = Vec::new();
		let mut rename_chain: IndexMap<String, String> = IndexMap::new(); // original_name -> current_name

		for operation in merged {
			match &operation {
				Operation::RenameTable { old_name, new_name } => {
					// Find if any existing chain ends with this old_name
					let mut found_chain = None;
					for (original, current) in &rename_chain {
						if current == old_name {
							found_chain = Some(original.clone());
							break;
						}
					}

					if let Some(original) = found_chain {
						// Extend existing chain: original -> new_name
						rename_chain.insert(original, new_name.clone());
					} else {
						// Start new chain: old_name -> new_name
						rename_chain.insert(old_name.clone(), new_name.clone());
					}
				}
				_ => {
					// Flush accumulated RenameTable chains before non-RenameTable operation
					for (original_name, final_name) in rename_chain.drain(..) {
						chained.push(Operation::RenameTable {
							old_name: original_name,
							new_name: final_name,
						});
					}
					chained.push(operation);
				}
			}
		}

		// Flush remaining RenameTable chains
		for (original_name, final_name) in rename_chain {
			chained.push(Operation::RenameTable {
				old_name: original_name,
				new_name: final_name,
			});
		}

		chained
	}
}

#[cfg(feature = "sqlite")]
pub(crate) fn parse_sqlite_generated_column(
	create_sql: Option<&str>,
	column_name: &str,
	hidden: i64,
) -> Option<super::GeneratedColumnDefinition> {
	if !matches!(hidden, 2 | 3) {
		return None;
	}

	let column_sql = find_sqlite_column_definition(create_sql?, column_name)?;
	let raw_sql = extract_sqlite_generated_expr(&column_sql)?;
	let storage = if hidden == 3 || column_sql.to_ascii_uppercase().contains(" STORED") {
		super::GeneratedStorage::Stored
	} else {
		super::GeneratedStorage::Virtual
	};
	Some(super::GeneratedColumnDefinition::raw_sql(raw_sql, storage))
}

#[cfg(feature = "sqlite")]
fn find_sqlite_column_definition(create_sql: &str, column_name: &str) -> Option<String> {
	let body = sqlite_create_table_body(create_sql)?;
	split_sqlite_top_level_list(body)
		.into_iter()
		.find(|definition| {
			sqlite_column_name(definition)
				.as_deref()
				.is_some_and(|name| name.eq_ignore_ascii_case(column_name))
		})
		.map(str::to_string)
}

#[cfg(feature = "sqlite")]
fn sqlite_create_table_body(create_sql: &str) -> Option<&str> {
	let start = create_sql.find('(')?;
	let end = find_matching_sqlite_paren(create_sql, start)?;
	create_sql.get(start + 1..end)
}

#[cfg(feature = "sqlite")]
fn split_sqlite_top_level_list(sql: &str) -> Vec<&str> {
	let mut parts = Vec::new();
	let mut start = 0;
	let mut depth = 0usize;
	let mut quote: Option<char> = None;
	let mut line_comment = false;
	let mut block_comment = false;
	let mut chars = sql.char_indices().peekable();

	while let Some((index, ch)) = chars.next() {
		if line_comment {
			if ch == '\n' {
				line_comment = false;
			}
			continue;
		}
		if block_comment {
			if ch == '*' && chars.peek().is_some_and(|(_, next)| *next == '/') {
				chars.next();
				block_comment = false;
			}
			continue;
		}
		if let Some(quote_ch) = quote {
			if ch == quote_ch {
				if chars.peek().is_some_and(|(_, next)| *next == quote_ch) {
					chars.next();
				} else {
					quote = None;
				}
			}
			continue;
		}

		match ch {
			'-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
				chars.next();
				line_comment = true;
			}
			'/' if chars.peek().is_some_and(|(_, next)| *next == '*') => {
				chars.next();
				block_comment = true;
			}
			'\'' | '"' | '`' => quote = Some(ch),
			'[' => quote = Some(']'),
			'(' => depth += 1,
			')' => depth = depth.saturating_sub(1),
			',' if depth == 0 => {
				parts.push(sql[start..index].trim());
				start = index + ch.len_utf8();
			}
			_ => {}
		}
	}

	let tail = sql[start..].trim();
	if !tail.is_empty() {
		parts.push(tail);
	}
	parts
}

#[cfg(feature = "sqlite")]
#[derive(Debug)]
struct SqliteFkMetadata {
	name: Option<String>,
	deferrable: Option<super::operations::DeferrableOption>,
}

#[cfg(feature = "sqlite")]
#[derive(Debug, PartialEq, Eq)]
struct SqliteUniqueConstraintMetadata {
	name: Option<String>,
	columns: Vec<String>,
	indexed_columns: Vec<SqliteIndexedColumnMetadata>,
	raw_sql: Option<String>,
}

#[cfg(feature = "sqlite")]
#[derive(Debug, PartialEq, Eq)]
struct SqliteIndexedColumnMetadata {
	name: String,
	collation: Option<String>,
	descending: Option<bool>,
}

#[cfg(feature = "sqlite")]
fn sqlite_top_level_word_index(tokens: &[SqliteDdlToken], word: &str) -> Option<usize> {
	let mut depth = 0usize;
	for (index, token) in tokens.iter().enumerate() {
		match token {
			SqliteDdlToken::OpenParen => depth += 1,
			SqliteDdlToken::CloseParen => depth = depth.saturating_sub(1),
			token if depth == 0 && sqlite_token_is_word(token, word) => return Some(index),
			_ => {}
		}
	}
	None
}

#[cfg(feature = "sqlite")]
fn sqlite_unique_conflict_mode(tokens: &[SqliteDdlToken], unique_index: usize) -> Option<String> {
	let mut depth = 0usize;
	for (index, token) in tokens.iter().enumerate().skip(unique_index + 1) {
		match token {
			SqliteDdlToken::OpenParen => depth += 1,
			SqliteDdlToken::CloseParen => depth = depth.saturating_sub(1),
			token
				if depth == 0
					&& sqlite_token_is_word(token, "ON")
					&& tokens
						.get(index + 1)
						.is_some_and(|token| sqlite_token_is_word(token, "CONFLICT")) =>
			{
				return tokens.get(index + 2).and_then(sqlite_token_identifier);
			}
			token
				if depth == 0
					&& [
						"CONSTRAINT",
						"PRIMARY",
						"NOT",
						"CHECK",
						"DEFAULT",
						"COLLATE",
						"REFERENCES",
						"GENERATED",
						"UNIQUE",
					]
					.iter()
					.any(|keyword| sqlite_token_is_word(token, keyword)) =>
			{
				return None;
			}
			_ => {}
		}
	}
	None
}

#[cfg(feature = "sqlite")]
fn sqlite_canonical_unique_sql(
	name: Option<&str>,
	columns: &[SqliteIndexedColumnMetadata],
	conflict_mode: Option<&str>,
) -> String {
	let quote = super::sqlite_pragma::quote_sqlite_identifier;
	let columns = columns
		.iter()
		.map(|column| {
			let collation = column.collation.as_deref().unwrap_or("BINARY");
			let ordering = if column.descending == Some(true) {
				"DESC"
			} else {
				"ASC"
			};
			format!(
				"{} COLLATE {} {ordering}",
				quote(&column.name),
				quote(collation)
			)
		})
		.collect::<Vec<_>>()
		.join(", ");
	let mut sql = match name {
		Some(name) => format!("CONSTRAINT {} UNIQUE ({columns})", quote(name)),
		None => format!("UNIQUE ({columns})"),
	};
	if let Some(conflict_mode) = conflict_mode {
		sql.push_str(" ON CONFLICT ");
		sql.push_str(conflict_mode);
	}
	sql
}

#[cfg(feature = "sqlite")]
#[derive(Debug)]
enum SqliteDdlToken {
	Word(String),
	Identifier(String),
	OpenParen,
	CloseParen,
	Comma,
}

#[cfg(feature = "sqlite")]
fn parse_sqlite_fk_metadata(
	create_sql: &str,
) -> std::collections::HashMap<(Vec<String>, String, Vec<String>), SqliteFkMetadata> {
	let Some(body) = sqlite_create_table_body(create_sql) else {
		return std::collections::HashMap::new();
	};
	let mut metadata = std::collections::HashMap::new();
	for definition in split_sqlite_top_level_list(body) {
		let tokens = tokenize_sqlite_definition(definition);
		let Some(references_index) = tokens
			.iter()
			.position(|token| sqlite_token_is_word(token, "REFERENCES"))
		else {
			continue;
		};
		let Some(referenced_table) = tokens
			.get(references_index + 1)
			.and_then(sqlite_token_identifier)
		else {
			continue;
		};
		let referenced_columns = tokens
			.iter()
			.enumerate()
			.skip(references_index + 2)
			.find(|(_, token)| matches!(token, SqliteDdlToken::OpenParen))
			.map(|(index, _)| sqlite_identifier_list(&tokens, index))
			.unwrap_or_default();

		let foreign_index = tokens.windows(2).position(|pair| {
			sqlite_token_is_word(&pair[0], "FOREIGN") && sqlite_token_is_word(&pair[1], "KEY")
		});
		let (name, source_columns) = if let Some(foreign_index) = foreign_index {
			let source_columns = tokens
				.iter()
				.enumerate()
				.skip(foreign_index + 2)
				.find(|(_, token)| matches!(token, SqliteDdlToken::OpenParen))
				.map(|(index, _)| sqlite_identifier_list(&tokens, index))
				.unwrap_or_default();
			let name = if tokens
				.first()
				.is_some_and(|token| sqlite_token_is_word(token, "CONSTRAINT"))
			{
				tokens.get(1).and_then(sqlite_token_identifier)
			} else {
				None
			};
			(name, source_columns)
		} else {
			let Some(column) = tokens.first().and_then(sqlite_token_identifier) else {
				continue;
			};
			if sqlite_token_is_word(&tokens[0], "CONSTRAINT") {
				continue;
			}
			let name = tokens[..references_index]
				.windows(2)
				.rev()
				.find_map(|pair| {
					sqlite_token_is_word(&pair[0], "CONSTRAINT")
						.then(|| sqlite_token_identifier(&pair[1]))
						.flatten()
				});
			(name, vec![column])
		};
		if source_columns.is_empty() {
			continue;
		}

		let deferrable = sqlite_fk_deferrable(&tokens);
		metadata.insert(
			(source_columns, referenced_table, referenced_columns),
			SqliteFkMetadata { name, deferrable },
		);
	}
	metadata
}

#[cfg(feature = "sqlite")]
fn parse_sqlite_column_collations(create_sql: &str) -> Vec<(String, String)> {
	let Some(body) = sqlite_create_table_body(create_sql) else {
		return Vec::new();
	};
	split_sqlite_top_level_list(body)
		.into_iter()
		.filter_map(|definition| {
			let column = sqlite_column_name(definition)?;
			let tokens = tokenize_sqlite_definition(definition);
			let collation = sqlite_top_level_collation(tokens.iter())?;
			Some((column, collation))
		})
		.collect()
}

#[cfg(feature = "sqlite")]
fn parse_sqlite_unique_constraint_metadata(
	create_sql: &str,
) -> Vec<SqliteUniqueConstraintMetadata> {
	let Some(body) = sqlite_create_table_body(create_sql) else {
		return Vec::new();
	};
	let column_collations = parse_sqlite_column_collations(create_sql);
	let normalize_indexed_columns = |columns: &mut [SqliteIndexedColumnMetadata]| {
		for column in columns {
			if column.collation.is_none() {
				column.collation = column_collations
					.iter()
					.find(|(name, _)| name.eq_ignore_ascii_case(&column.name))
					.map(|(_, collation)| collation.clone())
					.or_else(|| Some("BINARY".to_string()));
			}
			column.descending.get_or_insert(false);
		}
	};
	let mut metadata = Vec::new();
	for definition in split_sqlite_top_level_list(body) {
		let tokens = tokenize_sqlite_definition(definition);
		let table_unique = if tokens.len() >= 4
			&& sqlite_token_is_word(&tokens[0], "CONSTRAINT")
			&& sqlite_token_is_word(&tokens[2], "UNIQUE")
		{
			Some((sqlite_token_identifier(&tokens[1]), 3, 2))
		} else if tokens
			.first()
			.is_some_and(|token| sqlite_token_is_word(token, "UNIQUE"))
		{
			Some((None, 1, 0))
		} else {
			None
		};
		if let Some((name, tokens_to_skip, unique_index)) = table_unique {
			let Some(open_index) = tokens
				.iter()
				.skip(tokens_to_skip)
				.position(|token| matches!(token, SqliteDdlToken::OpenParen))
				.map(|index| index + tokens_to_skip)
			else {
				continue;
			};
			let mut indexed_columns = sqlite_indexed_column_metadata(&tokens, open_index);
			normalize_indexed_columns(&mut indexed_columns);
			if !indexed_columns.is_empty() {
				let conflict_mode = sqlite_unique_conflict_mode(&tokens, unique_index);
				let raw_sql = sqlite_canonical_unique_sql(
					name.as_deref(),
					&indexed_columns,
					conflict_mode.as_deref(),
				);
				metadata.push(SqliteUniqueConstraintMetadata {
					name,
					columns: indexed_columns
						.iter()
						.map(|column| column.name.clone())
						.collect(),
					indexed_columns,
					raw_sql: Some(raw_sql),
				});
			}
			continue;
		}

		let Some(column) = sqlite_column_name(definition) else {
			continue;
		};
		let Some(unique_index) = sqlite_top_level_word_index(&tokens, "UNIQUE") else {
			continue;
		};
		let mut depth = 0usize;
		let mut name = None;
		for (index, token) in tokens.iter().take(unique_index).enumerate() {
			match token {
				SqliteDdlToken::OpenParen => depth += 1,
				SqliteDdlToken::CloseParen => depth = depth.saturating_sub(1),
				token if depth == 0 && sqlite_token_is_word(token, "CONSTRAINT") => {
					name = tokens.get(index + 1).and_then(sqlite_token_identifier);
				}
				_ => {}
			}
		}
		let mut indexed_columns = vec![SqliteIndexedColumnMetadata {
			name: column.clone(),
			collation: None,
			descending: None,
		}];
		normalize_indexed_columns(&mut indexed_columns);
		let conflict_mode = sqlite_unique_conflict_mode(&tokens, unique_index);
		let raw_sql = sqlite_canonical_unique_sql(
			name.as_deref(),
			&indexed_columns,
			conflict_mode.as_deref(),
		);
		metadata.push(SqliteUniqueConstraintMetadata {
			name,
			columns: vec![column.clone()],
			indexed_columns,
			raw_sql: Some(raw_sql),
		});
	}
	metadata
}

#[cfg(feature = "sqlite")]
fn sqlite_unique_index_match_score(
	declared: &[SqliteIndexedColumnMetadata],
	actual: &[SqliteIndexedColumnMetadata],
) -> Option<usize> {
	if declared.len() != actual.len() {
		return None;
	}
	let mut score = 0;
	for (declared, actual) in declared.iter().zip(actual) {
		if !declared.name.eq_ignore_ascii_case(&actual.name) {
			return None;
		}
		if let Some(collation) = &declared.collation {
			if !actual
				.collation
				.as_deref()
				.is_some_and(|actual| collation.eq_ignore_ascii_case(actual))
			{
				return None;
			}
			score += 1;
		}
		if let Some(descending) = declared.descending {
			if actual.descending != Some(descending) {
				return None;
			}
			score += 1;
		}
	}
	Some(score)
}

#[cfg(feature = "sqlite")]
fn tokenize_sqlite_definition(definition: &str) -> Vec<SqliteDdlToken> {
	fn flush_word(tokens: &mut Vec<SqliteDdlToken>, word: &mut String) {
		if !word.is_empty() {
			tokens.push(SqliteDdlToken::Word(std::mem::take(word)));
		}
	}

	let mut tokens = Vec::new();
	let mut word = String::new();
	let mut chars = definition.chars().peekable();
	while let Some(ch) = chars.next() {
		match ch {
			'-' if chars.peek() == Some(&'-') => {
				flush_word(&mut tokens, &mut word);
				chars.next();
				for next in chars.by_ref() {
					if next == '\n' {
						break;
					}
				}
			}
			'/' if chars.peek() == Some(&'*') => {
				flush_word(&mut tokens, &mut word);
				chars.next();
				while let Some(next) = chars.next() {
					if next == '*' && chars.peek() == Some(&'/') {
						chars.next();
						break;
					}
				}
			}
			'"' | '`' | '\'' | '[' => {
				flush_word(&mut tokens, &mut word);
				let closing = if ch == '[' { ']' } else { ch };
				let mut identifier = String::new();
				while let Some(next) = chars.next() {
					if next == closing {
						if chars.peek() == Some(&closing) {
							identifier.push(closing);
							chars.next();
							continue;
						}
						break;
					}
					identifier.push(next);
				}
				tokens.push(SqliteDdlToken::Identifier(identifier));
			}
			'(' => {
				flush_word(&mut tokens, &mut word);
				tokens.push(SqliteDdlToken::OpenParen);
			}
			')' => {
				flush_word(&mut tokens, &mut word);
				tokens.push(SqliteDdlToken::CloseParen);
			}
			',' => {
				flush_word(&mut tokens, &mut word);
				tokens.push(SqliteDdlToken::Comma);
			}
			ch if ch.is_whitespace() => flush_word(&mut tokens, &mut word),
			_ => word.push(ch),
		}
	}
	flush_word(&mut tokens, &mut word);
	tokens
}

#[cfg(feature = "sqlite")]
fn sqlite_token_is_word(token: &SqliteDdlToken, expected: &str) -> bool {
	matches!(token, SqliteDdlToken::Word(word) if word.eq_ignore_ascii_case(expected))
}

#[cfg(feature = "sqlite")]
fn sqlite_token_identifier(token: &SqliteDdlToken) -> Option<String> {
	match token {
		SqliteDdlToken::Word(identifier) | SqliteDdlToken::Identifier(identifier) => {
			Some(identifier.clone())
		}
		_ => None,
	}
}

#[cfg(feature = "sqlite")]
fn sqlite_top_level_collation<'a>(
	tokens: impl IntoIterator<Item = &'a SqliteDdlToken>,
) -> Option<String> {
	let mut depth = 0usize;
	let mut collation = None;
	let mut tokens = tokens.into_iter().peekable();
	while let Some(token) = tokens.next() {
		match token {
			SqliteDdlToken::OpenParen => depth += 1,
			SqliteDdlToken::CloseParen => depth = depth.saturating_sub(1),
			token if depth == 0 && sqlite_token_is_word(token, "COLLATE") => {
				collation = tokens
					.peek()
					.and_then(|token| sqlite_token_identifier(token));
			}
			_ => {}
		}
	}
	collation
}

#[cfg(feature = "sqlite")]
fn sqlite_fk_deferrable(tokens: &[SqliteDdlToken]) -> Option<super::operations::DeferrableOption> {
	if tokens.windows(2).any(|pair| {
		sqlite_token_is_word(&pair[0], "NOT") && sqlite_token_is_word(&pair[1], "DEFERRABLE")
	}) {
		return None;
	}
	let explicit_mode = tokens.windows(3).find_map(|sequence| {
		if !sqlite_token_is_word(&sequence[0], "DEFERRABLE")
			|| !sqlite_token_is_word(&sequence[1], "INITIALLY")
		{
			return None;
		}
		if sqlite_token_is_word(&sequence[2], "DEFERRED") {
			Some(super::operations::DeferrableOption::Deferred)
		} else if sqlite_token_is_word(&sequence[2], "IMMEDIATE") {
			Some(super::operations::DeferrableOption::Immediate)
		} else {
			None
		}
	});
	explicit_mode.or_else(|| {
		tokens
			.iter()
			.any(|token| sqlite_token_is_word(token, "DEFERRABLE"))
			.then_some(super::operations::DeferrableOption::Immediate)
	})
}

#[cfg(feature = "sqlite")]
fn sqlite_identifier_list(tokens: &[SqliteDdlToken], open_index: usize) -> Vec<String> {
	tokens
		.iter()
		.skip(open_index + 1)
		.take_while(|token| !matches!(token, SqliteDdlToken::CloseParen))
		.filter_map(sqlite_token_identifier)
		.collect()
}

#[cfg(feature = "sqlite")]
fn sqlite_indexed_column_metadata(
	tokens: &[SqliteDdlToken],
	open_index: usize,
) -> Vec<SqliteIndexedColumnMetadata> {
	fn parse_column(tokens: &[&SqliteDdlToken]) -> Option<SqliteIndexedColumnMetadata> {
		let name = tokens
			.first()
			.and_then(|token| sqlite_token_identifier(token))?;
		let collation = sqlite_top_level_collation(tokens.iter().copied());
		let descending = tokens.iter().find_map(|token| {
			if sqlite_token_is_word(token, "DESC") {
				Some(true)
			} else if sqlite_token_is_word(token, "ASC") {
				Some(false)
			} else {
				None
			}
		});
		Some(SqliteIndexedColumnMetadata {
			name,
			collation,
			descending,
		})
	}

	let mut columns = Vec::new();
	let mut current = Vec::new();
	for token in tokens.iter().skip(open_index + 1) {
		match token {
			SqliteDdlToken::CloseParen => {
				if let Some(column) = parse_column(&current) {
					columns.push(column);
				}
				break;
			}
			SqliteDdlToken::Comma => {
				if let Some(column) = parse_column(&current) {
					columns.push(column);
				}
				current.clear();
			}
			_ => current.push(token),
		}
	}
	columns
}

#[cfg(feature = "sqlite")]
fn sqlite_column_name(definition: &str) -> Option<String> {
	let tokens = tokenize_sqlite_definition(definition);
	let first = tokens.first()?;
	if ["CONSTRAINT", "PRIMARY", "FOREIGN", "UNIQUE", "CHECK"]
		.iter()
		.any(|keyword| sqlite_token_is_word(first, keyword))
	{
		return None;
	}
	sqlite_token_identifier(first)
}

#[cfg(feature = "sqlite")]
fn extract_sqlite_generated_expr(column_sql: &str) -> Option<String> {
	let upper = column_sql.to_ascii_uppercase();
	let generated_index = upper
		.find("GENERATED ALWAYS AS")
		.or_else(|| find_sqlite_keyword(column_sql, "AS"))?;
	let expr_start = column_sql[generated_index..].find('(')? + generated_index;
	let expr_end = find_matching_sqlite_paren(column_sql, expr_start)?;
	Some(column_sql[expr_start + 1..expr_end].trim().to_string())
}

#[cfg(feature = "sqlite")]
fn find_sqlite_keyword(sql: &str, keyword: &str) -> Option<usize> {
	let upper = sql.to_ascii_uppercase();
	let mut quote: Option<char> = None;

	for (index, ch) in sql.char_indices() {
		if let Some(quote_ch) = quote {
			if ch == quote_ch {
				quote = None;
			}
			continue;
		}

		match ch {
			'\'' | '"' | '`' => {
				quote = Some(ch);
				continue;
			}
			'[' => {
				quote = Some(']');
				continue;
			}
			_ => {}
		}

		let Some(candidate) = upper.get(index..) else {
			continue;
		};
		if !candidate.starts_with(keyword) {
			continue;
		}

		let before = upper[..index].chars().next_back();
		let after = upper[index + keyword.len()..].chars().next();
		if before.is_none_or(sqlite_keyword_boundary) && after.is_none_or(sqlite_keyword_boundary) {
			return Some(index);
		}
	}

	None
}

#[cfg(feature = "sqlite")]
fn sqlite_keyword_boundary(ch: char) -> bool {
	!ch.is_ascii_alphanumeric() && ch != '_'
}

#[cfg(feature = "sqlite")]
fn find_matching_sqlite_paren(sql: &str, open_index: usize) -> Option<usize> {
	let mut depth = 0usize;
	let mut quote: Option<char> = None;
	let mut line_comment = false;
	let mut block_comment = false;
	let mut chars = sql
		.char_indices()
		.filter(|(index, _)| *index >= open_index)
		.peekable();

	while let Some((index, ch)) = chars.next() {
		if line_comment {
			if ch == '\n' {
				line_comment = false;
			}
			continue;
		}
		if block_comment {
			if ch == '*' && chars.peek().is_some_and(|(_, next)| *next == '/') {
				chars.next();
				block_comment = false;
			}
			continue;
		}
		if let Some(quote_ch) = quote {
			if ch == quote_ch {
				if chars.peek().is_some_and(|(_, next)| *next == quote_ch) {
					chars.next();
				} else {
					quote = None;
				}
			}
			continue;
		}

		match ch {
			'-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
				chars.next();
				line_comment = true;
			}
			'/' if chars.peek().is_some_and(|(_, next)| *next == '*') => {
				chars.next();
				block_comment = true;
			}
			'\'' | '"' | '`' => quote = Some(ch),
			'[' => quote = Some(']'),
			'(' => depth += 1,
			')' => {
				depth = depth.checked_sub(1)?;
				if depth == 0 {
					return Some(index);
				}
			}
			_ => {}
		}
	}

	None
}

#[cfg(all(test, feature = "sqlite"))]
mod sqlite_generated_column_tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn parse_sqlite_fk_metadata_restores_escaped_inline_constraint() {
		// Arrange
		let create_sql = r#"CREATE TABLE child (
			"a""b" INTEGER CONSTRAINT "inline""fk" REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED
		)"#;

		// Act
		let metadata = parse_sqlite_fk_metadata(create_sql);
		let parsed = metadata
			.get(&(
				vec![r#"a"b"#.to_string()],
				"parent".to_string(),
				vec!["id".to_string()],
			))
			.expect("escaped inline foreign key metadata should be parsed");

		// Assert
		assert_eq!(parsed.name.as_deref(), Some(r#"inline"fk"#));
		assert_eq!(
			parsed.deferrable,
			Some(super::super::operations::DeferrableOption::Deferred)
		);
	}

	#[rstest]
	fn parse_sqlite_fk_metadata_maps_bare_deferrable_to_immediate() {
		// Arrange
		let create_sql = "CREATE TABLE child (parent_id INTEGER, FOREIGN KEY (parent_id) REFERENCES parent(id) DEFERRABLE)";

		// Act
		let metadata = parse_sqlite_fk_metadata(create_sql);
		let parsed = metadata
			.get(&(
				vec!["parent_id".to_string()],
				"parent".to_string(),
				vec!["id".to_string()],
			))
			.expect("bare deferrable foreign key metadata should be parsed");

		// Assert
		assert_eq!(parsed.name, None);
		assert_eq!(
			parsed.deferrable,
			Some(super::super::operations::DeferrableOption::Immediate)
		);
	}

	#[test]
	fn parse_sqlite_generated_column_restores_virtual_column() {
		let create_sql = r#"CREATE TABLE "users" (
			"id" integer primary key,
			"first_name" text not null,
			"last_name" text not null,
			"full_name" text GENERATED ALWAYS AS ("first_name" || ' ' || "last_name") VIRTUAL
		)"#;

		let generated = parse_sqlite_generated_column(Some(create_sql), "full_name", 2)
			.expect("generated column should be parsed");

		assert_eq!(
			generated.raw_sql.as_deref(),
			Some(r#""first_name" || ' ' || "last_name""#)
		);
		assert_eq!(generated.storage, super::super::GeneratedStorage::Virtual);
	}

	#[test]
	fn parse_sqlite_generated_column_restores_stored_column() {
		let create_sql = r#"CREATE TABLE users (
			id integer primary key,
			total integer GENERATED ALWAYS AS ((subtotal + tax)) STORED
		)"#;

		let generated = parse_sqlite_generated_column(Some(create_sql), "total", 3)
			.expect("stored generated column should be parsed");

		assert_eq!(generated.raw_sql.as_deref(), Some("(subtotal + tax)"));
		assert_eq!(generated.storage, super::super::GeneratedStorage::Stored);
	}

	#[test]
	fn parse_sqlite_generated_column_restores_shorthand_column() {
		let create_sql = r#"CREATE TABLE users (
			id integer primary key,
			normalized_name text AS (lower(name)) STORED
		)"#;

		let generated = parse_sqlite_generated_column(Some(create_sql), "normalized_name", 3)
			.expect("shorthand generated column should be parsed");

		assert_eq!(generated.raw_sql.as_deref(), Some("lower(name)"));
		assert_eq!(generated.storage, super::super::GeneratedStorage::Stored);
	}

	#[test]
	fn parse_sqlite_generated_column_ignores_regular_columns() {
		let create_sql = r#"CREATE TABLE users (id integer primary key, name text)"#;

		assert!(parse_sqlite_generated_column(Some(create_sql), "name", 0).is_none());
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_preserves_quoted_composite_constraint() {
		// Arrange
		let create_sql = r#"CREATE TABLE jobs (
			"tenant id" TEXT,
			"code,value" TEXT,
			note TEXT CHECK (note != 'CONSTRAINT fake UNIQUE (code)'),
			CONSTRAINT "unique jobs code" UNIQUE ("tenant id", "code,value")
		)"#;

		// Act
		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		// Assert
		assert_eq!(
			metadata,
			vec![SqliteUniqueConstraintMetadata {
				name: Some("unique jobs code".to_string()),
				columns: vec!["tenant id".to_string(), "code,value".to_string()],
				indexed_columns: vec![
					SqliteIndexedColumnMetadata {
						name: "tenant id".to_string(),
						collation: Some("BINARY".to_string()),
						descending: Some(false),
					},
					SqliteIndexedColumnMetadata {
						name: "code,value".to_string(),
						collation: Some("BINARY".to_string()),
						descending: Some(false),
					},
				],
				raw_sql: Some(
					"CONSTRAINT \"unique jobs code\" UNIQUE (\"tenant id\" COLLATE \"BINARY\" ASC, \"code,value\" COLLATE \"BINARY\" ASC)"
						.to_string(),
				),
			}]
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_preserves_inline_constraint() {
		// Arrange
		let create_sql =
			"CREATE TABLE jobs (code TEXT CONSTRAINT `unique jobs code` UNIQUE, status TEXT)";

		// Act
		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		// Assert
		assert_eq!(
			metadata,
			vec![SqliteUniqueConstraintMetadata {
				name: Some("unique jobs code".to_string()),
				columns: vec!["code".to_string()],
				indexed_columns: vec![SqliteIndexedColumnMetadata {
					name: "code".to_string(),
					collation: Some("BINARY".to_string()),
					descending: Some(false),
				}],
				raw_sql: Some(
					"CONSTRAINT \"unique jobs code\" UNIQUE (\"code\" COLLATE \"BINARY\" ASC)"
						.to_string(),
				),
			}]
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_preserves_duplicate_column_constraints() {
		// Arrange
		let create_sql = r#"CREATE TABLE jobs (
			tenant TEXT,
			code TEXT,
			CONSTRAINT uq_jobs_primary UNIQUE (tenant, code),
			CONSTRAINT uq_jobs_secondary UNIQUE (tenant, code)
		)"#;

		// Act
		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		// Assert
		assert_eq!(
			metadata,
			vec![
				SqliteUniqueConstraintMetadata {
					name: Some("uq_jobs_primary".to_string()),
					columns: vec!["tenant".to_string(), "code".to_string()],
					indexed_columns: vec![
						SqliteIndexedColumnMetadata {
							name: "tenant".to_string(),
							collation: Some("BINARY".to_string()),
							descending: Some(false),
						},
						SqliteIndexedColumnMetadata {
							name: "code".to_string(),
							collation: Some("BINARY".to_string()),
							descending: Some(false),
						},
					],
					raw_sql: Some(
						"CONSTRAINT \"uq_jobs_primary\" UNIQUE (\"tenant\" COLLATE \"BINARY\" ASC, \"code\" COLLATE \"BINARY\" ASC)"
							.to_string(),
					),
				},
				SqliteUniqueConstraintMetadata {
					name: Some("uq_jobs_secondary".to_string()),
					columns: vec!["tenant".to_string(), "code".to_string()],
					indexed_columns: vec![
						SqliteIndexedColumnMetadata {
							name: "tenant".to_string(),
							collation: Some("BINARY".to_string()),
							descending: Some(false),
						},
						SqliteIndexedColumnMetadata {
							name: "code".to_string(),
							collation: Some("BINARY".to_string()),
							descending: Some(false),
						},
					],
					raw_sql: Some(
						"CONSTRAINT \"uq_jobs_secondary\" UNIQUE (\"tenant\" COLLATE \"BINARY\" ASC, \"code\" COLLATE \"BINARY\" ASC)"
							.to_string(),
					),
				},
			]
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_preserves_collation_sql() {
		// Arrange
		let create_sql = r#"CREATE TABLE jobs (
			code TEXT,
			CONSTRAINT "uq_jobs_nocase" UNIQUE ("code" COLLATE NOCASE),
			CONSTRAINT "uq_jobs_binary" UNIQUE ("code" COLLATE BINARY)
		)"#;

		// Act
		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		// Assert
		assert_eq!(metadata.len(), 2);
		assert_eq!(
			metadata[0].raw_sql.as_deref(),
			Some("CONSTRAINT \"uq_jobs_nocase\" UNIQUE (\"code\" COLLATE \"NOCASE\" ASC)")
		);
		assert_eq!(
			metadata[1].raw_sql.as_deref(),
			Some("CONSTRAINT \"uq_jobs_binary\" UNIQUE (\"code\" COLLATE \"BINARY\" ASC)")
		);
		assert_eq!(
			metadata[0].indexed_columns[0].collation.as_deref(),
			Some("NOCASE")
		);
		assert_eq!(
			metadata[1].indexed_columns[0].collation.as_deref(),
			Some("BINARY")
		);
		assert!(
			sqlite_unique_index_match_score(
				&metadata[0].indexed_columns,
				&metadata[1].indexed_columns
			)
			.is_none()
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_normalizes_default_binary_collation() {
		// Arrange
		let create_sql = r#"CREATE TABLE jobs (
			code TEXT,
			UNIQUE (code),
			UNIQUE (code COLLATE BINARY)
		)"#;

		// Act
		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		// Assert
		assert_eq!(metadata.len(), 2);
		assert_eq!(
			metadata[0].indexed_columns[0].collation.as_deref(),
			Some("BINARY")
		);
		assert_eq!(metadata[0].indexed_columns[0].descending, Some(false));
		assert_eq!(
			sqlite_unique_index_match_score(
				&metadata[0].indexed_columns,
				&metadata[1].indexed_columns
			),
			Some(2)
		);
	}

	#[rstest]
	#[case("code TEXT CHECK (code COLLATE NOCASE <> '')", "BINARY")]
	#[case(
		"code TEXT GENERATED ALWAYS AS (source COLLATE NOCASE) STORED",
		"BINARY"
	)]
	#[case("code TEXT COLLATE NOCASE COLLATE RTRIM", "RTRIM")]
	#[case(
		"code TEXT COLLATE NOCASE CHECK (code COLLATE BINARY <> '') COLLATE RTRIM",
		"RTRIM"
	)]
	fn parse_sqlite_unique_metadata_uses_last_top_level_column_collation(
		#[case] column_definition: &str,
		#[case] expected_collation: &str,
	) {
		let create_sql =
			format!("CREATE TABLE jobs (source TEXT, {column_definition}, UNIQUE (code))");

		let metadata = parse_sqlite_unique_constraint_metadata(&create_sql);

		assert_eq!(
			metadata[0].indexed_columns[0].collation.as_deref(),
			Some(expected_collation)
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_uses_last_indexed_column_collation() {
		let create_sql =
			"CREATE TABLE jobs (code TEXT, UNIQUE (code COLLATE NOCASE COLLATE RTRIM))";

		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		assert_eq!(
			metadata[0].indexed_columns[0].collation.as_deref(),
			Some("RTRIM")
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_ignores_parentheses_inside_comments() {
		let create_sql = "CREATE TABLE jobs (code TEXT, UNIQUE (code /* ) */ COLLATE NOCASE), -- )\n UNIQUE (code COLLATE BINARY))";

		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		assert_eq!(metadata.len(), 2);
		assert_eq!(
			metadata[0].indexed_columns[0].collation.as_deref(),
			Some("NOCASE")
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_preserves_inline_conflict_mode() {
		let create_sql =
			"CREATE TABLE jobs (code TEXT CONSTRAINT uq_jobs_code UNIQUE ON CONFLICT IGNORE)";

		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		assert_eq!(
			metadata[0].raw_sql.as_deref(),
			Some(
				"CONSTRAINT \"uq_jobs_code\" UNIQUE (\"code\" COLLATE \"BINARY\" ASC) ON CONFLICT IGNORE"
			)
		);
	}

	#[rstest]
	#[case(
		"code TEXT NOT NULL ON CONFLICT IGNORE UNIQUE ON CONFLICT REPLACE",
		"REPLACE"
	)]
	#[case(
		"code TEXT PRIMARY KEY ON CONFLICT FAIL UNIQUE ON CONFLICT REPLACE",
		"REPLACE"
	)]
	fn parse_sqlite_unique_metadata_attributes_inline_conflict_to_unique_constraint(
		#[case] column_definition: &str,
		#[case] expected_mode: &str,
	) {
		let create_sql = format!("CREATE TABLE jobs ({column_definition})");
		let expected_sql =
			format!("UNIQUE (\"code\" COLLATE \"BINARY\" ASC) ON CONFLICT {expected_mode}");

		let metadata = parse_sqlite_unique_constraint_metadata(&create_sql);

		assert_eq!(metadata[0].raw_sql.as_deref(), Some(expected_sql.as_str()));
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_does_not_use_later_constraint_conflict_mode() {
		let create_sql = "CREATE TABLE jobs (code TEXT UNIQUE NOT NULL ON CONFLICT IGNORE)";

		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		assert_eq!(
			metadata[0].raw_sql.as_deref(),
			Some("UNIQUE (\"code\" COLLATE \"BINARY\" ASC)")
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_quotes_keyword_identifiers() {
		let create_sql =
			r#"CREATE TABLE jobs ("select" TEXT, CONSTRAINT "unique" UNIQUE ("select"))"#;

		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		assert_eq!(
			metadata[0].raw_sql.as_deref(),
			Some("CONSTRAINT \"unique\" UNIQUE (\"select\" COLLATE \"BINARY\" ASC)")
		);
	}

	#[rstest]
	fn parse_sqlite_unique_metadata_decodes_quoted_column_names() {
		let create_sql = r#"CREATE TABLE jobs ("a""b" TEXT COLLATE NOCASE, UNIQUE ("a""b"), 'c' TEXT COLLATE RTRIM, UNIQUE ('c'))"#;

		let metadata = parse_sqlite_unique_constraint_metadata(create_sql);

		assert_eq!(
			metadata[0].indexed_columns[0].collation.as_deref(),
			Some("NOCASE")
		);
		assert_eq!(
			metadata[1].indexed_columns[0].collation.as_deref(),
			Some("RTRIM")
		);
	}
}

impl Default for OperationOptimizer {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod optimizer_tests {
	use super::*;
	use crate::migrations::{ColumnDefinition, FieldType};
	#[cfg(feature = "pgvector")]
	use crate::{backends::error::PgvectorOperationKind, migrations::IndexType};

	struct SentinelPlanner;

	#[test]
	fn concurrent_postgres_index_disables_atomic_planning() {
		let mut migration = Migration::new("0001_index", "search");
		migration.operations.push(Operation::CreateIndex {
			table: "documents".to_string(),
			columns: vec!["title".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: true,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		});

		assert!(!migration_is_atomic(&migration, DatabaseType::Postgres));
		assert!(migration_is_atomic(&migration, DatabaseType::Sqlite));
	}

	#[async_trait]
	impl SqlPlanner for SentinelPlanner {
		async fn plan(
			&self,
			_connection: &DatabaseConnection,
			migration: &Migration,
			_state: &ProjectState,
			direction: MigrationDirection,
			_editor: &mut SchemaEditor,
		) -> Result<MigrationSqlPlan> {
			Ok(MigrationSqlPlan {
				atomic: migration.atomic,
				statements: vec![PlannedStatement::Sql(
					"CREATE TABLE \"planner_sentinel\" (\"id\" INTEGER)".to_string(),
				)],
				planned_operations: vec![None],
				sqlite_recreation_groups: vec![None],
				direction,
			})
		}
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn executor_executes_injected_plan_instead_of_compiling_operations() {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("connect to SQLite");
		let mut migration = Migration::new("0001_planner_spy", "planner");
		migration.operations.push(Operation::RunSQL {
			sql: "CREATE TABLE \"operation_compiler_table\" (\"id\" INTEGER)".to_string(),
			reverse_sql: Some("DROP TABLE \"operation_compiler_table\"".to_string()),
		});
		let mut executor = DatabaseMigrationExecutor::new_with_planner(
			connection.clone(),
			Arc::new(SentinelPlanner),
		);

		executor
			.apply_migrations(std::slice::from_ref(&migration))
			.await
			.expect("apply injected migration plan");

		let sentinel = connection
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'planner_sentinel'",
				vec![],
			)
			.await
			.expect("inspect sentinel table");
		let operation_table = connection
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'operation_compiler_table'",
				vec![],
			)
			.await
			.expect("inspect operation table");
		assert!(sentinel.is_some());
		assert!(operation_table.is_none());
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn rollback_vector_column_uses_reverse_executable_operation_context() {
		let operation = Operation::DropColumn {
			table: "documents".to_string(),
			column: "embedding".to_string(),
			old_definition: Some(ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 3 },
			)),
		};
		let planned_operation = operation
			.to_reverse_operation(&ProjectState::default())
			.expect("construct reverse operation")
			.expect("vector column drop is reversible");

		assert!(matches!(planned_operation, Operation::AddColumn { .. }));
		assert_eq!(
			planned_operation_context(Some(&planned_operation)),
			Some(PgvectorOperationKind::ColumnType)
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn rollback_vector_index_uses_reverse_executable_operation_context() {
		let operation = Operation::DropNamedIndex {
			table: "documents".to_string(),
			name: "documents_embedding_hnsw".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_l2_ops".to_string()),
		};
		let planned_operation = operation
			.to_reverse_operation(&ProjectState::default())
			.expect("construct reverse operation")
			.expect("named vector index drop is reversible");

		assert!(matches!(
			planned_operation,
			Operation::CreateNamedIndex { .. }
		));
		assert_eq!(
			planned_operation_context(Some(&planned_operation)),
			Some(PgvectorOperationKind::ApproximateIndex)
		);
	}

	#[test]
	fn test_optimizer_creation() {
		let optimizer = OperationOptimizer::new();
		let ops = vec![];
		let optimized = optimizer.optimize(ops);
		assert_eq!(optimized.len(), 0);
	}

	#[test]
	fn test_reorder_create_before_add() {
		let optimizer = OperationOptimizer::new();

		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("name", FieldType::VarChar(100)),
				mysql_options: None,
			},
			Operation::CreateTable {
				name: "users".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
		];

		let optimized = optimizer.optimize(ops);

		// CreateTable should come before AddColumn
		assert!(matches!(optimized[0], Operation::CreateTable { .. }));
		assert!(matches!(optimized[1], Operation::AddColumn { .. }));
	}

	#[test]
	fn test_remove_duplicate_create_table() {
		let optimizer = OperationOptimizer::new();

		let ops = vec![
			Operation::CreateTable {
				name: "users".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
			Operation::CreateTable {
				name: "users".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
		];

		let optimized = optimizer.optimize(ops);
		assert_eq!(optimized.len(), 1);
	}

	#[test]
	fn test_group_operations_by_table() {
		let optimizer = OperationOptimizer::new();

		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("name", FieldType::VarChar(100)),
				mysql_options: None,
			},
			Operation::CreateTable {
				name: "posts".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("email", FieldType::VarChar(255)),
				mysql_options: None,
			},
		];

		let optimized = optimizer.optimize(ops);
		assert_eq!(optimized.len(), 3);
	}

	#[cfg(test)]
	mod split_sql_tests {
		use crate::migrations::executor::split_sql_statements;

		#[test]
		fn test_split_simple_statements() {
			let sql = "CREATE TABLE t1 (id INT); CREATE TABLE t2 (id INT);";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
			assert_eq!(result[1], "CREATE TABLE t2 (id INT)");
		}

		#[test]
		fn test_split_with_string_literals() {
			let sql = r#"INSERT INTO t VALUES ('a;b'); INSERT INTO t VALUES ('c;d');"#;
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert_eq!(result[0], "INSERT INTO t VALUES ('a;b')");
			assert_eq!(result[1], "INSERT INTO t VALUES ('c;d')");
		}

		#[test]
		fn test_split_with_line_comments() {
			// Line comment after semicolon becomes part of next statement
			let sql =
				"CREATE TABLE t1 (id INT); -- comment; with semicolon\nCREATE TABLE t2 (id INT);";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
			assert!(result[1].contains("-- comment"));
			assert!(result[1].contains("CREATE TABLE t2"));
		}

		#[test]
		fn test_split_with_block_comments() {
			let sql =
				"CREATE TABLE t1 (id INT); /* comment; with semicolon */ CREATE TABLE t2 (id INT);";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert!(result[0].contains("CREATE TABLE t1"));
			assert!(result[1].contains("CREATE TABLE t2"));
		}

		#[test]
		fn test_split_with_dollar_quotes() {
			let sql = r#"CREATE FUNCTION f() RETURNS text AS $$SELECT 'value; with semicolon';$$ LANGUAGE sql; CREATE TABLE t1 (id INT);"#;
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert!(result[0].contains("CREATE FUNCTION"));
			assert!(result[0].contains("value; with semicolon"));
			assert!(result[1].contains("CREATE TABLE t1"));
		}

		#[test]
		fn test_split_with_escaped_quotes() {
			let sql = r#"INSERT INTO t VALUES ('it''s a test; value'); INSERT INTO t VALUES ('another');"#;
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert!(result[0].contains("it''s a test; value"));
			assert!(result[1].contains("another"));
		}

		#[test]
		fn test_split_empty_statements() {
			let sql = ";;; CREATE TABLE t1 (id INT); ;";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 1);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
		}

		#[test]
		fn test_split_no_semicolon() {
			let sql = "CREATE TABLE t1 (id INT)";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 1);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
		}

		#[test]
		fn test_split_whitespace_handling() {
			let sql = "  CREATE TABLE t1 (id INT)  ;  \n\n  CREATE TABLE t2 (id INT)  ;  ";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
			assert_eq!(result[1], "CREATE TABLE t2 (id INT)");
		}

		#[test]
		fn test_split_reinhardt_query_migration_sql() {
			// Actual SQL generated by reinhardt-query for polls migration (from diagnostic test)
			let sql = r###"CREATE TABLE "questions_table" ( "id" bigint GENERATED BY DEFAULT AS IDENTITY NOT NULL PRIMARY KEY, "question_text" text NOT NULL, "pub_date" timestamp with time zone NOT NULL );

CREATE TABLE "choices_table" ( "id" bigint GENERATED BY DEFAULT AS IDENTITY NOT NULL PRIMARY KEY, "question_id" bigint NOT NULL, "choice_text" text NOT NULL, "votes" integer NOT NULL DEFAULT 0, FOREIGN KEY ("question_id") REFERENCES "questions_table" ("id") ON DELETE CASCADE );"###;
			let result = split_sql_statements(sql);

			// Should split into exactly 2 statements (not 3 with empty string)
			assert_eq!(
				result.len(),
				2,
				"Expected 2 statements, got {}",
				result.len()
			);

			// First statement should be CREATE TABLE questions_table
			assert!(
				result[0].contains("questions_table"),
				"First statement should contain 'questions_table'"
			);
			assert!(
				result[0].contains("question_text"),
				"First statement should contain 'question_text'"
			);
			assert!(
				!result[0].contains("choices_table"),
				"First statement should not contain 'choices_table'"
			);

			// Second statement should be CREATE TABLE choices_table
			assert!(
				result[1].contains("choices_table"),
				"Second statement should contain 'choices_table'"
			);
			assert!(
				result[1].contains("choice_text"),
				"Second statement should contain 'choice_text'"
			);
			// Verify reference to questions_table, as FOREIGN KEY constraint contains referenced table name
			assert!(
				result[1].contains("FOREIGN KEY"),
				"Second statement should contain FOREIGN KEY constraint"
			);
			assert!(
				result[1].contains("REFERENCES \"questions_table\""),
				"FOREIGN KEY should reference questions_table"
			);
		}

		#[test]
		fn test_split_multiple_foreign_keys() {
			// Case where table has multiple FOREIGN KEY constraints
			let sql = r###"CREATE TABLE "posts" ("id" bigint PRIMARY KEY);
CREATE TABLE "users" ("id" bigint PRIMARY KEY);
CREATE TABLE "comments" (
	"id" bigint PRIMARY KEY,
	"post_id" bigint,
	"user_id" bigint,
	FOREIGN KEY ("post_id") REFERENCES "posts" ("id"),
	FOREIGN KEY ("user_id") REFERENCES "users" ("id")
);"###;

			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 3, "Expected 3 statements");

			// Third statement contains 2 FOREIGN KEY constraints
			assert_eq!(
				result[2].matches("FOREIGN KEY").count(),
				2,
				"Third statement should contain 2 FOREIGN KEY constraints"
			);
			assert!(
				result[2].contains("REFERENCES \"posts\""),
				"Should reference posts table"
			);
			assert!(
				result[2].contains("REFERENCES \"users\""),
				"Should reference users table"
			);
		}

		#[test]
		fn test_split_mixed_constraints() {
			// Case with mixed CHECK constraint and FOREIGN KEY
			let sql = r###"CREATE TABLE "tasks" ("id" bigint PRIMARY KEY);
CREATE TABLE "task_status" (
	"id" bigint PRIMARY KEY,
	"task_id" bigint,
	"status" text CHECK (status IN ('pending', 'completed')),
	FOREIGN KEY ("task_id") REFERENCES "tasks" ("id")
);"###;

			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);

			// Second statement contains both CHECK constraint and FOREIGN KEY constraint
			assert!(
				result[1].contains("CHECK"),
				"Second statement should contain CHECK constraint"
			);
			assert!(
				result[1].contains("FOREIGN KEY"),
				"Second statement should contain FOREIGN KEY constraint"
			);
		}

		#[test]
		fn test_split_self_referencing_foreign_key() {
			// Case with self-referencing FOREIGN KEY
			let sql = r###"CREATE TABLE "categories" (
	"id" bigint PRIMARY KEY,
	"parent_id" bigint,
	FOREIGN KEY ("parent_id") REFERENCES "categories" ("id")
);"###;

			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 1);

			// FOREIGN KEY referencing the same table
			assert!(
				result[0].contains("REFERENCES \"categories\""),
				"Should self-reference categories table"
			);
		}

		#[test]
		fn test_split_create_index_statements() {
			// Splitting CREATE INDEX statements
			let sql = r###"CREATE TABLE "products" ("id" bigint PRIMARY KEY, "name" text);
CREATE INDEX "idx_products_name" ON "products" ("name");
CREATE UNIQUE INDEX "idx_products_id" ON "products" ("id");"###;

			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 3);

			assert!(
				result[0].contains("CREATE TABLE"),
				"First statement should be CREATE TABLE"
			);
			assert!(
				result[1].contains("CREATE INDEX"),
				"Second statement should be CREATE INDEX"
			);
			assert!(
				result[2].contains("CREATE UNIQUE INDEX"),
				"Third statement should be CREATE UNIQUE INDEX"
			);
		}
	}
}

#[cfg(all(test, feature = "pgvector"))]
mod vector_index_validation_state_tests {
	use super::*;
	use crate::migrations::{ColumnDefinition, FieldType, Migration};

	fn vector_index(table: &str) -> Operation {
		Operation::CreateIndex {
			table: table.to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(crate::migrations::operations::IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		}
	}

	#[test]
	fn validation_state_carries_vector_columns_across_ordered_migrations() {
		// Arrange
		let mut create_table = Migration::new("0001_initial", "search");
		create_table.operations.push(Operation::CreateTable {
			name: "documents".to_string(),
			columns: vec![ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 1536 },
			)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		let mut create_index = Migration::new("0002_embedding_index", "search");
		create_index.operations.push(vector_index("documents"));
		let mut state = crate::migrations::ProjectState::new();

		// Act
		validate_and_advance_migration_state(&create_table, &mut state).unwrap();
		let result = validate_and_advance_migration_state(&create_index, &mut state);

		// Assert
		result.unwrap();
	}

	#[test]
	fn database_only_migration_advances_transient_vector_validation_state() {
		// Arrange
		let mut migration = Migration::new("0001_database_only", "search");
		migration.database_only = true;
		migration.operations.push(Operation::CreateTable {
			name: "documents".to_string(),
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		migration.operations.push(Operation::AddColumn {
			table: "documents".to_string(),
			column: ColumnDefinition::new("embedding", FieldType::Vector { dimensions: 1536 }),
			mysql_options: None,
		});
		migration.operations.push(vector_index("documents"));
		let mut state = crate::migrations::ProjectState::new();

		// Act
		let result = validate_and_advance_migration_state(&migration, &mut state);

		// Assert
		result.unwrap();
	}

	#[cfg(feature = "sqlite")]
	async fn sqlite_executor() -> DatabaseMigrationExecutor {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		DatabaseMigrationExecutor::new(connection)
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn incremental_apply_defers_unknown_vector_index_table_to_backend() {
		// Arrange
		let mut create_table = Migration::new("0001_initial", "search_partial");
		create_table.state_only = true;
		create_table.operations.push(Operation::CreateTable {
			name: "partial_documents".to_string(),
			columns: vec![ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 1536 },
			)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		let mut create_index = Migration::new("0002_embedding_index", "search_partial");
		create_index
			.operations
			.push(vector_index("partial_documents"));
		let mut executor = sqlite_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&create_table))
			.await
			.unwrap();

		// Act
		let result = executor
			.apply_migrations(std::slice::from_ref(&create_index))
			.await;

		// Assert
		assert!(matches!(
			result,
			Err(MigrationError::UnsupportedBackendFeature {
				feature: "approximate vector indexes",
				backend: "sqlite",
			})
		));
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn incremental_plan_defers_unknown_vector_index_table_to_backend() {
		// Arrange
		let mut create_table = Migration::new("0001_initial", "search_partial_plan");
		create_table.state_only = true;
		create_table.operations.push(Operation::CreateTable {
			name: "partial_plan_documents".to_string(),
			columns: vec![ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 1536 },
			)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		let mut create_index = Migration::new("0002_embedding_index", "search_partial_plan");
		create_index
			.operations
			.push(vector_index("partial_plan_documents"));
		let initial_plan = MigrationPlan::new().with_migration(create_table);
		let incremental_plan = MigrationPlan::new().with_migration(create_index);
		let mut executor = sqlite_executor().await;
		executor.apply(&initial_plan).await.unwrap();

		// Act
		let result = executor.apply(&incremental_plan).await;

		// Assert
		assert!(matches!(
			result,
			Err(MigrationError::UnsupportedBackendFeature {
				feature: "approximate vector indexes",
				backend: "sqlite",
			})
		));
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn apply_rejects_missing_vector_index_column_when_table_is_known() {
		// Arrange
		let mut create_table = Migration::new("0001_initial", "search_known");
		create_table.state_only = true;
		create_table.operations.push(Operation::CreateTable {
			name: "known_documents".to_string(),
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		let mut create_index = Migration::new("0002_embedding_index", "search_known");
		create_index
			.operations
			.push(vector_index("known_documents"));
		let mut executor = sqlite_executor().await;

		// Act
		let result = executor
			.apply_migrations(&[create_table, create_index])
			.await;

		// Assert
		assert!(matches!(
			result,
			Err(MigrationError::InvalidMigration(message))
				if message == "approximate vector index on table `known_documents` targets unknown column `embedding`"
		));
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn apply_rejects_scalar_vector_index_column_when_table_is_known() {
		// Arrange
		let mut create_table = Migration::new("0001_initial", "search_scalar");
		create_table.state_only = true;
		create_table.operations.push(Operation::CreateTable {
			name: "scalar_documents".to_string(),
			columns: vec![ColumnDefinition::new("embedding", FieldType::Text)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		let mut create_index = Migration::new("0002_embedding_index", "search_scalar");
		create_index
			.operations
			.push(vector_index("scalar_documents"));
		let mut executor = sqlite_executor().await;

		// Act
		let result = executor
			.apply_migrations(&[create_table, create_index])
			.await;

		// Assert
		assert!(matches!(
			result,
			Err(MigrationError::InvalidMigration(message))
				if message == "approximate vector index on table `scalar_documents` targets non-vector column `embedding`"
		));
	}
}

#[cfg(all(test, feature = "pgvector", feature = "sqlite"))]
mod cockroachdb_executor_dialect_tests {
	use super::*;
	use crate::migrations::{ColumnDefinition, FieldType};

	async fn cockroachdb_flavored_executor() -> DatabaseMigrationExecutor {
		let sqlite = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("the test SQLite backend should connect");
		let connection = DatabaseConnection::new_with_flavor(sqlite.backend(), true);
		DatabaseMigrationExecutor::new(connection)
	}

	fn create_extension_migration() -> Migration {
		let mut migration = Migration::new("0001_vector_extension", "search");
		migration.operations.push(Operation::CreateExtension {
			name: "vector".to_string(),
			if_not_exists: true,
			schema: None,
		});
		migration
	}

	#[tokio::test]
	async fn apply_path_rejects_extension_for_cockroachdb_flavor() {
		let executor = cockroachdb_flavored_executor().await;

		let result = executor
			.apply_migration(&create_extension_migration())
			.await;

		assert!(matches!(
			result,
			Err(MigrationError::UnsupportedBackendFeature {
				feature: "PostgreSQL extensions",
				backend: "cockroachdb",
			})
		));
	}

	#[tokio::test]
	async fn rollback_path_rejects_extension_for_cockroachdb_flavor() {
		let mut executor = cockroachdb_flavored_executor().await;

		let result = executor
			.rollback_migration(&create_extension_migration())
			.await;

		assert!(matches!(
			result,
			Err(MigrationError::UnsupportedBackendFeature {
				feature: "PostgreSQL extensions",
				backend: "cockroachdb",
			})
		));
	}

	#[tokio::test]
	async fn ordinary_cockroachdb_migration_still_executes() {
		let executor = cockroachdb_flavored_executor().await;
		let mut migration = Migration::new("0001_documents", "search");
		migration.operations.push(Operation::CreateTable {
			name: "documents".to_string(),
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});

		executor
			.apply_migration(&migration)
			.await
			.expect("ordinary CockroachDB migrations should remain executable");

		let table = executor
			.connection()
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
				vec!["documents".into()],
			)
			.await
			.expect("the test backend should inspect its schema");
		assert!(table.is_some());
	}
}

#[cfg(all(test, feature = "sqlite"))]
mod rollback_orchestration_tests {
	//! In-crate tests for [`DatabaseMigrationExecutor::rollback_migrations`] and
	//! the private [`DatabaseMigrationExecutor::rollback_migration`].
	//!
	//! These exercise the orchestration layer — reverse iteration of the input
	//! slice, recorder synchronisation, the `state_only` short-circuit, and the
	//! warn-and-continue behaviour for `RunSQL` operations that lack a
	//! `reverse_sql`. Per-`Operation` SQL generation is already covered by the
	//! `to_reverse_sql` tests in `operations.rs`; this module focuses on what
	//! sits above those primitives in `executor.rs`.
	//!
	//! A real SQLite `:memory:` connection is used rather than test doubles
	//! because [`DatabaseConnection`], [`DatabaseMigrationRecorder`], and
	//! [`SchemaEditor`] are concrete types with no trait abstraction today.
	//! `:memory:` is the lightest available substitute — sub-second per case,
	//! no external services, and gated behind the `sqlite` feature that the
	//! rollback path itself already references.
	//!
	//! Not covered here (require failure injection, which needs a trait
	//! refactor to land first): non-atomic partial-rollback bookkeeping when
	//! an operation fails mid-run, atomic-mode rollback-on-failure, and
	//! foreign-key violation behaviour during rollback. Those paths remain
	//! exercised by the container-backed suite in
	//! `tests/integration/tests/migrations/migration_rollback_integration.rs`.

	use super::*;
	use crate::backends::DatabaseConnection;
	use crate::field_domain::{FieldDomain, ModelEnumRepr, ModelEnumValue};
	use crate::migrations::recorder::DatabaseMigrationRecorder;
	use crate::migrations::{
		ColumnDefinition, Constraint, FieldType, ForeignKeyAction, GeneratedColumnDefinition,
		Migration,
	};
	use reinhardt_query::prelude::GeneratedStorage;
	use rstest::*;

	/// Open a fresh SQLite `:memory:` database and wrap it in a
	/// [`DatabaseMigrationExecutor`]. Each call returns an isolated database.
	async fn make_executor() -> DatabaseMigrationExecutor {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("failed to open sqlite :memory: connection");
		DatabaseMigrationExecutor::new(connection)
	}

	#[rstest]
	#[tokio::test]
	#[cfg(feature = "pgvector")]
	async fn sqlite_vector_alter_column_recreation_returns_a_structured_error() {
		let mut initial = Migration::new("0001_initial", "vector_recreation");
		initial.operations.push(Operation::CreateTable {
			name: "documents".to_owned(),
			columns: vec![ColumnDefinition::new("embedding", FieldType::Text)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		let mut alter = Migration::new("0002_vector", "vector_recreation");
		alter
			.dependencies
			.push(("vector_recreation".to_owned(), "0001_initial".to_owned()));
		alter.operations.push(Operation::AlterColumn {
			table: "documents".to_owned(),
			column: "embedding".to_owned(),
			old_definition: Some(ColumnDefinition::new("embedding", FieldType::Text)),
			new_definition: ColumnDefinition::new("embedding", FieldType::Vector { dimensions: 3 }),
			mysql_options: None,
		});
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&initial))
			.await
			.unwrap();

		let error = executor
			.apply_migrations(std::slice::from_ref(&alter))
			.await
			.unwrap_err();

		assert!(matches!(
			error,
			MigrationError::UnsupportedBackendFeature {
				feature: "vector field",
				backend: "sqlite",
			}
		));
	}

	/// Build a single-operation `CreateTable` migration with one integer
	/// primary key column.
	fn make_create_table_migration(name: &str, table: &str) -> Migration {
		let mut migration = Migration::new(name, "rolltest");
		migration.operations.push(Operation::CreateTable {
			name: table.to_string(),
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		migration
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_with_empty_input_returns_empty_result() {
		// Arrange
		let mut executor = make_executor().await;

		// Act
		let result = executor
			.rollback_migrations(&[])
			.await
			.expect("rollback of empty input should not fail");

		// Assert
		assert!(
			result.applied.is_empty(),
			"no migrations should be reported as rolled back"
		);
		assert!(result.failed.is_none(), "no failure should be reported");
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_iterates_input_slice_in_reverse_order() {
		// Arrange - apply three independent migrations.
		let m1 = make_create_table_migration("0001_a", "rolltest_a");
		let m2 = make_create_table_migration("0002_b", "rolltest_b");
		let m3 = make_create_table_migration("0003_c", "rolltest_c");
		let mut executor = make_executor().await;
		executor
			.apply_migrations(&[m1.clone(), m2.clone(), m3.clone()])
			.await
			.expect("apply m1..m3");

		// Act - pass the slice in declaration order; rollback must consume in reverse.
		let result = executor
			.rollback_migrations(&[m1.clone(), m2.clone(), m3.clone()])
			.await
			.expect("rollback m1..m3");

		// Assert - newest-first order, independent of how apply_migrations
		// ordered them via the topological sort.
		assert_eq!(
			result.applied,
			vec![m3.id(), m2.id(), m1.id()],
			"rollback_migrations must iterate input in reverse"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn replacement_history_is_recorded_as_a_single_rollback_unit() {
		// Arrange
		let initial = make_create_table_migration("0001_initial", "rolltest_replaced");
		let mut add_column = Migration::new("0002_add_name", "rolltest");
		add_column.operations.push(Operation::AddColumn {
			table: "rolltest_replaced".to_string(),
			column: ColumnDefinition::new("name", FieldType::VarChar(32)),
			mysql_options: None,
		});
		let mut replacement = initial.clone();
		replacement.name = "0001_squashed_0002".to_string();
		replacement.operations.extend(add_column.operations.clone());
		replacement.replaces = vec![
			(initial.app_label.clone(), initial.name.clone()),
			(add_column.app_label.clone(), add_column.name.clone()),
		];
		let mut executor = make_executor().await;
		executor
			.apply_migrations(&[initial.clone(), add_column.clone()])
			.await
			.expect("apply original history");

		// Act
		executor
			.apply_migrations(&[initial.clone(), add_column.clone(), replacement.clone()])
			.await
			.expect("record equivalent replacement history");
		let result = executor
			.rollback_migrations(&[initial, add_column, replacement.clone()])
			.await
			.expect("rollback replacement history");

		// Assert
		assert_eq!(result.applied, vec![replacement.id()]);
	}

	#[rstest]
	#[tokio::test]
	async fn partial_replacement_history_applies_the_remaining_original_migration() {
		// Arrange
		let initial = make_create_table_migration("0001_initial", "partial_replacement");
		let mut add_column = Migration::new("0002_add_name", "rolltest");
		add_column
			.dependencies
			.push((initial.app_label.clone(), initial.name.clone()));
		add_column.operations.push(Operation::AddColumn {
			table: "partial_replacement".to_string(),
			column: ColumnDefinition::new("name", FieldType::VarChar(32)),
			mysql_options: None,
		});
		let mut replacement = initial.clone();
		replacement.name = "0001_squashed_0002".to_string();
		replacement.operations.extend(add_column.operations.clone());
		replacement.replaces = vec![
			(initial.app_label.clone(), initial.name.clone()),
			(add_column.app_label.clone(), add_column.name.clone()),
		];
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&initial))
			.await
			.expect("apply the first original migration");

		// Act
		let result = executor
			.apply_migrations(&[initial.clone(), add_column.clone(), replacement.clone()])
			.await
			.expect("a partial original history must continue on the original chain");

		// Assert
		assert_eq!(result.applied, vec![add_column.id()]);
		let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
		assert_eq!(
			recorder
				.is_applied(&replacement.app_label, &replacement.name)
				.await
				.expect("query replacement recorder state"),
			false
		);
	}

	#[rstest]
	#[tokio::test]
	async fn partial_replacement_history_orders_descendants_after_remaining_originals() {
		// Arrange
		let initial = make_create_table_migration("0001_initial", "partial_descendant");
		let mut remaining = Migration::new("0003_add_name", "partial_descendant");
		remaining
			.dependencies
			.push((initial.app_label.clone(), initial.name.clone()));
		remaining.operations.push(Operation::AddColumn {
			table: "partial_descendant".to_string(),
			column: ColumnDefinition::new("name", FieldType::VarChar(32)),
			mysql_options: None,
		});
		let mut replacement = initial.clone();
		replacement.name = "0001_squashed_0003".to_string();
		replacement.operations.extend(remaining.operations.clone());
		replacement.replaces = vec![
			(initial.app_label.clone(), initial.name.clone()),
			(remaining.app_label.clone(), remaining.name.clone()),
		];
		let mut descendant = Migration::new("0002_after_squash", "partial_descendant");
		descendant
			.dependencies
			.push((replacement.app_label.clone(), replacement.name.clone()));
		descendant.operations.push(Operation::AddColumn {
			table: "partial_descendant".to_string(),
			column: ColumnDefinition::new("after_squash", FieldType::VarChar(32)),
			mysql_options: None,
		});
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&initial))
			.await
			.expect("apply the first original migration");

		// Act
		let result = executor
			.apply_migrations(&[initial, remaining.clone(), replacement, descendant.clone()])
			.await
			.expect("a descendant must wait for the remaining original migration");

		// Assert
		assert_eq!(result.applied, vec![remaining.id(), descendant.id()]);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_skips_migrations_that_were_never_applied() {
		// Arrange - apply only m1, leave m2 unrecorded.
		let m1 = make_create_table_migration("0001_first", "rolltest_first");
		let m2 = make_create_table_migration("0002_second", "rolltest_second");
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&m1))
			.await
			.expect("apply m1");

		// Act - ask to roll both back.
		let result = executor
			.rollback_migrations(&[m1.clone(), m2.clone()])
			.await
			.expect("rollback should succeed even with an unapplied entry");

		// Assert - only the actually-applied migration is reported as rolled back.
		assert_eq!(
			result.applied,
			vec![m1.id()],
			"unapplied migrations must be silently skipped, not rolled back"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_clears_recorder_state_for_rolled_back_migration() {
		// Arrange
		let migration = make_create_table_migration("0001_only", "rolltest_only");
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&migration))
			.await
			.expect("apply migration");

		let recorder_before = DatabaseMigrationRecorder::new(executor.connection().clone());
		assert!(
			recorder_before
				.is_applied(&migration.app_label, &migration.name)
				.await
				.expect("query recorder before"),
			"sanity: migration should be applied before rollback"
		);

		// Act
		executor
			.rollback_migrations(std::slice::from_ref(&migration))
			.await
			.expect("rollback");

		// Assert - recorder no longer marks the migration as applied.
		let recorder_after = DatabaseMigrationRecorder::new(executor.connection().clone());
		assert!(
			!recorder_after
				.is_applied(&migration.app_label, &migration.name)
				.await
				.expect("query recorder after"),
			"recorder must report unapplied after successful rollback"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_with_state_only_flag_skips_schema_changes() {
		// Arrange - apply a normal migration so the table really exists.
		let mut migration = make_create_table_migration("0001_state", "rolltest_state");
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&migration))
			.await
			.expect("apply migration to create the table");

		// Re-mark the migration as state_only for the rollback request,
		// which must hit the `state_only` short-circuit in
		// `rollback_migration` and skip schema operations entirely.
		migration.state_only = true;

		// Act
		let result = executor
			.rollback_migrations(std::slice::from_ref(&migration))
			.await
			.expect("state_only rollback should succeed without DB ops");

		// Assert - rollback is recorded, but the table is intentionally still
		// present because no DROP TABLE was issued.
		assert_eq!(result.applied, vec![migration.id()]);

		let table_still_present = executor
			.connection()
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
				vec!["rolltest_state".into()],
			)
			.await
			.expect("introspect sqlite_master")
			.is_some();
		assert!(
			table_still_present,
			"state_only rollback must not execute schema operations"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn apply_migrations_uses_one_side_of_a_replacement_set() {
		// Arrange
		let original = make_create_table_migration("0001_initial", "replacement");
		let mut replacement = original.clone();
		replacement.name = "0001_squashed".to_string();
		replacement.replaces = vec![("rolltest".to_string(), "0001_initial".to_string())];

		// Act - a fresh database must apply only the replacement migration.
		let mut fresh = make_executor().await;
		let fresh_result = fresh
			.apply_migrations(&[original.clone(), replacement.clone()])
			.await
			.expect("apply a fresh replacement set");

		// Assert
		let fresh_recorder = DatabaseMigrationRecorder::new(fresh.connection().clone());
		assert_eq!(fresh_result.applied, vec![replacement.id()]);
		assert!(
			fresh_recorder
				.is_applied("rolltest", "0001_squashed")
				.await
				.expect("query replacement recorder state")
		);
		assert!(
			!fresh_recorder
				.is_applied("rolltest", "0001_initial")
				.await
				.expect("query original recorder state")
		);

		// Act - an existing complete original set is adopted by the replacement.
		let mut existing = make_executor().await;
		existing
			.apply_migrations(std::slice::from_ref(&original))
			.await
			.expect("apply original migration");
		existing
			.apply_migrations(std::slice::from_ref(&replacement))
			.await
			.expect("adopt an existing original replacement set after its source file is removed");
		let existing_recorder = DatabaseMigrationRecorder::new(existing.connection().clone());
		assert!(
			existing_recorder
				.is_applied("rolltest", "0001_squashed")
				.await
				.expect("query replacement recorder state")
		);
		assert!(
			!existing_recorder
				.is_applied("rolltest", "0001_initial")
				.await
				.expect("query original recorder state")
		);

		let rollback = existing
			.rollback_migrations(&[original, replacement])
			.await
			.expect("rollback the adopted replacement once");
		assert_eq!(rollback.applied, vec!["rolltest.0001_squashed"]);
	}

	#[test]
	fn replacement_selection_recognizes_applied_nested_replacement_history() {
		let first = Migration::new("0001_first", "rolltest");
		let second = Migration::new("0002_second", "rolltest");
		let mut older = Migration::new("0001_squashed_0002", "rolltest");
		older.replaces = vec![
			("rolltest".to_string(), "0001_first".to_string()),
			("rolltest".to_string(), "0002_second".to_string()),
		];
		let mut newer = Migration::new("0001_squashed_0002_again", "rolltest");
		newer.replaces = vec![
			("rolltest".to_string(), "0001_first".to_string()),
			("rolltest".to_string(), "0002_second".to_string()),
			("rolltest".to_string(), "0001_squashed_0002".to_string()),
		];
		let applied = HashSet::from([super::super::MigrationKey::new(
			"rolltest",
			"0001_squashed_0002",
		)]);

		let migrations = [first, second, older, newer];
		let selection = select_replacement_migrations(&migrations, &applied)
			.expect("nested replacement history is complete through the applied squash");

		assert!(selection.migrations().is_empty());
		assert_eq!(
			selection
				.replacements_to_adopt()
				.iter()
				.map(|migration| migration.id())
				.collect::<Vec<_>>(),
			["rolltest.0001_squashed_0002_again"]
		);
	}

	#[rstest]
	#[tokio::test]
	async fn apply_migrations_does_not_adopt_before_validating_all_replacement_sets() {
		// Arrange - the first replacement can be adopted, while the second has
		// only one of two original migrations recorded.
		let original_one = make_create_table_migration("0001_initial", "replacement_one");
		let original_two = make_create_table_migration("0002_second", "replacement_two");
		let original_three = make_create_table_migration("0003_third", "replacement_three");

		let mut first_replacement = original_one.clone();
		first_replacement.name = "0001_squashed".to_string();
		first_replacement.replaces = vec![("rolltest".to_string(), "0001_initial".to_string())];

		let mut partial_replacement = original_two.clone();
		partial_replacement.name = "0002_squashed".to_string();
		partial_replacement.replaces = vec![
			("rolltest".to_string(), "0002_second".to_string()),
			("rolltest".to_string(), "0003_third".to_string()),
		];

		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&original_one))
			.await
			.expect("apply the first original migration");
		executor
			.apply_migrations(std::slice::from_ref(&original_two))
			.await
			.expect("apply one source migration from the partial set");

		// Act
		let error = executor
			.apply_migrations(&[
				original_one.clone(),
				original_two.clone(),
				original_three,
				first_replacement.clone(),
				partial_replacement,
			])
			.await
			.expect_err("a partial replacement set must fail");

		// Assert - no earlier adoption changed recorder state before the whole
		// replacement configuration was validated.
		assert!(matches!(error, MigrationError::InvalidMigration(_)));
		let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
		assert!(
			recorder
				.is_applied("rolltest", "0001_initial")
				.await
				.expect("query original recorder state"),
			"the original must remain recorded after a later replacement validation error"
		);
		assert!(
			!recorder
				.is_applied("rolltest", "0001_squashed")
				.await
				.expect("query replacement recorder state"),
			"the replacement must not be adopted before all replacement sets validate"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_run_sql_without_reverse_sql_completes_without_error() {
		// Pins the current contract of the `Operation::RunSQL` arm in
		// `rollback_migration` when `reverse_sql` is `None`: the rollback
		// logs a warning and continues. It does NOT return an error. If a
		// future change promotes that to a typed error, this test will fail
		// and force the change to be explicit rather than silent.
		let mut migration = Migration::new("0001_run_sql_noop", "rolltest");
		migration.operations.push(Operation::RunSQL {
			sql: "CREATE TABLE rolltest_runsql (id INTEGER PRIMARY KEY)".to_string(),
			reverse_sql: None,
		});

		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&migration))
			.await
			.expect("apply RunSQL migration");

		// Act
		let result = executor
			.rollback_migrations(std::slice::from_ref(&migration))
			.await
			.expect("rollback should succeed even when reverse_sql is missing");

		// Assert - the rollback is reported in `result.applied` and the
		// recorder is cleared even though no reverse SQL ran (the
		// warn-and-skip contract still updates bookkeeping).
		assert_eq!(result.applied, vec![migration.id()]);

		let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
		assert!(
			!recorder
				.is_applied(&migration.app_label, &migration.name)
				.await
				.expect("query recorder after"),
			"recorder must reflect unapplied state after warn-and-skip rollback"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_falls_back_to_reverse_sqlite_execution_after_opaque_sql() {
		// Arrange - rolling this migration back requires a SQLite table
		// recreation for `AddColumn`, while the RunSQL step is opaque to the
		// recreation planner. The planner must therefore fall back to the
		// inverse operation order instead of failing the rollback.
		let mut migration = Migration::new("0001_opaque_sqlite_rollback", "rolltest");
		migration.operations = vec![
			Operation::CreateTable {
				name: "opaque_sqlite_rollback".to_string(),
				columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::RunSQL {
				sql: "PRAGMA user_version = 7".to_string(),
				reverse_sql: Some("PRAGMA user_version = 0".to_string()),
			},
			Operation::AddColumn {
				table: "opaque_sqlite_rollback".to_string(),
				column: ColumnDefinition::new("name", FieldType::Text),
				mysql_options: None,
			},
		];

		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&migration))
			.await
			.expect("apply migration containing opaque SQL");

		// Act
		executor
			.rollback_migrations(std::slice::from_ref(&migration))
			.await
			.expect("rollback should use reverse sequential SQLite execution");

		// Assert - both the recreated table and the opaque operation are undone.
		let table = executor
			.connection()
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
				vec!["opaque_sqlite_rollback".into()],
			)
			.await
			.expect("inspect sqlite_master");
		assert!(table.is_none());

		let user_version = executor
			.connection()
			.fetch_optional("PRAGMA user_version", vec![])
			.await
			.expect("read SQLite user version")
			.expect("SQLite user_version always returns a row");
		assert_eq!(
			user_version
				.get::<i64>("user_version")
				.expect("read user_version column"),
			0
		);
	}

	#[rstest]
	#[tokio::test]
	async fn typed_enum_constraint_uses_sqlite_recreation_and_preserves_schema() {
		let mut parent_id = ColumnDefinition::new("id", FieldType::Integer);
		parent_id.primary_key = true;
		let mut job_id = ColumnDefinition::new("id", FieldType::Integer);
		job_id.primary_key = true;
		let parent_ref = ColumnDefinition::new("parent_id", FieldType::Integer);
		let mut status = ColumnDefinition::new("status", FieldType::VarChar(32));
		status.default = Some("'queued'".to_string());
		let mut status_copy = ColumnDefinition::new("status_copy", FieldType::VarChar(32));
		status_copy.generated = Some(GeneratedColumnDefinition::raw_sql(
			"status || '_copy'",
			GeneratedStorage::Stored,
		));

		let mut initial = Migration::new("0001_initial", "rolltest");
		initial.operations = vec![
			Operation::CreateTable {
				name: "enum_parents".to_string(),
				columns: vec![parent_id],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::CreateTable {
				name: "enum_jobs".to_string(),
				columns: vec![job_id, parent_ref, status, status_copy],
				constraints: vec![Constraint::ForeignKey {
					name: "enum_jobs_parent_fk".to_string(),
					columns: vec!["parent_id".to_string()],
					referenced_table: "enum_parents".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::NoAction,
					deferrable: None,
				}],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::CreateIndex {
				table: "enum_jobs".to_string(),
				columns: vec!["status".to_string()],
				unique: false,
				index_type: None,
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			},
		];
		let mut add_domain = Migration::new("0002_status_domain", "rolltest");
		add_domain
			.operations
			.push(Operation::AddConstraintDefinition {
				table: "enum_jobs".to_string(),
				constraint: Constraint::EnumDomain {
					name: "enum_jobs_status_model_enum_check".to_string(),
					column: "status".to_string(),
					domain: FieldDomain::Enum {
						repr: ModelEnumRepr::String,
						values: vec![
							ModelEnumValue::String("queued".to_string()),
							ModelEnumValue::String("running".to_string()),
						],
					},
				},
			});
		let mut executor = make_executor().await;

		executor
			.apply_migrations(&[initial, add_domain])
			.await
			.expect("typed enum constraint should recreate the SQLite table");

		let table_sql = executor
			.connection()
			.fetch_optional(
				"SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
				vec!["enum_jobs".into()],
			)
			.await
			.expect("read recreated table")
			.expect("enum_jobs should exist")
			.get::<String>("sql")
			.expect("table SQL should be text");
		assert!(
			table_sql.contains("enum_jobs_status_model_enum_check"),
			"{table_sql}"
		);
		assert!(table_sql.contains("REFERENCES enum_parents"), "{table_sql}");
		assert!(table_sql.contains("DEFAULT 'queued'"), "{table_sql}");
		assert!(table_sql.contains("GENERATED ALWAYS AS"), "{table_sql}");

		let index_exists = executor
			.connection()
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'index' AND name = ?",
				vec!["idx_enum_jobs_status".into()],
			)
			.await
			.expect("read recreated index")
			.is_some();
		assert_eq!(index_exists, true);
	}

	#[rstest]
	#[tokio::test]
	async fn enum_constraint_replacement_rollback_restores_old_domain_without_runtime_state() {
		let mut id = ColumnDefinition::new("id", FieldType::Integer);
		id.primary_key = true;
		let status = ColumnDefinition::new("status", FieldType::VarChar(32));
		let constraint_name = "enum_jobs_status_model_enum_check";
		let old_constraint = Constraint::EnumDomain {
			name: constraint_name.to_string(),
			column: "status".to_string(),
			domain: FieldDomain::Enum {
				repr: ModelEnumRepr::String,
				values: vec![ModelEnumValue::String("queued".to_string())],
			},
		};
		let new_constraint = Constraint::EnumDomain {
			name: constraint_name.to_string(),
			column: "status".to_string(),
			domain: FieldDomain::Enum {
				repr: ModelEnumRepr::String,
				values: vec![ModelEnumValue::String("running".to_string())],
			},
		};
		let mut initial = Migration::new("0001_initial", "rolltest");
		initial.operations.push(Operation::CreateTable {
			name: "enum_jobs".to_string(),
			columns: vec![id, status],
			constraints: vec![old_constraint.clone()],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		let mut replacement = Migration::new("0002_replace_status", "rolltest");
		replacement.operations = vec![
			Operation::DropConstraintDefinition {
				table: "enum_jobs".to_string(),
				constraint: old_constraint,
			},
			Operation::AddConstraintDefinition {
				table: "enum_jobs".to_string(),
				constraint: new_constraint,
			},
		];
		let mut executor = make_executor().await;
		executor
			.apply_migrations(&[initial, replacement.clone()])
			.await
			.expect("apply enum replacement");

		executor
			.rollback_migrations(std::slice::from_ref(&replacement))
			.await
			.expect("rollback enum replacement");

		let table_sql = executor
			.connection()
			.fetch_optional(
				"SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
				vec!["enum_jobs".into()],
			)
			.await
			.expect("read rolled-back table")
			.expect("enum_jobs should exist")
			.get::<String>("sql")
			.expect("table SQL should be text");
		assert!(table_sql.contains("IN ('queued')"), "{table_sql}");
		assert!(!table_sql.contains("IN ('running')"), "{table_sql}");
	}

	#[rstest]
	#[tokio::test]
	async fn typed_enum_constraint_recreation_preserves_without_rowid() {
		let mut id = ColumnDefinition::new("id", FieldType::Integer);
		id.primary_key = true;
		let status = ColumnDefinition::new("status", FieldType::VarChar(32));
		let mut initial = Migration::new("0001_without_rowid", "rolltest");
		initial.operations.push(Operation::CreateTable {
			name: "enum_jobs_without_rowid".to_string(),
			columns: vec![id, status],
			constraints: vec![],
			without_rowid: Some(true),
			interleave_in_parent: None,
			partition: None,
		});
		let mut add_domain = Migration::new("0002_status_domain", "rolltest");
		add_domain
			.operations
			.push(Operation::AddConstraintDefinition {
				table: "enum_jobs_without_rowid".to_string(),
				constraint: Constraint::EnumDomain {
					name: "enum_jobs_without_rowid_status_model_enum_check".to_string(),
					column: "status".to_string(),
					domain: FieldDomain::Enum {
						repr: ModelEnumRepr::String,
						values: vec![ModelEnumValue::String("queued".to_string())],
					},
				},
			});
		let mut executor = make_executor().await;

		executor
			.apply_migrations(&[initial, add_domain])
			.await
			.expect("typed enum constraint should recreate WITHOUT ROWID table");

		let table_sql = executor
			.connection()
			.fetch_optional(
				"SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
				vec!["enum_jobs_without_rowid".into()],
			)
			.await
			.expect("read recreated table")
			.expect("table should exist")
			.get::<String>("sql")
			.expect("table SQL should be text");
		assert!(
			table_sql.trim_end().ends_with("WITHOUT ROWID"),
			"{table_sql}"
		);
	}
}
