//! Test fixtures for DML integration tests

// ============================================================================
// Models
// ============================================================================

use reinhardt_macros::model;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

// Model structs are used by the `#[model]` macro to generate SQL-related code.
// They appear unused because the macro expansion happens at compile time.
#[allow(dead_code)]
#[model(table_name = "users")]
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Users {
	#[field(primary_key = true)]
	pub id: i32,

	#[field(max_length = 255)]
	pub name: String,

	#[field(max_length = 255, unique = true)]
	pub email: String,

	pub age: Option<i32>,

	#[field(default = true)]
	pub active: bool,
}

// Model structs are used by the `#[model]` macro to generate SQL-related code.
// They appear unused because the macro expansion happens at compile time.
#[allow(dead_code)]
#[model(table_name = "products")]
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Products {
	#[field(primary_key = true)]
	pub id: i32,

	#[field(max_length = 255)]
	pub name: String,

	#[field(max_length = 100)]
	pub sku: String,

	pub price: i64,

	pub stock: i32,

	#[field(default = true)]
	pub available: bool,
}

// Model structs are used by the `#[model]` macro to generate SQL-related code.
// They appear unused because the macro expansion happens at compile time.
#[allow(dead_code)]
#[model(table_name = "orders")]
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Orders {
	#[field(primary_key = true)]
	pub id: i32,

	pub user_id: i32,

	pub total_amount: i64,

	#[field(max_length = 50)]
	pub status: String,
}

// ============================================================================
// Database Fixtures
// ============================================================================

use rstest::*;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage, ImageExt, core::WaitFor, runners::AsyncRunner};

/// Inline PostgreSQL container fixture (replaces reinhardt-test dependency)
#[fixture]
pub async fn postgres_container() -> (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String)
{
	use testcontainers::core::IntoContainerPort;

	let image = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust");

	let postgres = image
		.start()
		.await
		.expect("Failed to start PostgreSQL container");

	// Wait before first port query to ensure container networking is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	// Retry getting port with exponential backoff
	let mut port_retry = 0;
	let max_port_retries = 7;
	let port = loop {
		match postgres.get_host_port_ipv4(5432).await {
			Ok(p) => break p,
			Err(_e) if port_retry < max_port_retries => {
				port_retry += 1;
				let delay = tokio::time::Duration::from_millis(200 * 2_u64.pow(port_retry));
				tokio::time::sleep(delay).await;
			}
			Err(e) => {
				panic!(
					"Failed to get PostgreSQL port after {} retries: {}",
					max_port_retries, e
				);
			}
		}
	};

	let database_url = format!(
		"postgres://postgres@localhost:{}/postgres?sslmode=disable",
		port
	);

	// Wait before first connection to ensure container is fully ready
	tokio::time::sleep(std::time::Duration::from_millis(500)).await;

	let mut retry_count = 0;
	let max_retries = 7;
	let pool = loop {
		match sqlx::postgres::PgPoolOptions::new()
			.max_connections(5)
			.min_connections(1)
			.acquire_timeout(std::time::Duration::from_secs(60))
			.idle_timeout(std::time::Duration::from_secs(600))
			.max_lifetime(std::time::Duration::from_secs(1800))
			.test_before_acquire(false)
			.connect(&database_url)
			.await
		{
			Ok(pool) => match sqlx::query("SELECT 1").fetch_one(&pool).await {
				Ok(_) => break pool,
				Err(_e) if retry_count < max_retries => {
					retry_count += 1;
					let delay = std::time::Duration::from_millis(200 * 2_u64.pow(retry_count));
					tokio::time::sleep(delay).await;
					continue;
				}
				Err(e) => {
					panic!(
						"PostgreSQL health check failed after {} retries: {}",
						max_retries, e
					);
				}
			},
			Err(_e) if retry_count < max_retries => {
				retry_count += 1;
				let delay = std::time::Duration::from_millis(200 * 2_u64.pow(retry_count));
				tokio::time::sleep(delay).await;
			}
			Err(e) => {
				panic!(
					"Failed to connect to PostgreSQL after {} retries: {}",
					max_retries, e
				);
			}
		}
	};

	(postgres, Arc::new(pool), port, database_url)
}

/// Wrapper that holds both the PostgreSQL container and connection pool.
///
/// The `ContainerAsync` must outlive the pool; dropping it destroys the
/// Docker container and makes every connection fail with `PoolTimedOut`.
pub(crate) struct TestPool {
	#[allow(dead_code)] // Container kept alive to prevent PoolTimedOut
	_container: ContainerAsync<GenericImage>,
	pool: Arc<PgPool>,
}

impl AsRef<PgPool> for TestPool {
	fn as_ref(&self) -> &PgPool {
		self.pool.as_ref()
	}
}

impl std::ops::Deref for TestPool {
	type Target = PgPool;

	fn deref(&self) -> &PgPool {
		self.pool.as_ref()
	}
}

/// PostgreSQL database pool fixture
#[fixture]
pub async fn pg_pool(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> TestPool {
	let (container, pool, _port, _url) = postgres_container.await;
	TestPool {
		_container: container,
		pool,
	}
}

/// Empty database pool fixture (alias for pg_pool)
#[fixture]
pub async fn empty_db(#[future] pg_pool: TestPool) -> TestPool {
	pg_pool.await
}

// ============================================================================
// Table Fixtures
// ============================================================================

use reinhardt_query::prelude::{
	ColumnDef, ForeignKeyAction, PostgresQueryBuilder, Query, QueryStatementBuilder,
};

/// Users table fixture
#[fixture]
pub async fn users_table(#[future] pg_pool: TestPool) -> TestPool {
	let pool = pg_pool.await;

	let mut create_table = Query::create_table();
	create_table
		.table("users")
		.if_not_exists()
		.col(
			ColumnDef::new("id")
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new("name").string_len(255).not_null(true))
		.col(
			ColumnDef::new("email")
				.string_len(255)
				.not_null(true)
				.unique(true),
		)
		.col(ColumnDef::new("age").integer())
		.col(ColumnDef::new("active").boolean().default(true.into()));

	let sql = create_table.to_string(PostgresQueryBuilder::new());
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create users table");

	pool
}

/// Products table fixture
#[fixture]
pub async fn products_table(#[future] pg_pool: TestPool) -> TestPool {
	let pool = pg_pool.await;

	let mut create_table = Query::create_table();
	create_table
		.table("products")
		.if_not_exists()
		.col(
			ColumnDef::new("id")
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new("name").string_len(255).not_null(true))
		.col(
			ColumnDef::new("sku")
				.string_len(100)
				.not_null(true)
				.unique(true),
		)
		.col(ColumnDef::new("price").big_integer().not_null(true))
		.col(ColumnDef::new("stock").integer().not_null(true))
		.col(ColumnDef::new("available").boolean().default(true.into()));

	let sql = create_table.to_string(PostgresQueryBuilder::new());
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create products table");

	pool
}

/// Orders table fixture (with foreign key to users)
#[fixture]
pub async fn orders_table(#[future] users_table: TestPool) -> TestPool {
	let pool = users_table.await;

	let mut create_table = Query::create_table();
	create_table
		.table("orders")
		.if_not_exists()
		.col(
			ColumnDef::new("id")
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new("user_id").integer().not_null(true))
		.col(ColumnDef::new("total_amount").big_integer().not_null(true))
		.col(ColumnDef::new("status").string_len(50).not_null(true))
		.foreign_key(
			vec!["user_id"],
			"users",
			vec!["id"],
			Some(ForeignKeyAction::Cascade),
			Some(ForeignKeyAction::Cascade),
		);

	let sql = create_table.to_string(PostgresQueryBuilder::new());
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create orders table");

	pool
}

/// Users table with sample data fixture
#[fixture]
pub async fn users_with_data(#[future] users_table: TestPool) -> (TestPool, Vec<i32>) {
	let pool = users_table.await;

	// Insert sample users using SQL
	let id1: i32 = sqlx::query(
		"INSERT INTO users (name, email, age, active) VALUES ($1, $2, $3, $4) RETURNING id",
	)
	.bind("Alice")
	.bind("alice@example.com")
	.bind(30i32)
	.bind(true)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to insert user1")
	.get("id");

	let id2: i32 = sqlx::query(
		"INSERT INTO users (name, email, age, active) VALUES ($1, $2, $3, $4) RETURNING id",
	)
	.bind("Bob")
	.bind("bob@example.com")
	.bind(25i32)
	.bind(true)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to insert user2")
	.get("id");

	let id3: i32 = sqlx::query(
		"INSERT INTO users (name, email, age, active) VALUES ($1, $2, $3, $4) RETURNING id",
	)
	.bind("Charlie")
	.bind("charlie@example.com")
	.bind(35i32)
	.bind(true)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to insert user3")
	.get("id");

	(pool, vec![id1, id2, id3])
}

/// Products table with sample data fixture
#[fixture]
pub async fn products_with_data(#[future] products_table: TestPool) -> (TestPool, Vec<i32>) {
	let pool = products_table.await;

	// Insert sample products using SQL
	let id1: i32 = sqlx::query("INSERT INTO products (name, sku, price, stock, available) VALUES ($1, $2, $3, $4, $5) RETURNING id")
		.bind("Laptop")
		.bind("SKU-001")
		.bind(100000i64)
		.bind(10i32)
		.bind(true)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert product1")
		.get("id");

	let id2: i32 = sqlx::query("INSERT INTO products (name, sku, price, stock, available) VALUES ($1, $2, $3, $4, $5) RETURNING id")
		.bind("Mouse")
		.bind("SKU-002")
		.bind(2000i64)
		.bind(50i32)
		.bind(true)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert product2")
		.get("id");

	let id3: i32 = sqlx::query("INSERT INTO products (name, sku, price, stock, available) VALUES ($1, $2, $3, $4, $5) RETURNING id")
		.bind("Keyboard")
		.bind("SKU-003")
		.bind(5000i64)
		.bind(30i32)
		.bind(true)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert product3")
		.get("id");

	(pool, vec![id1, id2, id3])
}
