//! Integration tests for ORM + Pool interaction
//!
//! These tests verify the integration between reinhardt-orm and reinhardt-pool
//! using TestContainers with actual database backends.

use reinhardt_db::pool::{ConnectionPool, PoolConfig};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// Connection Pool Integration Tests
// ============================================================================

/// Test basic connection acquisition from pool
#[rstest]
#[tokio::test]
async fn test_pool_connection_acquisition(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Acquire connection from pool
	let mut conn = pool
		.inner()
		.acquire()
		.await
		.expect("Failed to acquire connection");

	// Execute simple query
	let result = sqlx::query("SELECT 1 as value")
		.fetch_one(&mut *conn)
		.await
		.expect("Failed to execute query");

	let value: i32 = result.get("value");
	assert_eq!(value, 1);
}

/// Test connection pool can handle CREATE TABLE
#[rstest]
#[tokio::test]
async fn test_pool_create_table(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Create table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS test_users (
			id SERIAL PRIMARY KEY,
			username VARCHAR(255) NOT NULL,
			email VARCHAR(255) NOT NULL,
			created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
		)",
	)
	.execute(pool.inner())
	.await
	.expect("Failed to create table");

	// Verify table exists
	let result = sqlx::query(
		"SELECT table_name FROM information_schema.tables
		 WHERE table_schema = 'public' AND table_name = 'test_users'",
	)
	.fetch_one(pool.inner())
	.await
	.expect("Failed to query table");

	let table_name: String = result.get("table_name");
	assert_eq!(table_name, "test_users");
}

// ============================================================================
// CRUD Operations via Pool Tests
// ============================================================================

/// Test INSERT operation via pool
#[rstest]
#[tokio::test]
async fn test_pool_insert_operation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Create table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS users (
			id SERIAL PRIMARY KEY,
			username VARCHAR(255) NOT NULL
		)",
	)
	.execute(pool.inner())
	.await
	.expect("Failed to create table");

	// Insert user
	let result = sqlx::query("INSERT INTO users (username) VALUES ($1) RETURNING id")
		.bind("testuser")
		.fetch_one(pool.inner())
		.await
		.expect("Failed to insert user");

	let user_id: i32 = result.get("id");
	assert!(user_id > 0);
}

/// Test SELECT operation via pool
#[rstest]
#[tokio::test]
async fn test_pool_select_operation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Create and populate table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS products (
			id SERIAL PRIMARY KEY,
			name VARCHAR(255) NOT NULL,
			quantity INT NOT NULL
		)",
	)
	.execute(pool.inner())
	.await
	.expect("Failed to create table");

	sqlx::query("INSERT INTO products (name, quantity) VALUES ($1, $2)")
		.bind("Product A")
		.bind(100)
		.execute(pool.inner())
		.await
		.expect("Failed to insert product");

	// Select product
	let result = sqlx::query("SELECT name, quantity FROM products WHERE name = $1")
		.bind("Product A")
		.fetch_one(pool.inner())
		.await
		.expect("Failed to select product");

	let name: String = result.get("name");
	let quantity: i32 = result.get("quantity");

	assert_eq!(name, "Product A");
	assert_eq!(quantity, 100);
}

/// Test UPDATE operation via pool
#[rstest]
#[tokio::test]
async fn test_pool_update_operation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Create and populate table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS items (
			id SERIAL PRIMARY KEY,
			name VARCHAR(255) NOT NULL,
			quantity INT NOT NULL
		)",
	)
	.execute(pool.inner())
	.await
	.expect("Failed to create table");

	let result = sqlx::query("INSERT INTO items (name, quantity) VALUES ($1, $2) RETURNING id")
		.bind("Item 1")
		.bind(10)
		.fetch_one(pool.inner())
		.await
		.expect("Failed to insert item");

	let item_id: i32 = result.get("id");

	// Update quantity
	let updated_rows = sqlx::query("UPDATE items SET quantity = $1 WHERE id = $2")
		.bind(20)
		.bind(item_id)
		.execute(pool.inner())
		.await
		.expect("Failed to update item")
		.rows_affected();

	assert_eq!(updated_rows, 1);

	// Verify update
	let result = sqlx::query("SELECT quantity FROM items WHERE id = $1")
		.bind(item_id)
		.fetch_one(pool.inner())
		.await
		.expect("Failed to select item");

	let quantity: i32 = result.get("quantity");
	assert_eq!(quantity, 20);
}

/// Test DELETE operation via pool
#[rstest]
#[tokio::test]
async fn test_pool_delete_operation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Create and populate table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS records (
			id SERIAL PRIMARY KEY,
			data VARCHAR(255) NOT NULL
		)",
	)
	.execute(pool.inner())
	.await
	.expect("Failed to create table");

	let result = sqlx::query("INSERT INTO records (data) VALUES ($1) RETURNING id")
		.bind("test data")
		.fetch_one(pool.inner())
		.await
		.expect("Failed to insert record");

	let record_id: i32 = result.get("id");

	// Delete record
	let deleted_rows = sqlx::query("DELETE FROM records WHERE id = $1")
		.bind(record_id)
		.execute(pool.inner())
		.await
		.expect("Failed to delete record")
		.rows_affected();

	assert_eq!(deleted_rows, 1);

	// Verify deletion
	let result = sqlx::query("SELECT COUNT(*) as count FROM records WHERE id = $1")
		.bind(record_id)
		.fetch_one(pool.inner())
		.await
		.expect("Failed to count records");

	let count: i64 = result.get("count");
	assert_eq!(count, 0);
}

// ============================================================================
// Transaction Management Tests
// ============================================================================

/// Test transaction commit via pool
#[rstest]
#[tokio::test]
async fn test_pool_transaction_commit(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Create table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS accounts (
			id SERIAL PRIMARY KEY,
			balance NUMERIC(10, 2) NOT NULL
		)",
	)
	.execute(pool.inner())
	.await
	.expect("Failed to create table");

	// Begin transaction
	let mut tx = pool
		.inner()
		.begin()
		.await
		.expect("Failed to begin transaction");

	// Insert account in transaction
	sqlx::query("INSERT INTO accounts (balance) VALUES ($1)")
		.bind(1000.00)
		.execute(&mut *tx)
		.await
		.expect("Failed to insert account");

	// Commit transaction
	tx.commit().await.expect("Failed to commit transaction");

	// Verify data persisted
	let result = sqlx::query("SELECT COUNT(*) as count FROM accounts")
		.fetch_one(pool.inner())
		.await
		.expect("Failed to count accounts");

	let count: i64 = result.get("count");
	assert_eq!(count, 1);
}

/// Test transaction rollback via pool
#[rstest]
#[tokio::test]
async fn test_pool_transaction_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Create table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS orders (
			id SERIAL PRIMARY KEY,
			amount NUMERIC(10, 2) NOT NULL
		)",
	)
	.execute(pool.inner())
	.await
	.expect("Failed to create table");

	// Begin transaction
	let mut tx = pool
		.inner()
		.begin()
		.await
		.expect("Failed to begin transaction");

	// Insert order in transaction
	sqlx::query("INSERT INTO orders (amount) VALUES ($1)")
		.bind(500.00)
		.execute(&mut *tx)
		.await
		.expect("Failed to insert order");

	// Rollback transaction
	tx.rollback().await.expect("Failed to rollback transaction");

	// Verify data not persisted
	let result = sqlx::query("SELECT COUNT(*) as count FROM orders")
		.fetch_one(pool.inner())
		.await
		.expect("Failed to count orders");

	let count: i64 = result.get("count");
	assert_eq!(count, 0);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Test pool handles query errors gracefully
#[rstest]
#[tokio::test]
async fn test_pool_query_error_handling(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let config = PoolConfig::default();
	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Execute invalid SQL
	let result = sqlx::query("SELECT * FROM nonexistent_table")
		.fetch_one(pool.inner())
		.await;

	// Should fail
	assert!(result.is_err());

	// Pool should still work after error
	let result = sqlx::query("SELECT 1 as value")
		.fetch_one(pool.inner())
		.await
		.expect("Failed to execute query after error");

	let value: i32 = result.get("value");
	assert_eq!(value, 1);
}

/// Test pool handles connection timeout
#[rstest]
#[tokio::test]
async fn test_pool_connection_timeout_recovery(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let mut config = PoolConfig::default();
	config.max_connections = 2;
	config.min_connections = 0;
	config.acquire_timeout = Duration::from_millis(500);

	let pool = ConnectionPool::new_postgres(&url, config)
		.await
		.expect("Failed to create pool");

	// Acquire all available connections
	let _conn1 = pool
		.inner()
		.acquire()
		.await
		.expect("Failed to acquire conn1");
	let _conn2 = pool
		.inner()
		.acquire()
		.await
		.expect("Failed to acquire conn2");

	// Try to acquire 3rd connection (should timeout)
	let result = pool.inner().acquire().await;
	assert!(result.is_err());

	// Release one connection
	drop(_conn1);

	// Should be able to acquire again
	let _conn3 = pool
		.inner()
		.acquire()
		.await
		.expect("Failed to acquire conn3");
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

/// Test concurrent CRUD operations via pool
#[rstest]
#[tokio::test]
async fn test_pool_concurrent_crud_operations(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let mut config = PoolConfig::default();
	config.max_connections = 10;

	let pool = Arc::new(
		ConnectionPool::new_postgres(&url, config)
			.await
			.expect("Failed to create pool"),
	);

	// Create table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS concurrent_test (
			id SERIAL PRIMARY KEY,
			value INT NOT NULL
		)",
	)
	.execute(pool.inner())
	.await
	.expect("Failed to create table");

	// Spawn 20 concurrent tasks that each insert a record
	let mut handles = Vec::new();
	for i in 0..20 {
		let pool_clone = Arc::clone(&pool);
		let handle = tokio::spawn(async move {
			sqlx::query("INSERT INTO concurrent_test (value) VALUES ($1)")
				.bind(i)
				.execute(pool_clone.inner())
				.await
				.expect("Failed to insert value");
		});
		handles.push(handle);
	}

	// Wait for all tasks to complete
	for handle in handles {
		handle.await.expect("Task panicked");
	}

	// Verify all records inserted
	let result = sqlx::query("SELECT COUNT(*) as count FROM concurrent_test")
		.fetch_one(pool.inner())
		.await
		.expect("Failed to count records");

	let count: i64 = result.get("count");
	assert_eq!(count, 20);
}
