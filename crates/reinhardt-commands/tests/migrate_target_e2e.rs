//! E2E tests for Django-style migrate-with-target semantics.
//!
//! Covers the rollback scenarios from issue #4558 plus the forward-to-target
//! direction requested as part of resolving #4558 / #4607:
//!
//! Rollback (backward / zero):
//! 1. `rollback_default_one_step_succeeds` — `migrate myapp <prev>` rolls back the
//!    most recent migration when the target is exactly one step earlier than head.
//! 2. `rollback_multi_step` — `migrate myapp <2-back>` rolls back two migrations
//!    in a single invocation.
//! 3. `rollback_dry_run_does_not_modify_db` — `migrate myapp <prev> --plan`
//!    previews the action and leaves the database untouched.
//! 4. `rollback_no_applied_migrations_exits_cleanly` — `migrate myapp zero`
//!    against an empty applied set succeeds with an informational message.
//! 5. `plan_on_fresh_db_does_not_create_recorder_table` — `--plan` must not create
//!    the bookkeeping table as a side effect on a fresh database.
//! 6. `rollback_with_missing_reverse_sql_fails_loudly` — when a migration file
//!    referenced in the applied set is missing from disk, the error identifies it.
//!
//! Forward (apply to target):
//! 7. `forward_to_target_applies_up_to_target` — `migrate myapp <middle>` applies
//!    the target plus its dependency closure and stops there.
//! 8. `forward_to_target_skips_already_applied` — applying forward past an already
//!    applied migration only applies the remaining ones up to the target.
//! 9. `forward_to_target_plan_does_not_modify_db` — `--plan` on the forward path
//!    must not create the recorder table on a fresh database.
//!
//! These tests use the `migration_executor` fixture (TestContainers PostgreSQL).
//! The migration `.rs` files written to the tempdir declare `operations: vec![]`,
//! so no real schema operations are exercised; the focus is on the
//! direction-detection orchestration in `MigrateCommand`.

#![cfg(feature = "testcontainers")]

use reinhardt_commands::{BaseCommand, CommandContext, MigrateCommand};
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::DatabaseMigrationRecorder;
use reinhardt_test::fixtures::migrations::{MigrationExecutorFixture, migration_executor};
use rstest::rstest;
use serial_test::serial;
use std::path::Path;
use tempfile::TempDir;

/// Write a minimally valid migration `.rs` file under `<root>/<app>/<name>.rs`.
///
/// The `Migration` struct literal has `operations: vec![]` so the file parses
/// cleanly via `syn` but requires no real `Operation` types to be resolvable
/// (the `FilesystemSource` only parses the AST — it never compiles the file).
fn write_test_migration(
	root: &Path,
	app: &str,
	name: &str,
	deps: &[(&str, &str)],
) -> std::io::Result<()> {
	write_test_migration_with_replaces(root, app, name, deps, &[])
}

/// Write a minimally valid migration that replaces the supplied historical
/// migration rows.
fn write_test_migration_with_replaces(
	root: &Path,
	app: &str,
	name: &str,
	deps: &[(&str, &str)],
	replaces: &[(&str, &str)],
) -> std::io::Result<()> {
	let app_dir = root.join(app);
	std::fs::create_dir_all(&app_dir)?;
	let deps_literal = deps
		.iter()
		.map(|(a, n)| format!("(\"{}\".to_string(), \"{}\".to_string())", a, n))
		.collect::<Vec<_>>()
		.join(", ");
	let replaces_literal = replaces
		.iter()
		.map(|(a, n)| format!("(\"{}\".to_string(), \"{}\".to_string())", a, n))
		.collect::<Vec<_>>()
		.join(", ");
	let content = format!(
		"use reinhardt::db::migrations::prelude::*;\n\
		 pub(super) fn migration() -> Migration {{\n\
		 \tMigration {{\n\
		 \t\tapp_label: \"{app}\".to_string(),\n\
		 \t\tname: \"{name}\".to_string(),\n\
		 \t\toperations: vec![],\n\
		 \t\tdependencies: vec![{deps_literal}],\n\
		 \t\tatomic: true,\n\
		 \t\treplaces: vec![{replaces_literal}],\n\
		 \t\tinitial: None,\n\
		 \t\tstate_only: false,\n\
		 \t\tdatabase_only: false,\n\
		 \t\tswappable_dependencies: vec![],\n\
		 \t\toptional_dependencies: vec![],\n\
		 \t}}\n\
		 }}\n",
	);
	std::fs::write(app_dir.join(format!("{}.rs", name)), content)
}

/// Write the standard linear chain `0001_first <- 0002_second <- 0003_third`
/// for `myapp` under a fresh tempdir. Returns the tempdir (keep it alive) and
/// the migrations root path.
fn write_three_migrations() -> (TempDir, std::path::PathBuf) {
	let tempdir = tempfile::tempdir().expect("create tempdir");
	let migrations_root = tempdir.path().join("migrations");
	write_test_migration(&migrations_root, "myapp", "0001_first", &[]).unwrap();
	write_test_migration(
		&migrations_root,
		"myapp",
		"0002_second",
		&[("myapp", "0001_first")],
	)
	.unwrap();
	write_test_migration(
		&migrations_root,
		"myapp",
		"0003_third",
		&[("myapp", "0002_second")],
	)
	.unwrap();
	(tempdir, migrations_root)
}

/// Build a `CommandContext` targeting the given migrations directory and
/// database URL, with optional positional args and the `--plan` flag.
fn build_ctx(
	migrations_dir: &Path,
	database_url: &str,
	app_label: Option<&str>,
	target: Option<&str>,
	plan: bool,
) -> CommandContext {
	let mut ctx = CommandContext::default();
	if let Some(app) = app_label {
		ctx.add_arg(app.to_string());
		if let Some(t) = target {
			ctx.add_arg(t.to_string());
		}
	}
	ctx.set_option("database".to_string(), database_url.to_string());
	ctx.set_option(
		"migrations-dir".to_string(),
		migrations_dir.to_string_lossy().to_string(),
	);
	if plan {
		ctx.set_option("plan".to_string(), "true".to_string());
	}
	ctx
}

/// Set up a tempdir with three sequential migrations for `myapp` and mark them
/// all as applied in the recorder via `record_migration`. Returns the tempdir
/// (keep it alive) and the migrations root path.
///
/// On a fresh TestContainers Postgres the `reinhardt_migrations` table does not
/// exist yet — `record_migration` does NOT auto-create it, so we construct a
/// side recorder from `executor.connection()` and call `ensure_schema_table()`
/// once before recording.
async fn arrange_three_applied_migrations(
	executor: &mut reinhardt_db::migrations::DatabaseMigrationExecutor,
) -> (TempDir, std::path::PathBuf) {
	let (tempdir, migrations_root) = write_three_migrations();

	let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
	recorder
		.ensure_schema_table()
		.await
		.expect("ensure_schema_table on fresh Postgres");

	for name in ["0001_first", "0002_second", "0003_third"] {
		executor
			.record_migration("myapp", name)
			.await
			.expect("record_migration should succeed against a fresh TestContainers Postgres");
	}

	(tempdir, migrations_root)
}

/// Re-query the recorder for the current applied set for `app`. Builds a fresh
/// connection from `url` to avoid interfering with the fixture's executor.
async fn applied_for_app(url: &str, app: &str) -> Vec<String> {
	let connection = DatabaseConnection::connect_postgres(url)
		.await
		.expect("re-connect to test Postgres");
	let recorder = DatabaseMigrationRecorder::new(connection.clone());
	let applied = recorder
		.get_applied_migrations()
		.await
		.expect("query applied migrations");
	applied
		.into_iter()
		.filter(|r| r.app == app)
		.map(|r| r.name)
		.collect()
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn rollback_default_one_step_succeeds(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — 3 migrations applied, target is the second-to-last one.
	let (mut executor, _container, _pool, _port, url) = migration_executor.await;
	let (_tempdir, migrations_root) = arrange_three_applied_migrations(&mut executor).await;

	// Act — `migrate myapp 0002_second` should unapply 0003_third.
	let ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0002_second"),
		false,
	);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("rollback should succeed");

	// Assert — recorder should hold only 0001 and 0002.
	let remaining = applied_for_app(&url, "myapp").await;
	assert_eq!(remaining.len(), 2, "exactly two migrations should remain");
	assert!(
		remaining.iter().any(|n| n == "0001_first"),
		"0001_first must remain applied"
	);
	assert!(
		remaining.iter().any(|n| n == "0002_second"),
		"0002_second must remain applied"
	);
	assert!(
		!remaining.iter().any(|n| n == "0003_third"),
		"0003_third must have been rolled back"
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn rollback_multi_step(#[future] migration_executor: MigrationExecutorFixture) {
	// Arrange — 3 migrations applied, target two steps back.
	let (mut executor, _container, _pool, _port, url) = migration_executor.await;
	let (_tempdir, migrations_root) = arrange_three_applied_migrations(&mut executor).await;

	// Act — `migrate myapp 0001_first` should unapply both 0002 and 0003.
	let ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0001_first"),
		false,
	);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("multi-step rollback should succeed");

	// Assert — only 0001_first remains.
	let remaining = applied_for_app(&url, "myapp").await;
	assert_eq!(
		remaining,
		vec!["0001_first".to_string()],
		"only the target migration must remain"
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn rollback_dry_run_does_not_modify_db(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — 3 migrations applied; we'll preview a 2-step rollback.
	let (mut executor, _container, _pool, _port, url) = migration_executor.await;
	let (_tempdir, migrations_root) = arrange_three_applied_migrations(&mut executor).await;

	// Act — `migrate myapp 0001_first --plan` should print the plan but not
	// touch the database.
	let ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0001_first"),
		true,
	);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("dry-run should succeed");

	// Assert — all three migrations remain applied.
	let remaining = applied_for_app(&url, "myapp").await;
	assert_eq!(
		remaining.len(),
		3,
		"--plan must not modify the recorder; got remaining = {:?}",
		remaining
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn rollback_no_applied_migrations_exits_cleanly(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — recorder is empty; only the on-disk migration files exist.
	let (_executor, _container, _pool, _port, url) = migration_executor.await;
	let tempdir = tempfile::tempdir().expect("create tempdir");
	let migrations_root = tempdir.path().join("migrations");
	write_test_migration(&migrations_root, "myapp", "0001_first", &[]).unwrap();

	// Act — `migrate myapp zero` against an empty applied set should report
	// "nothing to do" and return Ok(()).
	let ctx = build_ctx(&migrations_root, &url, Some("myapp"), Some("zero"), false);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("zero-rollback on empty state should be a no-op success");

	// Assert — recorder still has no applied migrations for myapp.
	let remaining = applied_for_app(&url, "myapp").await;
	assert!(
		remaining.is_empty(),
		"recorder must remain empty; got {:?}",
		remaining
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn plan_on_fresh_db_does_not_create_recorder_table(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Regression test for the `--plan` no-mutation contract: on a fresh
	// container the `reinhardt_migrations` table has not been created yet. A
	// naïve implementation that unconditionally calls `ensure_schema_table()`
	// before querying the applied set would silently create the table as a side
	// effect, violating the dry-run guarantee. This test pins that behavior.
	let (_executor, _container, _pool, _port, url) = migration_executor.await;
	let tempdir = tempfile::tempdir().expect("create tempdir");
	let migrations_root = tempdir.path().join("migrations");
	write_test_migration(&migrations_root, "myapp", "0001_first", &[]).unwrap();

	// Act — `migrate myapp zero --plan` on a database without the recorder
	// table must succeed (planning an empty rollback) without touching the DB.
	let ctx = build_ctx(&migrations_root, &url, Some("myapp"), Some("zero"), true);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("`--plan` on a fresh DB must succeed without modifying state");

	// Assert — the recorder table still does not exist, so re-querying must err.
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("re-connect to test Postgres");
	let recorder = DatabaseMigrationRecorder::new(connection.clone());
	let result = recorder.get_applied_migrations().await;
	assert!(
		result.is_err(),
		"--plan must NOT create the recorder table as a side effect; \
		 got Ok({:?}) — table appears to exist after dry-run",
		result.ok()
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn rollback_with_missing_reverse_sql_fails_loudly(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — record three migrations as applied, but the `.rs` file for
	// 0003_third is intentionally absent from disk. The rollback path must
	// surface a clear error naming the missing migration when it tries to load
	// it via the FilesystemSource.
	let (mut executor, _container, _pool, _port, url) = migration_executor.await;
	let tempdir = tempfile::tempdir().expect("create tempdir");
	let migrations_root = tempdir.path().join("migrations");
	write_test_migration(&migrations_root, "myapp", "0001_first", &[]).unwrap();
	write_test_migration(
		&migrations_root,
		"myapp",
		"0002_second",
		&[("myapp", "0001_first")],
	)
	.unwrap();
	// Note: 0003_third is intentionally NOT written to disk.

	let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
	recorder.ensure_schema_table().await.unwrap();
	for name in ["0001_first", "0002_second", "0003_third"] {
		executor.record_migration("myapp", name).await.unwrap();
	}

	// Act — `migrate myapp 0002_second` will try to load 0003_third but the
	// file is missing.
	let ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0002_second"),
		false,
	);
	let result = MigrateCommand.execute(&ctx).await;

	// Assert — the error message names the offending migration.
	let err = result.expect_err("should fail when the migration file cannot be loaded");
	let msg = err.to_string();
	assert!(
		msg.contains("0003_third"),
		"error must name the offending migration; got: {}",
		msg
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn forward_to_target_applies_up_to_target(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — three migrations on disk, NONE applied.
	let (_executor, _container, _pool, _port, url) = migration_executor.await;
	let (_tempdir, migrations_root) = write_three_migrations();

	// Act — `migrate myapp 0002_second` should apply 0001 and 0002 (the target
	// plus its dependency closure) but NOT 0003.
	let ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0002_second"),
		false,
	);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("forward-to-target should succeed");

	// Assert — exactly 0001 and 0002 applied.
	let applied = applied_for_app(&url, "myapp").await;
	assert_eq!(applied.len(), 2, "exactly two migrations should be applied");
	assert!(
		applied.iter().any(|n| n == "0001_first"),
		"0001_first must be applied (dependency of the target)"
	);
	assert!(
		applied.iter().any(|n| n == "0002_second"),
		"0002_second (the target) must be applied"
	);
	assert!(
		!applied.iter().any(|n| n == "0003_third"),
		"0003_third is past the target and must NOT be applied"
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn forward_to_target_skips_already_applied(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — three migrations on disk; apply only 0001 first.
	let (_executor, _container, _pool, _port, url) = migration_executor.await;
	let (_tempdir, migrations_root) = write_three_migrations();

	let ctx_first = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0001_first"),
		false,
	);
	MigrateCommand
		.execute(&ctx_first)
		.await
		.expect("applying the first migration should succeed");

	// Act — `migrate myapp 0003_third` should apply the remaining 0002 and 0003,
	// skipping the already-applied 0001.
	let ctx_third = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0003_third"),
		false,
	);
	MigrateCommand
		.execute(&ctx_third)
		.await
		.expect("forward to head should succeed");

	// Assert — all three migrations are now applied.
	let applied = applied_for_app(&url, "myapp").await;
	assert_eq!(applied.len(), 3, "all three migrations should be applied");
	for name in ["0001_first", "0002_second", "0003_third"] {
		assert!(
			applied.iter().any(|n| n == name),
			"{} must be applied",
			name
		);
	}
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn forward_to_target_plan_does_not_modify_db(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — three migrations on disk, fresh database.
	let (_executor, _container, _pool, _port, url) = migration_executor.await;
	let (_tempdir, migrations_root) = write_three_migrations();

	// Act — `migrate myapp 0003_third --plan` previews the forward apply without
	// touching the database.
	let ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0003_third"),
		true,
	);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("forward `--plan` on a fresh DB must succeed without modifying state");

	// Assert — the recorder table was not created as a side effect.
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("re-connect to test Postgres");
	let recorder = DatabaseMigrationRecorder::new(connection.clone());
	let result = recorder.get_applied_migrations().await;
	assert!(
		result.is_err(),
		"forward `--plan` must NOT create the recorder table; \
		 got Ok({:?}) — table appears to exist after dry-run",
		result.ok()
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn fake_rollback_updates_recorder_without_loading_files(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — record 0001/0002/0003 as applied, but only 0001 and 0002 exist
	// on disk (0003's file is intentionally absent). A *real* rollback to
	// 0001_first fails because it cannot load 0003's reverse SQL (see
	// `rollback_with_missing_reverse_sql_fails_loudly`). A `--fake` rollback must
	// succeed: it only updates the recorder and never loads the file or runs SQL.
	let (mut executor, _container, _pool, _port, url) = migration_executor.await;
	let tempdir = tempfile::tempdir().expect("create tempdir");
	let migrations_root = tempdir.path().join("migrations");
	write_test_migration(&migrations_root, "myapp", "0001_first", &[]).unwrap();
	write_test_migration(
		&migrations_root,
		"myapp",
		"0002_second",
		&[("myapp", "0001_first")],
	)
	.unwrap();
	// Note: 0003_third is intentionally NOT written to disk.

	let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
	recorder.ensure_schema_table().await.unwrap();
	for name in ["0001_first", "0002_second", "0003_third"] {
		executor.record_migration("myapp", name).await.unwrap();
	}

	// Act — `migrate myapp 0001_first --fake` unapplies 0002 and 0003 in the
	// recorder without touching the (missing) files.
	let mut ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0001_first"),
		false,
	);
	ctx.set_option("fake".to_string(), "true".to_string());
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("fake rollback must succeed even when a later migration file is missing");

	// Assert — only 0001_first remains applied.
	let remaining = applied_for_app(&url, "myapp").await;
	assert_eq!(
		remaining,
		vec!["0001_first".to_string()],
		"fake rollback should leave only the target applied"
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn fake_forward_records_target_closure(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — three migrations on disk, NONE applied.
	let (_executor, _container, _pool, _port, url) = migration_executor.await;
	let (_tempdir, migrations_root) = write_three_migrations();

	// Act — `migrate myapp 0002_second --fake` records 0001 and 0002 as applied
	// without running their operations.
	let mut ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0002_second"),
		false,
	);
	ctx.set_option("fake".to_string(), "true".to_string());
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("fake forward should succeed");

	// Assert — exactly 0001 and 0002 are recorded.
	let applied = applied_for_app(&url, "myapp").await;
	assert_eq!(
		applied.len(),
		2,
		"exactly two migrations should be recorded"
	);
	assert!(applied.iter().any(|n| n == "0001_first"));
	assert!(applied.iter().any(|n| n == "0002_second"));
	assert!(
		!applied.iter().any(|n| n == "0003_third"),
		"0003_third is past the target and must not be recorded"
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn plan_apply_all_does_not_modify_db(#[future] migration_executor: MigrationExecutorFixture) {
	// Arrange — one migration on disk, fresh database, no target argument.
	let (_executor, _container, _pool, _port, url) = migration_executor.await;
	let tempdir = tempfile::tempdir().expect("create tempdir");
	let migrations_root = tempdir.path().join("migrations");
	write_test_migration(&migrations_root, "myapp", "0001_first", &[]).unwrap();

	// Act — `migrate myapp --plan` (no target) must preview the apply-all set
	// without touching the database.
	let ctx = build_ctx(&migrations_root, &url, Some("myapp"), None, true);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("apply-all `--plan` on a fresh DB must succeed without modifying state");

	// Assert — the recorder table was not created as a side effect.
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("re-connect to test Postgres");
	let recorder = DatabaseMigrationRecorder::new(connection.clone());
	let result = recorder.get_applied_migrations().await;
	assert!(
		result.is_err(),
		"apply-all `--plan` must NOT create the recorder table; \
		 got Ok({:?}) — table appears to exist after dry-run",
		result.ok()
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn target_reconciliation_removes_stale_replacement_rows_before_rollback(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — an interrupted reconciliation has renamed the first original
	// row to the squash but left a later original recorder row behind. Its source
	// file is intentionally absent so a rollback would prove the stale row was
	// interpreted as a real later migration.
	let (mut executor, _container, _pool, _port, url) = migration_executor.await;
	let tempdir = tempfile::tempdir().expect("create tempdir");
	let migrations_root = tempdir.path().join("migrations");
	write_test_migration(&migrations_root, "myapp", "0001_first", &[])
		.expect("write initial migration");
	write_test_migration_with_replaces(
		&migrations_root,
		"myapp",
		"0001_squashed_0002",
		&[],
		&[("myapp", "0001_first"), ("myapp", "0002_second")],
	)
	.expect("write replacement migration");
	let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
	recorder
		.ensure_schema_table()
		.await
		.expect("ensure recorder table");
	executor
		.record_migration("myapp", "0001_first")
		.await
		.expect("record first original migration");
	recorder
		.rename_applied("myapp", "0001_first", "myapp", "0001_squashed_0002")
		.await
		.expect("simulate the first reconciliation step");
	executor
		.record_migration("myapp", "0002_second")
		.await
		.expect("record stale superseded migration");

	// Act — targeting the original must reconcile the stale row instead of
	// attempting to roll it back.
	let ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0001_first"),
		false,
	);
	MigrateCommand
		.execute(&ctx)
		.await
		.expect("stale recorder rows must be reconciled before rollback selection");

	// Assert
	assert_eq!(
		applied_for_app(&url, "myapp").await,
		vec!["0001_squashed_0002".to_string()]
	);
}

#[rstest]
#[tokio::test]
#[serial(migrate_target_e2e)]
async fn fake_target_rejects_indirectly_covered_competing_replacement(
	#[future] migration_executor: MigrationExecutorFixture,
) {
	// Arrange — a competing replacement is already recorded, so it covers the
	// original history without providing an exact historical row for the target.
	let (mut executor, _container, _pool, _port, url) = migration_executor.await;
	let tempdir = tempfile::tempdir().expect("create tempdir");
	let migrations_root = tempdir.path().join("migrations");
	write_test_migration(&migrations_root, "myapp", "0001_first", &[])
		.expect("write original migration");
	for name in ["0001_squashed_0002_a", "0001_squashed_0002_b"] {
		write_test_migration_with_replaces(
			&migrations_root,
			"myapp",
			name,
			&[],
			&[("myapp", "0001_first")],
		)
		.expect("write competing replacement migration");
	}
	let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
	recorder
		.ensure_schema_table()
		.await
		.expect("ensure recorder table");
	executor
		.record_migration("myapp", "0001_squashed_0002_b")
		.await
		.expect("record competing replacement");

	// Act
	let mut ctx = build_ctx(
		&migrations_root,
		&url,
		Some("myapp"),
		Some("0001_squashed_0002_a"),
		false,
	);
	ctx.set_option("fake".to_string(), "true".to_string());
	let error = MigrateCommand
		.execute(&ctx)
		.await
		.expect_err("fake application must reject indirect competing coverage");

	// Assert
	assert_eq!(
		error.to_string(),
		"Execution error: Cannot fake replacement myapp:0001_squashed_0002_a because a competing replacement already covers its history"
	);
}
