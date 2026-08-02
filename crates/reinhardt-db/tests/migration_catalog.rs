use async_trait::async_trait;
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, DatabaseMigrationRecorder, DependencyResolutionContext, FieldType,
	FilesystemSource, Migration, MigrationCatalog, MigrationError, MigrationKey, MigrationSource,
	Operation, Result, SwappableDependency,
};
use rstest::*;
use std::fs;
use tempfile::TempDir;

struct TestSource {
	migrations: Vec<Migration>,
}

#[async_trait]
impl MigrationSource for TestSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		Ok(self.migrations.clone())
	}
}

struct ErrorSource;

#[async_trait]
impl MigrationSource for ErrorSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		Err(MigrationError::InvalidMigration(
			"catalog source failed".to_string(),
		))
	}
}

fn migration(app: &str, name: &str, dependencies: &[(&str, &str)]) -> Migration {
	let mut migration = Migration::new(name, app);
	migration.dependencies = dependencies
		.iter()
		.map(|(dependency_app, dependency_name)| {
			(
				(*dependency_app).to_string(),
				(*dependency_name).to_string(),
			)
		})
		.collect();
	migration
}

async fn catalog(migrations: Vec<Migration>) -> MigrationCatalog {
	MigrationCatalog::load_strict(&TestSource { migrations })
		.await
		.unwrap()
}

#[rstest]
#[tokio::test]
async fn catalog_resolves_swappable_dependencies_with_the_provided_context() {
	// Arrange
	let custom_user =
		Migration::new("0001_initial", "custom_auth").add_operation(Operation::CreateTable {
			name: "custom_auth_user".to_string(),
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
	let profile = Migration::new("0001_initial", "profiles").add_swappable_dependency(
		SwappableDependency::new("AUTH_USER_MODEL", "auth", "User", "0001_initial"),
	);
	let context =
		DependencyResolutionContext::new().with_setting("AUTH_USER_MODEL", "custom_auth.User");

	// Act
	let catalog = MigrationCatalog::load_strict_with_context(
		&TestSource {
			migrations: vec![profile, custom_user],
		},
		&context,
	)
	.await
	.unwrap();
	let state = catalog
		.state_before(&MigrationKey::new("profiles", "0001_initial"))
		.unwrap();

	// Assert
	assert!(state.find_model_by_table("custom_auth_user").is_some());
}

#[rstest]
#[tokio::test]
async fn snapshot_excludes_migrations_replaced_by_a_squash() {
	// Arrange
	let original = migration("blog", "0001_initial", &[]);
	let mut replacement = migration("blog", "0001_squashed_0002", &[]);
	replacement.replaces = vec![("blog".to_string(), "0001_initial".to_string())];
	let catalog = catalog(vec![original, replacement]).await;
	let recorder = DatabaseMigrationRecorder::new(
		DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("connect to SQLite"),
	);

	// Act
	let snapshot = catalog.snapshot(&recorder, &[]).await.unwrap();

	// Assert
	assert_eq!(snapshot.ordered.len(), 1);
	assert_eq!(snapshot.ordered[0].name, "0001_squashed_0002");
}

#[rstest]
#[tokio::test]
async fn state_before_original_migration_uses_original_history_not_its_replacement() {
	// Arrange - a squash replaces the original create/drop sequence with an
	// unrelated table. Historical state for the original destructive migration
	// must still replay the original sequence.
	let initial = Migration::new("0001_initial", "catalog").add_operation(Operation::CreateTable {
		name: "books".to_string(),
		columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
		constraints: Vec::new(),
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	});
	let destructive = Migration::new("0002_drop_books", "catalog")
		.add_dependency("catalog", "0001_initial")
		.add_operation(Operation::DropTable {
			name: "books".to_string(),
		});
	let mut replacement =
		Migration::new("0001_squashed_0002", "catalog").add_operation(Operation::CreateTable {
			name: "archived_books".to_string(),
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
	replacement.replaces = vec![
		("catalog".to_string(), "0001_initial".to_string()),
		("catalog".to_string(), "0002_drop_books".to_string()),
	];
	let catalog = catalog(vec![initial, destructive, replacement]).await;

	// Act
	let state = catalog
		.state_before(&MigrationKey::new("catalog", "0002_drop_books"))
		.expect("reconstruct the historical pre-drop state");

	// Assert
	assert!(state.find_model_by_table("books").is_some());
	assert!(state.find_model_by_table("archived_books").is_none());
}

#[rstest]
#[tokio::test]
async fn resolve_unique_prefix_prefers_an_exact_match() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0002", &[]),
		migration("blog", "0002_add_title", &[]),
	])
	.await;

	// Act
	let resolved = catalog.resolve_unique_prefix("blog", "0002").unwrap();

	// Assert
	assert_eq!(resolved, MigrationKey::new("blog", "0002"));
}

#[rstest]
#[tokio::test]
async fn resolve_unique_prefix_accepts_a_unique_partial_name() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_initial", &[]),
		migration("blog", "0002_add_title", &[("blog", "0001_initial")]),
	])
	.await;

	// Act
	let resolved = catalog.resolve_unique_prefix("blog", "0002_add").unwrap();

	// Assert
	assert_eq!(resolved, MigrationKey::new("blog", "0002_add_title"));
}

#[rstest]
#[tokio::test]
async fn resolve_unique_prefix_reports_sorted_ambiguous_candidates() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0002_add_title", &[]),
		migration("blog", "0002_add_body", &[]),
	])
	.await;

	// Act
	let error = catalog.resolve_unique_prefix("blog", "0002").unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: Ambiguous migration prefix '0002' for app 'blog'; candidates: \
		 0002_add_body, 0002_add_title"
	);
}

#[rstest]
#[case("missing", "0001", "Migration not found: app missing")]
#[case("blog", "9999", "Migration not found: blog.9999")]
#[tokio::test]
async fn resolve_unique_prefix_rejects_unknown_apps_and_names(
	#[case] app: &str,
	#[case] prefix: &str,
	#[case] expected: &str,
) {
	// Arrange
	let catalog = catalog(vec![migration("blog", "0001_initial", &[])]).await;

	// Act
	let error = catalog.resolve_unique_prefix(app, prefix).unwrap_err();

	// Assert
	assert_eq!(error.to_string(), expected);
}

#[rstest]
#[tokio::test]
async fn load_strict_propagates_source_errors() {
	// Act
	let error = MigrationCatalog::load_strict(&ErrorSource)
		.await
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: catalog source failed"
	);
}

#[rstest]
#[tokio::test]
async fn load_strict_rejects_a_missing_dependency() {
	// Arrange
	let source = TestSource {
		migrations: vec![migration(
			"blog",
			"0002_add_title",
			&[("blog", "0001_initial")],
		)],
	};

	// Act
	let error = MigrationCatalog::load_strict(&source).await.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Dependency error: Missing dependency blog.0001_initial required by blog.0002_add_title"
	);
}

#[rstest]
#[tokio::test]
async fn load_strict_rejects_cycles() {
	// Arrange
	let source = TestSource {
		migrations: vec![
			migration("blog", "0001_initial", &[("blog", "0002_add_title")]),
			migration("blog", "0002_add_title", &[("blog", "0001_initial")]),
		],
	};

	// Act
	let error = MigrationCatalog::load_strict(&source).await.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Circular dependency detected: blog.0001_initial, blog.0002_add_title"
	);
}

#[rstest]
#[tokio::test]
async fn load_strict_rejects_an_invalid_filesystem_migration() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let app_dir = temp_dir.path().join("blog");
	fs::create_dir(&app_dir).unwrap();
	fs::write(
		app_dir.join("0001_initial.rs"),
		"pub fn migration( -> Migration {",
	)
	.unwrap();
	let source = FilesystemSource::new(temp_dir.path());

	// Act
	let error = MigrationCatalog::load_strict(&source).await.unwrap_err();

	// Assert
	let MigrationError::InvalidMigration(message) = error else {
		panic!("expected InvalidMigration");
	};
	let expected_prefix = format!(
		"Failed to parse {}:",
		app_dir.join("0001_initial.rs").display()
	);
	// NOTE: The remaining parser diagnostic is generated by syn and is not part of this API.
	assert!(message.starts_with(&expected_prefix));
}

#[rstest]
#[case(
	r#"pub fn migration() -> Migration {
		Migration {
			operations: vec![Operation::UnknownOperation { value: 1 }],
			dependencies: vec![],
			replaces: vec![],
		}
	}"#,
	"operations[0].UnknownOperation is unsupported or malformed"
)]
#[case(
	r#"pub fn migration() -> Migration {
		Migration {
			dependencies: vec![],
			replaces: vec![],
		}
	}"#,
	"Migration metadata is missing required 'operations' field"
)]
#[tokio::test]
async fn load_strict_rejects_semantically_invalid_filesystem_migrations(
	#[case] source_code: &str,
	#[case] expected_suffix: &str,
) {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let app_dir = temp_dir.path().join("blog");
	fs::create_dir(&app_dir).unwrap();
	fs::write(app_dir.join("0001_initial.rs"), source_code).unwrap();
	let source = FilesystemSource::new(temp_dir.path());

	// Act
	let error = MigrationCatalog::load_strict(&source).await.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		format!(
			"Invalid migration: Failed to load {} as blog.0001_initial: Invalid migration: \
			 {expected_suffix}",
			app_dir.join("0001_initial.rs").display()
		)
	);
}

#[rstest]
#[tokio::test]
async fn filesystem_source_uses_path_derived_app_and_name() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let app_dir = temp_dir.path().join("blog");
	fs::create_dir(&app_dir).unwrap();
	fs::write(
		app_dir.join("0001_initial.rs"),
		r#"pub fn migration() -> Migration {
			Migration {
				app_label: "wrong_app".to_string(),
				name: "9999_wrong".to_string(),
				operations: vec![],
				dependencies: vec![],
				replaces: vec![],
			}
		}"#,
	)
	.unwrap();
	let source = FilesystemSource::new(temp_dir.path());

	// Act
	let migrations = source.all_migrations().await.unwrap();

	// Assert
	assert_eq!(migrations.len(), 1);
	assert_eq!(migrations[0].app_label, "blog");
	assert_eq!(migrations[0].name, "0001_initial");
}

#[rstest]
#[case(vec!["z", "a", "z", "a"])]
#[case(vec!["a", "z", "a", "z"])]
#[tokio::test]
async fn load_strict_reports_duplicate_keys_deterministically(#[case] apps: Vec<&str>) {
	// Arrange
	let migrations = apps
		.into_iter()
		.map(|app| migration(app, "0001_initial", &[]))
		.collect();

	// Act
	let error = MigrationCatalog::load_strict(&TestSource { migrations })
		.await
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: Duplicate migration: a.0001_initial"
	);
}

#[rstest]
#[case(vec![
	migration("z", "0001_initial", &[("z", "9999_missing")]),
	migration("a", "0001_initial", &[("a", "9999_missing")]),
])]
#[case(vec![
	migration("a", "0001_initial", &[("a", "9999_missing")]),
	migration("z", "0001_initial", &[("z", "9999_missing")]),
])]
#[tokio::test]
async fn load_strict_reports_missing_dependencies_deterministically(
	#[case] migrations: Vec<Migration>,
) {
	// Act
	let error = MigrationCatalog::load_strict(&TestSource { migrations })
		.await
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Dependency error: Missing dependency a.9999_missing required by a.0001_initial"
	);
}

#[rstest]
#[tokio::test]
async fn filesystem_source_discovers_migrations_in_deterministic_path_order() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	for app in ["z", "a"] {
		let app_dir = temp_dir.path().join(app);
		fs::create_dir(&app_dir).unwrap();
		fs::write(
			app_dir.join("0001_initial.rs"),
			r#"pub fn migration() -> Migration {
				Migration {
					operations: vec![],
					dependencies: vec![],
					replaces: vec![],
				}
			}"#,
		)
		.unwrap();
	}
	let source = FilesystemSource::new(temp_dir.path());

	// Act
	let migrations = source.all_migrations().await.unwrap();

	// Assert
	let keys: Vec<(&str, &str)> = migrations
		.iter()
		.map(|migration| (migration.app_label.as_str(), migration.name.as_str()))
		.collect();
	assert_eq!(keys, vec![("a", "0001_initial"), ("z", "0001_initial")]);
}

#[cfg(unix)]
#[rstest]
#[tokio::test]
async fn filesystem_source_propagates_walkdir_errors() {
	use std::os::unix::fs::symlink;

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let app_dir = temp_dir.path().join("blog");
	fs::create_dir(&app_dir).unwrap();
	symlink(&app_dir, app_dir.join("loop")).unwrap();
	let source = FilesystemSource::new(temp_dir.path());

	// Act
	let error = source.all_migrations().await.unwrap_err();

	// Assert
	let MigrationError::IoError(error) = error else {
		panic!("expected IoError");
	};
	assert_eq!(error.kind(), std::io::ErrorKind::Other);
}

#[cfg(unix)]
#[rstest]
#[tokio::test]
async fn filesystem_source_reports_the_first_sorted_walkdir_error() {
	use std::os::unix::fs::symlink;

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	for app in ["z", "a"] {
		let app_dir = temp_dir.path().join(app);
		fs::create_dir(&app_dir).unwrap();
		symlink(&app_dir, app_dir.join("loop")).unwrap();
	}
	let source = FilesystemSource::new(temp_dir.path());

	// Act
	let error = source.all_migrations().await.unwrap_err();

	// Assert
	let MigrationError::IoError(error) = error else {
		panic!("expected IoError");
	};
	let expected_path = temp_dir.path().join("a").join("loop");
	assert!(
		error
			.to_string()
			.contains(&expected_path.display().to_string()),
		"the deterministic first traversal error must identify the a/loop path"
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_resolves_an_explicit_start_in_topological_order() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_initial", &[]),
		migration("blog", "0002_add_title", &[("blog", "0001_initial")]),
		migration("blog", "0003_publish", &[("blog", "0002_add_title")]),
	])
	.await;

	// Act
	let range = catalog.squash_range("blog", Some("0002"), "0003").unwrap();

	// Assert
	let names: Vec<&str> = range
		.migrations
		.iter()
		.map(|migration| migration.name.as_str())
		.collect();
	assert_eq!(names, vec!["0002_add_title", "0003_publish"]);
	assert_eq!(
		range.external_dependencies,
		vec![("blog".to_string(), "0001_initial".to_string())]
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_uses_the_implicit_app_root() {
	// Arrange
	let catalog = catalog(vec![
		migration("accounts", "0001_initial", &[]),
		migration("blog", "0001_initial", &[("accounts", "0001_initial")]),
		migration("blog", "0002_add_title", &[("blog", "0001_initial")]),
		migration("blog", "0003_publish", &[("blog", "0002_add_title")]),
	])
	.await;

	// Act
	let range = catalog.squash_range("blog", None, "0003").unwrap();

	// Assert
	let names: Vec<&str> = range
		.migrations
		.iter()
		.map(|migration| migration.name.as_str())
		.collect();
	assert_eq!(
		names,
		vec!["0001_initial", "0002_add_title", "0003_publish"]
	);
	assert_eq!(
		range.external_dependencies,
		vec![("accounts".to_string(), "0001_initial".to_string())]
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_deduplicates_and_sorts_external_dependencies() {
	// Arrange
	let catalog = catalog(vec![
		migration("accounts", "0001_a", &[]),
		migration("accounts", "0001_z", &[]),
		migration(
			"blog",
			"0001_initial",
			&[
				("accounts", "0001_z"),
				("accounts", "0001_a"),
				("accounts", "0001_a"),
			],
		),
		migration("blog", "0002_post", &[("blog", "0001_initial")]),
	])
	.await;

	// Act
	let range = catalog.squash_range("blog", Some("0001"), "0002").unwrap();

	// Assert
	assert_eq!(
		range.external_dependencies,
		vec![
			("accounts".to_string(), "0001_a".to_string()),
			("accounts".to_string(), "0001_z".to_string()),
		]
	);
}

#[rstest]
#[case(Some("0002"), "0003_final", vec!["0002_change", "0003_final"])]
#[case(Some("0002"), "0002_change", vec!["0002_change"])]
#[tokio::test]
async fn squash_range_allows_external_paths_ending_before_an_explicit_start(
	#[case] start: Option<&str>,
	#[case] end: &str,
	#[case] expected_migrations: Vec<&str>,
) {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_initial", &[]),
		migration("accounts", "0001_initial", &[("blog", "0001_initial")]),
		migration(
			"blog",
			"0002_change",
			&[("blog", "0001_initial"), ("accounts", "0001_initial")],
		),
		migration("blog", "0003_final", &[("blog", "0002_change")]),
	])
	.await;

	// Act
	let range = catalog.squash_range("blog", start, end).unwrap();

	// Assert
	let migration_names: Vec<&str> = range
		.migrations
		.iter()
		.map(|migration| migration.name.as_str())
		.collect();
	assert_eq!(migration_names, expected_migrations);
	assert_eq!(
		range.external_dependencies,
		vec![
			("accounts".to_string(), "0001_initial".to_string()),
			("blog".to_string(), "0001_initial".to_string()),
		]
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_rejects_an_explicit_hidden_branch_not_before_start() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_predecessor", &[]),
		migration("blog", "0001_competing", &[]),
		migration("accounts", "0001_bridge", &[("blog", "0001_competing")]),
		migration(
			"blog",
			"0002_change",
			&[("blog", "0001_predecessor"), ("accounts", "0001_bridge")],
		),
	])
	.await;

	// Act
	let error = catalog
		.squash_range("blog", Some("0002"), "0002")
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: Migration ancestry for blog.0002_change crosses external-app nodes: \
		 accounts.0001_bridge -> blog.0001_competing"
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_rejects_same_app_ancestry_crossing_an_external_app() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_initial", &[]),
		migration("accounts", "0001_bridge", &[("blog", "0001_initial")]),
		migration("blog", "0002_post", &[("accounts", "0001_bridge")]),
	])
	.await;

	// Act
	let error = catalog.squash_range("blog", None, "0002_post").unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: Migration ancestry for blog.0002_post crosses external-app nodes: \
		 accounts.0001_bridge -> blog.0001_initial"
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_rejects_hidden_branches_through_external_apps() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_left", &[]),
		migration("blog", "0001_right", &[]),
		migration("accounts", "0001_bridge", &[("blog", "0001_right")]),
		migration(
			"blog",
			"0002_merge",
			&[("blog", "0001_left"), ("accounts", "0001_bridge")],
		),
	])
	.await;

	// Act
	let error = catalog
		.squash_range("blog", None, "0002_merge")
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: Migration ancestry for blog.0002_merge crosses external-app nodes: \
		 accounts.0001_bridge -> blog.0001_right"
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_rejects_contraction_cycles() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_initial", &[]),
		migration("accounts", "0001_bridge", &[("blog", "0001_initial")]),
		migration(
			"blog",
			"0002_post",
			&[("blog", "0001_initial"), ("accounts", "0001_bridge")],
		),
	])
	.await;

	// Act
	let error = catalog
		.squash_range("blog", Some("0001"), "0002")
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: Cannot squash range: external dependency accounts.0001_bridge \
		 depends on selected migration blog.0001_initial"
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_rejects_branched_ancestry() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_initial", &[]),
		migration("blog", "0002_left", &[("blog", "0001_initial")]),
		migration("blog", "0002_right", &[("blog", "0001_initial")]),
		migration(
			"blog",
			"0003_merge",
			&[("blog", "0002_right"), ("blog", "0002_left")],
		),
	])
	.await;

	// Act
	let error = catalog
		.squash_range("blog", None, "0003_merge")
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: Ambiguous migration ancestry for blog.0003_merge; parents: \
		 0002_left, 0002_right"
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_rejects_an_unselected_outgoing_branch() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_initial", &[]),
		migration("blog", "0002_left", &[("blog", "0001_initial")]),
		migration("blog", "0002_right", &[("blog", "0001_initial")]),
	])
	.await;

	// Act
	let error = catalog
		.squash_range("blog", Some("0001"), "0002_left")
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: Cannot squash range: blog.0002_right branches from selected migration \
		 blog.0001_initial"
	);
}

#[rstest]
#[tokio::test]
async fn squash_range_rejects_a_start_outside_the_end_ancestry() {
	// Arrange
	let catalog = catalog(vec![
		migration("blog", "0001_initial", &[]),
		migration("blog", "0002_left", &[("blog", "0001_initial")]),
		migration("blog", "0002_right", &[("blog", "0001_initial")]),
	])
	.await;

	// Act
	let error = catalog
		.squash_range("blog", Some("0002_left"), "0002_right")
		.unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid migration: blog.0002_left is not an ancestor of blog.0002_right"
	);
}
