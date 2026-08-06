//! Built-in commands
//!
//! Standard management commands included with Reinhardt.

use crate::{BaseCommand, CommandArgument, CommandContext, CommandOption, CommandResult};
use async_trait::async_trait;

#[cfg(feature = "migrations")]
use reinhardt_db::migrations::DatabaseMigrationExecutor;

#[cfg(feature = "migrations")]
use reinhardt_db::backends::{DatabaseConnection, DatabaseType};

// Import DatabaseConnection for database_url_from (without migrations feature)
#[cfg(all(feature = "reinhardt-db", not(feature = "migrations")))]
use reinhardt_db::backends::DatabaseConnection;

// Import DatabaseType for connect_database helper
#[cfg(all(feature = "reinhardt-db", not(feature = "migrations")))]
use reinhardt_db::backends::DatabaseType;

// Import ShutdownCoordinator for runall command

/// Database migration command
pub struct MigrateCommand;

#[async_trait]
impl BaseCommand for MigrateCommand {
	fn name(&self) -> &str {
		"migrate"
	}

	fn description(&self) -> &str {
		"Run database migrations"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![
			CommandArgument::optional("app", "App name to migrate"),
			CommandArgument::optional("migration", "Migration name"),
		]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(None, "fake", "Mark migrations as run without executing"),
			CommandOption::flag(
				None,
				"fake-initial",
				"Skip initial migration if tables exist",
			),
			CommandOption::flag(
				None,
				"plan",
				"Preview the migration plan without applying or rolling back",
			),
			CommandOption::option(
				None,
				"migrations-dir",
				"Root directory containing migration files (default: ./migrations)",
			),
			CommandOption::option(Some('d'), "database", "Database to migrate")
				.with_default("default"),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		ctx.info("Running migrations...");

		let app_label = ctx.arg(0).map(|s| s.to_string());
		let target = ctx.arg(1).map(|s| s.to_string());
		let is_fake = ctx.has_option("fake");
		let _is_fake_initial = ctx.has_option("fake-initial");
		#[cfg_attr(not(feature = "migrations"), allow(unused_variables))]
		let is_plan = ctx.has_option("plan");
		let _database = ctx
			.option("database")
			.map(|s| s.to_string())
			.unwrap_or_else(|| "default".to_string());

		if let Some(ref app_name) = app_label {
			if let Some(ref migration) = target {
				ctx.verbose(&format!("Migrating {} to {}", app_name, migration));
			} else {
				ctx.verbose(&format!("Migrating app: {}", app_name));
			}
		} else {
			ctx.verbose("Migrating all apps");
		}

		if is_fake {
			ctx.warning("Fake mode: Migrations will be marked as applied without running");
		}

		// Use reinhardt-migrations for migration execution
		#[cfg(feature = "migrations")]
		{
			use reinhardt_db::migrations::{
				FilesystemRepository, FilesystemSource, MigrationService,
			};
			use std::path::PathBuf;
			use std::sync::Arc;
			use tokio::sync::Mutex;

			ctx.verbose("Loading migrations from disk...");
			let migrations_dir = ctx
				.option("migrations-dir")
				.map(PathBuf::from)
				.unwrap_or_else(|| PathBuf::from("migrations"));

			let source = Arc::new(FilesystemSource::new(migrations_dir.clone()));
			let repository: Arc<Mutex<dyn reinhardt_db::migrations::MigrationRepository>> =
				Arc::new(Mutex::new(FilesystemRepository::new(migrations_dir)));
			let service = MigrationService::new(source, repository);
			// Filter by app if specified
			let all_migrations = service.load_all().await.map_err(|e| {
				crate::CommandError::ExecutionError(format!(
					"Failed to load all migrations: {:?}",
					e
				))
			})?;

			// 3. Validate database URL early (before filtering migrations)
			// Use database URL from context option if provided, otherwise fall back to environment
			let database_url = ctx
				.option("database")
				.map(|s| s.to_string())
				.or_else(|| std::env::var("DATABASE_URL").ok())
				.ok_or_else(|| {
					crate::CommandError::ExecutionError(
						"No database URL provided. Use --database option or set DATABASE_URL environment variable".to_string()
					)
				})?;

			// Validate database URL scheme
			if !database_url.starts_with("postgres://")
				&& !database_url.starts_with("postgresql://")
				&& !database_url.starts_with("sqlite://")
				&& !database_url.starts_with("sqlite:")
				&& !database_url.starts_with("mysql://")
			{
				return Err(crate::CommandError::ExecutionError(format!(
					"Unsupported database URL scheme: {}",
					database_url
				)));
			}

			// 4. Connect to database (auto-create if it doesn't exist for PostgreSQL)
			// This is done before filtering migrations to ensure connection errors are detected
			// even when no migrations need to be applied
			let connection: DatabaseConnection = if database_url.starts_with("postgres://")
				|| database_url.starts_with("postgresql://")
			{
				#[cfg(feature = "postgres")]
				{
					DatabaseConnection::connect_postgres_or_create(&database_url).await
				}
				#[cfg(not(feature = "postgres"))]
				{
					return Err(crate::CommandError::ExecutionError(
						"PostgreSQL support not enabled. Enable 'postgres' feature.".to_string(),
					));
				}
			} else if database_url.starts_with("mysql://") {
				#[cfg(feature = "mysql")]
				{
					DatabaseConnection::connect_mysql(&database_url).await
				}
				#[cfg(not(feature = "mysql"))]
				{
					return Err(crate::CommandError::ExecutionError(
						"MySQL support not enabled. Enable 'mysql' feature.".to_string(),
					));
				}
			} else {
				// Must be SQLite (validated above)
				#[cfg(feature = "sqlite")]
				{
					DatabaseConnection::connect_sqlite(&database_url).await
				}
				#[cfg(not(feature = "sqlite"))]
				{
					return Err(crate::CommandError::ExecutionError(
						"SQLite support not enabled. Enable 'sqlite' feature.".to_string(),
					));
				}
			}
			.map_err(|e| {
				crate::CommandError::ExecutionError(format!(
					"Failed to connect to database: {:?}",
					e
				))
			})?;

			// 4.5. Direction detection (Django-style migrate-with-target semantics).
			//
			// `manage migrate <app> <target>` expresses both apply and unapply in a
			// single command; the direction is resolved by comparing `<target>`
			// against the currently applied state for `<app>`:
			//
			//   * `<target> == "zero"`  -> unapply ALL migrations for `<app>`.
			//   * `<target>` is currently applied -> roll back every migration
			//                            applied AFTER it (backward).
			//   * `<target>` is NOT applied        -> apply `<target>` and its
			//                            intra-app dependency closure (forward).
			//
			// `--plan` previews the action without touching the database. On a fresh
			// database the bookkeeping table is created lazily for real execution but
			// NEVER for `--plan`, preserving the dry-run "leave the DB untouched"
			// contract.
			if let Some(target_name) = target.as_deref() {
				use reinhardt_db::migrations::DatabaseMigrationRecorder;
				use std::collections::HashSet;

				let app = app_label.as_ref().ok_or_else(|| {
					crate::CommandError::InvalidArguments(
						"<migration> requires <app>; usage: `migrate <app> <target>` or `migrate <app> zero`"
							.to_string(),
					)
				})?;

				let recorder = DatabaseMigrationRecorder::new(connection.clone());
				// For real execution, ensure the recorder table exists so the rollback
				// can persist its unapply records (`apply_migrations` does the same
				// before recording). For `--plan` we must NOT create the table; instead
				// `plan_applied_migrations` probes for it: a missing table on a fresh DB
				// degrades to an empty set, while a genuine DB error fails fast so the
				// preview never misreports the applied state.
				let applied = if is_plan {
					plan_applied_migrations(&connection, &recorder).await?
				} else {
					recorder.ensure_schema_table().await.map_err(|e| {
						crate::CommandError::ExecutionError(format!(
							"Failed to ensure migration recorder table: {}",
							e
						))
					})?;
					recorder.get_applied_migrations().await.map_err(|e| {
						crate::CommandError::ExecutionError(format!(
							"Failed to query applied migrations: {}",
							e
						))
					})?
				};
				let applied_for_app: Vec<_> =
					applied.iter().filter(|r| r.app == *app).cloned().collect();

				// Branch (a): `migrate <app> zero` -> unapply ALL applied migrations.
				if target_name == "zero" {
					if applied_for_app.is_empty() {
						ctx.info(&format!(
							"No applied migrations for app '{}'; nothing to do.",
							app
						));
						return Ok(());
					}

					// `applied_for_app` is ASC by applied time; rollback unapplies the
					// newest first. Plan and `--fake` operate purely on recorder records
					// and never load files; only a real rollback needs the on-disk
					// reverse SQL.
					if is_plan {
						ctx.info(&format!(
							"[plan] Would unapply {} migration(s) for app '{}':",
							applied_for_app.len(),
							app
						));
						for r in applied_for_app.iter().rev() {
							ctx.info(&format!("  - {}:{} (unapply)", r.app, r.name));
						}
						return Ok(());
					}

					if is_fake {
						ctx.info(
							"Faking rollback (updating recorder without executing reverse SQL):",
						);
						for r in applied_for_app.iter().rev() {
							recorder.unapply(&r.app, &r.name).await.map_err(|e| {
								crate::CommandError::ExecutionError(format!(
									"Failed to unapply {}:{}: {}",
									r.app, r.name, e
								))
							})?;
							ctx.success(&format!("  ✓ Faked rollback: {}:{}", r.app, r.name));
						}
						ctx.success(&format!(
							"Faked rollback of {} migration(s) for app '{}'",
							applied_for_app.len(),
							app
						));
						return Ok(());
					}

					let mut to_rollback = Vec::with_capacity(applied_for_app.len());
					for r in &applied_for_app {
						let migration = all_migrations
							.iter()
							.find(|m| m.app_label == r.app && m.name == r.name)
							.cloned()
							.ok_or_else(|| {
								crate::CommandError::ExecutionError(format!(
									"Migration {}:{} is recorded as applied but its file was not found on disk",
									r.app, r.name
								))
							})?;
						to_rollback.push(migration);
					}

					let mut executor = DatabaseMigrationExecutor::new(connection);
					let result = executor
						.rollback_migrations(&to_rollback)
						.await
						.map_err(|e| {
							crate::CommandError::ExecutionError(format!(
								"Failed to roll back migrations: {:?}",
								e
							))
						})?;
					for id in &result.applied {
						ctx.success(&format!("  ✓ Rolled back: {}", id));
					}
					ctx.success(&format!(
						"Rolled back {} migration(s) for app '{}'",
						result.applied.len(),
						app
					));
					return Ok(());
				}

				// Branch (b): target is currently applied -> roll back everything after it.
				if let Some(pos) = applied_for_app.iter().position(|r| r.name == target_name) {
					let to_rollback_records = &applied_for_app[pos + 1..];
					if to_rollback_records.is_empty() {
						ctx.info(&format!(
							"Already at {}:{}; nothing to do.",
							app, target_name
						));
						return Ok(());
					}

					// Plan and `--fake` operate purely on recorder records; only a real
					// rollback loads the on-disk reverse SQL.
					if is_plan {
						ctx.info(&format!(
							"[plan] Would unapply {} migration(s) for app '{}' to reach target '{}':",
							to_rollback_records.len(),
							app,
							target_name
						));
						for r in to_rollback_records.iter().rev() {
							ctx.info(&format!("  - {}:{} (unapply)", r.app, r.name));
						}
						return Ok(());
					}

					if is_fake {
						ctx.info(
							"Faking rollback (updating recorder without executing reverse SQL):",
						);
						for r in to_rollback_records.iter().rev() {
							recorder.unapply(&r.app, &r.name).await.map_err(|e| {
								crate::CommandError::ExecutionError(format!(
									"Failed to unapply {}:{}: {}",
									r.app, r.name, e
								))
							})?;
							ctx.success(&format!("  ✓ Faked rollback: {}:{}", r.app, r.name));
						}
						ctx.success(&format!(
							"Faked rollback to {}:{} ({} migration(s) unapplied)",
							app,
							target_name,
							to_rollback_records.len()
						));
						return Ok(());
					}

					let mut to_rollback = Vec::with_capacity(to_rollback_records.len());
					for r in to_rollback_records {
						let migration = all_migrations
							.iter()
							.find(|m| m.app_label == r.app && m.name == r.name)
							.cloned()
							.ok_or_else(|| {
								crate::CommandError::ExecutionError(format!(
									"Migration {}:{} is recorded as applied but its file was not found on disk",
									r.app, r.name
								))
							})?;
						to_rollback.push(migration);
					}

					let mut executor = DatabaseMigrationExecutor::new(connection);
					let result = executor
						.rollback_migrations(&to_rollback)
						.await
						.map_err(|e| {
							crate::CommandError::ExecutionError(format!(
								"Failed to roll back migrations: {:?}",
								e
							))
						})?;
					for id in &result.applied {
						ctx.success(&format!("  ✓ Rolled back: {}", id));
					}
					ctx.success(&format!(
						"Rolled back to {}:{} ({} migration(s) unapplied)",
						app,
						target_name,
						result.applied.len()
					));
					return Ok(());
				}

				// Branch (c): target is NOT currently applied -> forward to target.
				// Validate the target exists on disk first.
				let target_on_disk = all_migrations
					.iter()
					.any(|m| m.app_label == *app && m.name == target_name);
				if !target_on_disk {
					return Err(crate::CommandError::ExecutionError(format!(
						"Migration {}:{} does not exist on disk",
						app, target_name
					)));
				}

				// Applying "to target" means applying the target plus every migration
				// it transitively depends on within the same app. `apply_migrations`
				// re-sorts the slice topologically and skips already-applied entries,
				// so we only need to hand it the correct *set* of migrations. Cross-app
				// prerequisites are managed by their own `migrate <other_app>` run,
				// mirroring the app-scoped behavior of the apply-all path below.
				let mut needed: HashSet<(String, String)> = HashSet::new();
				let mut stack: Vec<(String, String)> = vec![(app.clone(), target_name.to_string())];
				while let Some((dep_app, dep_name)) = stack.pop() {
					if !needed.insert((dep_app.clone(), dep_name.clone())) {
						continue;
					}
					if let Some(migration) = all_migrations
						.iter()
						.find(|m| m.app_label == dep_app && m.name == dep_name)
					{
						for (da, dn) in &migration.dependencies {
							if *da == *app {
								stack.push((da.clone(), dn.clone()));
							}
						}
					}
				}

				let to_apply: Vec<_> = all_migrations
					.iter()
					.filter(|m| needed.contains(&(m.app_label.clone(), m.name.clone())))
					.cloned()
					.collect();

				let applied_names: HashSet<&str> =
					applied_for_app.iter().map(|r| r.name.as_str()).collect();
				let pending: Vec<_> = to_apply
					.iter()
					.filter(|m| !applied_names.contains(m.name.as_str()))
					.collect();
				let pending = dependency_ordered_migrations(pending)?;

				if pending.is_empty() {
					ctx.info(&format!(
						"Already at or past {}:{}; nothing to apply.",
						app, target_name
					));
					return Ok(());
				}

				if is_plan {
					ctx.info(&format!(
						"[plan] Would apply {} migration(s) for app '{}' to reach target '{}':",
						pending.len(),
						app,
						target_name
					));
					for migration in &pending {
						ctx.info(&format!(
							"  - {}:{} (apply)",
							migration.app_label, migration.name
						));
					}
					return Ok(());
				}

				if is_fake {
					ctx.info("Faking migrations (marking as applied without executing):");
					for migration in &pending {
						recorder
							.record_applied(&migration.app_label, &migration.name)
							.await
							.map_err(|e| {
								crate::CommandError::ExecutionError(format!(
									"Failed to record fake migration {}:{}: {}",
									migration.app_label, migration.name, e
								))
							})?;
						ctx.success(&format!(
							"  ✓ Faked: {}:{}",
							migration.app_label, migration.name
						));
					}
					ctx.success(&format!(
						"Faked {} migration(s) to reach {}:{}",
						pending.len(),
						app,
						target_name
					));
					return Ok(());
				}

				let mut executor = DatabaseMigrationExecutor::new(connection);
				let result = executor.apply_migrations(&to_apply).await.map_err(|e| {
					crate::CommandError::ExecutionError(format!(
						"Failed to apply migrations: {:?}",
						e
					))
				})?;
				for id in &result.applied {
					ctx.success(&format!("  ✓ Applied: {}", id));
				}
				ctx.success(&format!(
					"Applied {} migration(s) to reach {}:{}",
					result.applied.len(),
					app,
					target_name
				));
				return Ok(());
			}

			// 5. Filter and check migrations
			let migrations_to_apply: Vec<_> = if let Some(ref app) = app_label {
				all_migrations
					.into_iter()
					.filter(|m| m.app_label == *app)
					.collect()
			} else {
				all_migrations.into_iter().collect()
			};
			if migrations_to_apply.is_empty() {
				ctx.info(
					"No migrations to apply. \
					 (Migration files must be .rs files in the migrations/<app_label>/ directory. \
					 Use `makemigrations` to generate them.)",
				);
				return Ok(());
			}

			ctx.info(&format!(
				"Found {} migration(s) to apply",
				migrations_to_apply.len()
			));

			// 5.5. `--plan` previews the apply-all set without mutating the database.
			// Query the recorder to show only not-yet-applied migrations; on a fresh
			// DB the table may not exist (treated as "nothing applied"), but a real DB
			// error must fail fast rather than masquerade as an empty set. The table is
			// never created here (dry-run contract).
			if is_plan {
				use reinhardt_db::migrations::DatabaseMigrationRecorder;
				let recorder = DatabaseMigrationRecorder::new(connection.clone());
				let applied = plan_applied_migrations(&connection, &recorder).await?;
				let pending: Vec<_> = migrations_to_apply
					.iter()
					.filter(|m| {
						!applied
							.iter()
							.any(|r| r.app == m.app_label && r.name == m.name)
					})
					.collect();
				let pending = dependency_ordered_migrations(pending)?;
				if pending.is_empty() {
					ctx.info("[plan] No unapplied migrations.");
					return Ok(());
				}
				ctx.info(&format!(
					"[plan] Would apply {} migration(s):",
					pending.len()
				));
				for migration in &pending {
					ctx.info(&format!(
						"  - {}:{} (apply)",
						migration.app_label, migration.name
					));
				}
				return Ok(());
			}

			// 6. Apply migrations (or fake them
			if is_fake {
				ctx.info("Faking migrations (marking as applied without execution):");

				// Create migration executor for fake migrations
				let mut executor = DatabaseMigrationExecutor::new(connection);
				let migrations_to_fake = dependency_ordered_migrations(migrations_to_apply.iter())?;

				// Record each migration as applied without executing
				for migration in migrations_to_fake {
					executor
						.record_migration(&migration.app_label, &migration.name)
						.await
						.map_err(|e| {
							crate::CommandError::ExecutionError(format!(
								"Failed to record fake migration {}:{}: {:?}",
								migration.app_label, migration.name, e
							))
						})?;
					ctx.success(&format!(
						"  ✓ Faked: {}:{}",
						migration.app_label, migration.name
					));
				}
			} else {
				ctx.info("Applying migrations:");

				// Create migration executor
				let mut executor = DatabaseMigrationExecutor::new(connection);

				// Apply migrations
				match executor.apply_migrations(&migrations_to_apply[..]).await {
					Ok(result) => {
						for applied_id in &result.applied {
							ctx.success(&format!("  ✓ Applied: {}", applied_id));
						}
					}
					Err(e) => {
						return Err(crate::CommandError::ExecutionError(format!(
							"Failed to apply migrations: {:?}",
							e
						)));
					}
				}
			}

			ctx.info("");
			ctx.success(&format!(
				"Applied {} migration(s) successfully",
				migrations_to_apply.len()
			));

			Ok(())
		}

		#[cfg(not(feature = "migrations"))]
		{
			ctx.warning("Migrations feature not enabled");
			ctx.info("To use migrate, enable the 'migrations' feature");
			Ok(())
		}
	}
}

/// Sort migrations with the same dependency rules used by the migration executor.
#[cfg(feature = "migrations")]
fn dependency_ordered_migrations<'a>(
	migrations: impl IntoIterator<Item = &'a reinhardt_db::migrations::Migration>,
) -> CommandResult<Vec<&'a reinhardt_db::migrations::Migration>> {
	use reinhardt_db::migrations::{MigrationGraph, MigrationKey};
	use std::collections::HashMap;

	let migrations: Vec<_> = migrations.into_iter().collect();
	let mut by_key = HashMap::with_capacity(migrations.len());
	let mut graph = MigrationGraph::new();

	for migration in &migrations {
		let key = MigrationKey::new(migration.app_label.as_str(), migration.name.as_str());
		let dependencies = migration
			.dependencies
			.iter()
			.map(|(app, name)| MigrationKey::new(app.as_str(), name.as_str()))
			.collect();

		by_key.insert(key.clone(), *migration);
		graph.add_migration(key, dependencies);
	}

	graph
		.topological_sort()
		.map_err(|e| {
			crate::CommandError::ExecutionError(format!(
				"Failed to sort migration plan by dependencies: {}",
				e
			))
		})?
		.into_iter()
		.map(|key| {
			by_key.get(&key).copied().ok_or_else(|| {
				crate::CommandError::ExecutionError(format!(
					"Dependency-sorted migration not found: {}",
					key.id()
				))
			})
		})
		.collect()
}

/// Resolve the applied-migration set for a `--plan` preview without creating the
/// recorder table.
///
/// Dry-run contract: a missing recorder table on a fresh database means nothing
/// has been applied yet, so its absence degrades to an empty set. A genuine
/// database error (permissions, connectivity, malformed query) must NOT be
/// silently swallowed — doing so would print a plan that wrongly claims nothing
/// is applied. The existence probe therefore distinguishes "table missing" from
/// "real failure" and fails fast on the latter.
#[cfg(feature = "migrations")]
async fn plan_applied_migrations(
	connection: &DatabaseConnection,
	recorder: &reinhardt_db::migrations::DatabaseMigrationRecorder,
) -> CommandResult<Vec<reinhardt_db::migrations::recorder::MigrationRecord>> {
	use reinhardt_db::migrations::SchemaEditor;

	// Non-atomic editor: a pure existence probe issuing a single SELECT, with no
	// transaction and no DDL, so the dry-run contract (never mutate in `--plan`)
	// is preserved.
	let mut editor = SchemaEditor::new(connection.clone(), false, connection.database_type())
		.await
		.map_err(|e| {
			crate::CommandError::ExecutionError(format!(
				"Failed to inspect migration recorder table in --plan mode: {}",
				e
			))
		})?;

	if editor
		.table_exists("reinhardt_migrations")
		.await
		.map_err(|e| {
			crate::CommandError::ExecutionError(format!(
				"Failed to check migration recorder table existence in --plan mode: {}",
				e
			))
		})? {
		recorder.get_applied_migrations().await.map_err(|e| {
			crate::CommandError::ExecutionError(format!(
				"Failed to query applied migrations in --plan mode: {}",
				e
			))
		})
	} else {
		Ok(Vec::new())
	}
}

/// Build from_state from database history (preferred approach)
#[cfg(feature = "migrations")]
async fn build_from_state_from_db(
	migrations_dir: &std::path::Path,
	database_url: &str,
) -> Result<reinhardt_db::migrations::ProjectState, crate::CommandError> {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::{
		DatabaseMigrationRecorder, FilesystemSource, MigrationSource, MigrationStateLoader,
	};
	eprintln!("[DEBUG] Database URL: {}", database_url);

	// 2. Connect to database
	let connection = DatabaseConnection::connect(database_url)
		.await
		.map_err(|e| {
			crate::CommandError::ExecutionError(format!("Database connection failed: {}", e))
		})?;
	eprintln!("[DEBUG] Database connection successful");

	// 3. Build state from database history
	let recorder = DatabaseMigrationRecorder::new(connection.inner().clone());
	let applied_records = recorder.get_applied_migrations().await.map_err(|e| {
		crate::CommandError::ExecutionError(format!("Failed to get applied migrations: {}", e))
	})?;
	eprintln!(
		"[DEBUG] Applied migrations count: {}",
		applied_records.len()
	);
	for record in &applied_records {
		eprintln!("[DEBUG]   - {}/{}", record.app, record.name);
	}

	let source = FilesystemSource::new(migrations_dir);
	let all_migrations = source.all_migrations().await.map_err(|e| {
		crate::CommandError::ExecutionError(format!("Failed to load migrations from disk: {}", e))
	})?;
	eprintln!("[DEBUG] Migrations on disk: {}", all_migrations.len());
	for migration in &all_migrations {
		eprintln!("[DEBUG]   - {}/{}", migration.app_label, migration.name);
	}

	let loader = MigrationStateLoader::new(recorder, source);

	let state = loader.build_current_state().await.map_err(|e| {
		crate::CommandError::ExecutionError(format!("Failed to build state: {}", e))
	})?;

	eprintln!("[DEBUG] Built state - models count: {}", state.models.len());
	for (app, model_name) in state.models.keys() {
		eprintln!("[DEBUG]   - {}/{}", app, model_name);
	}

	Ok(state)
}

/// Build from_state from TestContainers (default approach)
///
/// Note: TestContainers integration requires the 'testcontainers' feature to be enabled.
#[cfg(all(feature = "migrations", feature = "testcontainers"))]
async fn build_from_state_from_testcontainers(
	migrations_dir: &std::path::Path,
) -> Result<reinhardt_db::migrations::ProjectState, crate::CommandError> {
	use reinhardt_db::backends::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use reinhardt_db::migrations::{
		DatabaseMigrationRecorder, FilesystemSource, MigrationSource, MigrationStateLoader,
	};
	use reinhardt_test::fixtures::postgres_container;

	// 1. Start temporary PostgreSQL container (panics on failure during tests)
	let (_container, _pool, _port, url) = postgres_container().await;

	// 2. Connect to temporary database
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.map_err(|e| {
			crate::CommandError::ExecutionError(format!("TestContainers connection failed: {}", e))
		})?;

	// 3. Load all existing migrations
	let source = FilesystemSource::new(migrations_dir);
	let all_migrations = source.all_migrations().await.map_err(|e| {
		crate::CommandError::ExecutionError(format!("Failed to load migrations: {}", e))
	})?;

	// 4. Apply all existing migrations
	if !all_migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.clone());
		executor
			.apply_migrations(&all_migrations)
			.await
			.map_err(|e| {
				crate::CommandError::ExecutionError(format!("Failed to apply migrations: {}", e))
			})?;
	}

	// 5. Build current state from applied migrations
	let recorder = DatabaseMigrationRecorder::new(connection.clone());
	let loader = MigrationStateLoader::new(recorder, source);

	loader.build_current_state().await.map_err(|e| {
		crate::CommandError::ExecutionError(format!(
			"Failed to build state from TestContainers: {}",
			e
		))
	})
}

/// Build from_state from TestContainers (stub when feature not enabled)
#[cfg(all(feature = "migrations", not(feature = "testcontainers")))]
async fn build_from_state_from_testcontainers(
	_migrations_dir: &std::path::Path,
) -> Result<reinhardt_db::migrations::ProjectState, crate::CommandError> {
	Err(crate::CommandError::ExecutionError(
		"TestContainers feature not enabled. Enable with --features testcontainers".to_string(),
	))
}

/// Build from_state by replaying migration files from disk (offline fallback)
///
/// This approach requires no database or Docker. It reads all `.rs` migration files,
/// builds a dependency graph, topologically sorts them, and replays all operations
/// to reconstruct the current `ProjectState`.
#[cfg(feature = "migrations")]
async fn build_from_state_from_files(
	migrations_dir: &std::path::Path,
) -> Result<reinhardt_db::migrations::ProjectState, crate::CommandError> {
	use reinhardt_db::migrations::{FilesystemSource, MigrationSource, build_state_from_files};

	let source = FilesystemSource::new(migrations_dir);

	// Check if there are any migrations on disk
	let all_migrations = source.all_migrations().await.map_err(|e| {
		crate::CommandError::ExecutionError(format!("Failed to load migrations from disk: {}", e))
	})?;

	if all_migrations.is_empty() {
		// No migration files found -- this is genuinely an initial migration
		return Ok(reinhardt_db::migrations::ProjectState::default());
	}

	eprintln!(
		"[DEBUG] Building state from {} migration files on disk",
		all_migrations.len()
	);
	for migration in &all_migrations {
		eprintln!("[DEBUG]   - {}/{}", migration.app_label, migration.name);
	}

	build_state_from_files(&source).await.map_err(|e| {
		crate::CommandError::ExecutionError(format!(
			"Failed to build state from migration files: {}",
			e
		))
	})
}

/// Make migrations command
#[cfg(feature = "migrations")]
pub struct MakeMigrationsCommand;

#[cfg(feature = "migrations")]
#[async_trait]
impl BaseCommand for MakeMigrationsCommand {
	fn name(&self) -> &str {
		"makemigrations"
	}

	fn description(&self) -> &str {
		"Create new migrations based on model changes"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::optional(
			"app",
			"App name to create migrations for",
		)]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(
				None,
				"dry-run",
				"Show what would be created without writing files",
			),
			CommandOption::flag(None, "empty", "Create empty migration"),
			CommandOption::flag(
				None,
				"from-db",
				"Use database history instead of TestContainers for state building",
			),
			CommandOption::flag(
				None,
				"force-empty-state",
				"Force using empty state when database/TestContainers is unavailable (dangerous)",
			),
			CommandOption::flag(Some('v'), "verbose", "Show detailed operation list"),
			CommandOption::option(Some('n'), "name", "Name for the migration"),
			CommandOption::option(None, "migrations-dir", "Directory for migration files")
				.with_default("migrations"),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		fn operation_description(operation: &reinhardt_db::migrations::Operation) -> String {
			use reinhardt_db::migrations::Operation;

			match operation {
				// Table operations (corresponds to Model operations in Django)
				Operation::CreateTable { name, .. } => format!("Create model {}", name),
				Operation::DropTable { name } => format!("Delete model {}", name),
				Operation::RenameTable { old_name, new_name } => {
					format!("Rename model {} to {}", old_name, new_name)
				}

				// Column operations (corresponds to Field operations in Django)
				Operation::AddColumn { table, column, .. } => {
					format!("Add field {} to {}", column.name, table)
				}
				Operation::DropColumn { table, column } => {
					format!("Remove field {} from {}", column, table)
				}
				Operation::AlterColumn { table, column, .. } => {
					format!("Alter field {} on {}", column, table)
				}
				Operation::RenameColumn {
					table,
					old_name,
					new_name,
				} => {
					format!("Rename field {} to {} on {}", old_name, new_name, table)
				}

				// Index operations
				Operation::CreateIndex {
					table,
					columns,
					unique,
					..
				} => {
					let index_type = if *unique { "unique index" } else { "index" };
					format!(
						"Create {} on {} ({})",
						index_type,
						table,
						columns.join(", ")
					)
				}
				Operation::DropIndex { table, columns } => {
					format!("Remove index on {} ({})", table, columns.join(", "))
				}

				// Constraint operations
				Operation::AddConstraint { table, .. } => {
					format!("Add constraint on {}", table)
				}
				Operation::DropConstraint {
					table,
					constraint_name,
				} => {
					format!("Remove constraint {} from {}", constraint_name, table)
				}

				// Special operations
				Operation::RunSQL { .. } => "Execute custom SQL".to_string(),
				Operation::RunRust { .. } => "Execute custom Rust code".to_string(),

				// Other operations
				_ => format!("{:?}", operation),
			}
		}
		use std::path::PathBuf;
		ctx.info("Detecting model changes...");

		let is_dry_run = ctx.has_option("dry-run");
		let is_empty = ctx.has_option("empty");
		let app_label = ctx.arg(0).map(|s| s.to_string());
		let migration_name_opt = ctx.option("name").map(|s| s.to_string());
		let migrations_dir_str = ctx
			.option("migrations-dir")
			.map(|s| s.to_string())
			.unwrap_or_else(|| "migrations".to_string());
		let migrations_dir = PathBuf::from(migrations_dir_str);

		// Validate that we are running inside a Reinhardt project directory.
		// A valid project must contain src/bin/manage.rs (the management command
		// entry point). Running makemigrations from the wrong directory would
		// silently create migration files in unexpected locations.
		if !PathBuf::from("src/bin/manage.rs").exists() {
			return Err(crate::CommandError::ExecutionError(
				"Cannot find src/bin/manage.rs in the current directory. \
				 Please run makemigrations from your Reinhardt project root \
				 (the directory containing src/bin/manage.rs)."
					.to_string(),
			));
		}

		if is_dry_run {
			ctx.warning("Dry run mode: No files will be created");
		}

		if let Some(ref app_name) = app_label {
			ctx.verbose(&format!("Creating migrations for: {}", app_name));
		} else {
			ctx.verbose("Creating migrations for all apps");
		}

		#[cfg(feature = "migrations")]
		{
			use crate::CommandError;
			use reinhardt_db::migrations::{
				FilesystemRepository, FilesystemSource, MigrationGraph, MigrationKey,
				MigrationNamer, MigrationNumbering, MigrationService, autodetector::ProjectState,
			};
			use std::sync::Arc;
			use tokio::sync::Mutex;

			// Build a MigrationGraph from a list of Migration structs
			fn build_migration_graph(
				migrations: &[reinhardt_db::migrations::Migration],
			) -> MigrationGraph {
				let mut graph = MigrationGraph::new();
				for migration in migrations {
					let key =
						MigrationKey::new(migration.app_label.clone(), migration.name.clone());
					let deps: Vec<MigrationKey> = migration
						.dependencies
						.iter()
						.map(|(app, name)| MigrationKey::new(app.clone(), name.clone()))
						.collect();
					graph.add_migration(key, deps);
				}
				graph
			}

			let source = Arc::new(FilesystemSource::new(migrations_dir.clone()));
			let repository = Arc::new(Mutex::new(FilesystemRepository::new(
				migrations_dir.clone(),
			)));
			let service = MigrationService::new(source.clone(), repository.clone());

			// Helper to get the last migration for an app
			let get_last_migration = |app: String| {
				let source = source.clone();
				let repository = repository.clone();
				async move {
					let service = MigrationService::new(source, repository);
					let all_migrations = service.load_all().await.ok()?;
					let mut app_migrations: Vec<_> = all_migrations
						.into_iter()
						.filter(|m| m.app_label == *app)
						.collect();

					// Simple sort by name (assumes timestamp prefix)
					app_migrations.sort_by(|a, b| a.name.cmp(&b.name));

					app_migrations.last().cloned()
				}
			};

			// Handle --merge flag for resolving migration conflicts
			let is_merge = ctx.has_option("merge");
			if is_merge {
				if is_empty {
					return Err(CommandError::ExecutionError(
						"--merge and --empty are mutually exclusive options".to_string(),
					));
				}

				// Load all existing migrations and build the graph
				let all_migrations = service.load_all().await.map_err(|e| {
					CommandError::ExecutionError(format!("Failed to load migrations: {}", e))
				})?;

				let graph = build_migration_graph(&all_migrations);

				// Detect conflicts
				let mut conflicts = graph.detect_conflicts();

				// Apply app_label filter if specified
				if let Some(ref app_name) = app_label {
					conflicts.retain(|app, _| app == app_name);
				}

				if conflicts.is_empty() {
					ctx.info("No conflicts detected");
					return Ok(());
				}

				// Generate merge migration for each conflicting app
				let mut conflict_apps: Vec<String> = conflicts.keys().cloned().collect();
				conflict_apps.sort();

				for conflict_app in &conflict_apps {
					let leaf_keys = &conflicts[conflict_app];
					let leaf_names: Vec<&str> = leaf_keys.iter().map(|k| k.name.as_str()).collect();

					// Generate merge name
					let base_name = migration_name_opt
						.clone()
						.unwrap_or_else(|| MigrationNamer::generate_merge_name(&leaf_names));
					let migration_number =
						MigrationNumbering::next_number(&migrations_dir, conflict_app);
					let final_name = format!("{}_{}", migration_number, base_name);

					// Dependencies = all conflicting leaves
					let dependencies: Vec<(String, String)> = leaf_keys
						.iter()
						.map(|k| (k.app_label.clone(), k.name.clone()))
						.collect();

					let merge_migration = reinhardt_db::migrations::Migration {
						app_label: conflict_app.clone(),
						name: final_name.clone(),
						operations: Vec::new(),
						dependencies,
						atomic: true,
						replaces: Vec::new(),
						initial: None,
						state_only: false,
						database_only: false,
						optional_dependencies: Vec::new(),
						swappable_dependencies: Vec::new(),
					};

					if !is_dry_run {
						service
							.save_migration(&merge_migration)
							.await
							.map_err(|e| {
								CommandError::ExecutionError(format!(
									"Failed to save merge migration: {}",
									e
								))
							})?;
						ctx.success(&format!(
							"Created merge migration for '{}': {}",
							conflict_app, final_name
						));
					} else {
						ctx.info(&format!(
							"Would create merge migration for '{}': {}",
							conflict_app, final_name
						));
					}

					// Show merged leaves
					for leaf in leaf_keys {
						ctx.verbose(&format!("  Merging: {}", leaf.name));
					}
				}

				return Ok(());
			}

			// Handle --empty flag for manual migrations
			if is_empty {
				let app_name = app_label.ok_or_else(|| {
					CommandError::ExecutionError(
						"App label is required when creating an empty migration".to_string(),
					)
				})?;

				let last_migration = get_last_migration(app_name.clone()).await;
				let dependencies: Vec<(String, String)> = if let Some(ref last) = last_migration {
					vec![(app_name.clone(), last.name.clone())]
				} else {
					Vec::new()
				};

				// Generate migration name using new naming system
				let migration_number = MigrationNumbering::next_number(&migrations_dir, &app_name);
				let base_name = migration_name_opt.unwrap_or_else(|| "custom".to_string());
				let name = format!("{}_{}", migration_number, base_name);
				let new_migration = reinhardt_db::migrations::Migration {
					app_label: app_name.clone(),
					name: name.clone(),
					operations: Vec::new(),
					dependencies,
					atomic: true,
					replaces: Vec::new(),
					initial: None,
					state_only: false,
					database_only: false,
					optional_dependencies: Vec::new(),
					swappable_dependencies: Vec::new(),
				};

				if !is_dry_run {
					service
						.save_migration(&new_migration)
						.await
						.map_err(|e| CommandError::ExecutionError(format!("Save error: {}", e)))?;
					ctx.success(&format!(
						"Created empty migration for {}: {}",
						app_name, name
					));
				} else {
					ctx.info(&format!(
						"Would create empty migration for {}: {}",
						app_name, name
					));
				}
				return Ok(());
			}

			// 1. Get target project state from global model registry
			let target_project_state = ProjectState::from_global_registry();

			// Determine which apps to process
			let app_names: Vec<String> = if let Some(label) = app_label {
				// Explicit app label specified
				vec![label]
			} else {
				// Extract all app labels from ProjectState
				let changed_apps: Vec<String> = target_project_state
					.models
					.keys()
					.map(|(app_label, _)| app_label.clone())
					.collect::<std::collections::HashSet<_>>()
					.into_iter()
					.collect();

				if changed_apps.is_empty() {
					return Err(CommandError::ExecutionError(
						"No models found. Cannot determine app_label automatically.".to_string(),
					));
				}

				changed_apps
			};

			let is_verbose = ctx.has_option("verbose");

			// Get database URL from context option or environment, falling back
			// to the project's composed settings (`[core.databases.default]`)
			// when neither is provided (#5042). An empty string preserves the
			// TestContainers `from_state` path for offline runs.
			let database_url = ctx
				.option("database")
				.map(|s| s.to_string())
				.or_else(|| std::env::var("DATABASE_URL").ok())
				.or_else(|| {
					ctx.settings
						.as_ref()
						.and_then(|s| DatabaseConnection::database_url_from(s.as_ref(), None).ok())
				})
				.unwrap_or_default();

			// 2. Build from_state from database history or TestContainers
			// This ensures all models are treated as new, generating complete migrations
			struct MigrationResult {
				app_name: String,
				migration: reinhardt_db::migrations::Migration,
			}

			let mut results: Vec<MigrationResult> = Vec::new();

			// Build from_state based on strategy (default: TestContainers)
			//
			// #3871: Check --force-empty-state before any TestContainers or DB call.
			// postgres_container() panics when Docker is unavailable, so the flag must
			// be respected before attempting container startup, not as a fallback.
			let from_db_flag = ctx.has_option("from-db");
			let from_state = if ctx.has_option("force-empty-state") {
				ctx.warning("⚠️  Using empty state as requested (--force-empty-state)");
				ctx.warning("This may create duplicate migrations!");
				ProjectState::new()
			} else if from_db_flag {
				// When --from-db flag is specified: prioritize database history
				match build_from_state_from_db(&migrations_dir, &database_url).await {
					Ok(state) => {
						ctx.verbose("Built state from database history");
						state
					}
					Err(e) => {
						ctx.warning(&format!("Failed to connect to database: {}", e));
						ctx.info("Falling back to TestContainers...");
						match build_from_state_from_testcontainers(&migrations_dir).await {
							Ok(state) => {
								ctx.verbose("Built state from TestContainers");
								state
							}
							Err(e) => {
								ctx.warning(&format!("Failed to use TestContainers: {}", e));
								ctx.info("Falling back to file-based state reconstruction...");
								match build_from_state_from_files(&migrations_dir).await {
									Ok(state) => {
										ctx.verbose("Built state from migration files (offline)");
										state
									}
									Err(e_files) => {
										ctx.error(&format!(
											"Failed file-based reconstruction: {}",
											e_files
										));
										ctx.error(
											"⚠️  CRITICAL: Cannot build from_state from existing migrations!",
										);
										ctx.error(
											"This will cause ALL tables to be regenerated, creating duplicate migrations.",
										);
										ctx.error("");
										ctx.error("Possible solutions:");
										ctx.error("  1. Fix TestContainers setup (recommended)");
										ctx.error(
											"  2. Use --from-db flag to build from database history",
										);
										ctx.error(
											"  3. Use --force-empty-state to proceed anyway (dangerous)",
										);
										ctx.error("");

										return Err("from_state construction failed. Please fix TestContainers, use --from-db, or use --force-empty-state to continue anyway.".to_string().into());
									}
								}
							}
						}
					}
				}
			} else {
				// Default: prioritize TestContainers
				match build_from_state_from_testcontainers(&migrations_dir).await {
					Ok(state) => {
						ctx.verbose("Built state from TestContainers");
						state
					}
					Err(e) => {
						ctx.warning(&format!("Failed to use TestContainers: {}", e));
						ctx.info("Falling back to database history...");
						match build_from_state_from_db(&migrations_dir, &database_url).await {
							Ok(state) => {
								ctx.verbose("Built state from database history");
								state
							}
							Err(e) => {
								ctx.warning(&format!("Failed to connect to database: {}", e));
								ctx.info("Falling back to file-based state reconstruction...");
								match build_from_state_from_files(&migrations_dir).await {
									Ok(state) => {
										ctx.verbose("Built state from migration files (offline)");
										state
									}
									Err(e_files) => {
										ctx.error(&format!(
											"Failed file-based reconstruction: {}",
											e_files
										));
										ctx.error(
											"⚠️  CRITICAL: Cannot build from_state from existing migrations!",
										);
										ctx.error(
											"This will cause ALL tables to be regenerated, creating duplicate migrations.",
										);
										ctx.error("");
										ctx.error("Possible solutions:");
										ctx.error("  1. Fix database connection (recommended)");
										ctx.error(
											"  2. Use TestContainers (default behavior without --from-db)",
										);
										ctx.error(
											"  3. Use --force-empty-state to proceed anyway (dangerous)",
										);
										ctx.error("");

										return Err("from_state construction failed. Please fix database connection, remove --from-db, or use --force-empty-state to continue anyway.".to_string().into());
									}
								}
							}
						}
					}
				}
			};

			// Check for migration conflicts before proceeding
			{
				let all_migrations = service.load_all().await.map_err(|e| {
					CommandError::ExecutionError(format!(
						"Failed to load migrations for conflict check: {}",
						e
					))
				})?;
				if !all_migrations.is_empty() {
					let graph = build_migration_graph(&all_migrations);

					let conflicts = graph.detect_conflicts();
					if !conflicts.is_empty() {
						let mut conflict_apps: Vec<&String> = conflicts.keys().collect();
						conflict_apps.sort();
						for app in &conflict_apps {
							let leaves = &conflicts[*app];
							let leaf_names: Vec<&str> =
								leaves.iter().map(|k| k.name.as_str()).collect();
							ctx.error(&format!(
								"Conflicting migrations detected for '{}': {}",
								app,
								leaf_names.join(", ")
							));
						}
						return Err(CommandError::ExecutionError(
							"Run 'makemigrations --merge' to resolve migration conflicts."
								.to_string(),
						));
					}
				}
			}

			for app_name in &app_names {
				// Filter target state for this app only
				let app_target_state = target_project_state.filter_by_app(app_name);
				eprintln!(
					"[DEBUG] app_target_state for '{}': {} models",
					app_name,
					app_target_state.models.len()
				);
				for ((app, model_name), model_state) in &app_target_state.models {
					eprintln!(
						"[DEBUG]   - {}/{} ({} fields)",
						app,
						model_name,
						model_state.fields.len()
					);
				}

				// Filter from_state for this app only
				let app_from_state = from_state.filter_by_app(app_name);
				eprintln!(
					"[DEBUG] app_from_state for '{}': {} models",
					app_name,
					app_from_state.models.len()
				);
				for ((app, model_name), model_state) in &app_from_state.models {
					eprintln!(
						"[DEBUG]   - {}/{} ({} fields)",
						app,
						model_name,
						model_state.fields.len()
					);
				}

				// Use MigrationAutodetector for proper ManyToMany support
				let detector = reinhardt_db::migrations::MigrationAutodetector::new(
					app_from_state,
					app_target_state,
				);
				let generated_migrations = detector.generate_migrations();

				// Process generated migrations for this app
				for migration in generated_migrations {
					if migration.app_label == app_name.as_str() {
						// Generate migration name
						let migration_number =
							MigrationNumbering::next_number(&migrations_dir, app_name);
						let is_initial = migration_number == "0001";
						let base_name = migration_name_opt.clone().unwrap_or_else(|| {
							MigrationNamer::generate_name(&migration.operations, is_initial)
						});
						let final_name = format!("{}_{}", migration_number, base_name);

						// Determine dependencies
						let dependencies = if migration_number == "0001" {
							Vec::new() // Initial migration has no dependencies
						} else {
							// Get previous migration number
							let prev_number_int = migration_number.parse::<u32>().map_err(|e| {
								CommandError::ParseError(format!(
									"invalid migration number '{}': {}",
									migration_number, e
								))
							})? - 1;
							let prev_number = format!("{:04}", prev_number_int);
							// Find the previous migration by scanning the directory
							let prev_migration_name = if let Ok(entries) =
								std::fs::read_dir(migrations_dir.join(app_name))
							{
								let mut prev_names: Vec<String> = entries
									.filter_map(|entry| {
										let path = entry.ok()?.path();
										let filename = path.file_stem()?.to_str()?.to_string();
										if filename.starts_with(&prev_number) {
											Some(filename)
										} else {
											None
										}
									})
									.collect();
								prev_names.sort();
								prev_names.first().cloned()
							} else {
								None
							};

							if let Some(prev_name) = prev_migration_name {
								vec![(app_name.clone(), prev_name)]
							} else {
								Vec::new()
							}
						};

						let new_migration = reinhardt_db::migrations::Migration {
							app_label: app_name.clone(),
							name: final_name,
							operations: migration.operations,
							dependencies,
							atomic: true,
							replaces: Vec::new(),
							initial: if migration_number == "0001" {
								Some(true)
							} else {
								None
							},
							state_only: false,
							database_only: false,
							optional_dependencies: Vec::new(),
							swappable_dependencies: Vec::new(),
						};

						results.push(MigrationResult {
							app_name: app_name.clone(),
							migration: new_migration,
						});
					}
				}
			}

			// 4. Write all migrations
			if !results.is_empty() {
				for result in results {
					ctx.info(&format!("Migrations for '{}':", result.app_name));

					// Build the correct file path from migration name
					let migration_file_path = migrations_dir
						.join(&result.app_name)
						.join(format!("{}.rs", result.migration.name));

					if !is_dry_run {
						service
							.save_migration(&result.migration)
							.await
							.map_err(|e| {
								let err_msg = e.to_string();
								if err_msg.contains("already exists") {
									CommandError::ExecutionError(format!(
										"Migration file already exists: {}
									
									Possible solutions:
									1. If the operations are identical, you don't need a new migration
									2. If you want to modify the migration, delete the existing file first:
									   rm migrations/{}/{{migration_file}}.rs
									3. If you want to keep both, manually rename the existing file",
										e, result.app_name
									))
								} else {
									CommandError::ExecutionError(format!("Save error: {}", e))
								}
							})?;
						ctx.success(&format!("  {}", migration_file_path.display()));

						// Show detailed operations if --verbose
						if is_verbose {
							for operation in &result.migration.operations {
								let description = operation_description(operation);
								ctx.info(&format!("    - {}", description));
							}
						}
					} else {
						ctx.info(&format!(
							"  Would create: {}",
							migration_file_path.display()
						));

						if is_verbose {
							for operation in &result.migration.operations {
								let description = operation_description(operation);
								ctx.info(&format!("    - {}", description));
							}
						}
					}
				}
			} else {
				ctx.info("No changes detected");
			}

			Ok(())
		}

		#[cfg(not(feature = "migrations"))]
		{
			ctx.warning("Migrations feature not enabled");
			ctx.info("To use makemigrations, enable the 'migrations' feature");
			Ok(())
		}
	}
}

/// Interactive shell command
pub struct ShellCommand;

#[async_trait]
impl BaseCommand for ShellCommand {
	fn name(&self) -> &str {
		"shell"
	}

	fn description(&self) -> &str {
		"Start an interactive Rust REPL"
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![CommandOption::option(
			Some('c'),
			"command",
			"Execute a command and exit",
		)]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		if let Some(command) = ctx.option("command") {
			ctx.info(&format!("Executing: {}", command));
			// Execute the command
			return Ok(());
		}

		ctx.info("Starting interactive shell...");
		ctx.info("Type 'exit' or press Ctrl+D to quit");

		#[cfg(feature = "shell")]
		{
			use rustyline::DefaultEditor;
			use rustyline::error::ReadlineError;

			let mut rl = DefaultEditor::new().map_err(|e| {
				crate::CommandError::ExecutionError(format!("Failed to create REPL: {}", e))
			})?;

			loop {
				let readline = rl.readline(">>> ");
				match readline {
					Ok(line) => {
						let trimmed = line.trim();
						if trimmed == "exit" || trimmed == "quit" {
							ctx.info("Goodbye!");
							break;
						}

						if !trimmed.is_empty() {
							let _ = rl.add_history_entry(line.as_str());

							// Evaluate code using Rhai engine
							#[cfg(feature = "shell-rhai")]
							{
								Self::eval_rhai(ctx, trimmed)?;
							}
							#[cfg(not(feature = "shell-rhai"))]
							{
								ctx.warning(
									"Rhai engine not enabled. Enable 'shell-rhai' feature.",
								);
							}
						}
					}
					Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
						ctx.info("Goodbye!");
						break;
					}
					Err(err) => {
						return Err(crate::CommandError::ExecutionError(format!(
							"REPL error: {}",
							err
						)));
					}
				}
			}

			Ok(())
		}

		#[cfg(not(feature = "shell"))]
		{
			ctx.warning("Shell feature not enabled");
			ctx.info("To use shell, enable the 'shell' feature in Cargo.toml:");
			ctx.info("  reinhardt-commands = { version = \"*\", features = [\"shell\"] }");
			Ok(())
		}
	}
}

impl ShellCommand {
	/// Evaluate code using Rhai engine
	#[cfg(feature = "shell-rhai")]
	fn eval_rhai(ctx: &CommandContext, code: &str) -> CommandResult<()> {
		use rhai::{Engine, EvalAltResult};

		let mut engine = Engine::new();

		// Register helper functions
		engine.register_fn("println", |s: &str| {
			println!("{}", s);
		});

		// Evaluate the code
		match engine.eval::<rhai::Dynamic>(code) {
			Ok(result) => {
				// Display result if not Unit type
				if !result.is_unit() {
					ctx.info(&format!("=> {}", result));
				}
				Ok(())
			}
			Err(e) => {
				let error_msg = match *e {
					EvalAltResult::ErrorParsing(ref err, _) => {
						format!("Parse error: {}", err)
					}
					EvalAltResult::ErrorRuntime(ref msg, _) => {
						format!("Runtime error: {}", msg)
					}
					_ => format!("Error: {}", e),
				};
				ctx.warning(&error_msg);
				Ok(())
			}
		}
	}
}

/// Development server command
pub struct RunServerCommand;

/// Pure runserver settings derived from a command context before startup work.
///
/// Keeping this translation separate makes the option contract testable without
/// registering routes, probing ports, or building the WASM frontend.
struct RunServerExecutionOptions {
	address: String,
	noreload: bool,
	no_wasm_rebuild: bool,
	#[cfg(feature = "autoreload")]
	watch_delay: std::time::Duration,
	insecure: bool,
	no_docs: bool,
	with_pages: bool,
	static_dir: String,
	no_spa: bool,
	no_project_static: bool,
	no_wasm: bool,
	no_override_wasm: bool,
	force_wasm_legacy: bool,
	wasm_optional: bool,
	index: Option<String>,
}

impl RunServerExecutionOptions {
	fn from_context(ctx: &CommandContext) -> Self {
		Self {
			address: ctx
				.arg(0)
				.map(ToString::to_string)
				.unwrap_or_else(|| "127.0.0.1:8000".to_string()),
			noreload: ctx.has_option("noreload"),
			no_wasm_rebuild: ctx.has_option("no-wasm-rebuild"),
			#[cfg(feature = "autoreload")]
			watch_delay: ctx
				.option("watch-delay")
				.and_then(|raw| raw.parse::<u64>().ok())
				.map(std::time::Duration::from_millis)
				.unwrap_or(crate::debounced_watcher::DEBOUNCE_WINDOW),
			insecure: ctx.has_option("insecure"),
			no_docs: ctx.has_option("no_docs"),
			with_pages: ctx.has_option("with-pages"),
			static_dir: ctx
				.option("static-dir")
				.map(ToString::to_string)
				.unwrap_or_else(|| "dist".to_string()),
			no_spa: ctx.has_option("no-spa"),
			no_project_static: ctx.has_option("no-project-static"),
			no_wasm: ctx.has_option("no-wasm"),
			no_override_wasm: ctx.has_option("no-override-wasm"),
			force_wasm_legacy: ctx.has_option("force-wasm"),
			wasm_optional: ctx.has_option("wasm-optional"),
			index: ctx.option("index").map(ToString::to_string),
		}
	}
}

#[cfg(all(feature = "server", feature = "autoreload"))]
struct AutoreloadChildOptions<'a> {
	address: &'a str,
	insecure: bool,
	no_docs: bool,
	with_pages: bool,
	static_dir: &'a str,
	no_spa: bool,
	no_project_static: bool,
	index: Option<&'a str>,
	hmr_port: Option<u16>,
	no_wasm: bool,
	no_override_wasm: bool,
	force_wasm: bool,
	wasm_optional: bool,
}

impl RunServerCommand {
	/// Consume `UrlPatternsRegistration` `inventory` entries and install the
	/// merged `ServerRouter` as the process-wide HTTP router.
	///
	/// This is the canonical, named consumer of the `#[routes]`-emitted
	/// server-side `inventory::submit!` block. It is invoked explicitly
	/// from [`RunServerCommand::execute`] so that the registration step
	/// is **visible at the call site** rather than hidden inside the
	/// command-dispatch loop (Refs #4453 DP-1).
	///
	/// Users running the canonical `cargo run --bin manage runserver`
	/// path get registration "for free" — `execute(..)` calls this method.
	/// Users who bypass `--bin manage` and assemble their own server
	/// entrypoint can call this method directly on `RunServerCommand`
	/// instead of going through [`crate::auto_register_router`].
	///
	/// # Errors
	///
	/// - Zero `#[routes]` registrations found (missing `#[routes]` or
	///   `src/lib.rs` + `src/bin/manage.rs` linker drop — the error
	///   message includes the fix)
	/// - Multiple `#[routes]` registrations (linker marker should normally
	///   catch this at link time; we provide a clear fallback message)
	/// - The async server factory returned an error
	/// - A router is already registered when this method is called
	///   (DP-7 confusable-API guard). Users who intentionally pre-register
	///   via [`reinhardt_urls::routers::register_router`] should not also
	///   call this method.
	///
	/// Refs #4453.
	#[cfg(feature = "routers")]
	pub async fn register_http_routes_from_inventory(
		&self,
	) -> Result<(), Box<dyn std::error::Error>> {
		use reinhardt_urls::routers::{
			UrlPatternsRegistration, is_router_registered, register_router_arc,
		};

		// DP-7 confusable-API guard: reject if a router is already registered.
		// Callers that intentionally pre-register a hand-built `ServerRouter`
		// must not also call this method; the two paths are mutually exclusive.
		if is_router_registered() {
			return Err("ServerRouter is already registered.\n\
				`RunServerCommand::register_http_routes_from_inventory()` and \
				`reinhardt_urls::routers::register_router(..)` are mutually \
				exclusive: choose exactly one. If you want the inventory path, \
				do not pre-register a router; if you want manual registration, \
				skip this method and let `RunServerCommand::execute(..)` reuse \
				the pre-registered router."
				.into());
		}

		// Collect all server-side registrations for validation.
		let registrations: Vec<_> = inventory::iter::<UrlPatternsRegistration>().collect();

		match registrations.len() {
			0 => {
				return Err("No URL patterns registered.\n\
					Add the `#[routes]` attribute to your routes function in src/config/urls.rs:\n\n\
					#[routes]\n\
					pub fn routes() -> UnifiedRouter {\n\
					    UnifiedRouter::new()\n\
					}\n\n\
					If your project uses a library/binary split (src/lib.rs + src/bin/manage.rs),\n\
					the linker may silently discard route registrations from the library crate.\n\
					Fix: add `use your_crate_name as _;` to src/bin/manage.rs to force-link\n\
					the library and preserve its side-effectful route registrations."
					.into());
			}
			1 => { /* expected case */ }
			n => {
				return Err(format!(
					"Multiple #[routes] functions detected ({n} found).\n\
					Only one function in the entire project should be annotated with #[routes].\n\n\
					Please ensure that:\n\
					1. Only one #[routes] attribute exists in your codebase\n\
					2. Check src/config/urls.rs and any other files that might have #[routes]\n\
					3. If you have multiple router configurations, combine them into a single function\n\n\
					Example:\n\
					#[routes]\n\
					pub fn routes() -> UnifiedRouter {{\n\
					    UnifiedRouter::new()\n\
					        .mount(\"/api/\", api::routes())  // NOT annotated with #[routes]\n\
					        .mount(\"/admin/\", admin::routes())\n\
					}}"
				)
				.into());
			}
		}

		let registration = &registrations[0];
		let router = registration
			.server_router_async()
			.await
			.map_err(|e| format!("Failed to create router from #[routes] function: {e}"))?;
		register_router_arc(router);
		Ok(())
	}

	/// No-op when the `routers` feature is disabled.
	///
	/// Kept public to preserve API stability across feature-flag toggles.
	///
	/// # Errors
	///
	/// Never returns an error in the `routers`-disabled build — always
	/// resolves to `Ok(())`. The signature matches the active variant so
	/// downstream callers can stay feature-flag-agnostic.
	#[cfg(not(feature = "routers"))]
	pub async fn register_http_routes_from_inventory(
		&self,
	) -> Result<(), Box<dyn std::error::Error>> {
		Ok(())
	}
}

#[async_trait]
impl BaseCommand for RunServerCommand {
	fn name(&self) -> &str {
		"runserver"
	}

	fn description(&self) -> &str {
		"Start the development server"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![
			CommandArgument::optional("address", "Server address (default: 127.0.0.1:8000)")
				.with_default("127.0.0.1:8000"),
		]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(None, "noreload", "Disable auto-reload"),
			CommandOption::flag(
				None,
				"no-wasm-rebuild",
				"Disable WASM rebuild during hot-reload (server pipeline still runs)",
			),
			CommandOption::option(
				None,
				"watch-delay",
				"Watch delay in milliseconds for file change debouncing",
			)
			.with_default("120"),
			CommandOption::flag(None, "nothreading", "Disable threading"),
			CommandOption::flag(None, "insecure", "Serve static files in production mode"),
			CommandOption::flag(
				None,
				"with-pages",
				"Enable WASM frontend serving (serves static files from dist/)",
			),
			CommandOption::option(
				None,
				"static-dir",
				"Static files directory for WASM frontend",
			)
			.with_default("dist"),
			CommandOption::flag(None, "no-spa", "Disable SPA mode (no index.html fallback)"),
			CommandOption::flag(
				None,
				"no-project-static",
				"Disable auto-serving of <project-root>/static/ at /static/ (--with-pages only)",
			),
			CommandOption::flag(None, "no-wasm", "Skip WASM build at startup"),
			CommandOption::flag(
				None,
				"no-override-wasm",
				"Reuse existing WASM artifacts in dist/ if present (default: rebuild)",
			),
			CommandOption::flag(
				None,
				"force-wasm",
				"DEPRECATED: rebuild is now the default. Use --no-override-wasm to opt out.",
			),
			CommandOption::flag(
				None,
				"wasm-optional",
				"Allow server to start even if WASM build fails",
			),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		// Explicit HTTP route registration (Refs #4453 DP-1):
		// the inventory consumption used to be hidden inside the dispatch
		// chain (cli.rs's `auto_register_router()` called before this body).
		// We now make the call site visible here, on `RunServerCommand`,
		// so that readers of `execute(..)` see exactly when and how the
		// `#[routes]`-emitted server inventory becomes the global router.
		//
		// Opt-out path: users who hand-build a `ServerRouter` and call
		// `reinhardt_urls::routers::register_router(..)` before this body
		// runs will short-circuit through the `is_router_registered()`
		// guard and skip the inventory pull — preserving the existing
		// escape hatch unchanged.
		#[cfg(feature = "routers")]
		{
			if !reinhardt_urls::routers::is_router_registered() {
				self.register_http_routes_from_inventory()
					.await
					.map_err(|e| crate::CommandError::ExecutionError(e.to_string()))?;
			} else {
				ctx.verbose(
					"ServerRouter already registered before RunServerCommand::execute; \
					 skipping inventory pull (manual setter opt-out path).",
				);
			}
		}

		#[cfg_attr(not(feature = "server"), allow(unused_variables))]
		let RunServerExecutionOptions {
			address,
			noreload,
			no_wasm_rebuild,
			#[cfg(feature = "autoreload")]
			watch_delay,
			insecure,
			no_docs,
			with_pages,
			static_dir: static_dir_raw,
			no_spa,
			no_project_static,
			no_wasm,
			no_override_wasm,
			force_wasm_legacy,
			wasm_optional,
			index,
		} = RunServerExecutionOptions::from_context(ctx);
		// Build WASM frontend if --with-pages and not --no-wasm
		#[cfg(feature = "pages")]
		{
			if force_wasm_legacy {
				ctx.warning(concat!(
					"--force-wasm is now the default behavior; this flag is deprecated. ",
					"Use --no-override-wasm to opt out of rebuilds."
				));
			}
			let force = !no_override_wasm;
			if with_pages
				&& !no_wasm && let Err(e) = Self::build_pages_wasm(ctx, force)
			{
				if wasm_optional {
					ctx.warning(&format!(
						"Pages WASM build failed: {}. Server will start without WASM frontend.",
						e
					));
				} else {
					ctx.error(&format!(
						"WASM build failed: {}. Fix compilation errors or use --wasm-optional to start without WASM.",
						e
					));
					return Ok(());
				}
			}
		}

		// Find available port early (before displaying banner)
		#[cfg(feature = "server")]
		let actual_address = {
			let default_address = "127.0.0.1:8000";
			let is_default_address = address == default_address;

			let mut addr: std::net::SocketAddr = address.parse().map_err(|e| {
				crate::CommandError::ExecutionError(format!("Invalid address '{}': {}", address, e))
			})?;

			// Find available port if using default address
			if is_default_address {
				use tokio::net::TcpListener;

				loop {
					match TcpListener::bind(addr).await {
						Ok(_) => {
							// Port is available
							break;
						}
						Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
							// Port in use, try next port
							let current_port = addr.port();
							let new_port = current_port + 1;

							if new_port > 9000 {
								return Err(crate::CommandError::ExecutionError(
									"Could not find available port in range 8000-9000".to_string(),
								));
							}

							ctx.info(&format!(
								"⚠️  Port {} already in use, trying {}...",
								current_port, new_port
							));

							addr.set_port(new_port);
						}
						Err(e) => {
							// Other error, fail
							return Err(crate::CommandError::ExecutionError(format!(
								"Failed to bind to {}: {}",
								addr, e
							)));
						}
					}
				}
			}

			addr.to_string()
		};

		#[cfg(not(feature = "server"))]
		let actual_address = address.to_string();

		// Determine if running in autoreload parent process
		// In autoreload mode, the parent process should not display the startup banner
		// because the child process will display it
		#[cfg(all(feature = "server", feature = "autoreload"))]
		let is_autoreload_parent = !noreload;
		#[cfg(not(all(feature = "server", feature = "autoreload")))]
		let is_autoreload_parent = false;

		// Display startup banner with actual address (skip in autoreload parent)
		if !is_autoreload_parent {
			ctx.info("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
			ctx.info(&format!("🚀 Server:  http://{}", actual_address));

			if with_pages {
				let spa_status = if no_spa { "disabled" } else { "enabled" };
				ctx.info(&format!(
					"📦 WASM:    {} (SPA mode: {})",
					static_dir_raw, spa_status
				));
			}

			// Display index file info (Refs #2869)
			if with_pages
				&& !no_spa && let Some(index_str) = index.as_deref()
			{
				let path = std::path::Path::new(&index_str);
				if path.exists() {
					ctx.info(&format!("📄 Index:   {} (specified)", index_str));
				} else {
					ctx.warning(&format!(
						"📄 Index:   {} (specified, missing — will be ignored)",
						index_str
					));
				}
			}

			#[cfg(feature = "openapi-router")]
			if !no_docs {
				ctx.info(&format!("📖 Docs:    http://{}/api/docs", actual_address));
			}

			#[cfg(all(feature = "pages", feature = "routers"))]
			if with_pages {
				use reinhardt_urls::routers::registration::iter_registered_url_patterns;
				// `client_router()` returns `Arc<ClientRouter>` (owned), and
				// `route_patterns()` borrows from it, so the inner `collect`
				// is required to terminate the borrow within the closure.
				let mut routes: Vec<(String, Option<String>)> = iter_registered_url_patterns()
					.filter_map(|reg| reg.client_router())
					.flat_map(|cr| {
						cr.route_patterns()
							.map(|(pat, name)| (pat.to_string(), name.map(|n| n.to_string())))
							.collect::<Vec<_>>()
					})
					.collect();
				// inventory::iter order is linker-dependent; sort for a
				// stable, diff-friendly startup banner across builds.
				routes.sort();
				if !routes.is_empty() {
					ctx.info("🗺  Routes (WASM-bound):");
					for (pat, name) in &routes {
						if let Some(n) = name {
							ctx.info(&format!("     {}  →  {}", pat, n));
						} else {
							ctx.info(&format!("     {}", pat));
						}
					}
				}
			}

			ctx.info("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

			if insecure {
				ctx.warning("Running with --insecure: Static files will be served");
			}

			ctx.info("");
			ctx.info("Press CTRL-C to quit");
			ctx.info("");
		} else {
			// Autoreload parent: show minimal message, child will show full banner
			#[cfg(all(feature = "server", feature = "autoreload"))]
			{
				ctx.verbose("Auto-reload enabled");
			}
		}

		#[cfg(all(feature = "server", not(feature = "autoreload")))]
		if !noreload {
			ctx.warning(
				"Auto-reload disabled: Enable 'autoreload' feature to use this functionality",
			);
		}

		// Server implementation with conditional features
		#[cfg(feature = "server")]
		{
			// Autoreload PARENT path: run only stateless hook validation,
			// then dispatch directly to the watcher. The full bring-up
			// (DI / DB / on_server_start / HttpServer / static-files
			// middleware) lives in the spawned `--noreload` child so any
			// listeners opened by `on_server_start` hooks (e.g. gRPC) do
			// not collide with the child's HTTP bind (#4244).
			#[cfg(feature = "autoreload")]
			if !noreload {
				Self::validate_hooks_only(ctx).await?;
				return Self::run_with_autoreload(
					ctx,
					&actual_address,
					insecure,
					no_docs,
					with_pages,
					&static_dir_raw,
					no_spa,
					no_project_static,
					index.as_deref(),
					no_wasm_rebuild,
					no_wasm,
					no_override_wasm,
					force_wasm_legacy,
					wasm_optional,
					watch_delay,
				)
				.await;
			}

			// `--noreload` (autoreload child or explicit opt-out) OR
			// `feature = "autoreload"` is off: full bring-up + listen.
			Self::run_server(
				ctx,
				&actual_address,
				noreload,
				no_wasm_rebuild,
				insecure,
				no_docs,
				with_pages,
				&static_dir_raw,
				no_spa,
				no_project_static,
				no_wasm,
				no_override_wasm,
				force_wasm_legacy,
				wasm_optional,
			)
			.await
		}

		#[cfg(not(feature = "server"))]
		{
			ctx.warning("Server feature not enabled");
			ctx.info("To use runserver, enable the 'server' feature in Cargo.toml:");
			ctx.info("  reinhardt-commands = { version = \"*\", features = [\"server\"] }");
			ctx.info("");
			ctx.info("Alternatively, implement your own server using:");
			ctx.info("  use reinhardt_server::HttpServer;");
			ctx.info("  use reinhardt_urls::routers::DefaultRouter;");
			ctx.info("");
			ctx.info("  let router = DefaultRouter::new();");
			ctx.info("  // Register your routes");
			ctx.info("  let server = HttpServer::new(Arc::new(router));");
			ctx.info(&format!(
				"  server.listen(\"{}\".parse()?).await?;",
				actual_address
			));

			Ok(())
		}
	}
}

impl RunServerCommand {
	/// Collect runserver hooks from inventory and run their `validate()` phase.
	///
	/// Used in two places:
	/// - `run_server` (the actual listening process) calls this and then reuses
	///   the returned hooks for `on_server_start`.
	/// - The autoreload parent path in `execute()` calls this for fail-fast
	///   validation but discards the returned hooks; `on_server_start` only
	///   runs in the spawned `--noreload` child so hooks that open listeners
	///   do not collide with the child's HTTP bind (#4244).
	#[cfg(feature = "server")]
	async fn validate_hooks_only(
		ctx: &CommandContext,
	) -> CommandResult<Vec<crate::runserver_hooks::CollectedRunserverHook>> {
		let hooks = crate::runserver_hooks::collect_hooks();
		if !hooks.is_empty() {
			ctx.verbose(&format!("Found {} runserver hook(s)", hooks.len()));
		}
		for collected in &hooks {
			collected.hook.validate().await.map_err(|e| {
				crate::CommandError::ExecutionError(format!(
					"Runserver hook validation failed for {}: {}",
					collected.type_name, e
				))
			})?;
		}
		Ok(hooks)
	}

	/// Test-only entry point exercising the autoreload-parent validation path.
	///
	/// Mirrors `validate_hooks_only` but returns `()` so it can cross the
	/// crate boundary without leaking the crate-private
	/// `CollectedRunserverHook` type. Re-exported via
	/// `crate::__hot_reload_test_api::validate_hooks_only` for integration
	/// tests; not part of the public API.
	#[cfg(feature = "server")]
	pub(crate) async fn validate_hooks_only_for_tests(ctx: &CommandContext) -> CommandResult<()> {
		Self::validate_hooks_only(ctx).await.map(|_| ())
	}

	/// Run the development server
	#[cfg(feature = "server")]
	// Allow many arguments: CLI command handler needs to accept all server configuration options
	#[allow(clippy::too_many_arguments)]
	async fn run_server(
		// Context parameter reserved for future extensions (e.g., accessing global config)
		#[allow(unused_variables)] ctx: &CommandContext,
		address: &str,
		noreload: bool,
		// Only consumed by the autoreload pipeline; allow unused when feature is off.
		#[cfg_attr(not(feature = "autoreload"), allow(unused_variables))] no_wasm_rebuild: bool,
		_insecure: bool,
		no_docs: bool,
		with_pages: bool,
		static_dir: &str,
		no_spa: bool,
		no_project_static: bool,
		no_wasm: bool,
		no_override_wasm: bool,
		force_wasm: bool,
		wasm_optional: bool,
	) -> CommandResult<()> {
		use reinhardt_server::{HttpServer, ShutdownCoordinator};

		use std::time::Duration;

		// Get registered router
		if !reinhardt_urls::routers::is_router_registered() {
			return Err(crate::CommandError::ExecutionError(
                "No router registered. Call reinhardt_urls::routers::register_router() or reinhardt_urls::routers::register_router_arc() before running the server.".to_string()
            ));
		}

		let base_router = reinhardt_urls::routers::get_router().ok_or_else(|| {
			crate::CommandError::ExecutionError("Failed to get registered router".to_string())
		})?;

		// Wrap with OpenAPI endpoints if enabled
		#[cfg(feature = "openapi-router")]
		let router = if !no_docs {
			use reinhardt_http::Handler;
			use reinhardt_openapi::OpenApiRouter;
			let wrapped = OpenApiRouter::wrap(base_router)
				.map_err(|e| format!("Failed to initialize OpenAPI router: {}", e))?;
			std::sync::Arc::new(wrapped) as std::sync::Arc<dyn Handler>
		} else {
			base_router
		};

		#[cfg(not(feature = "openapi-router"))]
		let router = base_router;

		// Parse socket address
		let addr: std::net::SocketAddr = address.parse().map_err(|e| {
			crate::CommandError::ExecutionError(format!("Invalid address '{}': {}", address, e))
		})?;

		// Create shutdown coordinator with 30s graceful shutdown timeout
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));

		// Spawn CTRL-C signal handler
		let shutdown_tx = coordinator.clone();
		tokio::spawn(async move {
			if let Err(e) = tokio::signal::ctrl_c().await {
				eprintln!("Failed to listen for CTRL-C: {}", e);
				return;
			}
			println!("\nReceived CTRL-C, shutting down gracefully...");
			shutdown_tx.shutdown();
		});

		// Collect and validate runserver hooks (#3442). The validate phase is
		// shared with the autoreload-parent path so misconfigured hooks fail
		// fast in the parent before any child is spawned (#4244).
		let hooks = Self::validate_hooks_only(ctx).await?;

		// OpenAPI documentation is shown in startup banner above

		// Resolve DI context: reuse user-provided context from router, or create a new one.
		// When the user attaches a DI context via UnifiedRouter::with_di_context(),
		// we must register server-managed singletons (e.g., DatabaseConnection)
		// into that context's singleton scope rather than creating a separate one.
		let (singleton_scope, user_provided_context) =
			if let Some(existing_ctx) = reinhardt_urls::routers::get_router_di_context() {
				ctx.verbose("Using user-provided DI context from router configuration");
				(existing_ctx.singleton_scope().clone(), Some(existing_ctx))
			} else {
				let scope = std::sync::Arc::new(reinhardt_di::SingletonScope::new());
				// Apply deferred DI registrations only when no user context exists.
				// When a user context is present, UnifiedRouter::flush_di_registrations
				// has already applied them to the user's singleton scope.
				if let Some(registrations) = reinhardt_urls::routers::take_di_registrations() {
					ctx.verbose("Applying deferred DI registrations from route configuration");
					registrations.apply_to(&scope);
				}
				(scope, None)
			};

		// Register DatabaseConnection in DI context when database feature is enabled.
		// ORM is already initialized by run_command_with_registry() via
		// initialize_orm_database(), so we only need to get the connection
		// and register it in the DI singleton scope. (#3186)
		#[cfg(feature = "reinhardt-db")]
		{
			match reinhardt_db::orm::get_connection().await {
				Ok(db_conn) => {
					// Register DatabaseConnection directly (not wrapped in Arc)
					// The DI system wraps it in Arc internally via SingletonScope::set
					singleton_scope.set(db_conn);
					let url = std::env::var("DATABASE_URL").ok().unwrap_or_default();
					ctx.info(&format!(
						"💾 Database: {} (DI registered)",
						sanitize_database_url(&url)
					));
				}
				Err(e) => {
					ctx.warning(&format!(
						"⚠️ Failed to get database connection for DI: {}",
						e
					));
				}
			}
		}

		// Build or reuse the DI context
		let di_context = match user_provided_context {
			Some(ctx) => ctx,
			None => std::sync::Arc::new(
				reinhardt_di::InjectionContext::builder(singleton_scope).build(),
			),
		};

		// Invoke runserver hook startup phase (#3442)
		if !hooks.is_empty() {
			let runserver_ctx = crate::runserver_hooks::RunserverContext {
				shutdown_coordinator: coordinator.clone(),
				di_context: di_context.clone(),
			};
			for collected in &hooks {
				collected
					.hook
					.on_server_start(&runserver_ctx)
					.await
					.map_err(|e| {
						crate::CommandError::ExecutionError(format!(
							"Runserver hook startup failed for {}: {}",
							collected.type_name, e
						))
					})?;
			}
		}

		// Create HTTP server with DI context and logging middleware
		let mut server = HttpServer::new(router)
			.with_di_context(di_context)
			.with_middleware(reinhardt_middleware::LoggingMiddleware::new());

		// Add static files middleware for WASM frontend if enabled
		if with_pages {
			use reinhardt_utils::staticfiles::PathResolver;
			use reinhardt_utils::staticfiles::caching::CacheControlConfig;
			use reinhardt_utils::staticfiles::middleware::{
				StaticFilesConfig, StaticFilesMiddleware,
			};

			// Auto-mount <project-root>/static/ at /static/ unless opted out.
			// This is registered BEFORE the dist/ middleware so that, when
			// MiddlewareChain::handle reverses registration order at request
			// time, the project-static middleware sits outermost and runs
			// first; misses fall through to the dist/ middleware and then to
			// the application router (Issue #4484).
			if !no_project_static && let Some(project_root) = PathResolver::find_project_root() {
				let project_static_dir = project_root.join("static");
				if project_static_dir.is_dir() {
					let mut project_static_config =
						StaticFilesConfig::new(project_static_dir.clone())
							.url_prefix("/static/")
							.spa_mode(false)
							.auto_inject_wasm(false)
							.passthrough_prefixes(vec!["/static/admin/".to_string()]);
					// Disable long-lived caching in dev (mirrors #4383 for the
					// dist/ bundle so hot-reload picks up CSS/JS edits).
					#[cfg(debug_assertions)]
					{
						project_static_config =
							project_static_config.cache_config(CacheControlConfig::disabled());
					}
					server =
						server.with_middleware(StaticFilesMiddleware::new(project_static_config));
					ctx.verbose(&format!(
						"Project static files middleware enabled: {} (mounted at /static/)",
						project_static_dir.display()
					));
				}
			}

			// Automatically resolve static directory path
			let resolved_static_dir = PathResolver::resolve_static_dir(static_dir);

			let mut static_config = StaticFilesConfig::new(resolved_static_dir.clone())
				.url_prefix("/")
				.spa_mode(!no_spa)
				// Exclude framework-managed route prefixes from SPA fallback
				// so that API endpoints and admin panel are handled by the
				// application router instead of receiving index.html.
				.excluded_prefixes(vec![
					"/api/".to_string(),
					"/admin/".to_string(),
					"/static/admin/".to_string(),
				]);

			// Issue #4383: In debug builds (dev runserver), disable the
			// long-lived `public, immutable, max-age=31536000` Cache-Control
			// policy that is applied by default to `.js` / `.wasm` / `.css`
			// bundle assets. Without this, browsers never re-validate during
			// development and hot-reload appears broken. Release builds keep
			// the immutable policy for production-grade caching.
			#[cfg(debug_assertions)]
			{
				static_config = static_config.cache_config(CacheControlConfig::disabled());
			}

			#[cfg(feature = "pages")]
			if let Some(hmr_port) = Self::autoreload_hmr_port_from_env(ctx) {
				static_config = static_config
					.trusted_html_injection(reinhardt_pages::hmr::hmr_script_tag(hmr_port));
			}

			// Resolve index file for SPA fallback (only when SPA mode is enabled)
			// Refs #2869: Separate index.html (source) from dist/ (build output)
			if !no_spa {
				let index_option = ctx.option("index").map(|s| s.to_string());
				let index_path = match &index_option {
					Some(path) => {
						let resolved = PathResolver::resolve_static_dir(path);
						if resolved.exists() {
							Some(resolved)
						} else {
							ctx.warning(&format!("Index file not found: {}", path));
							None
						}
					}
					None => {
						// Auto-detect from project root
						let candidate = PathResolver::find_project_root()
							.unwrap_or_else(|| std::env::current_dir().unwrap())
							.join("index.html");
						if candidate.exists() {
							Some(candidate)
						} else {
							None // Fallback to existing behavior
						}
					}
				};

				if let Some(ref path) = index_path {
					static_config = static_config.index_file(path.clone());
				}
			}

			server = server.with_middleware(StaticFilesMiddleware::new(static_config));
			ctx.verbose(&format!(
				"Static files middleware enabled: {} (resolved from: {})",
				resolved_static_dir.display(),
				static_dir
			));
		}

		// Run with or without auto-reload
		if !noreload {
			#[cfg(feature = "autoreload")]
			{
				let index_raw = ctx.option("index").map(|s| s.to_string());
				Self::run_with_autoreload(
					ctx,
					address,
					_insecure,
					no_docs,
					with_pages,
					static_dir,
					no_spa,
					no_project_static,
					index_raw.as_deref(),
					no_wasm_rebuild,
					no_wasm,
					no_override_wasm,
					force_wasm,
					wasm_optional,
					crate::debounced_watcher::DEBOUNCE_WINDOW,
				)
				.await
			}
			#[cfg(not(feature = "autoreload"))]
			{
				server
					.listen_with_shutdown(addr, ShutdownCoordinator::clone(&coordinator))
					.await
					.map_err(|e| crate::CommandError::ExecutionError(e.to_string()))
			}
		} else {
			server
				.listen_with_shutdown(addr, ShutdownCoordinator::clone(&coordinator))
				.await
				.map_err(|e| crate::CommandError::ExecutionError(e.to_string()))
		}
	}

	/// Start the browser-facing HMR WebSocket listener for autoreload mode.
	#[cfg(all(feature = "server", feature = "autoreload", feature = "pages"))]
	async fn start_autoreload_hmr(
		ctx: &CommandContext,
		with_pages: bool,
	) -> CommandResult<Option<(reinhardt_pages::hmr::HmrServer, u16)>> {
		if !with_pages {
			return Ok(None);
		}

		let requested_port = std::env::var("REINHARDT_HMR_PORT")
			.ok()
			.and_then(|raw| raw.parse::<u16>().ok())
			.unwrap_or(35729);

		let server = reinhardt_pages::hmr::HmrServer::new(
			reinhardt_pages::hmr::HmrConfig::builder()
				.ws_port(requested_port)
				.build(),
		);

		let addr = match server.start_listener_only().await {
			Ok(addr) => addr,
			Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
				ctx.warning(&format!(
					"[hot-reload] HMR port {} is in use, using an ephemeral port",
					requested_port
				));
				let fallback = reinhardt_pages::hmr::HmrServer::new(
					reinhardt_pages::hmr::HmrConfig::builder()
						.ws_port(0)
						.build(),
				);
				let addr = fallback.start_listener_only().await.map_err(|err| {
					crate::CommandError::ExecutionError(format!(
						"Failed to start HMR WebSocket listener: {}",
						err
					))
				})?;
				ctx.info(&format!("[hot-reload] HMR websocket: ws://{}/hmr", addr));
				return Ok(Some((fallback, addr.port())));
			}
			Err(e) => {
				return Err(crate::CommandError::ExecutionError(format!(
					"Failed to start HMR WebSocket listener: {}",
					e
				)));
			}
		};

		ctx.info(&format!("[hot-reload] HMR websocket: ws://{}/hmr", addr));
		Ok(Some((server, addr.port())))
	}

	#[cfg(all(feature = "server", feature = "pages"))]
	fn autoreload_hmr_port_from_env(ctx: &CommandContext) -> Option<u16> {
		let raw = std::env::var("REINHARDT_HMR_PORT").ok()?;
		match raw.parse::<u16>() {
			Ok(port) => Some(port),
			Err(_) => {
				ctx.warning(&format!(
					"Ignoring invalid REINHARDT_HMR_PORT value: {}",
					raw
				));
				None
			}
		}
	}

	/// Run server with file watching, debounced dispatch to the wasm + server
	/// rebuild pipelines, and Django-style outer-loop resilience.
	///
	/// Pipeline failures are logged but never propagate as `Err` — only
	/// watcher infrastructure errors (e.g. notify subscribe failed) do.
	#[cfg(all(feature = "server", feature = "autoreload"))]
	// Allow many arguments: autoreload handler mirrors run_server configuration options
	#[allow(clippy::too_many_arguments)]
	async fn run_with_autoreload(
		ctx: &CommandContext,
		address: &str,
		insecure: bool,
		no_docs: bool,
		with_pages: bool,
		static_dir: &str,
		no_spa: bool,
		no_project_static: bool,
		index: Option<&str>,
		no_wasm_rebuild: bool,
		no_wasm: bool,
		no_override_wasm: bool,
		force_wasm: bool,
		wasm_optional: bool,
		debounce_window: std::time::Duration,
	) -> CommandResult<()> {
		// Resolve the cargo metadata for the current working directory.
		let metadata = cargo_metadata::MetadataCommand::new().exec().map_err(|e| {
			crate::CommandError::ExecutionError(format!("cargo metadata failed: {}", e))
		})?;

		let cwd = std::env::current_dir().map_err(|e| {
			crate::CommandError::ExecutionError(format!("Failed to get current directory: {}", e))
		})?;
		let cwd_manifest = cwd.join("Cargo.toml");

		let roots = crate::source_roots::SourceRoots::from_metadata(&metadata, &cwd_manifest);

		// Derive the bin name from the current executable file stem. The
		// child server is always re-spawned by re-execing the same binary,
		// but `cargo build --bin <name>` needs the cargo bin name and that
		// matches the executable file stem in every project layout we
		// ship templates for.
		let current_exe_for_bin = std::env::current_exe().map_err(|e| {
			crate::CommandError::ExecutionError(format!("Failed to get current executable: {}", e))
		})?;
		let bin_name = current_exe_for_bin
			.file_stem()
			.and_then(|s| s.to_str())
			.unwrap_or("manage")
			.to_string();

		// Optional diagnostic snapshot at autoreload startup. See issue #4236.
		// Gated on `REINHARDT_AUTORELOAD_DEBUG=1` so production runs stay
		// quiet but the user can capture full state with one env flip.
		if Self::autoreload_debug_enabled() {
			eprintln!(
				"[autoreload-debug] bin_name={} {}",
				bin_name,
				Self::spawn_diagnostics(&current_exe_for_bin, true),
			);
		}

		// Startup banner (spec §7).
		ctx.info("[hot-reload] enabled");
		ctx.info(&format!(
			"  watching: {} source roots",
			roots.src_dirs.len() + roots.manifest_files.len()
		));
		for dir in &roots.src_dirs {
			ctx.info(&format!("    - {}", dir.display()));
		}
		for manifest in &roots.manifest_files {
			ctx.info(&format!("    - {}", manifest.display()));
		}
		#[cfg(feature = "pages")]
		{
			if with_pages && !no_wasm_rebuild {
				ctx.info("  pipelines: server rebuild + restart, wasm rebuild");
			} else {
				ctx.info("  pipelines: server rebuild + restart");
			}
		}
		#[cfg(not(feature = "pages"))]
		{
			ctx.info("  pipelines: server rebuild + restart");
		}
		ctx.info("  on failure: keep watching (Ctrl+C to quit)");

		// Set up Ctrl+C handler.
		let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
		let ctx_clone = ctx.clone();
		tokio::spawn(async move {
			if let Err(e) = tokio::signal::ctrl_c().await {
				eprintln!("Failed to listen for Ctrl+C: {}", e);
				return;
			}
			ctx_clone.info("\nReceived Ctrl+C, shutting down...");
			let _ = shutdown_tx.send(());
		});

		// Captured state for the respawn closure.
		let address_owned = address.to_string();
		let static_dir_owned = static_dir.to_string();
		let index_owned = index.map(|s| s.to_string());
		#[cfg(feature = "pages")]
		let hmr = Self::start_autoreload_hmr(ctx, with_pages).await?;
		#[cfg(feature = "pages")]
		let hmr_port = hmr.as_ref().map(|(_, port)| *port);
		#[cfg(not(feature = "pages"))]
		let hmr_port: Option<u16> = None;
		#[cfg(feature = "pages")]
		let hmr_tx = hmr.as_ref().map(|(server, _)| server.sender());

		let respawn = move || -> std::io::Result<tokio::process::Child> {
			Self::spawn_server_process(
				&address_owned,
				insecure,
				no_docs,
				with_pages,
				&static_dir_owned,
				no_spa,
				no_project_static,
				index_owned.as_deref(),
				hmr_port,
				no_wasm,
				no_override_wasm,
				force_wasm,
				wasm_optional,
			)
			.map_err(|e| std::io::Error::other(e.to_string()))
		};

		// Spawn the initial child via the respawn closure so identical
		// argument handling applies to startup and every reload.
		let child = respawn().map_err(|e| {
			crate::CommandError::ExecutionError(format!("Failed to spawn initial server: {}", e))
		})?;

		let cfg = crate::debounced_watcher::WatcherConfig {
			bin_name,
			address: address.to_string(),
			roots,
			debounce_window,
			server_address: Some(address.to_string()),
			no_wasm_rebuild,
			#[cfg(feature = "pages")]
			pages_enabled: with_pages,
			#[cfg(feature = "pages")]
			hmr_tx,
		};

		crate::debounced_watcher::run_watcher(ctx, &cfg, shutdown_rx, child, respawn)
			.await
			.map_err(|e| {
				crate::CommandError::ExecutionError(format!("File watcher error: {}", e))
			})?;

		Ok(())
	}

	/// Collect diagnostic state about the candidate spawn target.
	///
	/// Two output modes guard against accidental filesystem-layout leakage
	/// in shared log bundles (issue #4250) while preserving the issue #4236
	/// root-cause signals (`(deleted)` suffix on `/proc/self/exe`,
	/// `raw_os_error == 2` / `ENOENT`):
	///
	/// - `debug == false` (default): redacted allowlist of non-sensitive
	///   fields (`exe_filename`, `exists`, `inode`/`nlink`, `pid`,
	///   `proc_self_exe_deleted_suffix`). Embedded into the always-on
	///   `Failed to spawn server process: ...` error message.
	/// - `debug == true` (`REINHARDT_AUTORELOAD_DEBUG=1`): full snapshot
	///   including absolute paths, canonicalised path, file metadata, and
	///   toolchain environment values. Opt-in only.
	///
	/// `raw_os_error` and the `io::ErrorKind` are emitted by the
	/// surrounding `CommandError::ExecutionError` format string and are not
	/// duplicated here.
	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn spawn_diagnostics(exe: &std::path::Path, debug: bool) -> String {
		if debug {
			Self::spawn_diagnostics_full(exe)
		} else {
			Self::spawn_diagnostics_redacted(exe)
		}
	}

	/// Redacted always-on diagnostic body (issue #4250).
	///
	/// Strict allowlist that intentionally excludes any field carrying an
	/// absolute path, file size/mtime, or toolchain environment values.
	/// `proc_self_exe_deleted_suffix` is emitted as `true`, `false`, or
	/// `unknown` so the issue #4236 signal is preserved across read_link
	/// failures.
	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn spawn_diagnostics_redacted(exe: &std::path::Path) -> String {
		let mut parts: Vec<String> = Vec::with_capacity(5);
		let filename = exe
			.file_name()
			.map(|s| s.to_string_lossy().into_owned())
			.unwrap_or_else(|| "?".to_string());
		parts.push(format!("exe_filename={}", filename));
		parts.push(format!("exists={}", exe.exists()));

		#[cfg(unix)]
		{
			if let Ok(m) = exe.metadata() {
				use std::os::unix::fs::MetadataExt;
				parts.push(format!("inode={} nlink={}", m.ino(), m.nlink()));
			}
		}

		parts.push(format!("pid={}", std::process::id()));

		#[cfg(target_os = "linux")]
		{
			// Linux marks a path read from /proc/self/exe whose backing
			// inode was unlinked with the literal " (deleted)" suffix.
			// Match the suffix exactly so paths that legitimately contain
			// "(deleted)" elsewhere don't trip the flag.
			let suffix = match std::fs::read_link("/proc/self/exe") {
				Ok(p) => {
					if p.to_string_lossy().ends_with(" (deleted)") {
						"true"
					} else {
						"false"
					}
				}
				Err(_) => "unknown",
			};
			parts.push(format!("proc_self_exe_deleted_suffix={}", suffix));
		}

		parts.join(" ")
	}

	/// Full diagnostic snapshot, opt-in via `REINHARDT_AUTORELOAD_DEBUG=1`.
	///
	/// Includes absolute paths, the canonicalised path, file metadata, and
	/// toolchain environment values that may leak filesystem layout. Only
	/// invoked from explicit debug code paths.
	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn spawn_diagnostics_full(exe: &std::path::Path) -> String {
		let mut parts: Vec<String> = Vec::with_capacity(12);
		parts.push(format!("exe={}", exe.display()));
		parts.push(format!("exists={}", exe.exists()));

		match exe.metadata() {
			Ok(m) => {
				parts.push(format!("size={}", m.len()));
				#[cfg(unix)]
				{
					use std::os::unix::fs::MetadataExt;
					parts.push(format!("inode={} nlink={}", m.ino(), m.nlink()));
				}
				if let Ok(mt) = m.modified()
					&& let Ok(d) = mt.duration_since(std::time::UNIX_EPOCH)
				{
					parts.push(format!("mtime_unix={}", d.as_secs()));
				}
			}
			Err(e) => parts.push(format!("metadata_err={}", e)),
		}

		match exe.canonicalize() {
			Ok(c) => parts.push(format!("canonical={}", c.display())),
			Err(e) => parts.push(format!("canonical_err={}", e)),
		}

		#[cfg(target_os = "linux")]
		{
			match std::fs::read_link("/proc/self/exe") {
				Ok(p) => {
					let s = p.to_string_lossy();
					let deleted_suffix = s.ends_with(" (deleted)");
					parts.push(format!(
						"proc_self_exe={} deleted_suffix={}",
						p.display(),
						deleted_suffix
					));
				}
				Err(e) => parts.push(format!("proc_self_exe_err={}", e)),
			}
		}

		parts.push(format!("pid={}", std::process::id()));
		parts.push(format!(
			"CARGO_TARGET_DIR={:?}",
			std::env::var_os("CARGO_TARGET_DIR")
		));
		parts.push(format!(
			"RUSTC_WRAPPER={:?}",
			std::env::var_os("RUSTC_WRAPPER")
		));
		parts.push(format!(
			"REINHARDT_IS_AUTORELOAD_CHILD={:?}",
			std::env::var_os("REINHARDT_IS_AUTORELOAD_CHILD")
		));

		parts.join(" ")
	}

	/// Returns true when the user opted into autoreload debug logging via
	/// `REINHARDT_AUTORELOAD_DEBUG=1`. Off by default to keep release output
	/// quiet; on for issue #4236 root-cause diagnosis.
	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn autoreload_debug_enabled() -> bool {
		match std::env::var("REINHARDT_AUTORELOAD_DEBUG") {
			Ok(v) => {
				let v = v.trim();
				v == "1" || v.eq_ignore_ascii_case("true")
			}
			Err(_) => false,
		}
	}

	/// Spawn server in child process
	#[cfg(all(feature = "server", feature = "autoreload"))]
	// Allow many arguments: mirrors run_server's CLI surface and forwards each
	// flag the autoreload parent received to its `--noreload` child.
	#[allow(clippy::too_many_arguments)]
	fn spawn_server_process(
		address: &str,
		insecure: bool,
		no_docs: bool,
		with_pages: bool,
		static_dir: &str,
		no_spa: bool,
		no_project_static: bool,
		index: Option<&str>,
		hmr_port: Option<u16>,
		no_wasm: bool,
		no_override_wasm: bool,
		force_wasm: bool,
		wasm_optional: bool,
	) -> CommandResult<tokio::process::Child> {
		let current_exe = std::env::current_exe().map_err(|e| {
			crate::CommandError::ExecutionError(format!("Failed to get current executable: {}", e))
		})?;

		// Diagnostics policy (issue #4236):
		// - Debug enabled: snapshot BEFORE the spawn syscall and emit so the
		//   pre-spawn state is logged even on success.
		// - Debug disabled: defer the snapshot to the error path to avoid
		//   per-spawn filesystem cost on the success hot path. The post-spawn
		//   read is acceptable because the kernel-visible state for an ENOENT
		//   failure is effectively unchanged in the few microseconds between
		//   the syscall returning and reading the diagnostics.
		let debug = Self::autoreload_debug_enabled();
		let pre_spawn_diag = if debug {
			let d = Self::spawn_diagnostics(&current_exe, true);
			eprintln!("[autoreload-debug] pre-spawn {}", d);
			Some(d)
		} else {
			None
		};

		let mut cmd = tokio::process::Command::new(&current_exe);
		let child_options = AutoreloadChildOptions {
			address,
			insecure,
			no_docs,
			with_pages,
			static_dir,
			no_spa,
			no_project_static,
			index,
			hmr_port,
			no_wasm,
			no_override_wasm,
			force_wasm,
			wasm_optional,
		};
		cmd.args(Self::build_autoreload_child_args(&child_options));
		if let Some(port) = child_options.hmr_port {
			cmd.env("REINHARDT_HMR_PORT", port.to_string());
		}

		// Set environment variable to indicate this is a child process (prevent log duplication, etc.)
		cmd.env("REINHARDT_IS_AUTORELOAD_CHILD", "1");

		// Inherit stdout/stderr from parent process
		cmd.stdout(std::process::Stdio::inherit());
		cmd.stderr(std::process::Stdio::inherit());

		cmd.spawn().map_err(|e| {
			// Always-on enrichment (#4236): include the resolved path,
			// `raw_os_error`, error kind, and Linux `/proc/self/exe` state in
			// the error so the first failure log is actionable. Reuse the
			// pre-spawn snapshot when debug emitted one; otherwise compute it
			// lazily here so the success path pays no diagnostic cost.
			let diag = pre_spawn_diag
				.clone()
				.unwrap_or_else(|| Self::spawn_diagnostics(&current_exe, false));
			crate::CommandError::ExecutionError(format!(
				"Failed to spawn server process: {} (raw_os_error={:?} kind={:?}) [{}]",
				e,
				e.raw_os_error(),
				e.kind(),
				diag,
			))
		})
	}

	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn build_autoreload_child_args(options: &AutoreloadChildOptions<'_>) -> Vec<String> {
		let mut args = vec![
			"runserver".to_string(),
			options.address.to_string(),
			"--noreload".to_string(),
		];

		if options.insecure {
			args.push("--insecure".to_string());
		}
		if options.no_docs {
			args.push("--no-docs".to_string());
		}
		if options.with_pages {
			args.push("--with-pages".to_string());
		}
		if !options.static_dir.is_empty() {
			args.push("--static-dir".to_string());
			args.push(options.static_dir.to_string());
		}
		if options.no_spa {
			args.push("--no-spa".to_string());
		}
		if options.no_project_static {
			args.push("--no-project-static".to_string());
		}
		if let Some(index_path) = options.index {
			args.push("--index".to_string());
			args.push(index_path.to_string());
		}
		if options.no_wasm {
			args.push("--no-wasm".to_string());
		}
		if options.no_override_wasm {
			args.push("--no-override-wasm".to_string());
		}
		if options.force_wasm {
			args.push("--force-wasm".to_string());
		}
		if options.wasm_optional {
			args.push("--wasm-optional".to_string());
		}

		args
	}

	/// Build the pages WASM bundle from the current project (if it declares cdylib).
	///
	/// Mirrors the logic in the standalone runserver binary. Returns an error
	/// when the WASM compilation fails so the caller can decide whether to
	/// abort or continue.
	#[cfg(feature = "pages")]
	pub(crate) fn build_pages_wasm(
		ctx: &CommandContext,
		force: bool,
	) -> Result<(), crate::wasm_builder::WasmBuildError> {
		let cwd = match std::env::current_dir() {
			Ok(d) => d,
			Err(e) => {
				ctx.warning(&format!("Failed to get current directory: {}", e));
				return Ok(());
			}
		};
		let cargo_toml_path = cwd.join("Cargo.toml");

		// Only build if this project exports cdylib
		if !crate::wasm_builder::detect_cdylib_in_cargo_toml(&cargo_toml_path) {
			return Ok(());
		}

		// Parse the crate name from Cargo.toml
		let crate_name = match std::fs::read_to_string(&cargo_toml_path) {
			Ok(content) => {
				let mut name = String::new();
				for line in content.lines() {
					let trimmed = line.trim();
					if trimmed.starts_with("name")
						&& trimmed.contains('=')
						&& let Some(val) = trimmed.split('=').nth(1)
					{
						name = val.trim().trim_matches('"').trim_matches('\'').to_string();
						break;
					}
				}
				if name.is_empty() {
					ctx.warning("Could not determine crate name from Cargo.toml");
					return Ok(());
				}
				name
			}
			Err(e) => {
				ctx.warning(&format!("Failed to read Cargo.toml: {}", e));
				return Ok(());
			}
		};

		let js_name = crate_name.replace('-', "_");
		let artifact = cwd.join("dist").join(format!("{}_bg.wasm", js_name));
		if !force && !crate::wasm_builder::is_wasm_stale(&cwd, &artifact) {
			ctx.info("Pages WASM: artifacts up to date, skipping build (--no-override-wasm)");
			return Ok(());
		}

		let reason = if force {
			"forced rebuild"
		} else if artifact.exists() {
			"source changed since last build"
		} else {
			"no existing artifact"
		};
		ctx.info(&format!(
			"Building pages WASM for {} ({})...",
			crate_name, reason
		));
		let config = crate::wasm_builder::WasmBuildConfig::new(".").output_dir("dist");
		match crate::wasm_builder::WasmBuilder::new(config).build() {
			Ok(_) => {
				ctx.info("Pages WASM build succeeded.");
				Ok(())
			}
			Err(e) => Err(e),
		}
	}
}

/// Show all URLs command
#[cfg(feature = "routers")]
pub struct ShowUrlsCommand;

#[cfg(feature = "routers")]
#[async_trait]
impl BaseCommand for ShowUrlsCommand {
	fn name(&self) -> &str {
		"showurls"
	}

	fn description(&self) -> &str {
		"Display all registered server URL patterns"
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![CommandOption::flag(None, "names", "Show only named URLs")]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		// Check if router is registered
		if !reinhardt_urls::routers::is_router_registered() {
			ctx.warning(
				"No router registered. Call reinhardt_urls::routers::register_router() in your application startup.",
			);
			ctx.info("");
			ctx.info("Example:");
			ctx.info("  let router = UnifiedRouter::new()");
			ctx.info("      .with_prefix(\"/api\")");
			ctx.info("      .endpoint(health_check);");
			ctx.info("");
			ctx.info("  reinhardt_urls::routers::register_router(Arc::new(router));");
			return Ok(());
		}

		// Get registered router
		let router = reinhardt_urls::routers::get_router()
			.expect("Router should be registered (checked above)");

		// Get all routes
		let routes = router.get_all_routes();

		if routes.is_empty() {
			ctx.info("No server routes registered.");
			return Ok(());
		}

		// Check if --names flag is set
		let names_only = ctx.has_option("names");

		// Display header
		ctx.info("Registered server URL patterns:");
		ctx.info("");

		if names_only {
			// Show only named URLs
			let named_routes: Vec<_> = routes
				.iter()
				.filter(|(_, name, _, _)| name.is_some())
				.collect();

			if named_routes.is_empty() {
				ctx.info("No named URLs registered.");
				return Ok(());
			}

			ctx.info(&format!(
				"{:<40} {:<30} {:<20}",
				"URL Pattern", "Name", "Namespace"
			));
			ctx.info(&"=".repeat(90));

			for (path, name, namespace, _) in named_routes {
				let name_str = name.as_ref().map(|s| s.as_str()).unwrap_or("-");
				let namespace_str = namespace.as_ref().map(|s| s.as_str()).unwrap_or("-");

				ctx.info(&format!(
					"{:<40} {:<30} {:<20}",
					path, name_str, namespace_str
				));
			}
		} else {
			// Show all URLs with methods
			ctx.info(&format!(
				"{:<40} {:<20} {:<15} {:<20}",
				"URL Pattern", "Methods", "Name", "Namespace"
			));
			ctx.info(&"=".repeat(95));

			for (path, name, namespace, methods) in &routes {
				let methods_str = if methods.is_empty() {
					"ALL".to_string()
				} else {
					methods
						.iter()
						.map(|m| m.as_str())
						.collect::<Vec<_>>()
						.join(", ")
				};

				let name_str = name.as_ref().map(|s| s.as_str()).unwrap_or("-");
				let namespace_str = namespace.as_ref().map(|s| s.as_str()).unwrap_or("-");

				ctx.info(&format!(
					"{:<40} {:<20} {:<15} {:<20}",
					path, methods_str, name_str, namespace_str
				));
			}
		}

		ctx.info("");
		ctx.success(&format!("Total server routes: {}", routes.len()));

		Ok(())
	}
}

/// Check system command
pub struct CheckCommand;

#[async_trait]
impl BaseCommand for CheckCommand {
	fn name(&self) -> &str {
		"check"
	}

	fn description(&self) -> &str {
		"Check for common problems"
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![CommandOption::flag(
			None,
			"deploy",
			"Check deployment settings",
		)]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		ctx.info("System check:");
		ctx.info("");

		let is_deploy = ctx.has_option("deploy");
		let mut checks_passed = 0;
		let mut checks_failed = 0;

		// 1. Database connectivity check. Prefer the URL exposed by the
		// composed `ProjectSettings` attached to `ctx.settings`; fall back
		// to the `DATABASE_URL` env var so users running the CLI without
		// settings plumbing keep getting the legacy behavior.
		let database_url = Self::resolve_database_url(ctx);
		if let Some(database_url) = database_url.as_deref() {
			ctx.info("Checking database connectivity...");
			match Self::check_database(database_url).await {
				Ok(_) => {
					ctx.success("  ✓ Database connection successful");
					checks_passed += 1;
				}
				Err(e) => {
					ctx.warning(&format!("  ✗ Database connection failed: {}", e));
					checks_failed += 1;
				}
			}
		} else {
			ctx.info(
				"Skipping database check (no DATABASE_URL env var and no [core.databases.default])",
			);
		}

		// 2. Settings validation
		ctx.info("Checking settings...");
		checks_passed += Self::check_settings(ctx, is_deploy);

		// 3. Migration status check (only when we have a database URL).
		if database_url.is_some() {
			ctx.info("Checking migrations...");
			match Self::check_migrations().await {
				Ok(count) => {
					if count == 0 {
						ctx.success("  ✓ All migrations applied");
						checks_passed += 1;
					} else {
						ctx.warning(&format!("  ⚠ {} unapplied migrations found", count));
					}
				}
				Err(e) => {
					ctx.warning(&format!("  ✗ Migration check failed: {}", e));
					checks_failed += 1;
				}
			}
		}

		// 4. Static files verification. Use the composed settings'
		// `static_files.root` when available, else fall back to the
		// `STATIC_ROOT` env var.
		ctx.info("Checking static files...");
		if Self::resolve_static_root_configured(ctx) {
			ctx.success("  ✓ static files root configured");
			checks_passed += 1;
		} else if is_deploy {
			ctx.warning("  ✗ static files root not set (required for deployment)");
			checks_failed += 1;
		} else {
			ctx.info("  ⚠ static files root not set (optional for development)");
		}

		// 5. Security settings check (if --deploy)
		if is_deploy {
			ctx.info("Checking security settings...");
			checks_passed += Self::check_security(ctx);
		}

		ctx.info("");
		ctx.info(&format!(
			"System check complete: {} passed, {} failed",
			checks_passed, checks_failed
		));

		if checks_failed > 0 {
			Err(crate::CommandError::ExecutionError(format!(
				"{} check(s) failed",
				checks_failed
			)))
		} else {
			Ok(())
		}
	}
}

impl CheckCommand {
	/// Resolve the database URL from the composed settings on `ctx`,
	/// falling back to the `DATABASE_URL` environment variable.
	///
	/// Returns `None` when neither source produces a URL.
	fn resolve_database_url(ctx: &CommandContext) -> Option<String> {
		let env_database_url = std::env::var("DATABASE_URL").ok();

		#[cfg(feature = "reinhardt-db")]
		if let Some(settings) = ctx.settings.as_ref()
			&& let Ok(url) = DatabaseConnection::database_url_from(
				settings.as_ref(),
				env_database_url.as_deref(),
			) {
			sync_database_url_to_env(env_database_url.as_deref(), &url, ctx);
			return Some(url);
		}

		#[cfg(not(feature = "reinhardt-db"))]
		if let Some(settings) = ctx.settings.as_ref()
			&& let Some(db) = settings.core().databases.get("default")
		{
			return Some(db.to_url());
		}

		if let Some(url) = env_database_url {
			return Some(url);
		}

		#[cfg(feature = "reinhardt-db")]
		if let Ok(url) = get_database_url_from_settings() {
			sync_database_url_to_env(None, &url, ctx);
			return Some(url);
		}

		None
	}

	/// Returns true when a static-files root is configured, either via
	/// composed settings or via the `STATIC_ROOT` env var.
	fn resolve_static_root_configured(_ctx: &CommandContext) -> bool {
		// CoreSettings does not own the static-files root; downstream
		// projects compose `StaticSettings` separately. Without
		// `HasStaticSettings` in `HasCommonSettings` we cannot peek at
		// it generically here, so we currently only consult the env
		// var. This preserves existing behavior while issue #4282's
		// follow-up wires the static-files fragment through the
		// CommandContext.
		std::env::var("STATIC_ROOT").is_ok()
	}

	/// Check database connectivity
	async fn check_database(database_url: &str) -> Result<(), String> {
		if database_url.is_empty() {
			return Err("Empty database URL".to_string());
		}

		#[cfg(feature = "migrations")]
		{
			// Actually connect to database and verify connectivity
			match connect_database(database_url).await {
				Ok((db_type, connection)) => {
					// Execute a simple query to verify connection
					match db_type {
						DatabaseType::Postgres | DatabaseType::Sqlite => {
							connection
								.execute("SELECT 1", vec![])
								.await
								.map_err(|e| format!("Query failed: {}", e))?;
						}
						_ => {
							// MySQL or other database types that don't have SQL execution support yet
						}
					}
					Ok(())
				}
				Err(e) => Err(format!("Connection failed: {:?}", e)),
			}
		}

		#[cfg(not(feature = "migrations"))]
		{
			// Basic URL validation only
			Ok(())
		}
	}

	/// Check settings configuration
	fn check_settings(ctx: &CommandContext, is_deploy: bool) -> u32 {
		let mut passed = 0;

		// Check SECRET_KEY (always required in deployment)
		if is_deploy {
			if let Ok(secret_key) = std::env::var("SECRET_KEY") {
				if secret_key.len() >= 32 {
					ctx.success("  ✓ SECRET_KEY configured");
					passed += 1;
				} else {
					ctx.warning("  ✗ SECRET_KEY too short (minimum 32 characters)");
				}
			} else {
				ctx.warning("  ✗ SECRET_KEY not set (required for deployment)");
			}
		}

		// Check DEBUG setting
		if let Ok(debug) = std::env::var("DEBUG") {
			if is_deploy && debug == "true" {
				ctx.warning("  ✗ DEBUG=true in deployment (should be false)");
			} else {
				ctx.success("  ✓ DEBUG setting appropriate");
				passed += 1;
			}
		}

		passed
	}

	/// Check migrations status
	async fn check_migrations() -> Result<u32, String> {
		#[cfg(feature = "migrations")]
		{
			use reinhardt_db::migrations::{
				DatabaseMigrationRecorder, FilesystemRepository, FilesystemSource, MigrationService,
			};
			use std::path::PathBuf;
			use std::sync::Arc;
			use tokio::sync::Mutex;

			// 1. Load migration files from disk using FilesystemSource and Repository
			let migrations_dir = PathBuf::from("migrations");
			let source = Arc::new(FilesystemSource::new(migrations_dir.clone()));
			let repository = Arc::new(Mutex::new(FilesystemRepository::new(migrations_dir)));
			let service = MigrationService::new(source, repository);

			let all_migrations = service
				.load_all()
				.await
				.map_err(|e| format!("Failed to load all migrations: {:?}", e))?;

			// 2. Connect to database
			let database_url =
				std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL not set".to_string())?;

			let (_db_type, connection) = connect_database(&database_url)
				.await
				.map_err(|e| format!("Database connection failed: {:?}", e))?;

			// 3. Check applied migrations using Recorder
			let recorder = DatabaseMigrationRecorder::new(connection);
			recorder
				.ensure_schema_table()
				.await
				.map_err(|e| format!("Failed to create migration table: {}", e))?;

			// 4. Count unapplied migrations
			let mut unapplied_count = 0;
			for migration in &all_migrations {
				let is_applied = recorder
					.is_applied(&migration.app_label, &migration.name)
					.await
					.map_err(|e| format!("Failed to check migration: {}", e))?;

				if !is_applied {
					unapplied_count += 1;
				}
			}

			Ok(unapplied_count)
		}

		#[cfg(not(feature = "migrations"))]
		{
			// Without migrations feature, assume no unapplied migrations
			Ok(0)
		}
	}

	/// Check security settings
	fn check_security(ctx: &CommandContext) -> u32 {
		let mut passed = 0;

		// Check ALLOWED_HOSTS
		if std::env::var("ALLOWED_HOSTS").is_ok() {
			ctx.success("  ✓ ALLOWED_HOSTS configured");
			passed += 1;
		} else {
			ctx.warning("  ✗ ALLOWED_HOSTS not set (required for deployment)");
		}

		// Check SECURE_SSL_REDIRECT
		if let Ok(ssl_redirect) = std::env::var("SECURE_SSL_REDIRECT")
			&& ssl_redirect == "true"
		{
			ctx.success("  ✓ SECURE_SSL_REDIRECT enabled");
			passed += 1;
		}

		passed
	}
}

/// Syncs the `DATABASE_URL` environment variable from the resolved settings URL when
/// it is not already set, or warns if the environment value differs from settings.
///
/// This ensures that all database connection paths (ORM, sqlx pools, etc.) use the
/// same URL derived from a single configuration source.
#[cfg(feature = "reinhardt-db")]
fn sync_database_url_to_env(
	env_database_url: Option<&str>,
	resolved_url: &str,
	ctx: &CommandContext,
) {
	if env_database_url.is_none() {
		// SAFETY: Called before spawning server threads. The tokio runtime is
		// running but no request-handling tasks have been spawned yet, so no
		// concurrent reads of this env var are possible at this point.
		unsafe {
			std::env::set_var("DATABASE_URL", resolved_url);
		}
		ctx.verbose("Synced DATABASE_URL from settings configuration");
	} else if let Some(env_url) = env_database_url {
		// Both env var and settings exist - warn if they differ.
		// Prefer the composed `ProjectSettings` on `ctx` when present
		// (no disk I/O); otherwise fall back to re-loading TOML.
		let settings_url_result = match ctx.settings.as_ref() {
			Some(settings) => DatabaseConnection::database_url_from(settings.as_ref(), None)
				.map_err(|e| crate::CommandError::ExecutionError(e.to_string())),
			None => get_database_url_from_settings(),
		};
		if let Ok(settings_url) = settings_url_result
			&& env_url != settings_url
		{
			ctx.warning(&format!(
				"⚠️ DATABASE_URL mismatch: env var ({}) differs from settings ({}). Using env var.",
				sanitize_database_url(env_url),
				sanitize_database_url(&settings_url),
			));
		}
	}
}

/// Sanitizes a database URL for display, removing credentials.
///
/// Replaces `user:password@` with `***@` to prevent credential leakage
/// in logs and startup banners.
#[cfg(feature = "reinhardt-db")]
fn sanitize_database_url(url: &str) -> String {
	if url == "sqlite::memory:" {
		return url.to_string();
	}

	if url.starts_with("sqlite:") {
		return "sqlite:***".to_string();
	}

	// Match scheme://user:pass@host pattern and redact credentials.
	if let Some(scheme_end) = url.find("://") {
		let after_scheme = &url[scheme_end + 3..];
		if let Some(at_pos) = after_scheme.find('@') {
			let host_part = &after_scheme[at_pos..];
			return format!("{}://***{}", &url[..scheme_end], host_part);
		}
	}

	url.to_string()
}

/// Initialize the ORM database connection from settings.
///
/// Resolves the database URL from `ctx.settings` (via
/// [`DatabaseConnection::database_url_from`]) or falls back
/// to the `DATABASE_URL` environment variable, syncs the
/// resolved URL to `DATABASE_URL`, and initializes the ORM
/// global connection pool.
///
/// This function does **not** handle DI registration — that remains
/// the responsibility of `runserver` since only HTTP-serving commands
/// need the `DatabaseConnection` registered in the DI context.
///
/// # Errors
///
/// Returns [`CommandError::ExecutionError`] if the database URL cannot
/// be resolved or the ORM connection pool fails to initialize.
#[cfg(feature = "reinhardt-db")]
pub(crate) async fn initialize_orm_database(
	ctx: &CommandContext,
) -> Result<(), crate::CommandError> {
	let env_database_url = std::env::var("DATABASE_URL").ok();
	let url =
		match ctx.settings.as_ref() {
			Some(settings) => DatabaseConnection::database_url_from(
				settings.as_ref(),
				env_database_url.as_deref(),
			)
			.map_err(|e| {
				crate::CommandError::ExecutionError(format!("Failed to get database URL: {}", e))
			})?,
			None => match env_database_url.clone() {
				Some(url) => url,
				// No `ctx.settings` and no `DATABASE_URL`: fall back to the disk
				// loader that reads `settings/*.toml` directly. This restores
				// parity with the pre-refactor behaviour and is symmetric with
				// `sync_database_url_to_env`, which already re-resolves from
				// settings on its `None` arm (#5042).
				None => get_database_url_from_settings()?,
			},
		};

	sync_database_url_to_env(env_database_url.as_deref(), &url, ctx);

	reinhardt_db::orm::init_database(&url).await.map_err(|e| {
		crate::CommandError::ExecutionError(format!("Failed to initialize ORM database: {}", e))
	})?;

	ctx.verbose("ORM database initialized");
	ctx.info(&format!(
		"💾 Database: {} (connected)",
		sanitize_database_url(&url)
	));
	Ok(())
}

/// Helper function to get DATABASE_URL from settings files only (ignoring env var).
///
/// Used for startup validation to detect configuration mismatches between
/// the `DATABASE_URL` environment variable and `settings/*.toml` files.
///
/// Visibility is `pub(crate)` so the in-crate regression tests for issue
/// #4247 can call the real loader without going through a public API.
#[cfg(feature = "reinhardt-db")]
pub(crate) fn get_database_url_from_settings() -> Result<String, crate::CommandError> {
	use std::env;

	let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
	let profile = reinhardt_conf::settings::profile::Profile::parse(&profile_str);

	let base_dir = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
	let settings_dir = base_dir.join("settings");

	let merged = reinhardt_conf::settings::builder::SettingsBuilder::new()
		.profile(profile)
		.add_source(
			reinhardt_conf::settings::sources::DefaultSource::new()
				.with_value("debug", serde_json::Value::Bool(false))
				.with_value(
					"language_code",
					serde_json::Value::String("en-us".to_string()),
				)
				.with_value("time_zone", serde_json::Value::String("UTC".to_string())),
		)
		.add_source(
			reinhardt_conf::settings::sources::LowPriorityEnvSource::new()
				.with_prefix("REINHARDT_"),
		)
		// Explicitly opt in to ${VAR:-default} interpolation so the
		// validation comparison sees the same expanded host that the ORM
		// init path uses; otherwise a literal `${...}` host would be
		// compared against an env-derived URL and produce a false
		// "DATABASE_URL mismatch" warning (issue #4235).
		.add_source(
			reinhardt_conf::settings::sources::TomlFileSource::new(settings_dir.join("base.toml"))
				.with_interpolation(),
		)
		.add_source(
			reinhardt_conf::settings::sources::TomlFileSource::new(
				settings_dir.join(format!("{}.toml", profile_str)),
			)
			.with_interpolation(),
		)
		.build()
		.map_err(|e| {
			crate::CommandError::ExecutionError(format!("Failed to load settings: {}", e))
		})?;

	// Locate the database configuration. Prefer the legacy/Django-style
	// top-level `[database]` block, then fall back to the canonical
	// `[core.databases.default]` nested schema so this disk loader understands
	// the same shape `database_url_from` reads from composed settings (#5042).
	let db_val = merged.get_raw("database").or_else(|| {
		merged
			.get_raw("core")
			.and_then(|core| core.get("databases"))
			.and_then(|databases| databases.get("default"))
	});

	let db_config: reinhardt_conf::settings::DatabaseConfig = db_val
		.and_then(|db_val| {
			serde_json::from_value(db_val.clone()).ok().or_else(|| {
				if let serde_json::Value::Object(db_map) = db_val {
					let engine = db_map
						.get("engine")
						.and_then(|v| v.as_str())
						.unwrap_or("sqlite")
						.to_string();
					let name = db_map
						.get("name")
						.and_then(|v| v.as_str())
						.map(|s| s.to_string())
						.unwrap_or_else(|| "db.sqlite3".to_string());

					let mut config = reinhardt_conf::settings::DatabaseConfig::new(engine, name);
					if let Some(user) = db_map.get("user").and_then(|v| v.as_str()) {
						config = config.with_user(user);
					}
					if let Some(password) = db_map.get("password").and_then(|v| v.as_str()) {
						config = config.with_password(password);
					}
					if let Some(host) = db_map.get("host").and_then(|v| v.as_str()) {
						config = config.with_host(host);
					}
					if let Some(port) = db_map.get("port").and_then(|v| v.as_u64()) {
						config = config.with_port(port as u16);
					}
					Some(config)
				} else {
					None
				}
			})
		})
		.ok_or_else(|| {
			crate::CommandError::ExecutionError(
				"No database configuration found in settings files".to_string(),
			)
		})?;

	Ok(db_config.to_url())
}

/// Helper function to connect to database
#[cfg(feature = "reinhardt-db")]
async fn connect_database(url: &str) -> CommandResult<(DatabaseType, DatabaseConnection)> {
	let db_type = if url.starts_with("postgres://") || url.starts_with("postgresql://") {
		DatabaseType::Postgres
	} else if url.starts_with("sqlite://")
		|| url.starts_with("sqlite:")
		|| url.starts_with(":memory:")
	{
		DatabaseType::Sqlite
	} else {
		return Err(crate::CommandError::ExecutionError(format!(
			"Unsupported database URL: {}",
			url
		)));
	};

	match db_type {
		DatabaseType::Postgres => {
			#[cfg(feature = "postgres")]
			{
				let conn = DatabaseConnection::connect_postgres(url)
					.await
					.map_err(|e| {
						crate::CommandError::ExecutionError(format!(
							"Database connection failed: {}",
							e
						))
					})?;
				Ok((db_type, conn))
			}
			#[cfg(not(feature = "postgres"))]
			{
				Err(crate::CommandError::ExecutionError(
					"PostgreSQL support not enabled. Enable 'postgres' feature.".to_string(),
				))
			}
		}
		DatabaseType::Sqlite => {
			#[cfg(feature = "sqlite")]
			{
				let conn = DatabaseConnection::connect_sqlite(url).await.map_err(|e| {
					crate::CommandError::ExecutionError(format!(
						"Database connection failed: {}",
						e
					))
				})?;
				Ok((db_type, conn))
			}
			#[cfg(not(feature = "sqlite"))]
			{
				return Err(crate::CommandError::ExecutionError(
					"SQLite support not enabled. Enable 'sqlite' feature.".to_string(),
				));
			}
		}
		_ => {
			// MySQL or other database types
			Err(crate::CommandError::ExecutionError(format!(
				"Database type {:?} is not yet supported in this feature configuration",
				db_type
			)))
		}
	}
}

/// DI dependency graph check command
pub struct CheckDiCommand;

#[async_trait]
impl BaseCommand for CheckDiCommand {
	fn name(&self) -> &str {
		"check-di"
	}

	fn description(&self) -> &str {
		"Check DI dependency graph for circular dependencies and other issues"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		ctx.info("🔍 Checking DI dependency graph...");

		#[cfg(feature = "di")]
		{
			// Get the global registry
			let registry = reinhardt_di::global_registry();

			// Count registered dependencies
			let registered_count = registry.len();

			ctx.info(&format!(
				"✓ Found {} registered dependencies",
				registered_count
			));

			if registered_count == 0 {
				ctx.warning("No dependencies registered");
				ctx.info(
					"Make sure to import modules that use #[injectable] or register_dependency!",
				);
				return Err(crate::CommandError::ExecutionError(
					"No dependencies found".to_string(),
				));
			}

			ctx.success("No circular dependencies detected at compile time");
			ctx.success("All checks passed");
			ctx.info("");
			ctx.info("Note: Runtime circular dependency detection is active.");
			ctx.info("      Any circular dependencies will be caught during resolution.");

			Ok(())
		}

		#[cfg(not(feature = "di"))]
		{
			ctx.warning("DI feature is not enabled");
			Err(crate::CommandError::ExecutionError(
				"check-di command requires 'di' feature to be enabled".to_string(),
			))
		}
	}
}

/// Database schema introspection command
///
/// Generates Reinhardt ORM models from existing database schema.
pub struct IntrospectCommand;

#[cfg(feature = "migrations")]
#[async_trait]
impl BaseCommand for IntrospectCommand {
	fn name(&self) -> &str {
		"introspect"
	}

	fn description(&self) -> &str {
		"Generate Reinhardt ORM models from existing database schema"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::option(Some('d'), "database", "Database URL to introspect"),
			CommandOption::option(Some('o'), "output", "Output directory for generated files")
				.with_default("src/models/generated"),
			CommandOption::option(Some('a'), "app-label", "App label for generated models")
				.with_default("app"),
			CommandOption::option(Some('c'), "config", "Path to configuration TOML file"),
			CommandOption::option(None, "include", "Regex pattern for tables to include"),
			CommandOption::option(None, "exclude", "Regex pattern for tables to exclude"),
			CommandOption::flag(
				None,
				"dry-run",
				"Show what would be generated without writing",
			),
			CommandOption::flag(None, "force", "Overwrite existing files"),
			CommandOption::flag(Some('v'), "verbose", "Show detailed output"),
			CommandOption::flag(None, "single-file", "Generate all models in a single file"),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		use crate::CommandError;
		use reinhardt_db::migrations::{
			DatabaseIntrospector, IntrospectConfig, generate_models, preview_output, write_output,
		};
		use std::path::PathBuf;

		ctx.info("🔍 Introspecting database schema...");

		let is_dry_run = ctx.has_option("dry-run");
		let is_force = ctx.has_option("force");
		let is_verbose = ctx.has_option("verbose");

		// Build configuration
		let mut config = if let Some(config_path) = ctx.option("config") {
			ctx.verbose(&format!("Loading config from: {}", config_path));
			IntrospectConfig::from_file(config_path)
				.map_err(|e| CommandError::ExecutionError(format!("Config error: {}", e)))?
		} else {
			IntrospectConfig::default()
		};

		// Override with CLI options
		if let Some(db_url) = ctx.option("database") {
			config = config.with_database_url(db_url);
		} else if config.database.url.is_empty() {
			// Try environment variable
			if let Ok(url) = std::env::var("DATABASE_URL") {
				config = config.with_database_url(&url);
			} else {
				return Err(CommandError::ExecutionError(
					"Database URL required. Use --database or set DATABASE_URL environment variable."
						.to_string(),
				));
			}
		}

		if let Some(output_dir) = ctx.option("output") {
			config = config.with_output_dir(PathBuf::from(output_dir));
		}

		if let Some(app_label) = ctx.option("app-label") {
			config = config.with_app_label(app_label);
		}

		if ctx.has_option("single-file") {
			config.output.single_file = true;
		}

		// Handle include/exclude patterns
		if let Some(include) = ctx.option("include") {
			config.tables.include = vec![include.to_string()];
		}

		if let Some(exclude) = ctx.option("exclude") {
			config.tables.exclude.push(exclude.to_string());
		}

		if is_verbose {
			ctx.info(&format!(
				"  Database: {}",
				mask_db_password(&config.database.url)
			));
			ctx.info(&format!("  Output: {:?}", config.output.directory));
			ctx.info(&format!("  App Label: {}", config.generation.app_label));
		}

		// Resolve database URL
		let db_url = config
			.database
			.resolve_url()
			.map_err(|e| CommandError::ExecutionError(format!("URL resolution error: {}", e)))?;

		// Determine database type and create introspector
		let db_type = detect_database_type(&db_url)?;
		ctx.verbose(&format!("Detected database type: {:?}", db_type));

		// Connect and introspect
		ctx.info("Connecting to database...");

		let schema: reinhardt_db::migrations::introspection::DatabaseSchema = match db_type {
			DatabaseType::Postgres => {
				#[cfg(feature = "postgres")]
				{
					use sqlx::postgres::PgPoolOptions;
					let pool = PgPoolOptions::new()
						.max_connections(1)
						.connect(&db_url)
						.await
						.map_err(|e| {
							CommandError::ExecutionError(format!("Connection error: {}", e))
						})?;

					let introspector =
						reinhardt_db::migrations::introspection::PostgresIntrospector::new(pool);
					introspector.read_schema().await.map_err(|e| {
						CommandError::ExecutionError(format!("Introspection error: {}", e))
					})?
				}
				#[cfg(not(feature = "postgres"))]
				{
					return Err(CommandError::ExecutionError(
						"PostgreSQL support not enabled. Enable 'postgres' feature.".to_string(),
					));
				}
			}
			DatabaseType::Mysql => {
				#[cfg(feature = "mysql")]
				{
					use sqlx::mysql::MySqlPoolOptions;
					let pool = MySqlPoolOptions::new()
						.max_connections(1)
						.connect(&db_url)
						.await
						.map_err(|e| {
							CommandError::ExecutionError(format!("Connection error: {}", e))
						})?;

					let introspector =
						reinhardt_db::migrations::introspection::MySQLIntrospector::new(pool);
					introspector.read_schema().await.map_err(|e| {
						CommandError::ExecutionError(format!("Introspection error: {}", e))
					})?
				}
				#[cfg(not(feature = "mysql"))]
				{
					return Err(CommandError::ExecutionError(
						"MySQL support not enabled. Enable 'mysql' feature.".to_string(),
					));
				}
			}
			DatabaseType::Sqlite => {
				#[cfg(feature = "sqlite")]
				{
					use sqlx::sqlite::SqlitePoolOptions;
					let pool = SqlitePoolOptions::new()
						.max_connections(1)
						.connect(&db_url)
						.await
						.map_err(|e| {
							CommandError::ExecutionError(format!("Connection error: {}", e))
						})?;

					let introspector =
						reinhardt_db::migrations::introspection::SQLiteIntrospector::new(pool);
					introspector.read_schema().await.map_err(|e| {
						CommandError::ExecutionError(format!("Introspection error: {}", e))
					})?
				}
				#[cfg(not(feature = "sqlite"))]
				{
					return Err(CommandError::ExecutionError(
						"SQLite support not enabled. Enable 'sqlite' feature.".to_string(),
					));
				}
			}
		};

		ctx.info(&format!("Found {} tables", schema.tables.len()));

		if schema.tables.is_empty() {
			ctx.warning("No tables found in database");
			return Ok(());
		}

		// Generate code
		ctx.info("Generating models...");
		let output = generate_models(&config, &schema)
			.map_err(|e| CommandError::ExecutionError(format!("Generation error: {}", e)))?;

		if output.files.is_empty() {
			ctx.warning("No models generated (tables may be filtered out)");
			return Ok(());
		}

		ctx.info(&format!("Generated {} files", output.files.len()));

		// Show or write output
		if is_dry_run {
			ctx.warning("Dry run mode: showing generated code");
			let preview = preview_output(&output);
			println!("{}", preview);
		} else {
			write_output(&output, is_force)
				.map_err(|e| CommandError::ExecutionError(format!("Write error: {}", e)))?;

			for file in &output.files {
				ctx.success(&format!("  Created: {:?}", file.path));
			}
		}

		ctx.success("✓ Introspection complete");
		Ok(())
	}
}

#[cfg(not(feature = "migrations"))]
#[async_trait]
impl BaseCommand for IntrospectCommand {
	fn name(&self) -> &str {
		"introspect"
	}

	fn description(&self) -> &str {
		"Generate Reinhardt ORM models from existing database schema"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		ctx.warning("Migrations feature is not enabled");
		ctx.info("To use introspect, enable the 'migrations' feature");
		Err(crate::CommandError::ExecutionError(
			"introspect command requires 'migrations' feature to be enabled".to_string(),
		))
	}
}

/// Mask password in database URL for display
#[cfg(feature = "migrations")]
fn mask_db_password(url: &str) -> String {
	if let Some(at_pos) = url.find('@')
		&& let Some(colon_pos) = url[..at_pos].rfind(':')
		&& let Some(slash_pos) = url[..colon_pos].rfind('/')
		&& let Some(user_end) = url[slash_pos + 1..].find(':').map(|p| slash_pos + 1 + p)
	{
		let prefix = &url[..slash_pos + 1];
		let user = &url[slash_pos + 1..user_end];
		let suffix = &url[at_pos..];
		return format!("{}{}:****{}", prefix, user, suffix);
	}
	url.to_string()
}

/// Detect database type from URL
#[cfg(feature = "migrations")]
fn detect_database_type(url: &str) -> Result<DatabaseType, crate::CommandError> {
	if url.starts_with("postgres://") || url.starts_with("postgresql://") {
		Ok(DatabaseType::Postgres)
	} else if url.starts_with("mysql://") || url.starts_with("mariadb://") {
		Ok(DatabaseType::Mysql)
	} else if url.starts_with("sqlite://") || url.starts_with("sqlite:") {
		Ok(DatabaseType::Sqlite)
	} else {
		Err(crate::CommandError::ExecutionError(format!(
			"Unknown database type in URL: {}",
			url
		)))
	}
}

// Additional command metadata and execution tests
#[cfg(test)]
mod tests {
	use super::*;

	#[cfg(feature = "reinhardt-db")]
	struct EnvVarGuard {
		key: &'static str,
		original: Option<std::ffi::OsString>,
	}

	#[cfg(feature = "reinhardt-db")]
	impl EnvVarGuard {
		fn capture(key: &'static str) -> Self {
			Self {
				key,
				original: std::env::var_os(key),
			}
		}
	}

	#[cfg(feature = "reinhardt-db")]
	impl Drop for EnvVarGuard {
		fn drop(&mut self) {
			// SAFETY: tests that mutate process environment are serial-protected.
			unsafe {
				match &self.original {
					Some(value) => std::env::set_var(self.key, value),
					None => std::env::remove_var(self.key),
				}
			}
		}
	}

	#[cfg(feature = "reinhardt-db")]
	struct CurrentDirGuard {
		original: std::path::PathBuf,
	}

	#[cfg(feature = "reinhardt-db")]
	impl CurrentDirGuard {
		fn enter(path: &std::path::Path) -> Self {
			let original = std::env::current_dir().expect("read current directory");
			std::env::set_current_dir(path).expect("set current directory");
			Self { original }
		}
	}

	#[cfg(feature = "reinhardt-db")]
	impl Drop for CurrentDirGuard {
		fn drop(&mut self) {
			let _ = std::env::set_current_dir(&self.original);
		}
	}

	#[test]
	#[cfg(feature = "autoreload")]
	fn runserver_execution_options_preserve_all_context_values() {
		// Arrange
		let mut ctx = CommandContext::new(vec!["127.0.0.1:9123".to_string()]);
		for flag in [
			"noreload",
			"no-wasm-rebuild",
			"insecure",
			"no_docs",
			"with-pages",
			"no-spa",
			"no-project-static",
			"no-wasm",
			"no-override-wasm",
			"force-wasm",
			"wasm-optional",
		] {
			ctx.set_option(flag.to_string(), "true".to_string());
		}
		ctx.set_option("watch-delay".to_string(), "275".to_string());
		ctx.set_option("static-dir".to_string(), "web-dist".to_string());
		ctx.set_option("index".to_string(), "shell.html".to_string());

		// Act
		let options = RunServerExecutionOptions::from_context(&ctx);

		// Assert
		assert_eq!(options.address, "127.0.0.1:9123");
		assert_eq!(options.watch_delay, std::time::Duration::from_millis(275));
		assert_eq!(options.static_dir, "web-dist");
		assert_eq!(options.index.as_deref(), Some("shell.html"));
		assert!(options.noreload && options.with_pages && options.wasm_optional);
		assert!(
			options.no_wasm_rebuild
				&& options.insecure
				&& options.no_docs
				&& options.no_spa
				&& options.no_project_static
				&& options.no_wasm
				&& options.no_override_wasm
				&& options.force_wasm_legacy
		);
	}

	#[test]
	#[cfg(feature = "autoreload")]
	fn runserver_execution_options_use_debounce_window_for_invalid_watch_delay() {
		// Arrange
		let mut ctx = CommandContext::default();
		ctx.set_option("watch-delay".to_string(), "not-a-duration".to_string());

		// Act
		let options = RunServerExecutionOptions::from_context(&ctx);

		// Assert
		assert_eq!(
			options.watch_delay,
			crate::debounced_watcher::DEBOUNCE_WINDOW
		);
	}

	#[test]
	#[cfg(feature = "migrations")]
	fn test_dependency_ordered_migrations_sorts_cross_app_plan() {
		// Arrange
		let users = reinhardt_db::migrations::Migration::new("0001_initial", "users");
		let profiles = reinhardt_db::migrations::Migration::new("0001_initial", "profiles")
			.add_dependency("users", "0001_initial");
		let articles = reinhardt_db::migrations::Migration::new("0001_initial", "articles")
			.add_dependency("profiles", "0001_initial");
		let unordered = vec![profiles, users, articles];

		// Act
		let ordered = dependency_ordered_migrations(unordered.iter()).expect("sort migration plan");

		// Assert
		let ids: Vec<_> = ordered.iter().map(|migration| migration.id()).collect();
		assert_eq!(
			ids,
			vec![
				"users.0001_initial",
				"profiles.0001_initial",
				"articles.0001_initial"
			]
		);
	}

	#[test]
	#[cfg(feature = "migrations")]
	fn dependency_ordered_migrations_rejects_a_cycle_with_the_exact_error() {
		// Arrange
		let cycle = reinhardt_db::migrations::Migration::new("0001_initial", "cycle")
			.add_dependency("cycle", "0001_initial");

		// Act
		let error = dependency_ordered_migrations([&cycle])
			.expect_err("a self-referential migration must not have an execution order");

		// Assert
		assert!(matches!(error, crate::CommandError::ExecutionError(_)));
		assert_eq!(
			error.to_string(),
			"Execution error: Failed to sort migration plan by dependencies: Circular dependency detected: Circular dependency detected: cycle.0001_initial"
		);
	}

	#[test]
	#[cfg(feature = "migrations")]
	fn dependency_ordered_migrations_orders_branched_graph_deterministically() {
		// Arrange
		let base = reinhardt_db::migrations::Migration::new("0001_initial", "accounts");
		let audit = reinhardt_db::migrations::Migration::new("0001_initial", "audit")
			.add_dependency("accounts", "0001_initial");
		let profiles = reinhardt_db::migrations::Migration::new("0001_initial", "profiles")
			.add_dependency("accounts", "0001_initial");
		let unordered = vec![profiles, base, audit];

		// Act
		let ordered = dependency_ordered_migrations(unordered.iter())
			.expect("a branched acyclic graph must have an execution order");

		// Assert
		let ids: Vec<_> = ordered.iter().map(|migration| migration.id()).collect();
		assert_eq!(
			ids,
			vec![
				"accounts.0001_initial",
				"audit.0001_initial",
				"profiles.0001_initial",
			]
		);
	}

	#[test]
	#[cfg(feature = "migrations")]
	fn dependency_ordered_migrations_preserves_pending_subset_with_applied_dependency() {
		// Arrange: migrations passed to this helper are the pending subset, so
		// dependencies absent from that subset have already been applied.
		let pending = reinhardt_db::migrations::Migration::new("0002_profile", "accounts")
			.add_dependency("accounts", "0001_initial");

		// Act
		let ordered = dependency_ordered_migrations([&pending])
			.expect("an already-applied dependency must not block the pending plan");

		// Assert
		let ids: Vec<_> = ordered.iter().map(|migration| migration.id()).collect();
		assert_eq!(ids, vec!["accounts.0002_profile"]);
	}

	#[rstest::rstest]
	#[cfg(feature = "migrations")]
	#[case("postgres://user:secret@host/db", "postgres://user:****@host/db")]
	#[case("postgresql://user:secret@host/db", "postgresql://user:****@host/db")]
	#[case("mysql://user:secret@host/db", "mysql://user:****@host/db")]
	#[case("sqlite:///tmp/app.db", "sqlite:///tmp/app.db")]
	fn mask_db_password_redacts_only_passwords(#[case] url: &str, #[case] expected: &str) {
		// Act
		let masked = mask_db_password(url);

		// Assert
		assert_eq!(masked, expected);
	}

	#[rstest::rstest]
	#[cfg(feature = "migrations")]
	#[case("postgres://user:secret@host/db", DatabaseType::Postgres)]
	#[case("postgresql://user:secret@host/db", DatabaseType::Postgres)]
	#[case("mysql://user:secret@host/db", DatabaseType::Mysql)]
	#[case("sqlite:///tmp/app.db", DatabaseType::Sqlite)]
	fn detect_database_type_recognizes_supported_schemes(
		#[case] url: &str,
		#[case] expected: DatabaseType,
	) {
		// Act
		let database_type = detect_database_type(url).expect("supported scheme must be recognized");

		// Assert
		assert_eq!(database_type, expected);
	}

	#[test]
	#[cfg(feature = "migrations")]
	fn detect_database_type_rejects_unknown_scheme_with_exact_error() {
		// Act
		let error = detect_database_type("invalid://value")
			.expect_err("unknown scheme must not select a database backend");

		// Assert
		assert!(matches!(error, crate::CommandError::ExecutionError(_)));
		assert_eq!(
			error.to_string(),
			"Execution error: Unknown database type in URL: invalid://value"
		);
	}

	#[test]
	#[cfg(feature = "reinhardt-db")]
	#[serial_test::serial(builtin_env)]
	fn test_check_resolves_database_url_from_settings_files() {
		// Arrange
		let _database_url = EnvVarGuard::capture("DATABASE_URL");
		let _reinhardt_env = EnvVarGuard::capture("REINHARDT_ENV");
		// SAFETY: env mutation in this test is protected by #[serial(builtin_env)].
		unsafe {
			std::env::remove_var("DATABASE_URL");
			std::env::set_var("REINHARDT_ENV", "local");
		}

		let temp_dir = tempfile::TempDir::new().expect("create temp dir");
		let settings_dir = temp_dir.path().join("settings");
		std::fs::create_dir_all(&settings_dir).expect("create settings dir");
		std::fs::write(
			settings_dir.join("base.toml"),
			r#"
[core]
secret_key = "test-secret"

[core.databases.default]
engine = "sqlite"
name = "db.sqlite3"
"#,
		)
		.expect("write base settings");
		std::fs::write(settings_dir.join("local.toml"), "").expect("write local settings");
		let _cwd = CurrentDirGuard::enter(temp_dir.path());

		// Act
		let resolved = CheckCommand::resolve_database_url(&CommandContext::default())
			.expect("database URL should resolve from settings files");

		// Assert
		assert_eq!(resolved, "sqlite:db.sqlite3");
		assert_eq!(
			std::env::var("DATABASE_URL").expect("DATABASE_URL should be synced"),
			resolved
		);
	}

	#[tokio::test]
	#[cfg(feature = "routers")]
	async fn test_showurls_command() {
		let cmd = ShowUrlsCommand;
		let ctx = CommandContext::default();

		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	#[serial_test::serial(builtin_env)]
	#[cfg(feature = "migrations")]
	async fn test_makemigrations_command() {
		use reinhardt_db::migrations::model_registry::{
			FieldMetadata, ModelMetadata, global_registry,
		};
		use reinhardt_db::prelude::FieldType;
		use tempfile::TempDir;

		// Create a temporary project directory with the required structure
		let project_dir = TempDir::new().unwrap();
		std::fs::create_dir_all(project_dir.path().join("src/bin")).unwrap();
		std::fs::write(project_dir.path().join("src/bin/manage.rs"), "fn main() {}").unwrap();
		let _database_url = EnvVarGuard::capture("DATABASE_URL");
		let _cwd = CurrentDirGuard::enter(project_dir.path());

		// Create a temporary directory for migrations
		let temp_dir = TempDir::new().unwrap();
		let migrations_dir = temp_dir.path();
		std::fs::create_dir_all(migrations_dir).unwrap();

		// Register a test model
		let registry = global_registry();
		let mut metadata = ModelMetadata::new("testapp", "TestModel", "testapp_testmodel");
		metadata.add_field(
			"id".to_string(),
			FieldMetadata::new(FieldType::Integer).with_param("primary_key", "true"),
		);
		metadata.add_field(
			"name".to_string(),
			FieldMetadata::new(FieldType::VarChar(100)).with_param("max_length", "100"),
		);
		registry.register_model(metadata);

		// SAFETY: env mutation in this test is protected by #[serial(builtin_env)].
		unsafe { std::env::set_var("DATABASE_URL", "sqlite::memory:") };

		let cmd = MakeMigrationsCommand;
		let mut ctx = CommandContext::default();
		ctx.add_arg("testapp".to_string());
		ctx.set_option(
			"migrations-dir".to_string(),
			migrations_dir.to_string_lossy().to_string(),
		);
		ctx.set_option("empty".to_string(), "true".to_string());

		let result = cmd.execute(&ctx).await;

		// Should succeed (creates an empty migration)
		assert!(result.is_ok(), "Failed with: {:?}", result.err());
	}

	#[tokio::test]
	#[serial_test::serial(builtin_env)]
	#[cfg(feature = "migrations")]
	async fn test_makemigrations_with_dry_run() {
		use reinhardt_db::{
			migrations::model_registry::{FieldMetadata, ModelMetadata, global_registry},
			prelude::FieldType,
		};
		use tempfile::TempDir;

		// Create a temporary project directory with the required structure
		let project_dir = TempDir::new().unwrap();
		std::fs::create_dir_all(project_dir.path().join("src/bin")).unwrap();
		std::fs::write(project_dir.path().join("src/bin/manage.rs"), "fn main() {}").unwrap();
		let _database_url = EnvVarGuard::capture("DATABASE_URL");
		let _cwd = CurrentDirGuard::enter(project_dir.path());

		// Create a temporary directory for migrations
		let temp_dir = TempDir::new().unwrap();
		let migrations_dir = temp_dir.path();
		std::fs::create_dir_all(migrations_dir).unwrap();

		// Register a test model
		let registry = global_registry();
		let mut metadata = ModelMetadata::new("testapp2", "TestModel2", "testapp2_testmodel");
		metadata.add_field(
			"id".to_string(),
			FieldMetadata::new(FieldType::Integer).with_param("primary_key", "true"),
		);
		metadata.add_field(
			"email".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("max_length", "255"),
		);
		registry.register_model(metadata);

		// SAFETY: env mutation in this test is protected by #[serial(builtin_env)].
		unsafe { std::env::set_var("DATABASE_URL", "sqlite::memory:") };

		let cmd = MakeMigrationsCommand;
		let mut ctx = CommandContext::default();
		ctx.add_arg("testapp2".to_string());
		ctx.set_option(
			"migrations-dir".to_string(),
			migrations_dir.to_string_lossy().to_string(),
		);
		ctx.set_option("dry-run".to_string(), "true".to_string());
		ctx.set_option("empty".to_string(), "true".to_string());

		let result = cmd.execute(&ctx).await;

		// Should succeed (dry-run mode, no actual files created)
		assert!(result.is_ok(), "Failed with: {:?}", result.err());
	}

	#[tokio::test]
	#[serial_test::serial(runserver)]
	async fn test_runserver_command() {
		// Test without server feature - should show warnings
		#[cfg(not(feature = "server"))]
		{
			let cmd = RunServerCommand;
			let ctx = CommandContext::default();
			let result = cmd.execute(&ctx).await;
			assert!(result.is_ok());
		}

		// Test with server feature - spawn server with timeout
		// Server blocks indefinitely, so timeout is expected
		#[cfg(feature = "server")]
		{
			use reinhardt_urls::routers::ServerRouter;

			// Register a dummy router for the test
			let router = ServerRouter::new();
			reinhardt_urls::routers::register_router(router);

			// Create context with noreload option to disable autoreload
			let mut ctx = CommandContext::default();
			ctx.set_option("noreload".to_string(), "true".to_string());

			// Spawn server in background task
			let server_task = tokio::spawn(async move {
				let cmd = RunServerCommand;
				cmd.execute(&ctx).await
			});

			// Abort the server task (server blocks, so we need to abort)
			server_task.abort();

			// Wait for task to be aborted
			let result = server_task.await;

			// Cleanup: clear the registered router
			reinhardt_urls::routers::clear_router();

			// Task should have been cancelled
			assert!(result.is_err(), "Server task should have been cancelled");
		}
	}

	// Confusable-API guard: explicit invocation of
	// `register_http_routes_from_inventory` after a manual
	// `register_router(..)` must be rejected with a clear message
	// (Refs #4453 DP-7).
	#[cfg(all(feature = "routers", feature = "server"))]
	#[tokio::test]
	#[serial_test::serial(runserver)]
	async fn test_register_http_routes_from_inventory_rejects_pre_registered_router() {
		use reinhardt_urls::routers::ServerRouter;

		// RAII guard so the manually registered router is cleared even if
		// an assertion below panics. Keeps the `runserver` serial group
		// hygienic without ordering the cleanup against `expect_err`.
		struct RouterCleanupGuard;
		impl Drop for RouterCleanupGuard {
			fn drop(&mut self) {
				reinhardt_urls::routers::clear_router();
			}
		}

		// Arrange: a router is already registered via the manual setter
		// (simulating an opt-out user who hand-built a ServerRouter and
		// called `register_router(..)` themselves before invoking
		// `RunServerCommand`).
		reinhardt_urls::routers::register_router(ServerRouter::new());
		let _cleanup_guard = RouterCleanupGuard;

		// Act: explicit call to the inventory consumer must error.
		let result = RunServerCommand.register_http_routes_from_inventory().await;

		// Assert: error mentions the mutual-exclusion contract so the
		// caller can fix the bootstrap rather than silently overriding.
		let err = result
			.expect_err("expected DP-7 confusable-API rejection when a router is pre-registered");
		let msg = err.to_string();
		assert!(
			msg.contains("already registered"),
			"error must call out the pre-registered router; got: {msg}"
		);
		assert!(
			msg.contains("mutually exclusive"),
			"error must state the two paths are mutually exclusive; got: {msg}"
		);
	}

	// ==================== Command Metadata Tests ====================

	#[test]
	fn test_shell_command_metadata() {
		let cmd = ShellCommand;
		assert_eq!(cmd.name(), "shell");
		assert_eq!(cmd.description(), "Start an interactive Rust REPL");

		let options = cmd.options();
		assert_eq!(options.len(), 1);
		// Only option: -c/--command
		assert_eq!(options[0].short, Some('c'));
		assert_eq!(options[0].long, "command");
	}

	#[test]
	fn test_checkdi_command_metadata() {
		let cmd = CheckDiCommand;
		assert_eq!(cmd.name(), "check-di");
		assert_eq!(
			cmd.description(),
			"Check DI dependency graph for circular dependencies and other issues"
		);

		let arguments = cmd.arguments();
		assert!(arguments.is_empty());

		let options = cmd.options();
		assert!(options.is_empty());
	}

	#[test]
	fn test_migrate_command_metadata() {
		let cmd = MigrateCommand;
		assert_eq!(cmd.name(), "migrate");
		assert_eq!(cmd.description(), "Run database migrations");

		let arguments = cmd.arguments();
		assert_eq!(arguments.len(), 2);
		assert_eq!(arguments[0].name, "app");
		assert_eq!(arguments[1].name, "migration");

		let options = cmd.options();
		// Should have migration-related options
		assert!(!options.is_empty());
	}

	#[test]
	fn test_runserver_command_options_include_no_override_wasm() {
		// Arrange
		let cmd = RunServerCommand;

		// Act
		let options = cmd.options();
		let option_names: Vec<&str> = options.iter().map(|o| o.long.as_str()).collect();

		// Assert: --no-override-wasm replaces --force-wasm as the supported opt-out;
		// --force-wasm is retained as a deprecated alias.
		assert!(
			option_names.contains(&"no-override-wasm"),
			"--no-override-wasm must be registered as a runserver option"
		);
		assert!(
			option_names.contains(&"force-wasm"),
			"--force-wasm must remain registered (deprecated alias)"
		);
		assert!(option_names.contains(&"no-wasm"));
		let watch_delay = options
			.iter()
			.find(|option| option.long == "watch-delay")
			.expect("--watch-delay must be registered as a runserver option");
		assert_eq!(
			watch_delay.default.as_deref(),
			Some("120"),
			"--watch-delay default must match the hot-reload debounce default"
		);
	}

	#[test]
	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn test_autoreload_child_args_forward_wasm_startup_flags() {
		let args = RunServerCommand::build_autoreload_child_args(&AutoreloadChildOptions {
			address: "127.0.0.1:8000",
			insecure: true,
			no_docs: true,
			with_pages: true,
			static_dir: "dist",
			no_spa: true,
			no_project_static: true,
			index: Some("index.html"),
			hmr_port: Some(35729),
			no_wasm: true,
			no_override_wasm: true,
			force_wasm: true,
			wasm_optional: true,
		});

		assert_eq!(
			args,
			vec![
				"runserver",
				"127.0.0.1:8000",
				"--noreload",
				"--insecure",
				"--no-docs",
				"--with-pages",
				"--static-dir",
				"dist",
				"--no-spa",
				"--no-project-static",
				"--index",
				"index.html",
				"--no-wasm",
				"--no-override-wasm",
				"--force-wasm",
				"--wasm-optional",
			]
		);
	}

	#[test]
	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn test_autoreload_child_args_omit_disabled_wasm_startup_flags() {
		let args = RunServerCommand::build_autoreload_child_args(&AutoreloadChildOptions {
			address: "127.0.0.1:8000",
			insecure: false,
			no_docs: false,
			with_pages: false,
			static_dir: "",
			no_spa: false,
			no_project_static: false,
			index: None,
			hmr_port: None,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
		});

		assert_eq!(args, vec!["runserver", "127.0.0.1:8000", "--noreload"]);
	}

	#[test]
	#[cfg(feature = "migrations")]
	fn test_makemigrations_command_metadata() {
		let cmd = MakeMigrationsCommand;
		assert_eq!(cmd.name(), "makemigrations");
		assert_eq!(
			cmd.description(),
			"Create new migrations based on model changes"
		);

		let arguments = cmd.arguments();
		assert_eq!(arguments.len(), 1);
		assert_eq!(arguments[0].name, "app");

		let options = cmd.options();
		let option_names: Vec<&str> = options.iter().map(|o| o.long.as_str()).collect();
		assert!(option_names.contains(&"dry-run"));
		assert!(option_names.contains(&"empty"));
	}

	#[tokio::test]
	#[cfg(not(feature = "di"))]
	async fn test_checkdi_command_execution() {
		// Arrange
		let cmd = CheckDiCommand;
		let ctx = CommandContext::default();

		// Act
		let error = cmd
			.execute(&ctx)
			.await
			.expect_err("the command must require the disabled DI feature");

		// Assert
		assert_eq!(
			error.to_string(),
			"Execution error: check-di command requires 'di' feature to be enabled"
		);
	}

	#[tokio::test]
	async fn test_shell_command_with_command_option() {
		let cmd = ShellCommand;
		let mut ctx = CommandContext::default();
		ctx.set_option("command".to_string(), "let x = 1 + 2".to_string());

		// Execute with a simple command
		let result = cmd.execute(&ctx).await;

		// Should succeed (command is processed and returned)
		assert!(result.is_ok());
	}

	#[cfg(feature = "reinhardt-db")]
	mod database_config_unification_tests {
		use super::*;
		use rstest::rstest;
		use serial_test::serial;

		#[rstest]
		fn test_sanitize_database_url_redacts_credentials() {
			// Arrange
			let url = "postgresql://user:secret@localhost:5432/mydb";

			// Act
			let sanitized = sanitize_database_url(url);

			// Assert
			assert_eq!(sanitized, "postgresql://***@localhost:5432/mydb");
		}

		#[rstest]
		fn test_sanitize_database_url_redacts_sqlite_path() {
			// Arrange
			let url = "sqlite:db.sqlite3";

			// Act
			let sanitized = sanitize_database_url(url);

			// Assert
			assert_eq!(sanitized, "sqlite:***");
		}

		#[rstest]
		fn test_sanitize_database_url_redacts_absolute_sqlite_path() {
			// Arrange
			let url = "sqlite:///tmp/secret.sqlite3";

			// Act
			let sanitized = sanitize_database_url(url);

			// Assert
			assert_eq!(sanitized, "sqlite:***");
		}

		#[rstest]
		fn test_sanitize_database_url_memory_sqlite() {
			// Arrange
			let url = "sqlite::memory:";

			// Act
			let sanitized = sanitize_database_url(url);

			// Assert
			assert_eq!(sanitized, "sqlite::memory:");
		}

		#[rstest]
		#[serial(builtin_env)]
		fn test_get_database_url_from_settings_with_toml() {
			// Arrange
			let _database_url = EnvVarGuard::capture("DATABASE_URL");
			let _reinhardt_env = EnvVarGuard::capture("REINHARDT_ENV");
			let temp_dir = tempfile::TempDir::new().unwrap();
			let settings_dir = temp_dir.path().join("settings");
			std::fs::create_dir_all(&settings_dir).unwrap();

			// Create a base.toml with database config
			let toml_content = r#"
[database]
engine = "postgresql"
name = "testdb"
user = "testuser"
password = "testpass"
host = "localhost"
port = 5432
"#;
			std::fs::write(settings_dir.join("base.toml"), toml_content).unwrap();
			std::fs::write(settings_dir.join("local.toml"), "").unwrap();

			// Remove DATABASE_URL to ensure settings-only resolution
			unsafe { std::env::remove_var("DATABASE_URL") };
			unsafe { std::env::set_var("REINHARDT_ENV", "local") };
			let _cwd = CurrentDirGuard::enter(temp_dir.path());

			// Act
			let result = get_database_url_from_settings();

			// Assert
			assert!(
				result.is_ok(),
				"get_database_url_from_settings failed: {:?}",
				result.err()
			);
			let url = result.unwrap();
			assert_eq!(url, "postgresql://testuser:testpass@localhost:5432/testdb");
		}

		#[rstest]
		#[serial(builtin_env)]
		fn test_get_database_url_from_settings_with_core_databases_default() {
			// Arrange: the canonical nested schema (`[core.databases.default]`)
			// rather than the legacy flat top-level `[database]` block (#5042).
			let _database_url = EnvVarGuard::capture("DATABASE_URL");
			let _reinhardt_env = EnvVarGuard::capture("REINHARDT_ENV");
			let temp_dir = tempfile::TempDir::new().unwrap();
			let settings_dir = temp_dir.path().join("settings");
			std::fs::create_dir_all(&settings_dir).unwrap();

			let toml_content = r#"
[core]
secret_key = "test-secret"

[core.databases.default]
engine = "postgresql"
name = "nesteddb"
user = "testuser"
password = "testpass"
host = "localhost"
port = 5432
"#;
			std::fs::write(settings_dir.join("base.toml"), toml_content).unwrap();
			std::fs::write(settings_dir.join("local.toml"), "").unwrap();

			// Remove DATABASE_URL to ensure settings-only resolution
			unsafe { std::env::remove_var("DATABASE_URL") };
			unsafe { std::env::set_var("REINHARDT_ENV", "local") };
			let _cwd = CurrentDirGuard::enter(temp_dir.path());

			// Act
			let result = get_database_url_from_settings();

			// Assert
			assert!(
				result.is_ok(),
				"get_database_url_from_settings failed for nested schema: {:?}",
				result.err()
			);
			let url = result.unwrap();
			assert_eq!(
				url,
				"postgresql://testuser:testpass@localhost:5432/nesteddb"
			);
		}

		#[rstest]
		#[serial(builtin_env)]
		fn test_get_database_url_from_settings_returns_error_without_config() {
			// Arrange
			let _database_url = EnvVarGuard::capture("DATABASE_URL");
			let _reinhardt_env = EnvVarGuard::capture("REINHARDT_ENV");
			let temp_dir = tempfile::TempDir::new().unwrap();
			let settings_dir = temp_dir.path().join("settings");
			std::fs::create_dir_all(&settings_dir).unwrap();

			// Create empty settings files (no database config)
			std::fs::write(settings_dir.join("base.toml"), "").unwrap();
			std::fs::write(settings_dir.join("local.toml"), "").unwrap();

			unsafe { std::env::remove_var("DATABASE_URL") };
			unsafe { std::env::set_var("REINHARDT_ENV", "local") };
			let _cwd = CurrentDirGuard::enter(temp_dir.path());

			// Act
			let result = get_database_url_from_settings();

			// Assert
			let error = result.expect_err("missing database settings must fail");
			assert_eq!(
				error.to_string(),
				"Execution error: No database configuration found in settings files"
			);
		}

		#[rstest]
		#[serial(builtin_env)]
		fn test_get_database_url_from_settings_rejects_invalid_toml() {
			// Arrange
			let _database_url = EnvVarGuard::capture("DATABASE_URL");
			let _reinhardt_env = EnvVarGuard::capture("REINHARDT_ENV");
			let temp_dir = tempfile::TempDir::new().expect("create temporary project directory");
			let settings_dir = temp_dir.path().join("settings");
			std::fs::create_dir_all(&settings_dir).expect("create settings directory");
			std::fs::write(settings_dir.join("base.toml"), "[database\n")
				.expect("write invalid settings");
			std::fs::write(settings_dir.join("local.toml"), "").expect("write local settings");
			unsafe {
				std::env::remove_var("DATABASE_URL");
				std::env::set_var("REINHARDT_ENV", "local");
			}
			let _cwd = CurrentDirGuard::enter(temp_dir.path());

			// Act
			let error = get_database_url_from_settings()
				.expect_err("invalid settings TOML must not be treated as missing settings");

			// Assert
			assert!(matches!(error, crate::CommandError::ExecutionError(_)));
			assert!(
				error
					.to_string()
					.starts_with("Execution error: Failed to load settings:"),
				"invalid TOML must preserve the settings-load error: {error}"
			);
		}

		#[rstest]
		#[serial(builtin_env)]
		fn test_sync_database_url_to_env_sets_env_when_not_present() {
			// Arrange: DATABASE_URL not set
			let _database_url = EnvVarGuard::capture("DATABASE_URL");
			unsafe { std::env::remove_var("DATABASE_URL") };

			let resolved_url = "postgresql://user:pass@localhost:5432/testdb";
			let ctx = CommandContext::default();

			// Act
			sync_database_url_to_env(None, resolved_url, &ctx);

			// Assert: DATABASE_URL is now set to resolved_url
			let result = std::env::var("DATABASE_URL");
			assert!(result.is_ok(), "DATABASE_URL should be set after sync");
			assert_eq!(result.unwrap(), resolved_url);
		}

		#[rstest]
		#[serial(builtin_env)]
		fn test_sync_database_url_to_env_does_not_override_existing_env_var() {
			// Arrange: DATABASE_URL already set - env var takes precedence
			let _database_url = EnvVarGuard::capture("DATABASE_URL");
			let existing_env_url = "postgresql://envuser:envpass@envhost:5433/envdb";
			unsafe { std::env::set_var("DATABASE_URL", existing_env_url) };

			let resolved_url = "postgresql://settingsuser:settingspass@localhost:5432/settingsdb";
			let ctx = CommandContext::default();

			// Act
			sync_database_url_to_env(Some(existing_env_url), resolved_url, &ctx);

			// Assert: DATABASE_URL remains unchanged (env var takes precedence)
			let result = std::env::var("DATABASE_URL");
			assert!(result.is_ok());
			assert_eq!(
				result.unwrap(),
				existing_env_url,
				"DATABASE_URL should not be changed when env var is already set"
			);
		}
	}

	// `is_relevant_change` unit tests now live in
	// `crate::debounced_watcher::tests` after the watcher refactor.

	// End-to-end regression coverage for issue #4247.
	//
	// `get_database_url_from_settings` opts into
	// `TomlFileSource::with_interpolation()` (PR #4239). The interpolation
	// tests in `reinhardt-conf/tests/interpolation.rs` build a fresh
	// `TomlFileSource`, so they will keep passing even if a future
	// refactor accidentally drops the opt-in here. These tests exercise
	// the real loader so that regression fails loudly.
	//
	// The loader reads cwd via `env::current_dir()`. Tests change cwd
	// under `#[serial(builtin_env)]` and restore it via the shared drop guard.
	#[cfg(feature = "reinhardt-db")]
	mod interpolation_4247 {
		use super::super::get_database_url_from_settings;
		use super::{CurrentDirGuard, EnvVarGuard};
		use rstest::rstest;
		use serial_test::serial;
		use std::env;
		use std::io::Write;
		use std::path::Path;
		use tempfile::TempDir;

		fn write_settings_dir(profile: &str, base_toml: &str) -> TempDir {
			let temp = TempDir::new().expect("create temp dir");
			let settings_dir = temp.path().join("settings");
			std::fs::create_dir_all(&settings_dir).expect("create settings dir");
			write_file(&settings_dir.join("base.toml"), base_toml);
			write_file(&settings_dir.join(format!("{profile}.toml")), "");
			temp
		}

		fn write_file(path: &Path, contents: &str) {
			let mut f = std::fs::File::create(path).expect("create file");
			f.write_all(contents.as_bytes()).expect("write file");
		}

		#[rstest]
		#[serial(builtin_env)]
		fn loader_expands_env_var_in_host() {
			// Arrange
			let _database_host = EnvVarGuard::capture("IT4247C_DB_HOST");
			let _reinhardt_env = EnvVarGuard::capture("REINHARDT_ENV");
			// SAFETY: env mutation in this test is protected by #[serial(builtin_env)].
			unsafe {
				env::set_var("REINHARDT_ENV", "local");
				env::set_var("IT4247C_DB_HOST", "production-db.example.com");
			}
			let temp = write_settings_dir(
				"local",
				r#"
[database]
engine = "postgresql"
name = "appdb"
user = "app"
password = "secret"
host = "${IT4247C_DB_HOST}"
port = 5432
"#,
			);
			let _cwd = CurrentDirGuard::enter(temp.path());

			// Act
			let url = get_database_url_from_settings().expect("loader returns a URL");

			// Assert
			assert!(
				url.contains("production-db.example.com"),
				"expected expanded host in URL, got: {url}"
			);
			assert!(
				!url.contains("${"),
				"URL still contains literal interpolation pattern: {url}"
			);
		}

		#[rstest]
		#[serial(builtin_env)]
		fn loader_uses_inline_default_when_var_unset() {
			// Arrange — declare the var in the guard even though we never set
			// it, so an ambient value cannot silence the inline `:-fallback`.
			let _database_host = EnvVarGuard::capture("IT4247C_DB_HOST_OPT");
			let _reinhardt_env = EnvVarGuard::capture("REINHARDT_ENV");
			// SAFETY: env mutation in this test is protected by #[serial(builtin_env)].
			unsafe {
				env::remove_var("IT4247C_DB_HOST_OPT");
				env::set_var("REINHARDT_ENV", "local");
			}
			let temp = write_settings_dir(
				"local",
				r#"
[database]
engine = "postgresql"
name = "appdb"
user = "app"
password = "secret"
host = "${IT4247C_DB_HOST_OPT:-fallback-host}"
port = 5432
"#,
			);
			let _cwd = CurrentDirGuard::enter(temp.path());

			// Act
			let url = get_database_url_from_settings().expect("loader returns a URL");

			// Assert
			assert!(
				url.contains("fallback-host"),
				"expected fallback host in URL, got: {url}"
			);
			assert!(
				!url.contains("${"),
				"URL still contains literal interpolation pattern: {url}"
			);
		}
	}

	// Issue #4250: the always-on spawn diagnostic must not leak absolute
	// filesystem paths or toolchain env values, while still preserving the
	// issue #4236 root-cause signals. Verify the redacted (debug=false)
	// allowlist and the full (debug=true) snapshot independently.
	#[cfg(all(feature = "server", feature = "autoreload"))]
	mod spawn_diagnostics_4250 {
		use super::*;
		use tempfile::TempDir;

		fn temp_exe() -> (TempDir, std::path::PathBuf) {
			let tmp = tempfile::tempdir().expect("create tempdir");
			let exe = tmp.path().join("manage_test_bin");
			std::fs::write(&exe, b"#!/bin/sh\nexit 0\n").expect("write fake exe");
			(tmp, exe)
		}

		#[test]
		fn redacted_omits_absolute_paths_and_keeps_filename() {
			// Arrange
			let (tmp, exe) = temp_exe();
			let parent = tmp.path().to_string_lossy().into_owned();

			// Act
			let out = RunServerCommand::spawn_diagnostics(&exe, false);

			// Assert: allowlist present.
			assert!(
				out.contains("exe_filename=manage_test_bin"),
				"redacted must include filename, got: {out}"
			);
			assert!(
				out.contains("exists=true"),
				"redacted missing exists: {out}"
			);
			assert!(out.contains("pid="), "redacted missing pid: {out}");

			// Assert: no absolute paths or path-bearing fields.
			assert!(
				!out.contains(&parent),
				"redacted leaks parent dir {parent}: {out}"
			);
			assert!(
				!out.contains("canonical="),
				"redacted leaks canonical path: {out}"
			);
			assert!(
				!out.contains("canonical_err="),
				"redacted leaks canonical_err message: {out}"
			);
			assert!(
				!out.contains("size="),
				"size dropped per #4250 allowlist: {out}"
			);
			assert!(
				!out.contains("mtime_unix="),
				"mtime dropped per #4250 allowlist: {out}"
			);
			assert!(
				!out.contains("CARGO_TARGET_DIR"),
				"redacted must not emit CARGO_TARGET_DIR in any form: {out}"
			);
			assert!(
				!out.contains("RUSTC_WRAPPER"),
				"redacted must not emit RUSTC_WRAPPER in any form: {out}"
			);

			// Assert: outer CommandError format owns raw_os_error/kind, so
			// spawn_diagnostics must not duplicate them.
			assert!(
				!out.contains("raw_os_error="),
				"redacted must not duplicate raw_os_error from outer format: {out}"
			);
		}

		#[test]
		#[cfg(target_os = "linux")]
		fn redacted_keeps_4236_deleted_suffix_signal() {
			// Arrange
			let (_tmp, exe) = temp_exe();

			// Act
			let out = RunServerCommand::spawn_diagnostics(&exe, false);

			// Assert: the load-bearing #4236 signal is preserved.
			assert!(
				out.contains("proc_self_exe_deleted_suffix="),
				"redacted missing #4236 deleted_suffix signal: {out}"
			);
			// The full /proc/self/exe path must be redacted out.
			assert!(
				!out.contains("proc_self_exe=/"),
				"redacted leaks /proc/self/exe path: {out}"
			);
		}

		#[test]
		#[cfg(unix)]
		fn redacted_includes_inode_nlink_when_metadata_ok() {
			// Arrange
			let (_tmp, exe) = temp_exe();

			// Act
			let out = RunServerCommand::spawn_diagnostics(&exe, false);

			// Assert
			assert!(
				out.contains("inode="),
				"redacted missing inode field: {out}"
			);
			assert!(
				out.contains("nlink="),
				"redacted missing nlink field: {out}"
			);
		}

		#[test]
		fn full_includes_absolute_paths_and_env_values() {
			// Arrange
			let (tmp, exe) = temp_exe();
			let parent = tmp.path().to_string_lossy().into_owned();

			// Act
			let out = RunServerCommand::spawn_diagnostics(&exe, true);

			// Assert: full mode intentionally leaks paths for debugging.
			assert!(
				out.contains(&parent),
				"full mode must keep parent path {parent}: {out}"
			);
			assert!(
				out.contains("canonical=") || out.contains("canonical_err="),
				"full mode must emit canonical: {out}"
			);
			// Full env-var values, not the redacted `_set` presence flag.
			assert!(
				out.contains("CARGO_TARGET_DIR="),
				"full mode must emit CARGO_TARGET_DIR value: {out}"
			);
			assert!(
				!out.contains("CARGO_TARGET_DIR_set="),
				"redacted-only field must not appear in full mode: {out}"
			);
			assert!(
				out.contains("REINHARDT_IS_AUTORELOAD_CHILD="),
				"full mode must emit REINHARDT_IS_AUTORELOAD_CHILD value: {out}"
			);
		}
	}
}
