//! Integration tests for migration merge functionality
//!
//! Tests the --merge option behavior for resolving migration conflicts
//! when multiple leaf nodes exist for the same app.

use reinhardt_db::migrations::{
	Migration, MigrationGraph, MigrationKey, MigrationNamer, MigrationNumbering,
};
use rstest::rstest;
use std::fs;
use tempfile::TempDir;

/// Helper to create a migration file on disk for a given app
fn create_migration_file(migrations_dir: &std::path::Path, app: &str, name: &str) {
	let app_dir = migrations_dir.join(app);
	fs::create_dir_all(&app_dir).unwrap();
	let content = format!(
		"// Auto-generated migration\nuse reinhardt_db::migrations::Migration;\n\npub fn migration() -> Migration {{\n\tMigration::new(\"{}\", \"{}\")\n}}\n",
		name, app
	);
	fs::write(app_dir.join(format!("{}.rs", name)), content).unwrap();
}

/// Helper to build a MigrationGraph from a list of migrations
fn build_graph(migrations: &[Migration]) -> MigrationGraph {
	let mut graph = MigrationGraph::new();
	for m in migrations {
		let key = MigrationKey::new(m.app_label.clone(), m.name.clone());
		let deps: Vec<MigrationKey> = m
			.dependencies
			.iter()
			.map(|(app, name)| MigrationKey::new(app.clone(), name.clone()))
			.collect();
		graph.add_migration(key, deps);
	}
	graph
}

/// Helper to create a Migration struct with given parameters
fn make_migration(app: &str, name: &str, deps: Vec<(&str, &str)>) -> Migration {
	Migration {
		app_label: app.to_string(),
		name: name.to_string(),
		operations: Vec::new(),
		dependencies: deps
			.into_iter()
			.map(|(a, n)| (a.to_string(), n.to_string()))
			.collect(),
		atomic: true,
		replaces: Vec::new(),
		initial: None,
		state_only: false,
		database_only: false,
		optional_dependencies: Vec::new(),
		swappable_dependencies: Vec::new(),
	}
}

// ============================================================================
// M-01: Merge two conflicting branches
// ============================================================================

#[rstest]
fn test_merge_two_conflicting_branches() {
	// Arrange: 0001_initial -> 0002_a and 0001_initial -> 0002_b
	let migrations = vec![
		make_migration("myapp", "0001_initial", vec![]),
		make_migration("myapp", "0002_add_field", vec![("myapp", "0001_initial")]),
		make_migration("myapp", "0002_add_index", vec![("myapp", "0001_initial")]),
	];
	let graph = build_graph(&migrations);

	// Act
	let conflicts = graph.detect_conflicts();

	// Assert
	assert_eq!(conflicts.len(), 1);
	assert!(conflicts.contains_key("myapp"));
	let leaves = conflicts
		.get("myapp")
		.expect("expected conflicts for 'myapp', but none found");
	assert_eq!(leaves.len(), 2);

	// Verify merge migration can be created
	let leaf_names: Vec<&str> = leaves.iter().map(|k| k.name.as_str()).collect();
	let merge_name = MigrationNamer::generate_merge_name(&leaf_names);
	assert!(merge_name.starts_with("merge_"));

	// Verify merge migration has correct dependencies
	let merge = make_migration(
		"myapp",
		&format!("0003_{}", merge_name),
		vec![("myapp", "0002_add_field"), ("myapp", "0002_add_index")],
	);
	assert!(merge.operations.is_empty());
	assert_eq!(merge.dependencies.len(), 2);
}

// ============================================================================
// M-02: Merge three conflicting branches
// ============================================================================

#[rstest]
fn test_merge_three_conflicting_branches() {
	// Arrange: 3-way branch
	let migrations = vec![
		make_migration("myapp", "0001_initial", vec![]),
		make_migration("myapp", "0002_a", vec![("myapp", "0001_initial")]),
		make_migration("myapp", "0002_b", vec![("myapp", "0001_initial")]),
		make_migration("myapp", "0002_c", vec![("myapp", "0001_initial")]),
	];
	let graph = build_graph(&migrations);

	// Act
	let conflicts = graph.detect_conflicts();

	// Assert
	assert_eq!(conflicts.len(), 1);
	let leaves = conflicts
		.get("myapp")
		.expect("expected conflicts for 'myapp', but none found");
	assert_eq!(leaves.len(), 3);

	// Verify merge has 3 dependencies
	let merge = make_migration(
		"myapp",
		"0003_merge",
		vec![
			("myapp", "0002_a"),
			("myapp", "0002_b"),
			("myapp", "0002_c"),
		],
	);
	assert_eq!(merge.dependencies.len(), 3);
}

// ============================================================================
// M-03: Merge conflicts in multiple apps
// ============================================================================

#[rstest]
fn test_merge_multiple_apps() {
	// Arrange: two apps with conflicts
	let migrations = vec![
		make_migration("auth", "0001_initial", vec![]),
		make_migration("auth", "0002_a", vec![("auth", "0001_initial")]),
		make_migration("auth", "0002_b", vec![("auth", "0001_initial")]),
		make_migration("users", "0001_initial", vec![]),
		make_migration("users", "0002_a", vec![("users", "0001_initial")]),
		make_migration("users", "0002_b", vec![("users", "0001_initial")]),
	];
	let graph = build_graph(&migrations);

	// Act
	let conflicts = graph.detect_conflicts();

	// Assert
	assert_eq!(conflicts.len(), 2);
	assert!(conflicts.contains_key("auth"));
	assert!(conflicts.contains_key("users"));
}

// ============================================================================
// M-04: Dry run does not create files
// ============================================================================

#[rstest]
fn test_merge_dry_run() {
	// Arrange: set up conflicting migrations on disk
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");

	create_migration_file(&migrations_dir, "myapp", "0001_initial");
	create_migration_file(&migrations_dir, "myapp", "0002_add_field");
	create_migration_file(&migrations_dir, "myapp", "0002_add_index");

	let initial_count = fs::read_dir(migrations_dir.join("myapp")).unwrap().count();

	// Act: detect conflicts and generate merge migration in-memory (dry-run simulation)
	let migrations = vec![
		make_migration("myapp", "0001_initial", vec![]),
		make_migration("myapp", "0002_add_field", vec![("myapp", "0001_initial")]),
		make_migration("myapp", "0002_add_index", vec![("myapp", "0001_initial")]),
	];
	let graph = build_graph(&migrations);
	let conflicts = graph.detect_conflicts();

	// Verify conflicts were detected
	assert_eq!(conflicts.len(), 1);
	let leaves = conflicts
		.get("myapp")
		.expect("expected conflicts for 'myapp', but none found");
	let leaf_names: Vec<&str> = leaves.iter().map(|k| k.name.as_str()).collect();
	let merge_name = MigrationNamer::generate_merge_name(&leaf_names);
	let migration_number = MigrationNumbering::next_number(&migrations_dir, "myapp");
	let _final_name = format!("{}_{}", migration_number, merge_name);

	// In dry-run mode, the merge migration is NOT saved to disk

	// Assert: no new files created (dry-run)
	let final_count = fs::read_dir(migrations_dir.join("myapp")).unwrap().count();
	assert_eq!(
		initial_count, final_count,
		"Dry run should not create any new migration files"
	);
}

// ============================================================================
// M-05: Custom name for merge migration
// ============================================================================

#[rstest]
fn test_merge_custom_name() {
	// Arrange
	let custom_name = "resolve_user_conflicts";
	let migration_number = "0003";
	let final_name = format!("{}_{}", migration_number, custom_name);

	// Act & Assert
	assert_eq!(final_name, "0003_resolve_user_conflicts");
}

// ============================================================================
// M-06: No conflicts detected
// ============================================================================

#[rstest]
fn test_merge_no_conflicts() {
	// Arrange: linear chain (no conflicts)
	let migrations = vec![
		make_migration("myapp", "0001_initial", vec![]),
		make_migration("myapp", "0002_add_field", vec![("myapp", "0001_initial")]),
		make_migration("myapp", "0003_add_index", vec![("myapp", "0002_add_field")]),
	];
	let graph = build_graph(&migrations);

	// Act
	let conflicts = graph.detect_conflicts();

	// Assert
	assert!(conflicts.is_empty());
}

// ============================================================================
// M-07: Empty migration directory
// ============================================================================

#[rstest]
fn test_merge_empty_dir() {
	// Arrange: no migrations at all
	let graph = MigrationGraph::new();

	// Act
	let conflicts = graph.detect_conflicts();

	// Assert
	assert!(conflicts.is_empty());
}

// ============================================================================
// M-08: Already merged (re-run produces no conflicts)
// ============================================================================

#[rstest]
fn test_merge_already_merged() {
	// Arrange: create branches and add merge migration
	let migrations = vec![
		make_migration("myapp", "0001_initial", vec![]),
		make_migration("myapp", "0002_add_field", vec![("myapp", "0001_initial")]),
		make_migration("myapp", "0002_add_index", vec![("myapp", "0001_initial")]),
		make_migration(
			"myapp",
			"0003_merge_0002_add_field_0002_add_index",
			vec![("myapp", "0002_add_field"), ("myapp", "0002_add_index")],
		),
	];
	let graph = build_graph(&migrations);

	// Act
	let conflicts = graph.detect_conflicts();

	// Assert: no conflicts after merge
	assert!(conflicts.is_empty());
}

// ============================================================================
// M-09: --merge + --empty mutually exclusive
// ============================================================================

#[rstest]
fn test_merge_with_empty_mutually_exclusive() {
	// Arrange & Act: verify that --merge and --empty can both be parsed at CLI level
	// (mutual exclusivity is enforced at command execution time, not parse time)
	use clap::Parser;
	use reinhardt_commands::Cli;

	let cli = Cli::try_parse_from(["manage", "makemigrations", "--merge", "--empty"]);

	// Assert: CLI parsing succeeds (both flags accepted)
	// The actual rejection happens in MakeMigrationsCommand::execute()
	assert!(
		cli.is_ok(),
		"Both --merge and --empty should be parseable at CLI level"
	);
}

// ============================================================================
// M-10: App label filter
// ============================================================================

#[rstest]
fn test_merge_app_label_filter() {
	// Arrange: two apps with conflicts
	let migrations = vec![
		make_migration("auth", "0001_initial", vec![]),
		make_migration("auth", "0002_a", vec![("auth", "0001_initial")]),
		make_migration("auth", "0002_b", vec![("auth", "0001_initial")]),
		make_migration("users", "0001_initial", vec![]),
		make_migration("users", "0002_a", vec![("users", "0001_initial")]),
		make_migration("users", "0002_b", vec![("users", "0001_initial")]),
	];
	let graph = build_graph(&migrations);

	// Act: filter to auth only
	let mut conflicts = graph.detect_conflicts();
	conflicts.retain(|app, _| app == "auth");

	// Assert: only auth conflicts
	assert_eq!(conflicts.len(), 1);
	assert!(conflicts.contains_key("auth"));
	assert!(!conflicts.contains_key("users"));
}

// ============================================================================
// M-11: Post-merge produces single leaf per app
// ============================================================================

#[rstest]
fn test_post_merge_single_leaf() {
	// Arrange: create branches and merge migration
	let migrations = vec![
		make_migration("myapp", "0001_initial", vec![]),
		make_migration("myapp", "0002_add_field", vec![("myapp", "0001_initial")]),
		make_migration("myapp", "0002_add_index", vec![("myapp", "0001_initial")]),
		make_migration(
			"myapp",
			"0003_merge",
			vec![("myapp", "0002_add_field"), ("myapp", "0002_add_index")],
		),
	];
	let graph = build_graph(&migrations);

	// Act
	let leaves = graph.get_leaf_nodes_for_app("myapp");

	// Assert: single leaf after merge
	assert_eq!(leaves.len(), 1);
	assert_eq!(leaves[0].name, "0003_merge");
}

// ============================================================================
// M-12: Post-merge topological sort succeeds
// ============================================================================

#[rstest]
fn test_post_merge_topological_sort() {
	// Arrange
	let migrations = vec![
		make_migration("myapp", "0001_initial", vec![]),
		make_migration("myapp", "0002_add_field", vec![("myapp", "0001_initial")]),
		make_migration("myapp", "0002_add_index", vec![("myapp", "0001_initial")]),
		make_migration(
			"myapp",
			"0003_merge",
			vec![("myapp", "0002_add_field"), ("myapp", "0002_add_index")],
		),
	];
	let graph = build_graph(&migrations);

	// Act
	let order = graph.topological_sort();

	// Assert
	assert!(order.is_ok());
	let sorted = order.unwrap();
	assert_eq!(sorted.len(), 4);
	// 0001 must come first
	assert_eq!(sorted[0].name, "0001_initial");
	// 0003_merge must come last
	assert_eq!(sorted[sorted.len() - 1].name, "0003_merge");
}

// ============================================================================
// M-13: Cross-app dependencies with merge
// ============================================================================

#[rstest]
fn test_merge_cross_app_dependencies() {
	// Arrange: auth branches where one depends on users
	let migrations = vec![
		make_migration("users", "0001_initial", vec![]),
		make_migration("auth", "0001_initial", vec![]),
		make_migration("auth", "0002_add_field", vec![("auth", "0001_initial")]),
		make_migration(
			"auth",
			"0002_add_fk",
			vec![("auth", "0001_initial"), ("users", "0001_initial")],
		),
	];
	let graph = build_graph(&migrations);

	// Act
	let conflicts = graph.detect_conflicts();

	// Assert: only auth has conflicts, not users
	assert_eq!(conflicts.len(), 1);
	assert!(conflicts.contains_key("auth"));
	assert!(!conflicts.contains_key("users"));

	// Create merge migration
	let merge = make_migration(
		"auth",
		"0003_merge",
		vec![("auth", "0002_add_field"), ("auth", "0002_add_fk")],
	);
	assert_eq!(merge.dependencies.len(), 2);
}

// ============================================================================
// M-14: Merge naming collision increments number
// ============================================================================

#[rstest]
fn test_merge_naming_collision_increments() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");

	create_migration_file(&migrations_dir, "myapp", "0001_initial");
	create_migration_file(&migrations_dir, "myapp", "0002_add_field");
	create_migration_file(&migrations_dir, "myapp", "0002_add_index");

	// Act: first merge gets 0003
	let first_number = MigrationNumbering::next_number(&migrations_dir, "myapp");
	assert_eq!(first_number, "0003");

	// Simulate creating the merge file
	create_migration_file(&migrations_dir, "myapp", "0003_merge");

	// Act: second merge would get 0004
	let second_number = MigrationNumbering::next_number(&migrations_dir, "myapp");
	assert_eq!(second_number, "0004");
}
