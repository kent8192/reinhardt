//! Admin panel test fixtures for reinhardt-admin workspace
//!
//! This module provides rstest fixtures for testing reinhardt-admin components,
//! including AdminSite, AdminDatabase, ModelAdminConfig, and server functions.
//!
//! ## Features
//!
//! These fixtures require the `admin` feature to be enabled in reinhardt-test.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use reinhardt_test::fixtures::admin_panel::admin_site;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_admin_site(#[future] admin_site: Arc<AdminSite>) {
//!     let site = admin_site.await;
//!     assert!(site.registered_models().is_empty());
//! }
//! ```

// Only compile when admin feature is enabled
#[cfg(feature = "admin")]
use {
	reinhardt_admin::core::{AdminDatabase, AdminSite, ModelAdminConfig},
	reinhardt_db::DatabaseConnection,
	rstest::*,
	std::sync::Arc,
};

// Import shared_db_pool fixture for testcontainers-based tests
#[cfg(all(feature = "admin", feature = "testcontainers"))]
use crate::fixtures::shared_postgres::shared_db_pool;

/// Fixture providing a basic AdminSite instance
///
/// This fixture creates a new AdminSite with default configuration.
/// The site starts with no registered models.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::admin_panel::admin_site;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_admin_site_registration(
///     #[future] admin_site: Arc<AdminSite>,
///     #[future] model_admin_config: ModelAdminConfig,
/// ) {
///     let site = admin_site.await;
///     let config = model_admin_config.await;
///
///     site.register("TestModel", config).unwrap();
///     assert_eq!(site.registered_models(), vec!["TestModel".to_string()]);
/// }
/// ```
#[cfg(feature = "admin")]
#[fixture]
pub async fn admin_site() -> Arc<AdminSite> {
	Arc::new(AdminSite::new("Test Admin Site"))
}

/// Fixture providing a ModelAdminConfig for testing
///
/// This fixture creates a ModelAdminConfig with typical test configuration:
/// - Model name: "TestModel"
/// - Table name: "test_models"
/// - Primary key field: "id"
/// - List display: ["id", "name", "created_at"]
/// - List filter: ["status"]
/// - Search fields: ["name", "description"]
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::admin_panel::model_admin_config;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_model_admin_config(#[future] model_admin_config: ModelAdminConfig) {
///     let config = model_admin_config.await;
///     assert_eq!(config.model_name(), "TestModel");
///     assert_eq!(config.table_name(), "test_models");
/// }
/// ```
#[cfg(feature = "admin")]
#[fixture]
pub async fn model_admin_config() -> ModelAdminConfig {
	ModelAdminConfig::builder()
		.model_name("TestModel")
		.table_name("test_models")
		.list_display(vec!["id", "name", "created_at"])
		.list_filter(vec!["status"])
		.search_fields(vec!["name", "description"])
		.build()
		.expect("model_admin_config fixture: model_name is set")
}

#[cfg(all(feature = "admin", feature = "testcontainers"))]
#[fixture]
pub async fn admin_database(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> Arc<AdminDatabase> {
	use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
	use reinhardt_db::backends::dialect::PostgresBackend;
	use std::sync::Arc as StdArc;

	let (pool, _database_name) = shared_db_pool.await;

	// Create backends connection from pool
	let backend = StdArc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);

	// Create ORM connection
	let connection = DatabaseConnection::new(
		reinhardt_db::orm::connection::DatabaseBackend::Postgres,
		backends_conn,
	);

	Arc::new(AdminDatabase::new(connection))
}

/// Fixture providing a test database with a pre-created table
///
/// This fixture creates a test table with a simple schema for testing
/// admin operations. The table has columns: id, name, status, created_at.
///
/// Returns a tuple of (PgPool, table_name) where table_name is the
/// created table's name.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::admin_panel::test_model_with_table;
/// use rstest::*;
/// use sqlx::PgPool;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_table(
///     #[future] test_model_with_table: (PgPool, String),
/// ) {
///     let (pool, table_name) = test_model_with_table.await;
///     // Use the pre-created table
/// }
/// ```
#[cfg(all(feature = "admin", feature = "testcontainers"))]
#[fixture]
pub async fn test_model_with_table(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (sqlx::PgPool, String) {
	use sqlx::Executor;

	let (pool, _database_name) = shared_db_pool.await;
	let table_name = format!("test_models_{}", uuid::Uuid::new_v4().simple());

	// Create test table
	let create_table_sql = format!(
		"CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            status VARCHAR(50) DEFAULT 'active',
            created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
            updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        )",
		table_name
	);

	pool.execute(create_table_sql.as_str())
		.await
		.expect("Failed to create test table");

	// Insert some test data
	let insert_sql = format!(
		"INSERT INTO {} (name, status) VALUES
        ('Test Item 1', 'active'),
        ('Test Item 2', 'inactive'),
        ('Test Item 3', 'active')",
		table_name
	);

	pool.execute(insert_sql.as_str())
		.await
		.expect("Failed to insert test data");

	(pool, table_name)
}

/// Fixture providing a complete server function test context
///
/// This fixture provides both AdminSite and AdminDatabase configured
/// for testing server functions. The AdminSite has a registered model,
/// and the AdminDatabase is connected to a test database with data.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::admin_panel::server_fn_test_context;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_server_function_context(
///     #[future] server_fn_test_context: (Arc<AdminSite>, Arc<AdminDatabase>),
/// ) {
///     let (site, db) = server_fn_test_context.await;
///     // Test server functions with proper DI context
/// }
/// ```
#[cfg(all(feature = "admin", feature = "testcontainers"))]
#[fixture]
pub async fn server_fn_test_context(
	#[future] admin_site: Arc<AdminSite>,
	#[future] model_admin_config: ModelAdminConfig,
	#[future] admin_database: Arc<AdminDatabase>,
) -> (Arc<AdminSite>, Arc<AdminDatabase>) {
	let site = admin_site.await;
	let config = model_admin_config.await;
	let db = admin_database.await;

	// Register the model in the site
	site.register("TestModel", config)
		.expect("Failed to register model");

	(site, db)
}

#[cfg(all(feature = "admin", feature = "testcontainers"))]
#[fixture]
pub async fn export_import_test_context(
	#[future] admin_site: Arc<AdminSite>,
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (Arc<AdminSite>, Arc<AdminDatabase>, String, sqlx::PgPool) {
	use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
	use reinhardt_db::backends::dialect::PostgresBackend;
	use reinhardt_query::prelude::{
		Alias, ColumnDef, Expr, Iden, PostgresQueryBuilder, Query, QueryStatementBuilder, Value,
	};
	use sqlx::Row;
	use std::sync::Arc as StdArc;
	use uuid::Uuid;

	let site = admin_site.await;
	let (pool, _database_name) = shared_db_pool.await;

	// Create backends connection from pool
	let backend = StdArc::new(PostgresBackend::new(pool.clone()));
	let backends_conn = BackendsConnection::new(backend);

	// Create ORM connection
	let connection = DatabaseConnection::new(
		reinhardt_db::orm::connection::DatabaseBackend::Postgres,
		backends_conn,
	);

	// Create AdminDatabase
	let db = Arc::new(AdminDatabase::new(connection));

	// Generate unique table name
	let table_name = format!("test_exports_{}", Uuid::new_v4().simple());

	// Table definition identifiers
	#[derive(Debug, Iden)]
	enum TestExportsTable {
		#[iden = "_"]
		Table,
		Id,
		Name,
		Email,
		Status,
		Age,
		Score,
		IsVerified,
		Bio,
		BirthDate,
		CreatedAt,
		Metadata,
	}

	// Create table using reinhardt-query
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(Alias::new(&table_name))
		.if_not_exists()
		.col(
			ColumnDef::new(TestExportsTable::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(TestExportsTable::Name)
				.string_len(255)
				.not_null(true),
		)
		.col(
			ColumnDef::new(TestExportsTable::Email)
				.string_len(255)
				.not_null(true),
		)
		.col(
			ColumnDef::new(TestExportsTable::Status)
				.string_len(50)
				.default("active".into()),
		)
		.col(ColumnDef::new(TestExportsTable::Age).integer())
		.col(ColumnDef::new(TestExportsTable::Score).double())
		.col(
			ColumnDef::new(TestExportsTable::IsVerified)
				.boolean()
				.default(false.into()),
		)
		.col(ColumnDef::new(TestExportsTable::Bio).text())
		.col(ColumnDef::new(TestExportsTable::BirthDate).date())
		.col(
			ColumnDef::new(TestExportsTable::CreatedAt)
				.timestamp_with_time_zone()
				.default(Expr::current_timestamp().into()),
		)
		.col(ColumnDef::new(TestExportsTable::Metadata).json_binary());

	let create_table_sql = create_stmt.to_string(PostgresQueryBuilder::new());

	sqlx::query(&create_table_sql)
		.execute(&pool)
		.await
		.expect("Failed to create test table");

	// Insert diverse test data (5 patterns)
	// Pre-create long strings for pattern 5
	let long_name = format!("Eve Martinez{}", "x".repeat(240));
	let long_bio = "Lorem ipsum dolor sit amet ".repeat(100);

	let test_records = vec![
		// Pattern 1: Standard data
		(
			"Alice Johnson",
			"alice@example.com",
			"active",
			Some(30_i32),
			Some(85.5_f64),
			true,
			Some("Software engineer with 5 years of experience"),
			Some("1994-03-15"),
			Some(r#"{"role": "admin", "department": "engineering"}"#),
		),
		// Pattern 2: NULL values
		(
			"Bob Smith",
			"bob@example.com",
			"inactive",
			None,
			None,
			false,
			None,
			None,
			None,
		),
		// Pattern 3: Special characters and Unicode
		(
			"Charlie O'Brien",
			"charlie+test@example.com",
			"pending",
			Some(25_i32),
			Some(92.7_f64),
			true,
			Some("Test with \"quotes\" and 日本語"),
			Some("1999-01-01"),
			Some(r#"{"tags": ["新規", "VIP"]}"#),
		),
		// Pattern 4: Boundary values
		(
			"David Lee",
			"david@example.com",
			"active",
			Some(0_i32),
			Some(0.0_f64),
			false,
			Some(""),
			Some("1900-01-01"),
			Some("{}"),
		),
		// Pattern 5: Maximum length edge case
		(
			long_name.as_str(),
			"eve@example.com",
			"active",
			Some(150_i32),
			Some(999.999_f64),
			true,
			Some(long_bio.as_str()),
			Some("2099-12-31"),
			Some(r#"{"nested": {"deep": {"value": 123}}}"#),
		),
	];

	for (name, email, status, age, score, is_verified, bio, birth_date, metadata) in test_records {
		let mut columns = vec![
			TestExportsTable::Name,
			TestExportsTable::Email,
			TestExportsTable::Status,
			TestExportsTable::IsVerified,
		];
		let mut values: Vec<Value> =
			vec![name.into(), email.into(), status.into(), is_verified.into()];

		if let Some(age_val) = age {
			columns.push(TestExportsTable::Age);
			values.push(age_val.into());
		}
		if let Some(score_val) = score {
			columns.push(TestExportsTable::Score);
			values.push(score_val.into());
		}
		if let Some(bio_val) = bio {
			columns.push(TestExportsTable::Bio);
			values.push(bio_val.into());
		}
		if let Some(date_val) = birth_date {
			columns.push(TestExportsTable::BirthDate);
			values.push(date_val.into());
		}
		if let Some(meta_val) = metadata {
			columns.push(TestExportsTable::Metadata);
			values.push(meta_val.into());
		}

		let mut insert_stmt = Query::insert();
		insert_stmt
			.into_table(Alias::new(&table_name))
			.columns(columns)
			.values_panic(values);

		let sql = insert_stmt.to_string(PostgresQueryBuilder::new());

		sqlx::query(&sql)
			.execute(&pool)
			.await
			.expect("Failed to insert test data");
	}

	// Verify data was inserted (debug)
	let count_sql = format!("SELECT COUNT(*) as count FROM {}", table_name);
	let count_row = sqlx::query(&count_sql)
		.fetch_one(&pool)
		.await
		.expect("Failed to count records");
	let count: i64 = count_row.try_get("count").expect("Failed to get count");
	println!(
		"[DEBUG] Inserted {} records into table {}",
		count, table_name
	);

	// モデルを再登録（テーブル名を更新）
	let config = ModelAdminConfig::builder()
		.model_name("TestModel")
		.table_name(&table_name)
		.list_display(vec![
			"id",
			"name",
			"email",
			"status",
			"age",
			"score",
			"is_verified",
		])
		.search_fields(vec!["name", "email", "bio"])
		.build()
		.expect("admin_with_database fixture: model_name is set");

	site.register("TestModel", config)
		.expect("Failed to register TestModel in AdminSite");

	(site, db, table_name, pool)
}

#[cfg(all(feature = "admin", test))]
mod tests {
	use super::*;
	use reinhardt_admin::core::ModelAdmin;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_admin_site_fixture(#[future] admin_site: Arc<AdminSite>) {
		let site = admin_site.await;
		assert_eq!(site.name(), "Test Admin Site");
		assert!(site.registered_models().is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn test_model_admin_config_fixture(#[future] model_admin_config: ModelAdminConfig) {
		let config = model_admin_config.await;
		assert_eq!(config.model_name(), "TestModel");
		assert_eq!(config.table_name(), "test_models");
		assert_eq!(config.list_display(), vec!["id", "name", "created_at"]);
	}

	// Note: Tests for database fixtures require testcontainers feature
	// and are typically run in integration tests rather than unit tests
}
