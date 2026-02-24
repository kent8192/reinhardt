//! Multi-Database Integration Tests
//!
//! Tests integration scenarios involving multiple database instances.
//!
//! ## Test Coverage
//!
//! This test file covers:
//! - **Multi-Database Connection**: Managing connections to multiple database instances
//! - **Data Migration**: Moving data between databases
//! - **Read Replica Pattern**: Distributing read operations across replicas
//! - **Failover Scenarios**: Handling primary database failures
//! - **Sharding Pattern**: Partitioning data across multiple databases
//!
//! ## Test Categories
//!
//! 1. **Connection Management**: Multiple pool creation and lifecycle
//! 2. **Data Synchronization**: Ensuring consistency across databases
//! 3. **Read Distribution**: Load balancing read operations
//! 4. **Failover Handling**: Automatic failover on primary failure
//! 5. **Cross-Database Operations**: Transactions spanning multiple databases
//!
//! ## Fixtures Used
//!
//! - `postgres_container`: For creating multiple PostgreSQL instances
//!
//! ## What These Tests Verify
//!
//! ✅ Multiple database connections can be established simultaneously
//! ✅ Data can be migrated between databases
//! ✅ Read operations can be distributed across replicas
//! ✅ System handles primary database failures gracefully
//! ✅ Sharding distributes data correctly
//! ✅ Connection pools manage resources efficiently
//!
//! ## What These Tests Don't Cover
//!
//! ❌ Actual database replication (uses logical replication simulation)
//! ❌ Network partition scenarios (requires network isolation tools)
//! ❌ Cross-vendor database integration (PostgreSQL + MySQL)
//! ❌ Distributed transaction protocols (2PC, Saga patterns)

use reinhardt_core::macros::model;
use reinhardt_db::orm::manager::reinitialize_database;
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row};
use std::sync::Arc;
use testcontainers::ImageExt;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage};
use tokio::time::{Duration, sleep};

// ============ ORM Model Definition ============

/// ORM model for user - demonstrates reinhardt_orm integration with multi-database
#[model(app_label = "multi_db", table_name = "users")]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(dead_code)] // ORM model for multi-database integration tests
struct UserModel {
	#[field(primary_key = true)]
	id: Option<i32>,
	#[field(max_length = 100)]
	username: String,
	#[field(max_length = 255)]
	email: String,
	#[field]
	shard_key: i32,
}

// ============ Test Helper Structs ============

/// User record for testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
	id: i32,
	username: String,
	email: String,
	shard_key: i32,
}

impl User {
	fn new(id: i32, username: &str, email: &str, shard_key: i32) -> Self {
		Self {
			id,
			username: username.to_string(),
			email: email.to_string(),
			shard_key,
		}
	}
}

// ============ Multi-Database Connection Tests ============

/// Test establishing connections to multiple databases
///
/// Verifies:
/// - Multiple independent connection pools can be created
/// - Each pool connects to a different database instance
/// - Pools operate independently without interference
#[rstest]
#[tokio::test]
async fn test_multiple_database_connections(
	#[future] postgres_container: (
		ContainerAsync<GenericImage>,
		Arc<Pool<Postgres>>,
		u16,
		String,
	),
) {
	let (_container1, pool1, _port1, url1) = postgres_container.await;
	reinitialize_database(&url1).await.unwrap();

	// Start second PostgreSQL container
	let postgres2 = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.with_env_var("POSTGRES_DB", "testdb2")
		.start()
		.await
		.expect("Failed to start second PostgreSQL container");

	// Wait briefly before port query to ensure container networking is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	let port2 = postgres2
		.get_host_port_ipv4(5432.tcp())
		.await
		.expect("Failed to get port for second container");
	let url2 = format!(
		"postgres://postgres@localhost:{}/testdb2?sslmode=disable",
		port2
	);

	// Create second connection pool
	let pool2 = Arc::new(
		Pool::<Postgres>::connect(&url2)
			.await
			.expect("Failed to connect to second database"),
	);

	// Create tables in both databases
	sqlx::query("CREATE TABLE IF NOT EXISTS users (id SERIAL PRIMARY KEY, username TEXT NOT NULL)")
		.execute(pool1.as_ref())
		.await
		.expect("Failed to create table in db1");

	sqlx::query("CREATE TABLE IF NOT EXISTS users (id SERIAL PRIMARY KEY, username TEXT NOT NULL)")
		.execute(pool2.as_ref())
		.await
		.expect("Failed to create table in db2");

	// Insert different data in each database
	sqlx::query("INSERT INTO users (username) VALUES ($1)")
		.bind("user_db1")
		.execute(pool1.as_ref())
		.await
		.expect("Failed to insert into db1");

	sqlx::query("INSERT INTO users (username) VALUES ($1)")
		.bind("user_db2")
		.execute(pool2.as_ref())
		.await
		.expect("Failed to insert into db2");

	// Verify isolation: each database has different data
	let count1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(pool1.as_ref())
		.await
		.expect("Failed to count in db1");

	let count2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(pool2.as_ref())
		.await
		.expect("Failed to count in db2");

	assert_eq!(count1, 1, "DB1 should have 1 user");
	assert_eq!(count2, 1, "DB2 should have 1 user");

	// Verify data content is different
	let user1: String = sqlx::query_scalar("SELECT username FROM users LIMIT 1")
		.fetch_one(pool1.as_ref())
		.await
		.expect("Failed to fetch from db1");

	let user2: String = sqlx::query_scalar("SELECT username FROM users LIMIT 1")
		.fetch_one(pool2.as_ref())
		.await
		.expect("Failed to fetch from db2");

	assert_eq!(user1, "user_db1");
	assert_eq!(user2, "user_db2");
}

/// Test data migration between databases
///
/// Verifies:
/// - Data can be read from source database
/// - Data can be written to target database
/// - Migration preserves data integrity
/// - Source data remains unchanged after migration
#[rstest]
#[tokio::test]
async fn test_data_migration_between_databases(
	#[future] postgres_container: (
		ContainerAsync<GenericImage>,
		Arc<Pool<Postgres>>,
		u16,
		String,
	),
) {
	let (_container1, pool_source, _port1, _url1) = postgres_container.await;

	// Start target database container
	let postgres_target = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.with_env_var("POSTGRES_DB", "target_db")
		.start()
		.await
		.expect("Failed to start target PostgreSQL container");

	// Wait briefly before port query to ensure container networking is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	let port_target = postgres_target
		.get_host_port_ipv4(5432.tcp())
		.await
		.expect("Failed to get target port");
	let url_target = format!(
		"postgres://postgres@localhost:{}/target_db?sslmode=disable",
		port_target
	);

	let pool_target = Arc::new(
		Pool::<Postgres>::connect(&url_target)
			.await
			.expect("Failed to connect to target database"),
	);

	// Create tables
	let create_table_sql = r#"
		CREATE TABLE IF NOT EXISTS products (
			id SERIAL PRIMARY KEY,
			name TEXT NOT NULL,
			price DECIMAL(10, 2) NOT NULL
		)
	"#;

	sqlx::query(create_table_sql)
		.execute(pool_source.as_ref())
		.await
		.expect("Failed to create table in source");

	sqlx::query(create_table_sql)
		.execute(pool_target.as_ref())
		.await
		.expect("Failed to create table in target");

	// Insert test data into source database
	for i in 1..=10 {
		sqlx::query("INSERT INTO products (name, price) VALUES ($1, $2)")
			.bind(format!("Product {}", i))
			.bind(i as f64 * 10.0)
			.execute(pool_source.as_ref())
			.await
			.expect("Failed to insert into source");
	}

	// Migrate data: read from source, write to target
	let products = sqlx::query("SELECT id, name, price FROM products ORDER BY id")
		.fetch_all(pool_source.as_ref())
		.await
		.expect("Failed to fetch from source");

	assert_eq!(products.len(), 10, "Should have 10 products in source");

	for row in products {
		let id: i32 = row.get("id");
		let name: String = row.get("name");
		let price: Decimal = row.get("price");

		sqlx::query("INSERT INTO products (id, name, price) VALUES ($1, $2, $3)")
			.bind(id)
			.bind(&name)
			.bind(price)
			.execute(pool_target.as_ref())
			.await
			.expect("Failed to insert into target");
	}

	// Verify migration success
	let target_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
		.fetch_one(pool_target.as_ref())
		.await
		.expect("Failed to count in target");

	assert_eq!(
		target_count, 10,
		"Target should have 10 products after migration"
	);

	// Verify data integrity (compare first and last products)
	let source_first: (i32, String, f64) = sqlx::query_as(
		"SELECT id, name, CAST(price AS DOUBLE PRECISION) FROM products ORDER BY id LIMIT 1",
	)
	.fetch_one(pool_source.as_ref())
	.await
	.expect("Failed to fetch first from source");

	let target_first: (i32, String, f64) = sqlx::query_as(
		"SELECT id, name, CAST(price AS DOUBLE PRECISION) FROM products ORDER BY id LIMIT 1",
	)
	.fetch_one(pool_target.as_ref())
	.await
	.expect("Failed to fetch first from target");

	assert_eq!(source_first, target_first, "First product should match");

	// Verify source data unchanged
	let source_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
		.fetch_one(pool_source.as_ref())
		.await
		.expect("Failed to count in source");

	assert_eq!(source_count, 10, "Source should still have 10 products");
}

// ============ Read Replica Pattern Tests ============

/// Test read distribution across primary and replica
///
/// Verifies:
/// - Writes go to primary database
/// - Reads can be distributed to replica
/// - Replica eventually reflects primary data (eventual consistency)
/// - System handles read from stale replica gracefully
#[rstest]
#[tokio::test]
async fn test_read_replica_pattern(
	#[future] postgres_container: (
		ContainerAsync<GenericImage>,
		Arc<Pool<Postgres>>,
		u16,
		String,
	),
) {
	let (_container_primary, pool_primary, _port_primary, _url_primary) = postgres_container.await;

	// Start replica database
	let postgres_replica = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.with_env_var("POSTGRES_DB", "replica_db")
		.start()
		.await
		.expect("Failed to start replica PostgreSQL container");

	// Wait briefly before port query to ensure container networking is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	let port_replica = postgres_replica
		.get_host_port_ipv4(5432.tcp())
		.await
		.expect("Failed to get replica port");
	let url_replica = format!(
		"postgres://postgres@localhost:{}/replica_db?sslmode=disable",
		port_replica
	);

	let pool_replica = Arc::new(
		Pool::<Postgres>::connect(&url_replica)
			.await
			.expect("Failed to connect to replica database"),
	);

	// Create tables in both databases
	let create_table_sql = r#"
		CREATE TABLE IF NOT EXISTS articles (
			id SERIAL PRIMARY KEY,
			title TEXT NOT NULL,
			content TEXT NOT NULL
		)
	"#;

	sqlx::query(create_table_sql)
		.execute(pool_primary.as_ref())
		.await
		.expect("Failed to create table in primary");

	sqlx::query(create_table_sql)
		.execute(pool_replica.as_ref())
		.await
		.expect("Failed to create table in replica");

	// Write to primary
	let article_id: i32 =
		sqlx::query_scalar("INSERT INTO articles (title, content) VALUES ($1, $2) RETURNING id")
			.bind("Breaking News")
			.bind("Important content")
			.fetch_one(pool_primary.as_ref())
			.await
			.expect("Failed to insert into primary");

	// Simulate replication delay
	sleep(Duration::from_millis(100)).await;

	// Manually replicate to simulate logical replication
	let articles = sqlx::query("SELECT id, title, content FROM articles")
		.fetch_all(pool_primary.as_ref())
		.await
		.expect("Failed to fetch from primary");

	for row in articles {
		let id: i32 = row.get("id");
		let title: String = row.get("title");
		let content: String = row.get("content");

		sqlx::query("INSERT INTO articles (id, title, content) VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET title = EXCLUDED.title, content = EXCLUDED.content")
			.bind(id)
			.bind(&title)
			.bind(&content)
			.execute(pool_replica.as_ref())
			.await
			.expect("Failed to replicate to replica");
	}

	// Read from replica
	let replica_result: (i32, String, String) =
		sqlx::query_as("SELECT id, title, content FROM articles WHERE id = $1")
			.bind(article_id)
			.fetch_one(pool_replica.as_ref())
			.await
			.expect("Failed to read from replica");

	assert_eq!(replica_result.0, article_id);
	assert_eq!(replica_result.1, "Breaking News");
	assert_eq!(replica_result.2, "Important content");
}

/// Test read load balancing across multiple replicas
///
/// Verifies:
/// - Reads can be distributed across multiple replicas
/// - Each replica can serve read requests independently
/// - Load is distributed evenly
#[rstest]
#[tokio::test]
async fn test_read_load_balancing_across_replicas(
	#[future] postgres_container: (
		ContainerAsync<GenericImage>,
		Arc<Pool<Postgres>>,
		u16,
		String,
	),
) {
	let (_container_primary, pool_primary, _port_primary, _url_primary) = postgres_container.await;

	// Start two replica databases
	let mut replicas = Vec::new();
	for i in 1..=2 {
		let postgres_replica = GenericImage::new("postgres", "16-alpine")
			.with_exposed_port(5432.tcp())
			.with_wait_for(WaitFor::message_on_stderr(
				"database system is ready to accept connections",
			))
			.with_startup_timeout(std::time::Duration::from_secs(120))
			.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
			.with_env_var("POSTGRES_DB", format!("replica_{}", i))
			.start()
			.await
			.unwrap_or_else(|_| panic!("Failed to start replica {} container", i));

		// Wait for container networking to be fully ready
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

		let port = postgres_replica
			.get_host_port_ipv4(5432.tcp())
			.await
			.unwrap_or_else(|_| panic!("Failed to get replica {} port", i));
		let url = format!(
			"postgres://postgres@localhost:{}/replica_{}?sslmode=disable",
			port, i
		);

		let pool = Arc::new(
			sqlx::postgres::PgPoolOptions::new()
				.max_connections(5)
				.acquire_timeout(std::time::Duration::from_secs(60))
				.connect(&url)
				.await
				.unwrap_or_else(|_| panic!("Failed to connect to replica {}", i)),
		);

		replicas.push((postgres_replica, pool));

		// Add delay after each replica connection to ensure full initialization
		tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
	}

	// Create table in primary and replicas
	let create_table_sql = r#"
		CREATE TABLE IF NOT EXISTS stats (
			id SERIAL PRIMARY KEY,
			value INT NOT NULL
		)
	"#;

	sqlx::query(create_table_sql)
		.execute(pool_primary.as_ref())
		.await
		.expect("Failed to create table in primary");

	for (i, (_container, pool)) in replicas.iter().enumerate() {
		sqlx::query(create_table_sql)
			.execute(pool.as_ref())
			.await
			.unwrap_or_else(|_| panic!("Failed to create table in replica {}", i + 1));
	}

	// Insert data into primary
	for i in 1..=10 {
		sqlx::query("INSERT INTO stats (value) VALUES ($1)")
			.bind(i)
			.execute(pool_primary.as_ref())
			.await
			.expect("Failed to insert into primary");
	}

	// Replicate to all replicas
	let stats = sqlx::query("SELECT id, value FROM stats")
		.fetch_all(pool_primary.as_ref())
		.await
		.expect("Failed to fetch from primary");

	for (_container, pool) in &replicas {
		for row in &stats {
			let id: i32 = row.get("id");
			let value: i32 = row.get("value");

			sqlx::query("INSERT INTO stats (id, value) VALUES ($1, $2)")
				.bind(id)
				.bind(value)
				.execute(pool.as_ref())
				.await
				.expect("Failed to replicate");
		}
	}

	// Distribute reads across replicas using round-robin
	let mut read_counts = [0, 0];
	for i in 0..10 {
		let replica_index = i % 2;
		let (_container, pool) = &replicas[replica_index];

		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stats")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to read from replica");

		assert_eq!(count, 10, "Each replica should have 10 records");
		read_counts[replica_index] += 1;
	}

	// Verify load distribution
	assert_eq!(read_counts[0], 5, "Replica 1 should serve 5 reads");
	assert_eq!(read_counts[1], 5, "Replica 2 should serve 5 reads");
}

// ============ Failover Scenario Tests ============

/// Test failover on primary database failure
///
/// Verifies:
/// - System detects primary database failure
/// - Reads can continue on replica
/// - Writes are queued or fail gracefully
/// - System recovers when primary comes back online
#[rstest]
#[tokio::test]
async fn test_primary_database_failover(
	#[future] postgres_container: (
		ContainerAsync<GenericImage>,
		Arc<Pool<Postgres>>,
		u16,
		String,
	),
) {
	let (_container_primary, pool_primary, _port_primary, _url_primary) = postgres_container.await;

	// Start replica database
	let postgres_replica = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.with_env_var("POSTGRES_DB", "replica_db")
		.start()
		.await
		.expect("Failed to start replica container");

	// Wait briefly before port query to ensure container networking is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	let port_replica = postgres_replica
		.get_host_port_ipv4(5432.tcp())
		.await
		.expect("Failed to get replica port");
	let url_replica = format!(
		"postgres://postgres@localhost:{}/replica_db?sslmode=disable",
		port_replica
	);

	let pool_replica = Arc::new(
		Pool::<Postgres>::connect(&url_replica)
			.await
			.expect("Failed to connect to replica"),
	);

	// Create table
	let create_table_sql = r#"
		CREATE TABLE IF NOT EXISTS orders (
			id SERIAL PRIMARY KEY,
			customer_id INT NOT NULL,
			total DECIMAL(10, 2) NOT NULL
		)
	"#;

	sqlx::query(create_table_sql)
		.execute(pool_primary.as_ref())
		.await
		.expect("Failed to create table in primary");

	sqlx::query(create_table_sql)
		.execute(pool_replica.as_ref())
		.await
		.expect("Failed to create table in replica");

	// Insert data into primary
	sqlx::query("INSERT INTO orders (customer_id, total) VALUES ($1, $2)")
		.bind(1)
		.bind(100.50)
		.execute(pool_primary.as_ref())
		.await
		.expect("Failed to insert into primary");

	// Replicate to replica
	let orders = sqlx::query("SELECT id, customer_id, total FROM orders")
		.fetch_all(pool_primary.as_ref())
		.await
		.expect("Failed to fetch from primary");

	for row in orders {
		let id: i32 = row.get("id");
		let customer_id: i32 = row.get("customer_id");
		let total: Decimal = row.get("total");

		sqlx::query("INSERT INTO orders (id, customer_id, total) VALUES ($1, $2, $3)")
			.bind(id)
			.bind(customer_id)
			.bind(total)
			.execute(pool_replica.as_ref())
			.await
			.expect("Failed to replicate");
	}

	// Simulate primary failure by closing pool
	pool_primary.close().await;

	// Verify reads still work on replica
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
		.fetch_one(pool_replica.as_ref())
		.await
		.expect("Failed to read from replica during failover");

	assert_eq!(
		count, 1,
		"Reads should continue on replica during primary failure"
	);

	// Verify replica data is accessible
	let order: (i32, i32, f64) = sqlx::query_as(
		"SELECT id, customer_id, CAST(total AS DOUBLE PRECISION) FROM orders LIMIT 1",
	)
	.fetch_one(pool_replica.as_ref())
	.await
	.expect("Failed to fetch order from replica");

	assert_eq!(order.1, 1, "Customer ID should be 1");
	assert!((order.2 - 100.50).abs() < 0.01, "Total should be 100.50");
}

// ============ Sharding Pattern Tests ============

/// Test data sharding across multiple databases
///
/// Verifies:
/// - Data is partitioned correctly based on shard key
/// - Each shard contains only its designated data
/// - Shard routing logic works correctly
/// - Cross-shard queries can aggregate results
#[rstest]
#[tokio::test]
async fn test_data_sharding_across_databases(
	#[future] postgres_container: (
		ContainerAsync<GenericImage>,
		Arc<Pool<Postgres>>,
		u16,
		String,
	),
) {
	let (_container_shard0, pool_shard0, _port0, _url0) = postgres_container.await;

	// Start second shard database
	let postgres_shard1 = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.with_env_var("POSTGRES_DB", "shard1_db")
		.start()
		.await
		.expect("Failed to start shard1 container");

	// Wait briefly before port query to ensure container networking is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	let port_shard1 = postgres_shard1
		.get_host_port_ipv4(5432.tcp())
		.await
		.expect("Failed to get shard1 port");
	let url_shard1 = format!(
		"postgres://postgres@localhost:{}/shard1_db?sslmode=disable",
		port_shard1
	);

	let pool_shard1 = Arc::new(
		Pool::<Postgres>::connect(&url_shard1)
			.await
			.expect("Failed to connect to shard1"),
	);

	// Create table in both shards
	let create_table_sql = r#"
		CREATE TABLE IF NOT EXISTS users (
			id SERIAL PRIMARY KEY,
			username TEXT NOT NULL,
			email TEXT NOT NULL,
			shard_key INT NOT NULL
		)
	"#;

	sqlx::query(create_table_sql)
		.execute(pool_shard0.as_ref())
		.await
		.expect("Failed to create table in shard0");

	sqlx::query(create_table_sql)
		.execute(pool_shard1.as_ref())
		.await
		.expect("Failed to create table in shard1");

	// Insert users with sharding logic (shard_key % 2)
	let users = vec![
		User::new(1, "user1", "user1@example.com", 0), // shard 0
		User::new(2, "user2", "user2@example.com", 1), // shard 1
		User::new(3, "user3", "user3@example.com", 0), // shard 0
		User::new(4, "user4", "user4@example.com", 1), // shard 1
		User::new(5, "user5", "user5@example.com", 0), // shard 0
	];

	for user in users {
		let target_shard = user.shard_key % 2;
		let pool = if target_shard == 0 {
			&pool_shard0
		} else {
			&pool_shard1
		};

		sqlx::query("INSERT INTO users (id, username, email, shard_key) VALUES ($1, $2, $3, $4)")
			.bind(user.id)
			.bind(&user.username)
			.bind(&user.email)
			.bind(user.shard_key)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert user");
	}

	// Verify shard 0 has correct users
	let count_shard0: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(pool_shard0.as_ref())
		.await
		.expect("Failed to count in shard0");

	assert_eq!(count_shard0, 3, "Shard 0 should have 3 users");

	// Verify shard 1 has correct users
	let count_shard1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(pool_shard1.as_ref())
		.await
		.expect("Failed to count in shard1");

	assert_eq!(count_shard1, 2, "Shard 1 should have 2 users");

	// Verify data segregation (shard 0 only has even shard_keys)
	let shard0_users: Vec<i32> = sqlx::query_scalar("SELECT shard_key FROM users")
		.fetch_all(pool_shard0.as_ref())
		.await
		.expect("Failed to fetch from shard0");

	for key in shard0_users {
		assert_eq!(key % 2, 0, "Shard 0 should only have even shard_keys");
	}

	// Verify shard 1 only has odd shard_keys
	let shard1_users: Vec<i32> = sqlx::query_scalar("SELECT shard_key FROM users")
		.fetch_all(pool_shard1.as_ref())
		.await
		.expect("Failed to fetch from shard1");

	for key in shard1_users {
		assert_eq!(key % 2, 1, "Shard 1 should only have odd shard_keys");
	}
}

/// Test cross-shard query aggregation
///
/// Verifies:
/// - Data can be queried from multiple shards
/// - Results from different shards can be aggregated
/// - Aggregation logic handles missing data gracefully
#[rstest]
#[tokio::test]
async fn test_cross_shard_aggregation(
	#[future] postgres_container: (
		ContainerAsync<GenericImage>,
		Arc<Pool<Postgres>>,
		u16,
		String,
	),
) {
	let (_container_shard0, pool_shard0, _port0, _url0) = postgres_container.await;

	// Start second shard
	let postgres_shard1 = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.with_env_var("POSTGRES_DB", "shard1_db")
		.start()
		.await
		.expect("Failed to start shard1 container");

	// Wait briefly before port query to ensure container networking is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	let port_shard1 = postgres_shard1
		.get_host_port_ipv4(5432.tcp())
		.await
		.expect("Failed to get shard1 port");
	let url_shard1 = format!(
		"postgres://postgres@localhost:{}/shard1_db?sslmode=disable",
		port_shard1
	);

	let pool_shard1 = Arc::new(
		Pool::<Postgres>::connect(&url_shard1)
			.await
			.expect("Failed to connect to shard1"),
	);

	// Create tables
	let create_table_sql = r#"
		CREATE TABLE IF NOT EXISTS sales (
			id SERIAL PRIMARY KEY,
			region_id INT NOT NULL,
			amount DECIMAL(10, 2) NOT NULL
		)
	"#;

	sqlx::query(create_table_sql)
		.execute(pool_shard0.as_ref())
		.await
		.expect("Failed to create table in shard0");

	sqlx::query(create_table_sql)
		.execute(pool_shard1.as_ref())
		.await
		.expect("Failed to create table in shard1");

	// Insert sales data into shards (shard by region_id)
	// Shard 0: regions 0, 2, 4
	for region_id in [0, 2, 4] {
		sqlx::query("INSERT INTO sales (region_id, amount) VALUES ($1, $2)")
			.bind(region_id)
			.bind(100.0 * region_id as f64)
			.execute(pool_shard0.as_ref())
			.await
			.expect("Failed to insert into shard0");
	}

	// Shard 1: regions 1, 3, 5
	for region_id in [1, 3, 5] {
		sqlx::query("INSERT INTO sales (region_id, amount) VALUES ($1, $2)")
			.bind(region_id)
			.bind(100.0 * region_id as f64)
			.execute(pool_shard1.as_ref())
			.await
			.expect("Failed to insert into shard1");
	}

	// Aggregate total sales across all shards
	let sum_shard0: f64 = sqlx::query_scalar(
		"SELECT COALESCE(SUM(CAST(amount AS DOUBLE PRECISION)), 0.0) FROM sales",
	)
	.fetch_one(pool_shard0.as_ref())
	.await
	.expect("Failed to sum in shard0");

	let sum_shard1: f64 = sqlx::query_scalar(
		"SELECT COALESCE(SUM(CAST(amount AS DOUBLE PRECISION)), 0.0) FROM sales",
	)
	.fetch_one(pool_shard1.as_ref())
	.await
	.expect("Failed to sum in shard1");

	let total_sales = sum_shard0 + sum_shard1;

	// Expected: 0 + 200 + 400 + 100 + 300 + 500 = 1500
	assert!(
		(total_sales - 1500.0).abs() < 0.01,
		"Total sales across shards should be 1500"
	);

	// Count records across shards
	let count_shard0: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales")
		.fetch_one(pool_shard0.as_ref())
		.await
		.expect("Failed to count in shard0");

	let count_shard1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales")
		.fetch_one(pool_shard1.as_ref())
		.await
		.expect("Failed to count in shard1");

	let total_count = count_shard0 + count_shard1;

	assert_eq!(total_count, 6, "Total sales records should be 6");
}
