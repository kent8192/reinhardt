//! Specialized fixtures for command integration tests
//!
//! This module provides specialized fixtures that wrap reinhardt-test's generic fixtures
//! to inject test-specific data for command testing.

use reinhardt_commands::{CommandContext, MigrateCommand};
use reinhardt_db::migrations::{Migration, Operation};
use reinhardt_query::prelude::{
	Alias, ColumnDef, PostgresQueryBuilder, Query, QueryStatementBuilder, Value,
};
use reinhardt_test::fixtures::{TestMigrationSource, postgres_container};
use rstest::*;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use testcontainers::ContainerAsync;
use testcontainers::GenericImage;

// ============================================================================
// MigrateCommand Fixtures
// ============================================================================

/// Specialized fixture for MigrateCommand testing
///
/// Wraps postgres_container and provides pre-configured migrations
pub(crate) struct MigrateCommandFixture {
	/// The migrate command instance
	#[allow(dead_code)] // Kept for future command execution tests
	pub(crate) command: MigrateCommand,
	/// Command context with default settings
	pub(crate) context: CommandContext,
	/// Test migration source with sample migrations
	pub(crate) migrations: TestMigrationSource,
}

impl MigrateCommandFixture {
	/// Create a new MigrateCommandFixture
	pub(crate) fn new() -> Self {
		Self {
			command: MigrateCommand,
			context: CommandContext::default(),
			migrations: TestMigrationSource::new(),
		}
	}

	/// Add a test migration to the source
	pub(crate) fn add_migration(
		&mut self,
		app_label: &str,
		name: &str,
		operations: Vec<Operation>,
	) {
		let migration = Migration {
			app_label: app_label.to_string(),
			name: name.to_string(),
			operations,
			dependencies: vec![],
			..Default::default()
		};
		self.migrations.add_migration(migration);
	}

	/// Create a simple CreateTable migration
	pub(crate) fn add_create_table_migration(
		&mut self,
		app_label: &str,
		name: &str,
		table_name: &str,
	) {
		// Create a simple table with id and name columns using reinhardt-query
		let mut create_table_stmt = Query::create_table();
		let create_table = create_table_stmt
			.table(Alias::new(table_name))
			.col(
				ColumnDef::new(Alias::new("id"))
					.integer()
					.not_null(true)
					.auto_increment(true)
					.primary_key(true),
			)
			.col(ColumnDef::new(Alias::new("name")).string().not_null(true))
			.to_string(PostgresQueryBuilder::new());

		let operation = Operation::RunSQL {
			sql: create_table,
			reverse_sql: Some(format!("DROP TABLE IF EXISTS {}", table_name)),
		};

		self.add_migration(app_label, name, vec![operation]);
	}

	/// Set context options for fake mode
	pub(crate) fn set_fake_mode(&mut self) {
		self.context
			.set_option("fake".to_string(), "true".to_string());
	}

	/// Set context options for fake-initial mode
	pub(crate) fn set_fake_initial_mode(&mut self) {
		self.context
			.set_option("fake-initial".to_string(), "true".to_string());
	}

	/// Set the database URL in context
	#[allow(dead_code)] // May be used in future tests
	pub(crate) fn set_database_url(&mut self, url: &str) {
		self.context
			.set_option("database".to_string(), url.to_string());
	}

	/// Set app label filter in context
	pub(crate) fn set_app_label(&mut self, app_label: &str) {
		self.context.add_arg(app_label.to_string());
	}
}

impl Default for MigrateCommandFixture {
	fn default() -> Self {
		Self::new()
	}
}

/// rstest fixture for MigrateCommandFixture
#[fixture]
pub fn migrate_command_fixture() -> MigrateCommandFixture {
	MigrateCommandFixture::new()
}

/// rstest fixture for MigrateCommandFixture with sample migrations
#[fixture]
pub fn migrate_command_with_migrations() -> MigrateCommandFixture {
	let mut fixture = MigrateCommandFixture::new();
	fixture.add_create_table_migration("auth", "0001_initial", "auth_user");
	fixture.add_create_table_migration("auth", "0002_add_profile", "auth_profile");
	fixture.add_create_table_migration("posts", "0001_initial", "blog_post");
	fixture
}

// ============================================================================
// PostgreSQL with Schema Fixtures
// ============================================================================

/// PostgreSQL container with pre-created test schema for introspect tests
pub(crate) struct PostgresWithSchema {
	/// The container instance (kept alive)
	#[allow(dead_code)] // Kept alive for container lifecycle
	pub(crate) container: ContainerAsync<GenericImage>,
	/// Database connection pool
	pub(crate) pool: Arc<PgPool>,
	/// Database URL
	#[allow(dead_code)] // Available for tests that need URL
	pub(crate) url: String,
}

impl PostgresWithSchema {
	/// Create tables for introspect testing using reinhardt-query
	pub(crate) async fn create_test_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
		// Create users table
		let mut create_users_stmt = Query::create_table();
		let create_users = create_users_stmt
			.table(Alias::new("users"))
			.col(
				ColumnDef::new(Alias::new("id"))
					.integer()
					.not_null(true)
					.auto_increment(true)
					.primary_key(true),
			)
			.col(
				ColumnDef::new(Alias::new("username"))
					.string()
					.not_null(true),
			)
			.col(ColumnDef::new(Alias::new("email")).string().not_null(true))
			.col(
				ColumnDef::new(Alias::new("is_active"))
					.boolean()
					.not_null(true)
					.default(true.into()),
			)
			.col(ColumnDef::new(Alias::new("created_at")).timestamp_with_time_zone())
			.to_string(PostgresQueryBuilder::new());

		sqlx::query(&create_users).execute(pool).await?;

		// Create posts table with foreign key
		let mut create_posts_stmt = Query::create_table();
		let create_posts = create_posts_stmt
			.table(Alias::new("posts"))
			.col(
				ColumnDef::new(Alias::new("id"))
					.integer()
					.not_null(true)
					.auto_increment(true)
					.primary_key(true),
			)
			.col(ColumnDef::new(Alias::new("title")).string().not_null(true))
			.col(ColumnDef::new(Alias::new("content")).text())
			.col(
				ColumnDef::new(Alias::new("author_id"))
					.integer()
					.not_null(true),
			)
			.col(
				ColumnDef::new(Alias::new("published"))
					.boolean()
					.not_null(true)
					.default(false.into()),
			)
			.to_string(PostgresQueryBuilder::new());

		sqlx::query(&create_posts).execute(pool).await?;

		// Add foreign key constraint
		sqlx::query(
			"ALTER TABLE posts ADD CONSTRAINT fk_author FOREIGN KEY (author_id) REFERENCES users(id)",
		)
		.execute(pool)
		.await?;

		// Create comments table
		let mut create_comments_stmt = Query::create_table();
		let create_comments = create_comments_stmt
			.table(Alias::new("comments"))
			.col(
				ColumnDef::new(Alias::new("id"))
					.integer()
					.not_null(true)
					.auto_increment(true)
					.primary_key(true),
			)
			.col(
				ColumnDef::new(Alias::new("post_id"))
					.integer()
					.not_null(true),
			)
			.col(
				ColumnDef::new(Alias::new("user_id"))
					.integer()
					.not_null(true),
			)
			.col(ColumnDef::new(Alias::new("body")).text().not_null(true))
			.to_string(PostgresQueryBuilder::new());

		sqlx::query(&create_comments).execute(pool).await?;

		// Add foreign keys for comments
		sqlx::query(
			"ALTER TABLE comments ADD CONSTRAINT fk_post FOREIGN KEY (post_id) REFERENCES posts(id)",
		)
		.execute(pool)
		.await?;

		sqlx::query(
			"ALTER TABLE comments ADD CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id)",
		)
		.execute(pool)
		.await?;

		Ok(())
	}
}

/// rstest fixture for PostgresWithSchema
#[fixture]
pub async fn postgres_with_schema() -> PostgresWithSchema {
	let (container, pool, _port, url) = postgres_container().await;

	// Create test schema
	PostgresWithSchema::create_test_schema(pool.as_ref())
		.await
		.expect("Failed to create test schema");

	PostgresWithSchema {
		container,
		pool,
		url,
	}
}

// ============================================================================
// TempMigrationDir Fixtures
// ============================================================================

/// Temporary directory for migration file testing
pub(crate) struct TempMigrationDir {
	/// The temp directory (dropped when fixture goes out of scope)
	#[allow(dead_code)] // Kept for cleanup on drop
	pub(crate) dir: TempDir,
	/// Path to the migrations directory
	pub(crate) migrations_path: PathBuf,
}

impl TempMigrationDir {
	/// Create a new TempMigrationDir
	pub(crate) fn new() -> Self {
		let dir = TempDir::new().expect("Failed to create temp directory");
		let migrations_path = dir.path().join("migrations");
		std::fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

		Self {
			dir,
			migrations_path,
		}
	}

	/// Get the path as a string
	#[allow(dead_code)] // Utility method for future use
	pub(crate) fn path_str(&self) -> &str {
		self.migrations_path.to_str().unwrap_or("")
	}

	/// Create a migration file in the temp directory
	pub(crate) fn create_migration_file(
		&self,
		app_label: &str,
		name: &str,
		content: &str,
	) -> PathBuf {
		let app_dir = self.migrations_path.join(app_label);
		std::fs::create_dir_all(&app_dir).expect("Failed to create app directory");

		let file_path = app_dir.join(format!("{}.rs", name));
		std::fs::write(&file_path, content).expect("Failed to write migration file");

		file_path
	}
}

impl Default for TempMigrationDir {
	fn default() -> Self {
		Self::new()
	}
}

/// rstest fixture for TempMigrationDir
#[fixture]
pub fn temp_migration_dir() -> TempMigrationDir {
	TempMigrationDir::new()
}

// ============================================================================
// Router Fixtures
// ============================================================================

/// Router fixture with pre-registered test routes
pub(crate) struct RouterFixture {
	/// List of registered route patterns
	pub(crate) patterns: Vec<(String, String)>, // (pattern, name)
}

impl RouterFixture {
	/// Create a new RouterFixture with default test routes
	pub(crate) fn new() -> Self {
		Self {
			patterns: vec![
				("/api/users/".to_string(), "user-list".to_string()),
				("/api/users/{id}/".to_string(), "user-detail".to_string()),
				("/api/posts/".to_string(), "post-list".to_string()),
				("/api/posts/{id}/".to_string(), "post-detail".to_string()),
				("/api/comments/".to_string(), "comment-list".to_string()),
			],
		}
	}

	/// Get the number of registered patterns
	pub(crate) fn pattern_count(&self) -> usize {
		self.patterns.len()
	}

	/// Check if a pattern exists
	pub(crate) fn has_pattern(&self, pattern: &str) -> bool {
		self.patterns.iter().any(|(p, _)| p == pattern)
	}

	/// Check if a named route exists
	pub(crate) fn has_named_route(&self, name: &str) -> bool {
		self.patterns.iter().any(|(_, n)| n == name)
	}
}

impl Default for RouterFixture {
	fn default() -> Self {
		Self::new()
	}
}

/// rstest fixture for RouterFixture
#[fixture]
pub fn router_fixture() -> RouterFixture {
	RouterFixture::new()
}

// ============================================================================
// Mock Crates.io Client Fixtures
// ============================================================================

/// Mock crates.io client for plugin command testing
pub(crate) struct MockCratesIoClient {
	/// Available packages (name -> version)
	pub(crate) packages: std::collections::HashMap<String, CrateInfo>,
}

/// Information about a crate
#[derive(Clone, Debug)]
pub(crate) struct CrateInfo {
	/// Crate name
	pub(crate) name: String,
	/// Latest version
	#[allow(dead_code)] // Available for version comparison tests
	pub(crate) version: String,
	/// Description
	pub(crate) description: String,
	/// Available versions
	#[allow(dead_code)] // Available for version listing tests
	pub(crate) versions: Vec<String>,
}

impl MockCratesIoClient {
	/// Create a new empty MockCratesIoClient
	pub(crate) fn new() -> Self {
		Self {
			packages: std::collections::HashMap::new(),
		}
	}

	/// Create with pre-configured packages
	pub(crate) fn with_packages(packages: Vec<(&str, &str, &str)>) -> Self {
		let mut client = Self::new();
		for (name, version, description) in packages {
			client.add_package(name, version, description);
		}
		client
	}

	/// Add a package to the mock
	pub(crate) fn add_package(&mut self, name: &str, version: &str, description: &str) {
		self.packages.insert(
			name.to_string(),
			CrateInfo {
				name: name.to_string(),
				version: version.to_string(),
				description: description.to_string(),
				versions: vec![version.to_string()],
			},
		);
	}

	/// Search for packages matching a query
	pub(crate) fn search(&self, query: &str) -> Vec<&CrateInfo> {
		self.packages
			.values()
			.filter(|c| c.name.contains(query) || c.description.contains(query))
			.collect()
	}

	/// Get a specific crate by name
	pub(crate) fn get_crate(&self, name: &str) -> Option<&CrateInfo> {
		self.packages.get(name)
	}
}

impl Default for MockCratesIoClient {
	fn default() -> Self {
		Self::with_packages(vec![
			(
				"reinhardt-delion-auth",
				"0.1.0",
				"Authentication plugin for Reinhardt",
			),
			(
				"reinhardt-delion-cache",
				"0.2.0",
				"Cache plugin for Reinhardt",
			),
			(
				"reinhardt-delion-storage",
				"0.1.5",
				"Storage plugin for Reinhardt",
			),
		])
	}
}

/// rstest fixture for MockCratesIoClient
#[fixture]
pub fn mock_crates_io_client() -> MockCratesIoClient {
	MockCratesIoClient::default()
}

// ============================================================================
// Plugin Manifest Fixtures
// ============================================================================

/// Plugin manifest fixture for plugin command testing
pub(crate) struct PluginManifestFixture {
	/// Temp directory containing the manifest
	#[allow(dead_code)] // Kept for cleanup on drop
	pub(crate) dir: TempDir,
	/// Path to the manifest file
	pub(crate) manifest_path: PathBuf,
	/// List of plugins in the manifest
	pub(crate) plugins: Vec<PluginEntry>,
}

/// Entry for a plugin in the manifest
#[derive(Clone, Debug)]
pub(crate) struct PluginEntry {
	/// Plugin name
	pub(crate) name: String,
	/// Plugin version
	pub(crate) version: String,
	/// Whether the plugin is enabled
	pub(crate) enabled: bool,
}

impl PluginManifestFixture {
	/// Create a new empty PluginManifestFixture
	pub(crate) fn new() -> Self {
		let dir = TempDir::new().expect("Failed to create temp directory");
		let manifest_path = dir.path().join("plugins.toml");

		let fixture = Self {
			dir,
			manifest_path,
			plugins: vec![],
		};

		// Create empty manifest file for consistency with with_plugins()
		fixture.write_manifest();

		fixture
	}

	/// Create with pre-configured plugins
	pub(crate) fn with_plugins(plugins: Vec<(&str, &str, bool)>) -> Self {
		let mut fixture = Self::new();
		for (name, version, enabled) in plugins {
			fixture.add_plugin(name, version, enabled);
		}
		fixture.write_manifest();
		fixture
	}

	/// Add a plugin to the manifest
	pub(crate) fn add_plugin(&mut self, name: &str, version: &str, enabled: bool) {
		self.plugins.push(PluginEntry {
			name: name.to_string(),
			version: version.to_string(),
			enabled,
		});
	}

	/// Write the manifest to disk
	pub(crate) fn write_manifest(&self) {
		let content = self
			.plugins
			.iter()
			.map(|p| {
				format!(
					"[[plugins]]\nname = \"{}\"\nversion = \"{}\"\nenabled = {}\n",
					p.name, p.version, p.enabled
				)
			})
			.collect::<Vec<_>>()
			.join("\n");

		std::fs::write(&self.manifest_path, content).expect("Failed to write manifest");
	}

	/// Enable a plugin by name
	pub(crate) fn enable_plugin(&mut self, name: &str) {
		if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name == name) {
			plugin.enabled = true;
		}
		self.write_manifest();
	}

	/// Disable a plugin by name
	pub(crate) fn disable_plugin(&mut self, name: &str) {
		if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name == name) {
			plugin.enabled = false;
		}
		self.write_manifest();
	}

	/// Get the number of enabled plugins
	#[allow(dead_code)] // Utility method for enabled plugin counting
	pub(crate) fn enabled_count(&self) -> usize {
		self.plugins.iter().filter(|p| p.enabled).count()
	}

	/// Get a plugin by name
	pub(crate) fn get_plugin(&self, name: &str) -> Option<&PluginEntry> {
		self.plugins.iter().find(|p| p.name == name)
	}

	/// Remove a plugin by name
	pub(crate) fn remove_plugin(&mut self, name: &str) {
		self.plugins.retain(|p| p.name != name);
		self.write_manifest();
	}
}

impl Default for PluginManifestFixture {
	fn default() -> Self {
		Self::with_plugins(vec![
			("reinhardt-delion-auth", "0.1.0", true),
			("reinhardt-delion-cache", "0.2.0", false),
		])
	}
}

/// rstest fixture for PluginManifestFixture (empty, just manifest directory)
#[fixture]
pub fn plugin_manifest_fixture() -> PluginManifestFixture {
	PluginManifestFixture::new()
}

/// rstest fixture for PluginManifestFixture with pre-configured plugins
#[fixture]
pub fn plugin_manifest_with_plugins() -> PluginManifestFixture {
	PluginManifestFixture::with_plugins(vec![
		("reinhardt-auth-delion", "0.1.0", true),
		("reinhardt-admin-delion", "0.2.0", true),
		("reinhardt-rest-delion", "0.1.5", true),
	])
}

// ============================================================================
// Test Data Injection Helpers
// ============================================================================

/// Helper to insert test data into a PostgreSQL database using reinhardt-query
#[allow(dead_code)] // May be used in future tests
pub(crate) async fn insert_test_users(pool: &PgPool) -> Result<Vec<i32>, sqlx::Error> {
	let mut user_ids = vec![];

	// Insert test users
	let users = [
		("alice", "alice@example.com"),
		("bob", "bob@example.com"),
		("charlie", "charlie@example.com"),
	];

	for (username, email) in users {
		let mut insert_stmt = Query::insert();
		let insert = insert_stmt
			.into_table(Alias::new("users"))
			.columns([Alias::new("username"), Alias::new("email")])
			.values_panic([Value::from(username), Value::from(email)])
			.returning_col(Alias::new("id"))
			.to_string(PostgresQueryBuilder::new());

		let row: (i32,) = sqlx::query_as(&insert).fetch_one(pool).await?;
		user_ids.push(row.0);
	}

	Ok(user_ids)
}

/// Helper to insert test posts into a PostgreSQL database using reinhardt-query
#[allow(dead_code)] // May be used in future tests
pub(crate) async fn insert_test_posts(
	pool: &PgPool,
	author_ids: &[i32],
) -> Result<Vec<i32>, sqlx::Error> {
	let mut post_ids = vec![];

	let posts = [
		("First Post", "Content of first post", true),
		("Second Post", "Content of second post", false),
		("Third Post", "Content of third post", true),
	];

	for (i, (title, content, published)) in posts.iter().enumerate() {
		let author_id = author_ids.get(i % author_ids.len()).copied().unwrap_or(1);

		let mut insert_stmt = Query::insert();
		let insert = insert_stmt
			.into_table(Alias::new("posts"))
			.columns([
				Alias::new("title"),
				Alias::new("content"),
				Alias::new("author_id"),
				Alias::new("published"),
			])
			.values_panic([
				Value::from(*title),
				Value::from(*content),
				Value::from(author_id),
				Value::from(*published),
			])
			.returning_col(Alias::new("id"))
			.to_string(PostgresQueryBuilder::new());

		let row: (i32,) = sqlx::query_as(&insert).fetch_one(pool).await?;
		post_ids.push(row.0);
	}

	Ok(post_ids)
}

// ============================================================================
// Migration Edge Case Fixtures
// ============================================================================

/// Creates migrations with Unicode identifiers (Japanese, emoji)
///
/// This fixture tests the system's ability to handle non-ASCII identifiers
/// in migration names and table/column names, including Japanese characters
/// and emoji. All identifiers are properly quoted for PostgreSQL.
#[allow(dead_code)]
pub(crate) fn create_unicode_migrations() -> Vec<Migration> {
	vec![
		// Japanese table name migration
		Migration {
			app_label: "unicode".to_string(),
			name: "0001_japanese_table".to_string(),
			operations: vec![Operation::RunSQL {
				// Create table with Japanese name using quoted identifier
				sql: Query::create_table()
					.table(Alias::new("\"ユーザー\""))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(
						ColumnDef::new(Alias::new("\"名前\""))
							.string()
							.not_null(true),
					)
					.col(
						ColumnDef::new(Alias::new("\"メール\""))
							.string()
							.not_null(true),
					)
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS \"ユーザー\"".to_string()),
			}],
			dependencies: vec![],
			..Default::default()
		},
		// Emoji column name migration
		Migration {
			app_label: "unicode".to_string(),
			name: "0002_emoji_columns".to_string(),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new("emoji_table"))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(ColumnDef::new(Alias::new("\"😀\"")).string())
					.col(ColumnDef::new(Alias::new("\"🚀\"")).string())
					.col(ColumnDef::new(Alias::new("\"💡\"")).text())
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS emoji_table".to_string()),
			}],
			dependencies: vec![("unicode".to_string(), "0001_japanese_table".to_string())],
			..Default::default()
		},
		// Mixed Unicode identifiers
		Migration {
			app_label: "unicode".to_string(),
			name: "0003_mixed_unicode".to_string(),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new("\"製品\""))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(ColumnDef::new(Alias::new("\"製品名\"")).string().not_null(true))
					.col(ColumnDef::new(Alias::new("\"価格\"")).integer().not_null(true))
					.col(ColumnDef::new(Alias::new("\"🏷️\"")).string()) // emoji tag
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS \"製品\"".to_string()),
			}],
			dependencies: vec![("unicode".to_string(), "0002_emoji_columns".to_string())],
			..Default::default()
		},
	]
}

/// Creates migrations for numbering overflow test (9998, 9999, 10000)
///
/// This fixture tests the 4-to-5 digit transition in migration numbering,
/// which can expose sorting and ordering issues when dealing with
/// numeric migration identifiers.
#[allow(dead_code)]
pub(crate) fn create_overflow_numbered_migrations() -> Vec<Migration> {
	vec![
		// Migration 9998
		Migration {
			app_label: "overflow".to_string(),
			name: "9998_create_users".to_string(),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new("overflow_users"))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(
						ColumnDef::new(Alias::new("username"))
							.string()
							.not_null(true),
					)
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS overflow_users".to_string()),
			}],
			dependencies: vec![],
			..Default::default()
		},
		// Migration 9999
		Migration {
			app_label: "overflow".to_string(),
			name: "9999_create_posts".to_string(),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new("overflow_posts"))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(ColumnDef::new(Alias::new("title")).string().not_null(true))
					.col(
						ColumnDef::new(Alias::new("user_id"))
							.integer()
							.not_null(true),
					)
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS overflow_posts".to_string()),
			}],
			dependencies: vec![("overflow".to_string(), "9998_create_users".to_string())],
			..Default::default()
		},
		// Migration 10000 (5 digits - tests overflow)
		Migration {
			app_label: "overflow".to_string(),
			name: "10000_create_comments".to_string(),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new("overflow_comments"))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(
						ColumnDef::new(Alias::new("post_id"))
							.integer()
							.not_null(true),
					)
					.col(ColumnDef::new(Alias::new("body")).text().not_null(true))
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS overflow_comments".to_string()),
			}],
			dependencies: vec![("overflow".to_string(), "9999_create_posts".to_string())],
			..Default::default()
		},
	]
}

/// Creates circular dependency chain A → B → C → A
///
/// This fixture creates migrations with intentional circular dependencies
/// to test how the system detects and handles dependency cycles.
/// Circular dependencies should be detected and reported as errors.
#[allow(dead_code)]
pub(crate) fn create_circular_dependency_migrations() -> Vec<Migration> {
	vec![
		// Migration A depends on C
		Migration {
			app_label: "circular".to_string(),
			name: "0001_migration_a".to_string(),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new("table_a"))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(ColumnDef::new(Alias::new("name")).string())
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS table_a".to_string()),
			}],
			dependencies: vec![("circular".to_string(), "0003_migration_c".to_string())],
			..Default::default()
		},
		// Migration B depends on A
		Migration {
			app_label: "circular".to_string(),
			name: "0002_migration_b".to_string(),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new("table_b"))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(ColumnDef::new(Alias::new("a_id")).integer())
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS table_b".to_string()),
			}],
			dependencies: vec![("circular".to_string(), "0001_migration_a".to_string())],
			..Default::default()
		},
		// Migration C depends on B (completing the cycle)
		Migration {
			app_label: "circular".to_string(),
			name: "0003_migration_c".to_string(),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new("table_c"))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(ColumnDef::new(Alias::new("b_id")).integer())
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some("DROP TABLE IF EXISTS table_c".to_string()),
			}],
			dependencies: vec![("circular".to_string(), "0002_migration_b".to_string())],
			..Default::default()
		},
	]
}

/// Creates a large set of migrations for scale testing
///
/// This fixture generates N migrations with sequential dependencies,
/// useful for testing performance and behavior with large migration sets.
/// Each migration creates a simple table and depends on the previous one.
#[allow(dead_code)]
pub(crate) fn create_large_migration_set(count: usize) -> Vec<Migration> {
	let mut migrations = Vec::with_capacity(count);

	for i in 0..count {
		let padded_num = format!("{:04}", i + 1);
		let table_name = format!("large_scale_table_{}", i + 1);

		let mut deps: Vec<(String, String)> = vec![];
		if i > 0 {
			let prev_num = format!("{:04}", i);
			deps.push(("large".to_string(), format!("{}_initial", prev_num)));
		}

		migrations.push(Migration {
			app_label: "large".to_string(),
			name: format!("{}_initial", padded_num),
			operations: vec![Operation::RunSQL {
				sql: Query::create_table()
					.table(Alias::new(&table_name))
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null(true)
							.auto_increment(true)
							.primary_key(true),
					)
					.col(ColumnDef::new(Alias::new("data")).text())
					.to_string(PostgresQueryBuilder::new()),
				reverse_sql: Some(format!("DROP TABLE IF EXISTS {}", table_name)),
			}],
			dependencies: deps,
			..Default::default()
		});
	}

	migrations
}

/// Creates a corrupted migration history table state
///
/// This fixture intentionally creates an inconsistent reinhardt_migrations table
/// to test the system's ability to detect and recover from corruption scenarios.
/// Corruption types include: wrong data types, duplicate entries, and NULL values
/// in required columns.
#[allow(dead_code)]
pub(crate) async fn create_corrupted_migration_history(pool: &PgPool) -> Result<(), sqlx::Error> {
	// Create the migrations table with standard structure first
	let create_table = Query::create_table()
		.table(Alias::new("reinhardt_migrations"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("app_label"))
				.string()
				.not_null(true),
		)
		.col(ColumnDef::new(Alias::new("name")).string().not_null(true))
		.col(ColumnDef::new(Alias::new("applied")).timestamp_with_time_zone())
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&create_table).execute(pool).await?;

	// Insert valid entry
	sqlx::query("INSERT INTO reinhardt_migrations (app_label, name, applied) VALUES ($1, $2, $3)")
		.bind("valid_app")
		.bind("0001_valid")
		.bind(chrono::Utc::now())
		.execute(pool)
		.await?;

	// Insert duplicate entry (same app_label and name)
	sqlx::query("INSERT INTO reinhardt_migrations (app_label, name, applied) VALUES ($1, $2, $3)")
		.bind("valid_app")
		.bind("0001_valid")
		.bind(chrono::Utc::now())
		.execute(pool)
		.await?;

	// Insert entry with NULL in applied column (should be set but testing corruption)
	sqlx::query("INSERT INTO reinhardt_migrations (app_label, name, applied) VALUES ($1, $2, NULL")
		.bind("corrupted_app")
		.bind("0002_null_applied")
		.execute(pool)
		.await?;

	// Insert entry with empty name (testing edge case)
	sqlx::query("INSERT INTO reinhardt_migrations (app_label, name, applied) VALUES ($1, $2, $3)")
		.bind("edge_case")
		.bind("")
		.bind(chrono::Utc::now())
		.execute(pool)
		.await?;

	Ok(())
}
