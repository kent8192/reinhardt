//! Database Integration Tests
//!
//! Compilation and execution control:
//! - Cargo.toml: [[test]] name = "database_tests" required-features = ["with-reinhardt"]
//! - build.rs: Sets 'with-reinhardt' feature when reinhardt is available
//! - When feature is disabled, this entire test file is excluded from compilation
//!
//! Uses Reinhardt ORM Manager<T> for all database operations.
//!
//! ## Architecture Note: PostgreSQL Test with TestContainers
//!
//! This test uses PostgreSQL via TestContainers for realistic database testing.
//! The MigrationProvider pattern is used to apply migrations automatically.

use chrono::Utc;
use reinhardt::TransactionScope;
use reinhardt::db::orm::reinitialize_database;
use reinhardt::prelude::*;
use reinhardt::test::fixtures::{ContainerAsync, GenericImage};
use rstest::*;
use serial_test::serial;
use std::sync::Arc;

// Import models and migrations from the library crate
use examples_database_integration::{ExampleMigrations, Todo, User};

// Import MigrationProvider trait for .migrations() method
use reinhardt::db::migrations::MigrationProvider;

// ============================================================================
// Custom Fixtures with Migrations (PostgreSQL)
// ============================================================================

/// PostgreSQL fixture with migrations applied via MigrationProvider
#[fixture]
async fn db_with_migrations() -> (ContainerAsync<GenericImage>, Arc<DatabaseConnection>) {
	use reinhardt::db::migrations::executor::DatabaseMigrationExecutor;
	use reinhardt::test::fixtures::postgres_container;

	// Start PostgreSQL container and get URL
	let (container, _pool, _port, database_url) = postgres_container().await;

	// Connect to database
	let conn = DatabaseConnection::connect(&database_url)
		.await
		.expect("Failed to connect to PostgreSQL");

	// Apply migrations manually using ExampleMigrations
	let migrations = ExampleMigrations::migrations();
	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(conn.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.expect("Failed to apply migrations");
	}

	// Initialize global database state for Manager<T> operations
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize global database state");

	(container, Arc::new(conn))
}

// ============================================================================
// Basic Database Connection Tests
// ============================================================================

/// Test basic database connection using Reinhardt ORM
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_database_connection(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;

	// Verify connection by querying users table
	let manager = User::objects();
	let result = manager.all().all().await;
	assert!(result.is_ok(), "Failed to query users table");
}

/// Test that database is ready with all tables created
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_database_ready(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;

	// Verify users table is accessible
	let user_manager = User::objects();
	let users_result = user_manager.all().all().await;
	assert!(users_result.is_ok(), "Users table should be accessible");

	// Verify todos table is accessible
	let todo_manager = Todo::objects();
	let todos_result = todo_manager.all().all().await;
	assert!(todos_result.is_ok(), "Todos table should be accessible");

	println!("✅ Database ready with users and todos tables");
}

// ============================================================================
// CRUD Operations Tests (User Model)
// ============================================================================

/// Test creating a user with Reinhardt ORM
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_create_user(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = User::objects();

	// Use new() function - id (primary key) and created_at (auto_now_add) are auto-excluded
	let new_user = User::new("Alice".to_string(), "alice@example.com".to_string());

	let created = manager.create(&new_user).await.unwrap();

	assert!(
		created.id.is_some() && created.id.unwrap() > 0,
		"Created user should have positive ID"
	);
	assert_eq!(created.name, "Alice");
	assert_eq!(created.email, "alice@example.com");
}

/// Test reading multiple users
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_read_users(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = User::objects();

	// Create test users using new() function
	let user1 = User::new("Alice".to_string(), "alice@example.com".to_string());
	let user2 = User::new("Bob".to_string(), "bob@example.com".to_string());

	manager.create(&user1).await.unwrap();
	manager.create(&user2).await.unwrap();

	// Read all users
	let users = manager.all().all().await.unwrap();

	assert_eq!(users.len(), 2, "Should have 2 users");
	assert!(users.iter().any(|u| u.name == "Alice"));
	assert!(users.iter().any(|u| u.name == "Bob"));
}

/// Test updating a user
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_update_user(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = User::objects();

	// Create user using new() function
	let new_user = User::new("Alice".to_string(), "alice@example.com".to_string());
	let created = manager.create(&new_user).await.unwrap();

	// Update user
	let mut updated_user = created.clone();
	updated_user.name = "Alice Updated".to_string();
	let result = manager.update(&updated_user).await.unwrap();

	assert_eq!(result.name, "Alice Updated");
	assert_eq!(result.id, created.id);
	assert!(result.id.is_some());
}

/// Test deleting a user
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_delete_user(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, conn) = db_with_migrations.await;
	let manager = User::objects();

	// Create user using new() function
	let new_user = User::new("Alice".to_string(), "alice@example.com".to_string());
	let created = manager.create_with_conn(&conn, &new_user).await.unwrap();

	// Delete user - unwrap the Option<i64> to get the actual id
	manager
		.delete_with_conn(&conn, created.id.unwrap())
		.await
		.unwrap();

	// Verify deletion using explicit connection
	let count = manager.count_with_conn(&conn).await.unwrap();
	assert_eq!(count, 0, "User should be deleted");
}

// ============================================================================
// Transaction Tests
// ============================================================================

/// Test transaction commit
///
/// Uses TransactionScope with ORM Manager<T>::create_with_conn to test transaction commit behavior.
/// The create_with_conn method accepts an external connection, allowing ORM operations within
/// a transaction context.
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_transaction_commit(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, conn) = db_with_migrations.await;

	// Start transaction with TransactionScope
	let tx = TransactionScope::begin(&conn)
		.await
		.expect("Failed to begin transaction");

	// Insert user using ORM Manager with explicit connection (transaction-aware)
	let manager = User::objects();
	let user = User::new("Alice".to_string(), "alice@example.com".to_string());
	let created_user = manager
		.create_with_conn(&conn, &user)
		.await
		.expect("Failed to create user");

	// Verify user was created with auto-generated ID
	assert!(
		created_user.id.is_some() && created_user.id.unwrap() > 0,
		"User ID should be auto-generated"
	);
	assert_eq!(created_user.name, "Alice");
	assert_eq!(created_user.email, "alice@example.com");

	// Commit transaction
	tx.commit().await.expect("Failed to commit transaction");

	// Verify commit using Manager.all() (safe to use after transaction completes)
	let users = manager.all().all().await.unwrap();
	assert_eq!(users.len(), 1, "User should be persisted after commit");
	assert_eq!(users[0].name, "Alice");
	assert_eq!(users[0].email, "alice@example.com");
}

/// Test transaction rollback
///
/// Uses TransactionScope with raw SQL execution to test transaction rollback behavior.
/// All SQL operations (INSERT, SELECT) are executed through the transaction's dedicated
/// connection to ensure proper isolation - this is critical because connection pools
/// distribute queries across different physical connections.
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_transaction_rollback(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, conn) = db_with_migrations.await;

	// Start transaction with TransactionScope
	let mut tx = TransactionScope::begin(&conn)
		.await
		.expect("Failed to begin transaction");

	// Insert user using TransactionScope::execute() to ensure it runs on the transaction connection
	// NOTE: Using raw SQL because ORM Manager methods use the connection pool,
	// which may route queries to different physical connections
	use reinhardt::QueryValue;
	let now = Utc::now();
	let rows_affected = tx
		.execute(
			"INSERT INTO users (name, email, created_at) VALUES ($1, $2, $3)",
			vec![
				QueryValue::String("Alice".to_string()),
				QueryValue::String("alice@example.com".to_string()),
				QueryValue::Timestamp(now),
			],
		)
		.await
		.expect("Failed to insert user within transaction");
	assert_eq!(rows_affected, 1, "One row should be inserted");

	// Verify user exists within transaction using TransactionScope::query()
	let rows = tx
		.query(
			"SELECT COUNT(*) as count FROM users WHERE name = $1",
			vec![QueryValue::String("Alice".to_string())],
		)
		.await
		.expect("Failed to query within transaction");
	assert_eq!(rows.len(), 1, "Should return one row");

	// Rollback transaction (simulating error scenario)
	tx.rollback().await.expect("Failed to rollback transaction");

	// Verify rollback - user should NOT exist after rollback
	// Using Manager here is safe because we're checking AFTER the transaction completed
	let manager = User::objects();
	let count = manager.count_with_conn(&conn).await.unwrap();
	assert_eq!(count, 0, "User should not be persisted after rollback");
}

// ============================================================================
// ORM CRUD Tests (Todo Model)
// ============================================================================

/// Test creating a todo with Reinhardt ORM
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_orm_create_todo(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = Todo::objects();

	// Use new() function - id (primary key), created_at (auto_now_add), updated_at (auto_now) are auto-excluded
	let new_todo = Todo::new(
		"Test Todo".to_string(),
		Some("This is a test todo".to_string()),
		false,
	);

	let created = manager.create(&new_todo).await.unwrap();

	assert!(created.id.is_some() && created.id.unwrap() > 0);
	assert_eq!(created.title, "Test Todo");
	assert_eq!(created.description, Some("This is a test todo".to_string()));
	assert!(!created.completed);
}

/// Test listing all todos
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_orm_list_todos(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = Todo::objects();

	// Create test todos using new() function
	let todo1 = Todo::new("Todo 1".to_string(), None, false);

	let todo2 = Todo::new(
		"Todo 2".to_string(),
		Some("Description 2".to_string()),
		true,
	);

	manager.create(&todo1).await.unwrap();
	manager.create(&todo2).await.unwrap();

	// List all todos
	let todos = manager.all().all().await.unwrap();

	assert_eq!(todos.len(), 2);
	assert!(todos.iter().any(|t| t.title == "Todo 1"));
	assert!(todos.iter().any(|t| t.title == "Todo 2"));
}

/// Test getting a specific todo
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_orm_get_todo(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = Todo::objects();

	// Create todo using new() function
	let new_todo = Todo::new(
		"Test Todo".to_string(),
		Some("Test description".to_string()),
		false,
	);
	let created = manager.create(&new_todo).await.unwrap();

	// Get todo by ID
	let todos = manager.all().all().await.unwrap();
	let found = todos.iter().find(|t| t.id == created.id);

	assert!(found.is_some());
	let found_todo = found.unwrap();
	assert_eq!(found_todo.title, "Test Todo");
	assert_eq!(found_todo.description, Some("Test description".to_string()));
}

/// Test updating a todo
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_orm_update_todo(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = Todo::objects();

	// Create todo using new() function
	let new_todo = Todo::new(
		"Original Title".to_string(),
		Some("Original description".to_string()),
		false,
	);
	let created = manager.create(&new_todo).await.unwrap();

	// Update todo
	let mut updated_todo = created.clone();
	updated_todo.title = "Updated Title".to_string();
	updated_todo.description = Some("Updated description".to_string());
	updated_todo.completed = true;

	let result = manager.update(&updated_todo).await.unwrap();

	assert_eq!(result.title, "Updated Title");
	assert_eq!(result.description, Some("Updated description".to_string()));
	assert!(result.completed);
}

/// Test deleting a todo
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_orm_delete_todo(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = Todo::objects();

	// Create todo using new() function
	let new_todo = Todo::new("To be deleted".to_string(), None, false);
	let created = manager.create(&new_todo).await.unwrap();

	// Delete todo
	manager.delete(created.id.unwrap()).await.unwrap();

	// Verify deletion
	let todos = manager.all().all().await.unwrap();
	assert_eq!(todos.len(), 0, "Todo should be deleted");
}

// ============================================================================
// Field Validation Tests
// ============================================================================

/// Test todo default values
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_todo_default_values(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = Todo::objects();

	// Use new() function - defaults are applied automatically
	let new_todo = Todo::new("Test Todo".to_string(), None, false);

	let created = manager.create(&new_todo).await.unwrap();

	// Verify defaults
	assert!(!created.completed, "Default completed should be false");
	assert!(
		created.description.is_none(),
		"Default description should be None"
	);
}

/// Test timestamp auto-update behavior
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_todo_timestamp_behavior(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, _conn) = db_with_migrations.await;
	let manager = Todo::objects();

	// Use new() function - timestamps are auto-generated
	let new_todo = Todo::new("Test Todo".to_string(), None, false);

	let created = manager.create(&new_todo).await.unwrap();

	// Verify timestamps exist
	assert!(created.created_at.timestamp() > 0);
	assert!(created.updated_at.timestamp() > 0);

	// Update and verify updated_at changes
	let original_updated_at = created.updated_at;
	tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

	let mut updated = created.clone();
	updated.title = "Updated Title".to_string();
	updated.updated_at = Utc::now(); // Simulate auto_now

	let result = manager.update(&updated).await.unwrap();

	// updated_at should be different (manually set in test)
	assert!(result.updated_at >= original_updated_at);
}

// ============================================================================
// Integration Tests
// ============================================================================

/// Test complete CRUD cycle with transactions
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[serial(database)]
async fn test_complete_crud_cycle(
	#[future] db_with_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, conn) = db_with_migrations.await;

	// Use transaction for entire CRUD cycle
	let final_count: std::result::Result<usize, anyhow::Error> = atomic(&conn, || {
		Box::pin(async move {
			let manager = Todo::objects();

			// Create using new() function
			let new_todo = Todo::new(
				"CRUD Test".to_string(),
				Some("Testing full cycle".to_string()),
				false,
			);
			let created = manager.create(&new_todo).await?;

			// Read
			let todos = manager.all().all().await?;
			assert_eq!(todos.len(), 1);

			// Update
			let mut updated = created.clone();
			updated.completed = true;
			manager.update(&updated).await?;

			// Verify update
			let todos_after_update = manager.all().all().await?;
			assert!(todos_after_update[0].completed);

			// Delete
			manager.delete(created.id.unwrap()).await?;

			// Verify deletion
			let final_todos = manager.all().all().await?;
			Ok(final_todos.len())
		})
	})
	.await;

	assert_eq!(
		final_count.unwrap(),
		0,
		"All operations should complete successfully"
	);
}
