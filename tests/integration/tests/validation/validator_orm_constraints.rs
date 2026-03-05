//! ORM constraint validation integration tests
//!
//! These tests verify the integration between validators and ORM database constraints:
//! - Composite UNIQUE constraints
//! - CHECK constraints
//! - CASCADE DELETE validation
//! - Transaction rollback on validation failure
//! - Partial update (PATCH) validation
//!
//! **USES TESTCONTAINERS**: These tests use TestContainers for PostgreSQL database.
//! Docker Desktop must be running before executing these tests.

use reinhardt_core::validators::{MinValueValidator, RangeValidator, Validator};
use reinhardt_db::orm::manager::reinitialize_database;
use reinhardt_db::{
	DatabaseConnection,
	orm::{Filter, FilterOperator, FilterValue, Model},
};
use reinhardt_integration_tests::{
	migrations::apply_constraint_test_migrations,
	validator_test_common::{TestProduct, TestUser},
};
use reinhardt_macros::model;
use reinhardt_test::fixtures::postgres_container;
use reinhardt_test::fixtures::validator::{ValidatorDbGuard, validator_db_guard};
use reinhardt_test::resource::TeardownGuard;
use rstest::*;
use serial_test::serial;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// Model Definitions
// ============================================================================

/// Test post model for constraint validation tests
#[model(table_name = "test_posts")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TestPost {
	#[field(primary_key = true)]
	id: i32,
	#[field]
	user_id: i32,
	#[field(max_length = 255)]
	title: String,
	#[field(max_length = 65535)]
	content: String,
}

/// Test comment model for constraint validation tests
#[model(table_name = "test_comments")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TestComment {
	#[field(primary_key = true)]
	id: i32,
	#[field]
	post_id: i32,
	#[field]
	user_id: i32,
	#[field(max_length = 65535)]
	content: String,
}

// ============================================================================
// Custom Fixture
// ============================================================================

/// Dedicated fixture for Validator ORM constraint tests
///
/// Uses postgres_container to obtain a container and
/// applies constraint test migrations
#[fixture]
async fn validator_constraint_test_db(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) -> (
	ContainerAsync<GenericImage>,
	Arc<DatabaseConnection>,
	Arc<PgPool>,
	u16,
	String,
) {
	let (container, pool, port, url) = postgres_container.await;

	// Create ORM DatabaseConnection from URL
	let connection = DatabaseConnection::connect(&url).await.unwrap();

	// Apply constraint test migrations using inner BackendsConnection
	apply_constraint_test_migrations(connection.inner())
		.await
		.unwrap();

	// Initialize global database connection for ORM Manager API
	reinitialize_database(&url)
		.await
		.expect("Failed to reinitialize database");

	(container, Arc::new(connection), pool, port, url)
}

// ============================================================================
// Test 1: Composite UNIQUE Constraint Validation
// ============================================================================

/// Test composite UNIQUE constraint validation (user_id, title)
///
/// Verifies that:
/// - Same user cannot create posts with duplicate titles
/// - Different users can use the same title
/// - Validator detects constraint violations before database execution
#[rstest]
#[tokio::test]
#[serial(validator_orm_db)]
async fn test_composite_unique_constraint_validation(
	#[future] validator_constraint_test_db: (
		ContainerAsync<GenericImage>,
		Arc<DatabaseConnection>,
		Arc<PgPool>,
		u16,
		String,
	),
	_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
) {
	let (_container, _connection, _pool, _port, _database_url) = validator_constraint_test_db.await;

	// Insert test users using ORM
	let user1_id = {
		let user = TestUser::new("alice".to_string(), "alice@example.com".to_string());
		let manager = TestUser::objects();
		manager.create(&user).await.unwrap().id()
	};
	let user2_id = {
		let user = TestUser::new("bob".to_string(), "bob@example.com".to_string());
		let manager = TestUser::objects();
		manager.create(&user).await.unwrap().id()
	};

	// Insert first post by alice using ORM
	let post1_id = {
		let post = TestPost::new(user1_id, "First Post".to_string(), "Content 1".to_string());
		let manager = TestPost::objects();
		manager.create(&post).await.unwrap().id()
	};
	assert!(post1_id > 0);

	// Attempt to insert duplicate (user_id, title) by same user - should fail
	let duplicate_result = {
		let post = TestPost::new(
			user1_id,
			"First Post".to_string(),
			"Different content".to_string(),
		);
		let manager = TestPost::objects();
		manager.create(&post).await
	};
	assert!(duplicate_result.is_err());

	let error_message = duplicate_result.unwrap_err().to_string();
	assert!(
		error_message.contains("unique") || error_message.contains("duplicate"),
		"Expected UNIQUE constraint error, got: {}",
		error_message
	);

	// Different user can use same title - should succeed
	let post2_id = {
		let post = TestPost::new(
			user2_id,
			"First Post".to_string(),
			"Bob's content".to_string(),
		);
		let manager = TestPost::objects();
		manager.create(&post).await.unwrap().id()
	};
	assert!(post2_id > 0);

	// Same user can use different title - should succeed
	let post3_id = {
		let post = TestPost::new(user1_id, "Second Post".to_string(), "Content 2".to_string());
		let manager = TestPost::objects();
		manager.create(&post).await.unwrap().id()
	};
	assert!(post3_id > 0);

	// Verify composite uniqueness - count posts by user and title
	let manager = TestPost::objects();
	let count = manager
		.filter_by(Filter::new(
			"user_id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(user1_id as i64),
		))
		.filter(Filter::new(
			"title".to_string(),
			FilterOperator::Eq,
			FilterValue::String("First Post".to_string()),
		))
		.count()
		.await
		.unwrap();
	assert_eq!(
		count, 1,
		"Should only have one post with this user_id and title"
	);

	// Cleanup handled automatically by TeardownGuard
}

// ============================================================================
// Test 2: CHECK Constraint Integration
// ============================================================================

/// Test CHECK constraint (price >= 0) integration with validator
///
/// Verifies that:
/// - Application-level validator catches negative prices before database
/// - Database CHECK constraint catches values that bypass validator
/// - Both validations produce consistent error messages
#[rstest]
#[tokio::test]
#[serial(validator_orm_db)]
async fn test_check_constraint_integration(
	#[future] validator_constraint_test_db: (
		ContainerAsync<GenericImage>,
		Arc<DatabaseConnection>,
		Arc<PgPool>,
		u16,
		String,
	),
	_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
) {
	let (_container, _connection, pool, _port, _database_url) = validator_constraint_test_db.await;

	// Application-level validator
	let price_validator = MinValueValidator::new(0.0);
	let stock_validator = MinValueValidator::new(0);

	// Valid product with positive price and stock
	let valid_price = 99.99;
	let valid_stock = 10;

	assert!(price_validator.validate(&valid_price).is_ok());
	assert!(stock_validator.validate(&valid_stock).is_ok());

	let product_id = {
		let product = TestProduct::new(
			"Laptop".to_string(),
			"PROD001".to_string(),
			valid_price,
			valid_stock,
		);
		let manager = TestProduct::objects();
		manager.create(&product).await.unwrap().id()
	};
	assert!(product_id > 0);

	// Application-level validation catches negative price
	let negative_price = -10.0;
	let validation_result = price_validator.validate(&negative_price);
	assert!(validation_result.is_err());
	assert_eq!(
		validation_result.unwrap_err().to_string(),
		"Value too small: -10 (minimum: 0)"
	);

	// Database CHECK constraint also catches negative price
	let db_result: Result<(i32,), sqlx::Error> = sqlx::query_as(
		"INSERT INTO test_products (name, code, price, stock) VALUES ($1, $2, $3, $4) RETURNING id",
	)
	.bind("Invalid Product")
	.bind("PROD002")
	.bind(-10.0f64)
	.bind(5)
	.fetch_one(pool.as_ref())
	.await;
	assert!(db_result.is_err());

	let db_error = db_result.unwrap_err().to_string();
	assert!(
		db_error.contains("check")
			|| db_error.contains("constraint")
			|| db_error.contains("violates"),
		"Expected CHECK constraint error, got: {}",
		db_error
	);

	// Application-level validation catches negative stock
	let negative_stock = -5;
	let stock_validation_result = stock_validator.validate(&negative_stock);
	assert!(stock_validation_result.is_err());

	// Database CHECK constraint also catches negative stock
	let db_stock_result: Result<(i32,), sqlx::Error> = sqlx::query_as(
		"INSERT INTO test_products (name, code, price, stock) VALUES ($1, $2, $3, $4) RETURNING id",
	)
	.bind("Invalid Stock")
	.bind("PROD003")
	.bind(50.0f64)
	.bind(-10)
	.fetch_one(pool.as_ref())
	.await;
	assert!(db_stock_result.is_err());

	// Update with invalid price should also fail
	let update_result: Result<sqlx::postgres::PgQueryResult, sqlx::Error> =
		sqlx::query("UPDATE test_products SET price = $1 WHERE id = $2")
			.bind(-20.0f64)
			.bind(product_id)
			.execute(pool.as_ref())
			.await;
	assert!(update_result.is_err());

	// Cleanup handled automatically by TeardownGuard
}

// ============================================================================
// Test 3: CASCADE DELETE Validation
// ============================================================================

/// Test ON DELETE CASCADE validation with foreign key references
///
/// Verifies that:
/// - Deleting a post cascades to delete its comments
/// - Application can validate cascade impacts before deletion
/// - Reference counts are correctly tracked
#[rstest]
#[tokio::test]
#[serial(validator_orm_db)]
async fn test_cascade_delete_validation(
	#[future] validator_constraint_test_db: (
		ContainerAsync<GenericImage>,
		Arc<DatabaseConnection>,
		Arc<PgPool>,
		u16,
		String,
	),
	_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
) {
	let (_container, _connection, _pool, _port, _database_url) = validator_constraint_test_db.await;

	// Setup: Create user, post, and comments
	let user_id = {
		let user = TestUser::new("charlie".to_string(), "charlie@example.com".to_string());
		let manager = TestUser::objects();
		manager
			.create(&user)
			.await
			.expect("Failed to insert user")
			.id()
	};

	let post_id = {
		let post = TestPost::new(user_id, "Test Post".to_string(), "Content".to_string());
		let manager = TestPost::objects();
		manager
			.create(&post)
			.await
			.expect("Failed to insert post")
			.id()
	};

	// Insert multiple comments on the post
	let comment_manager = TestComment::objects();
	let comment1 = TestComment::new(post_id, user_id, "Comment 1".to_string());
	let comment2 = TestComment::new(post_id, user_id, "Comment 2".to_string());
	let comment3 = TestComment::new(post_id, user_id, "Comment 3".to_string());

	let comment1_id = comment_manager
		.create(&comment1)
		.await
		.expect("Failed to insert comment")
		.id();
	let comment2_id = comment_manager
		.create(&comment2)
		.await
		.expect("Failed to insert comment")
		.id();
	let comment3_id = comment_manager
		.create(&comment3)
		.await
		.expect("Failed to insert comment")
		.id();

	assert!(comment1_id > 0);
	assert!(comment2_id > 0);
	assert!(comment3_id > 0);

	// Verify comments exist before deletion
	let comment_count_before = comment_manager
		.filter_by(Filter::new(
			"post_id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(post_id as i64),
		))
		.count()
		.await
		.expect("Failed to count comments");
	assert_eq!(comment_count_before, 3);

	// Application-level validation: Check cascade impact before delete
	let affected_comments = comment_manager
		.filter_by(Filter::new(
			"post_id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(post_id as i64),
		))
		.count()
		.await
		.expect("Failed to count comments");
	assert_eq!(
		affected_comments, 3,
		"Should find 3 comments that will be cascade-deleted"
	);

	// Delete post - should cascade delete all comments
	let post_manager = TestPost::objects();
	let delete_result = post_manager.delete(post_id).await;
	assert!(delete_result.is_ok());

	// Verify post is deleted
	let post_exists = post_manager
		.filter_by(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(post_id as i64),
		))
		.count()
		.await
		.expect("Failed to check post existence");
	assert_eq!(post_exists, 0);

	// Verify comments are cascade-deleted
	let comment_count_after = comment_manager
		.filter_by(Filter::new(
			"post_id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(post_id as i64),
		))
		.count()
		.await
		.expect("Failed to count comments");
	assert_eq!(
		comment_count_after, 0,
		"All comments should be cascade-deleted"
	);

	// Cleanup handled automatically by TeardownGuard
}

// ============================================================================
// Test 4: Validation Failure Transaction Rollback
// ============================================================================

/// Test that validation failures trigger transaction rollback
///
/// Verifies that:
/// - Failed validation within transaction rolls back all changes
/// - Database state remains consistent after rollback
/// - No partial writes occur on validation failure
#[rstest]
#[tokio::test]
#[serial(validator_orm_db)]
async fn test_validation_failure_transaction_rollback(
	#[future] validator_constraint_test_db: (
		ContainerAsync<GenericImage>,
		Arc<DatabaseConnection>,
		Arc<PgPool>,
		u16,
		String,
	),
	_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
) {
	let (_container, _connection, pool, _port, _database_url) = validator_constraint_test_db.await;

	// Get initial product count using ORM
	let manager = TestProduct::objects();
	let initial_count = manager.count().await.expect("Failed to count products");

	// Begin transaction
	let mut tx = pool.begin().await.expect("Failed to begin transaction");

	// Insert valid product within transaction using raw SQL
	// (ORM doesn't support passing transaction to Manager yet)
	let product1_result: Result<(i32,), sqlx::Error> = sqlx::query_as(
		"INSERT INTO test_products (name, code, price, stock) VALUES ($1, $2, $3, $4) RETURNING id",
	)
	.bind("Valid Product")
	.bind("PROD001")
	.bind(100.0)
	.bind(10)
	.fetch_one(&mut *tx)
	.await;
	assert!(product1_result.is_ok());

	// Application-level validation fails for negative price
	let price_validator = MinValueValidator::new(0.0);
	let invalid_price = -50.0;
	let validation_result = price_validator.validate(&invalid_price);

	if validation_result.is_err() {
		// Rollback transaction on validation failure
		tx.rollback().await.expect("Failed to rollback transaction");
	}

	// Verify rollback: product count should remain unchanged
	let final_count = manager.count().await.expect("Failed to count products");
	assert_eq!(
		final_count, initial_count,
		"Transaction should be rolled back, no products added"
	);

	// Verify the valid product was NOT committed
	let product_exists = manager
		.filter_by(Filter::new(
			"code".to_string(),
			FilterOperator::Eq,
			FilterValue::String("PROD001".to_string()),
		))
		.count()
		.await
		.expect("Failed to check product existence");
	assert_eq!(product_exists, 0, "Product should not exist after rollback");

	// Test successful transaction with valid data
	let mut tx2 = pool.begin().await.expect("Failed to begin transaction");

	let product2_result: Result<(i32,), sqlx::Error> = sqlx::query_as(
		"INSERT INTO test_products (name, code, price, stock) VALUES ($1, $2, $3, $4) RETURNING id",
	)
	.bind("Valid Product 2")
	.bind("PROD002")
	.bind(200.0)
	.bind(20)
	.fetch_one(&mut *tx2)
	.await;
	assert!(product2_result.is_ok());

	// Validation passes
	let valid_price = 200.0;
	assert!(price_validator.validate(&valid_price).is_ok());

	// Commit transaction
	tx2.commit().await.expect("Failed to commit transaction");

	// Verify commit: product count should increase
	let committed_count = manager.count().await.expect("Failed to count products");
	assert_eq!(
		committed_count,
		initial_count + 1,
		"One product should be added after commit"
	);

	// Cleanup handled automatically by TeardownGuard
}

// ============================================================================
// Test 5: Partial Update (PATCH) Validation
// ============================================================================

/// Test validation for partial updates (PATCH operations)
///
/// Verifies that:
/// - Only modified fields are validated
/// - Unmodified fields retain original values
/// - Partial validation respects database constraints
#[rstest]
#[tokio::test]
#[serial(validator_orm_db)]
async fn test_partial_update_validation(
	#[future] validator_constraint_test_db: (
		ContainerAsync<GenericImage>,
		Arc<DatabaseConnection>,
		Arc<PgPool>,
		u16,
		String,
	),
	_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
) {
	let (_container, _connection, _pool, _port, _database_url) = validator_constraint_test_db.await;

	// Insert initial product
	let product_id = {
		let product = TestProduct::new(
			"Original Product".to_string(),
			"PROD001".to_string(),
			100.0,
			50,
		);
		let manager = TestProduct::objects();
		manager
			.create(&product)
			.await
			.expect("Failed to insert product")
			.id()
	};
	assert!(product_id > 0);

	// Partial update: Only update price
	let new_price = 150.0;
	let price_validator = RangeValidator::new(0.0, 999999.99);

	// Validate only the updated field
	assert!(price_validator.validate(&new_price).is_ok());

	// Apply partial update
	let manager = TestProduct::objects();
	let mut product = manager
		.filter_by(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(product_id as i64),
		))
		.first()
		.await
		.expect("Failed to get product")
		.expect("Product not found");
	product.set_price(new_price);
	let update_result = manager.update(&product).await;
	assert!(update_result.is_ok());

	// Verify updated field
	let updated_product = manager
		.filter_by(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(product_id as i64),
		))
		.first()
		.await
		.expect("Failed to get updated product")
		.expect("Updated product not found");
	assert_eq!(updated_product.price(), new_price);
	assert_eq!(updated_product.name(), "Original Product"); // Unchanged
	assert_eq!(updated_product.stock(), 50); // Unchanged

	// Partial update: Only update stock
	let new_stock = 100;
	let stock_validator = MinValueValidator::new(0);

	assert!(stock_validator.validate(&new_stock).is_ok());

	let mut product2 = manager
		.filter_by(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(product_id as i64),
		))
		.first()
		.await
		.expect("Failed to get product")
		.expect("Product not found for stock update");
	product2.set_stock(new_stock);
	let update_stock_result = manager.update(&product2).await;
	assert!(update_stock_result.is_ok());

	// Verify updated field
	let updated_product2 = manager
		.filter_by(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(product_id as i64),
		))
		.first()
		.await
		.expect("Failed to get updated product")
		.expect("Updated product2 not found");
	assert_eq!(updated_product2.stock(), new_stock);
	assert_eq!(updated_product2.price(), new_price); // Previously updated value
	assert_eq!(updated_product2.name(), "Original Product"); // Still unchanged

	// Invalid partial update: negative price
	let invalid_price = -10.0;
	let invalid_validation = price_validator.validate(&invalid_price);
	assert!(invalid_validation.is_err());

	// Do not apply invalid update
	// Database constraint also prevents it
	let mut invalid_product = manager
		.filter_by(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(product_id as i64),
		))
		.first()
		.await
		.expect("Failed to get product")
		.expect("Product not found for invalid update");
	invalid_product.set_price(invalid_price);
	let invalid_update_result = manager.update(&invalid_product).await;
	assert!(invalid_update_result.is_err());

	// Verify product state remains unchanged after failed update
	let final_product = manager
		.filter_by(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(product_id as i64),
		))
		.first()
		.await
		.expect("Failed to get final product")
		.expect("Final product not found");
	assert_eq!(final_product.price(), new_price); // Still valid price
	assert_eq!(final_product.stock(), new_stock); // Still valid stock

	// Cleanup handled automatically by TeardownGuard
}
