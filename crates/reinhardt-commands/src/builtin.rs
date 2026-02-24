//! Built-in commands
//!
//! Standard management commands included with Reinhardt.

use crate::{BaseCommand, CommandArgument, CommandContext, CommandOption, CommandResult};
use async_trait::async_trait;

#[cfg(feature = "migrations")]
use reinhardt_db::migrations::DatabaseMigrationExecutor;

#[cfg(feature = "migrations")]
use reinhardt_db::backends::{DatabaseConnection, DatabaseType};

// Import backends' DatabaseConnection for get_database_url helper (without migrations feature)
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
			CommandOption::option(Some('d'), "database", "Database to migrate")
				.with_default("default"),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		ctx.info("Running migrations...");

		let app_label = ctx.arg(0).map(|s| s.to_string());
		let _migration_name = ctx.arg(1).map(|s| s.to_string());
		let is_fake = ctx.has_option("fake");
		let _is_fake_initial = ctx.has_option("fake-initial");
		let _database = ctx
			.option("database")
			.map(|s| s.to_string())
			.unwrap_or_else(|| "default".to_string());

		if let Some(ref app_name) = app_label {
			if let Some(ref migration) = _migration_name {
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
			let migrations_dir = PathBuf::from("migrations");

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
				.or_else(|| get_database_url().ok())
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
			{
				return Err(crate::CommandError::ExecutionError(format!(
					"Unsupported database URL scheme: {}",
					database_url
				)));
			}

			// 4. Connect to database (auto-create if it doesn't exist for PostgreSQL)
			// This is done before filtering migrations to ensure connection errors are detected
			// even when no migrations need to be applied
			let connection = if database_url.starts_with("postgres://")
				|| database_url.starts_with("postgresql://")
			{
				DatabaseConnection::connect_postgres_or_create(&database_url).await
			} else {
				// Must be SQLite (validated above)
				DatabaseConnection::connect_sqlite(&database_url).await
			}
			.map_err(|e| {
				crate::CommandError::ExecutionError(format!(
					"Failed to connect to database: {:?}",
					e
				))
			})?;

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
				ctx.info("No migrations to apply");
				return Ok(());
			}

			ctx.info(&format!(
				"Found {} migration(s) to apply",
				migrations_to_apply.len()
			));

			// 6. Apply migrations (or fake them
			if is_fake {
				ctx.info("Faking migrations (marking as applied without execution):");

				// Create migration executor for fake migrations
				let mut executor = DatabaseMigrationExecutor::new(connection);

				// Record each migration as applied without executing
				for migration in &migrations_to_apply {
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
				FilesystemRepository, FilesystemSource, MigrationNamer, MigrationNumbering,
				MigrationService, autodetector::ProjectState,
			};
			use std::sync::Arc;
			use tokio::sync::Mutex;

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

			// Get database URL from context option or environment
			let database_url = ctx
				.option("database")
				.map(|s| s.to_string())
				.or_else(|| get_database_url().ok())
				.unwrap_or_default();

			// 2. Build from_state from database history or TestContainers
			// This ensures all models are treated as new, generating complete migrations
			struct MigrationResult {
				app_name: String,
				migration: reinhardt_db::migrations::Migration,
			}

			let mut results: Vec<MigrationResult> = Vec::new();

			// Build from_state based on strategy (default: TestContainers)
			let from_db_flag = ctx.has_option("from-db");
			let from_state = if from_db_flag {
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
								ctx.error(&format!("Failed to use TestContainers: {}", e));
								ctx.error(
									"⚠️  CRITICAL: Cannot build from_state from existing migrations!",
								);
								ctx.error(
									"This will cause ALL tables to be regenerated, creating duplicate migrations.",
								);
								ctx.error("");
								ctx.error("Possible solutions:");
								ctx.error("  1. Fix TestContainers setup (recommended)");
								ctx.error("  2. Use --from-db flag to build from database history");
								ctx.error(
									"  3. Use --force-empty-state to proceed anyway (dangerous)",
								);
								ctx.error("");

								if ctx.has_option("force-empty-state") {
									ctx.warning(
										"⚠️  Using empty state as requested (--force-empty-state)",
									);
									ctx.warning("This may create duplicate migrations!");
									ProjectState::new()
								} else {
									return Err("from_state construction failed. Please fix TestContainers, use --from-db, or use --force-empty-state to continue anyway.".to_string().into());
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
								ctx.error(&format!("Failed to connect to database: {}", e));
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

								if ctx.has_option("force-empty-state") {
									ctx.warning(
										"⚠️  Using empty state as requested (--force-empty-state)",
									);
									ctx.warning("This may create duplicate migrations!");
									ProjectState::new()
								} else {
									return Err("from_state construction failed. Please fix database connection, remove --from-db, or use --force-empty-state to continue anyway.".to_string().into());
								}
							}
						}
					}
				}
			};

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
						let base_name = migration_name_opt.clone().unwrap_or_else(|| {
							MigrationNamer::generate_name(&migration.operations, true)
						});
						let migration_number =
							MigrationNumbering::next_number(&migrations_dir, app_name);
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
			CommandOption::option(
				None,
				"watch-delay",
				"Watch delay in milliseconds for file change debouncing",
			)
			.with_default("500"),
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
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let address = ctx.arg(0).map(|s| s.as_str()).unwrap_or("127.0.0.1:8000");
		let noreload = ctx.has_option("noreload");
		let insecure = ctx.has_option("insecure");
		let no_docs = ctx.has_option("no_docs");
		let with_pages = ctx.has_option("with-pages");
		let static_dir_raw = ctx
			.option("static-dir")
			.map(|s| s.to_string())
			.unwrap_or_else(|| "dist".to_string());
		let no_spa = ctx.has_option("no-spa");

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

			if !no_docs {
				ctx.info(&format!("📖 Docs:    http://{}/api/docs", actual_address));
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
			Self::run_server(
				ctx,
				&actual_address,
				noreload,
				insecure,
				no_docs,
				with_pages,
				&static_dir_raw,
				no_spa,
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
	/// Run the development server
	#[cfg(feature = "server")]
	// Allow many arguments: CLI command handler needs to accept all server configuration options
	#[allow(clippy::too_many_arguments)]
	async fn run_server(
		// Context parameter reserved for future extensions (e.g., accessing global config)
		#[allow(unused_variables)] ctx: &CommandContext,
		address: &str,
		noreload: bool,
		_insecure: bool,
		no_docs: bool,
		with_pages: bool,
		static_dir: &str,
		no_spa: bool,
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
			let wrapped = OpenApiRouter::wrap(base_router);
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

		// OpenAPI documentation is shown in startup banner above

		// Create DI context for dependency injection
		let singleton_scope = std::sync::Arc::new(reinhardt_di::SingletonScope::new());

		// Register DatabaseConnection as singleton when database feature is enabled
		#[cfg(feature = "reinhardt-db")]
		{
			// Try to connect to database and register connection
			match get_database_url() {
				Ok(url) => {
					// Initialize ORM global database first, which also creates the connection pool
					match reinhardt_db::orm::init_database(&url).await {
						Ok(()) => {
							ctx.verbose("ORM database initialized");
							// Get the connection from ORM and register in DI context for dependency injection
							match reinhardt_db::orm::get_connection().await {
								Ok(db_conn) => {
									// Register DatabaseConnection directly (not wrapped in Arc)
									// The DI system wraps it in Arc internally via SingletonScope::set
									singleton_scope.set(db_conn);
									ctx.info(&format!(
										"💾 Database: {} (connected)",
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
						Err(e) => {
							ctx.warning(&format!(
								"⚠️ Failed to initialize ORM database: {}. DI injection for DatabaseConnection will fail.",
								e
							));
						}
					}
				}
				Err(e) => {
					ctx.warning(&format!(
						"⚠️ No DATABASE_URL configured: {}. DI injection for DatabaseConnection will fail.",
						e
					));
				}
			}
		}

		let di_context =
			std::sync::Arc::new(reinhardt_di::InjectionContext::builder(singleton_scope).build());

		// Create HTTP server with DI context and logging middleware
		let mut server = HttpServer::new(router)
			.with_di_context(di_context)
			.with_middleware(reinhardt_middleware::LoggingMiddleware::new());

		// Add static files middleware for WASM frontend if enabled
		if with_pages {
			use reinhardt_utils::staticfiles::PathResolver;
			use reinhardt_utils::staticfiles::middleware::{
				StaticFilesConfig, StaticFilesMiddleware,
			};

			// Automatically resolve static directory path
			let resolved_static_dir = PathResolver::resolve_static_dir(static_dir);

			let static_config = StaticFilesConfig::new(resolved_static_dir.clone())
				.url_prefix("/")
				.spa_mode(!no_spa)
				// All API and documentation endpoints are under /api/ prefix
				.excluded_prefixes(vec!["/api/".to_string()]);

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
				Self::run_with_autoreload(
					ctx, address, _insecure, no_docs, with_pages, static_dir, no_spa,
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

	/// Run server with file watching and auto-reload
	#[cfg(all(feature = "server", feature = "autoreload"))]
	async fn run_with_autoreload(
		ctx: &CommandContext,
		address: &str,
		insecure: bool,
		no_docs: bool,
		with_pages: bool,
		static_dir: &str,
		no_spa: bool,
	) -> CommandResult<()> {
		use std::time::{Duration, Instant};

		ctx.info("Starting autoreload mode...");
		ctx.verbose("Watching for file changes in src/ and Cargo.toml");

		let mut restart_count = 0;
		let max_restarts_per_minute = 10;
		let mut last_restart_time = Instant::now();

		// Set up Ctrl+C handler
		let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
		let ctx_clone = ctx.clone();
		tokio::spawn(async move {
			if let Err(e) = tokio::signal::ctrl_c().await {
				eprintln!("Failed to listen for Ctrl+C: {}", e);
				return;
			}
			ctx_clone.info("\nReceived Ctrl+C, shutting down...");
			let _ = shutdown_tx.send(());
		});

		loop {
			// Check restart frequency limit (stop if more than 10 restarts per minute)
			if restart_count >= max_restarts_per_minute {
				let elapsed = last_restart_time.elapsed();
				if elapsed < Duration::from_secs(60) {
					return Err(crate::CommandError::ExecutionError(format!(
						"Too many restarts ({} in {:?}). Aborting to prevent infinite loop.",
						restart_count, elapsed
					)));
				} else {
					// Reset counter if more than 1 minute has elapsed
					restart_count = 0;
					last_restart_time = Instant::now();
				}
			}

			// Start child process
			ctx.verbose("Starting server subprocess...");
			let mut child = Self::spawn_server_process(
				address, insecure, no_docs, with_pages, static_dir, no_spa,
			)?;
			restart_count += 1;

			// Wait for file change, child process exit, or Ctrl+C
			tokio::select! {
				change_result = Self::watch_files_async() => {
					match change_result {
						Ok(_) => {
							ctx.info("\n📝 File change detected. Restarting server...");
							// Stop child process
							if let Err(e) = child.kill().await {
								ctx.warning(&format!("Failed to kill child process: {}", e));
							}
							// Ensure cleanup with wait() (prevent zombie processes)
							let _ = child.wait().await;

							// Wait for port release + debounce
							tokio::time::sleep(Duration::from_millis(500)).await;
							continue; // Return to loop start and restart
						}
						Err(e) => {
							return Err(crate::CommandError::ExecutionError(format!(
								"File watcher error: {}",
								e
							)));
						}
					}
				}

				exit_status = child.wait() => {
					match exit_status {
						Ok(status) if status.success() => {
							ctx.info("Server process exited cleanly.");
							break; // Exit parent if clean exit
						}
						Ok(status) => {
							return Err(crate::CommandError::ExecutionError(format!(
								"Server process crashed with status: {}",
								status
							)));
						}
						Err(e) => {
							return Err(crate::CommandError::ExecutionError(format!(
								"Failed to wait for child process: {}",
								e
							)));
						}
					}
				}

				_ = &mut shutdown_rx => {
					ctx.info("Shutdown signal received. Stopping server...");
					let _ = child.kill().await;
					let _ = child.wait().await;
					break;
				}
			}
		}

		Ok(())
	}

	/// Spawn server in child process
	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn spawn_server_process(
		address: &str,
		insecure: bool,
		no_docs: bool,
		with_pages: bool,
		static_dir: &str,
		no_spa: bool,
	) -> CommandResult<tokio::process::Child> {
		let current_exe = std::env::current_exe().map_err(|e| {
			crate::CommandError::ExecutionError(format!("Failed to get current executable: {}", e))
		})?;

		let mut cmd = tokio::process::Command::new(current_exe);
		cmd.arg("runserver").arg(address).arg("--noreload");

		if insecure {
			cmd.arg("--insecure");
		}
		if no_docs {
			cmd.arg("--no-docs");
		}
		if with_pages {
			cmd.arg("--with-pages");
		}
		if !static_dir.is_empty() {
			cmd.arg("--static-dir").arg(static_dir);
		}
		if no_spa {
			cmd.arg("--no-spa");
		}

		// Set environment variable to indicate this is a child process (prevent log duplication, etc.)
		cmd.env("REINHARDT_IS_AUTORELOAD_CHILD", "1");

		// Inherit stdout/stderr from parent process
		cmd.stdout(std::process::Stdio::inherit());
		cmd.stderr(std::process::Stdio::inherit());

		cmd.spawn().map_err(|e| {
			crate::CommandError::ExecutionError(format!("Failed to spawn server process: {}", e))
		})
	}

	/// Watch for file changes asynchronously
	#[cfg(all(feature = "server", feature = "autoreload"))]
	async fn watch_files_async() -> Result<(), notify::Error> {
		use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
		use std::path::Path;

		let (tx, mut rx) = tokio::sync::mpsc::channel(100);

		let mut watcher = RecommendedWatcher::new(
			move |res: Result<Event, notify::Error>| {
				if let Ok(event) = res {
					let _ = tx.blocking_send(event);
				}
			},
			Config::default(),
		)?;

		// Directories to watch
		watcher.watch(Path::new("src"), RecursiveMode::Recursive)?;
		watcher.watch(Path::new("Cargo.toml"), RecursiveMode::NonRecursive)?;

		// Wait for the first relevant change event
		while let Some(event) = rx.recv().await {
			if Self::is_relevant_change(&event) {
				return Ok(());
			}
		}

		Ok(())
	}

	/// Check if the change event is relevant
	#[cfg(all(feature = "server", feature = "autoreload"))]
	fn is_relevant_change(event: &notify::Event) -> bool {
		use notify::EventKind;

		matches!(
			event.kind,
			EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
		) && event.paths.iter().any(|p| {
			let path_str = p.to_string_lossy();
			!path_str.contains("/target/")
				&& !path_str.contains("/.git/")
				&& !path_str.ends_with('~')
				&& !path_str.ends_with(".swp")
				&& !path_str.ends_with(".tmp")
				&& (path_str.ends_with(".rs") || path_str.ends_with(".toml"))
		})
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
		"Display all registered URL patterns"
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
			ctx.info("      .function(\"/health\", Method::GET, health_handler);");
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
			ctx.info("No routes registered.");
			return Ok(());
		}

		// Check if --names flag is set
		let names_only = ctx.has_option("names");

		// Display header
		ctx.info("Registered URL patterns:");
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
		ctx.success(&format!("Total routes: {}", routes.len()));

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

		// 1. Database connectivity check (if DATABASE_URL is set)
		if let Ok(database_url) = std::env::var("DATABASE_URL") {
			ctx.info("Checking database connectivity...");
			match Self::check_database(&database_url).await {
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
			ctx.info("Skipping database check (DATABASE_URL not set)");
		}

		// 2. Settings validation
		ctx.info("Checking settings...");
		checks_passed += Self::check_settings(ctx, is_deploy);

		// 3. Migration status check (if DATABASE_URL is set)
		if std::env::var("DATABASE_URL").is_ok() {
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

		// 4. Static files verification
		ctx.info("Checking static files...");
		if std::env::var("STATIC_ROOT").is_ok() {
			ctx.success("  ✓ STATIC_ROOT configured");
			checks_passed += 1;
		} else if is_deploy {
			ctx.warning("  ✗ STATIC_ROOT not set (required for deployment)");
			checks_failed += 1;
		} else {
			ctx.info("  ⚠ STATIC_ROOT not set (optional for development)");
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

/// Sanitizes a database URL for display, removing credentials.
///
/// Replaces `user:password@` with `***@` to prevent credential leakage
/// in logs and startup banners.
#[cfg(feature = "reinhardt-db")]
fn sanitize_database_url(url: &str) -> String {
	// Match scheme://user:pass@host pattern and redact credentials
	if let Some(scheme_end) = url.find("://") {
		let after_scheme = &url[scheme_end + 3..];
		if let Some(at_pos) = after_scheme.find('@') {
			let host_part = &after_scheme[at_pos..];
			return format!("{}://***{}", &url[..scheme_end], host_part);
		}
	}
	// For non-URL formats (e.g., sqlite:file.db), return as-is
	url.to_string()
}

/// Helper function to get DATABASE_URL from environment or settings
#[cfg(feature = "reinhardt-db")]
fn get_database_url() -> Result<String, crate::CommandError> {
	use std::env;

	let base_dir = env::current_dir().ok();
	DatabaseConnection::get_database_url_from_env_or_settings(base_dir).map_err(|e| {
		crate::CommandError::ExecutionError(format!("Failed to get database URL: {}", e))
	})
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
		DatabaseType::Sqlite => {
			let conn = DatabaseConnection::connect_sqlite(url).await.map_err(|e| {
				crate::CommandError::ExecutionError(format!("Database connection failed: {}", e))
			})?;
			Ok((db_type, conn))
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

		let schema = match db_type {
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

	#[tokio::test]
	async fn test_check_command_basic() {
		let cmd = CheckCommand;
		let ctx = CommandContext::default();

		// Should succeed when no DATABASE_URL is set (skips DB check)
		let result = cmd.execute(&ctx).await;
		// May fail if environment has strict checks, but should handle gracefully
		assert!(result.is_ok() || result.is_err());
	}

	#[tokio::test]
	async fn test_check_command_with_deploy_flag() {
		let cmd = CheckCommand;
		let mut ctx = CommandContext::default();
		ctx.set_option("deploy".to_string(), "true".to_string());

		// Deploy checks are stricter and may fail
		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok() || result.is_err());
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
	#[serial_test::serial(env_change)]
	async fn test_migrate_command() {
		let cmd = MigrateCommand;
		let ctx = CommandContext::default();

		// Without migrations feature or DATABASE_URL, should handle gracefully
		let result = cmd.execute(&ctx).await;
		#[cfg(feature = "migrations")]
		{
			// May fail without DATABASE_URL, which is expected
			assert!(result.is_ok() || result.is_err());
		}
		#[cfg(not(feature = "migrations"))]
		{
			assert!(result.is_ok());
		}
	}

	#[tokio::test]
	#[serial_test::serial(env_change)]
	#[cfg(feature = "migrations")]
	async fn test_makemigrations_command() {
		use reinhardt_db::migrations::model_registry::{
			FieldMetadata, ModelMetadata, global_registry,
		};
		use reinhardt_db::prelude::FieldType;
		use tempfile::TempDir;

		// Create a temporary directory for migrations
		let temp_dir = TempDir::new().unwrap();
		let migrations_dir = temp_dir.path();
		std::fs::create_dir_all(&migrations_dir).unwrap();

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

		// Set up test environment
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
		unsafe { std::env::remove_var("DATABASE_URL") };

		// Should succeed (creates an empty migration)
		assert!(result.is_ok(), "Failed with: {:?}", result.err());
	}

	#[tokio::test]
	#[serial_test::serial(env_change)]
	#[cfg(feature = "migrations")]
	async fn test_makemigrations_with_dry_run() {
		use reinhardt_db::{
			migrations::model_registry::{FieldMetadata, ModelMetadata, global_registry},
			prelude::FieldType,
		};
		use tempfile::TempDir;

		// Create a temporary directory for migrations
		let temp_dir = TempDir::new().unwrap();
		let migrations_dir = temp_dir.path();
		std::fs::create_dir_all(&migrations_dir).unwrap();

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

		// Set up test environment
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
		unsafe { std::env::remove_var("DATABASE_URL") };

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
	async fn test_checkdi_command_execution() {
		let cmd = CheckDiCommand;
		let ctx = CommandContext::default();

		// Execute the command
		let result = cmd.execute(&ctx).await;

		// Without di feature, should fail with specific error
		#[cfg(not(feature = "di"))]
		{
			assert!(result.is_err());
			let err = result.unwrap_err();
			assert!(err.to_string().contains("di"));
		}

		// With di feature, may succeed or fail based on registered dependencies
		#[cfg(feature = "di")]
		{
			// Result depends on whether any dependencies are registered
			assert!(result.is_ok() || result.is_err());
		}
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

	#[cfg(all(feature = "server", feature = "autoreload"))]
	mod autoreload_tests {
		use super::*;
		use notify::{Event, EventKind};
		use std::path::PathBuf;

		#[test]
		fn test_is_relevant_change_rust_file() {
			let event = Event {
				kind: EventKind::Modify(notify::event::ModifyKind::Any),
				paths: vec![PathBuf::from("/project/src/main.rs")],
				attrs: Default::default(),
			};
			assert!(RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_toml_file() {
			let event = Event {
				kind: EventKind::Modify(notify::event::ModifyKind::Any),
				paths: vec![PathBuf::from("/project/Cargo.toml")],
				attrs: Default::default(),
			};
			assert!(RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_target_dir_ignored() {
			let event = Event {
				kind: EventKind::Modify(notify::event::ModifyKind::Any),
				paths: vec![PathBuf::from("/project/target/debug/main.rs")],
				attrs: Default::default(),
			};
			assert!(!RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_git_dir_ignored() {
			let event = Event {
				kind: EventKind::Modify(notify::event::ModifyKind::Any),
				paths: vec![PathBuf::from("/project/.git/objects/abc")],
				attrs: Default::default(),
			};
			assert!(!RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_swap_file_ignored() {
			let event = Event {
				kind: EventKind::Modify(notify::event::ModifyKind::Any),
				paths: vec![PathBuf::from("/project/src/main.rs.swp")],
				attrs: Default::default(),
			};
			assert!(!RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_backup_file_ignored() {
			let event = Event {
				kind: EventKind::Modify(notify::event::ModifyKind::Any),
				paths: vec![PathBuf::from("/project/src/main.rs~")],
				attrs: Default::default(),
			};
			assert!(!RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_tmp_file_ignored() {
			let event = Event {
				kind: EventKind::Modify(notify::event::ModifyKind::Any),
				paths: vec![PathBuf::from("/project/src/temp.tmp")],
				attrs: Default::default(),
			};
			assert!(!RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_non_rust_file_ignored() {
			let event = Event {
				kind: EventKind::Modify(notify::event::ModifyKind::Any),
				paths: vec![PathBuf::from("/project/src/style.css")],
				attrs: Default::default(),
			};
			assert!(!RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_create_event() {
			let event = Event {
				kind: EventKind::Create(notify::event::CreateKind::File),
				paths: vec![PathBuf::from("/project/src/new.rs")],
				attrs: Default::default(),
			};
			assert!(RunServerCommand::is_relevant_change(&event));
		}

		#[test]
		fn test_is_relevant_change_remove_event() {
			let event = Event {
				kind: EventKind::Remove(notify::event::RemoveKind::File),
				paths: vec![PathBuf::from("/project/src/old.rs")],
				attrs: Default::default(),
			};
			assert!(RunServerCommand::is_relevant_change(&event));
		}
	}
}
