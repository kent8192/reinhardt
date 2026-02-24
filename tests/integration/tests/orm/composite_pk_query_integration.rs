//! Integration tests for composite primary key query execution
//!
//! Tests the `get_composite()` method implementation that executes
//! database queries for records with composite primary keys.
//!
//! # Test Setup
//!
//! Uses `reinhardt_test::fixtures::postgres_container` directly.
//! Each test initializes its own database tables using the container's connection pool.

use reinhardt_db::orm::{QuerySet, composite_pk::PkValue, manager::reinitialize_database};
use reinhardt_macros::model;
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::{collections::HashMap, sync::Arc};
use testcontainers::{ContainerAsync, GenericImage};

/// Test model with composite primary key
#[derive(Serialize, Deserialize)]
#[model(app_label = "test_app", table_name = "post_tags")]
struct PostTag {
	#[field(primary_key = true)]
	post_id: i64,

	#[field(primary_key = true)]
	tag_id: i64,

	#[field(max_length = 200)]
	description: String,
}

impl std::fmt::Display for PostTag {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "PostTag({}, {})", self.post_id, self.tag_id)
	}
}

/// Another test model with composite primary key
#[derive(Serialize, Deserialize)]
#[model(app_label = "test_app", table_name = "user_roles")]
struct UserRole {
	#[field(primary_key = true)]
	user_id: i64,

	#[field(primary_key = true)]
	role_id: i64,

	#[field(max_length = 100, null = true)]
	granted_by: Option<String>,
}

impl std::fmt::Display for UserRole {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "UserRole({}, {})", self.user_id, self.role_id)
	}
}

#[rstest]
#[serial(composite_pk_query)]
#[tokio::test]
async fn test_get_composite_success(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify get_composite() successfully retrieves a record
	// with composite primary key from PostgreSQL database
	// Not intent: Error cases, NULL fields, string PK types, multiple records
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize reinhardt_orm manager with this database
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize database");

	// Create test table using the pool directly
	sqlx::query(
		"CREATE TABLE post_tags (
            post_id BIGINT NOT NULL,
            tag_id BIGINT NOT NULL,
            description VARCHAR(200) NOT NULL,
            PRIMARY KEY (post_id, tag_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create post_tags table");

	// Insert test data using pool directly
	sqlx::query("INSERT INTO post_tags (post_id, tag_id, description) VALUES (1, 10, 'First tag')")
		.execute(&*pool)
		.await
		.expect("Failed to insert test data");

	// Query using composite primary key
	let queryset = QuerySet::<PostTag>::new();
	let mut pk_values = HashMap::new();
	pk_values.insert("post_id".to_string(), PkValue::Int(1));
	pk_values.insert("tag_id".to_string(), PkValue::Int(10));

	let result = queryset.get_composite(&pk_values).await;
	assert!(result.is_ok(), "Query should succeed");

	let post_tag = result.unwrap();
	assert_eq!(post_tag.post_id, 1);
	assert_eq!(post_tag.tag_id, 10);
	assert_eq!(post_tag.description, "First tag");
}

#[rstest]
#[serial(composite_pk_query)]
#[tokio::test]
async fn test_get_composite_not_found(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify get_composite() returns error when querying
	// for non-existent composite primary key
	// Not intent: Success cases, validation errors, NULL handling
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize reinhardt_orm manager
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize database");

	// Create test table
	sqlx::query(
		"CREATE TABLE post_tags (
            post_id BIGINT NOT NULL,
            tag_id BIGINT NOT NULL,
            description VARCHAR(200) NOT NULL,
            PRIMARY KEY (post_id, tag_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create post_tags table");

	// Query for non-existent record
	let queryset = QuerySet::<PostTag>::new();
	let mut pk_values = HashMap::new();
	pk_values.insert("post_id".to_string(), PkValue::Int(999));
	pk_values.insert("tag_id".to_string(), PkValue::Int(999));

	let result = queryset.get_composite(&pk_values).await;
	assert!(result.is_err(), "Query should fail for non-existent record");

	let error = result.unwrap_err();
	assert!(
		error.to_string().contains("No record found"),
		"Error should indicate record not found"
	);
}

#[rstest]
#[serial(composite_pk_query)]
#[tokio::test]
async fn test_get_composite_missing_pk_field(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify get_composite() returns validation error when
	// required primary key field is missing from query parameters
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize reinhardt_orm manager
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize database");

	// Create test table
	sqlx::query(
		"CREATE TABLE post_tags (
            post_id BIGINT NOT NULL,
            tag_id BIGINT NOT NULL,
            description VARCHAR(200) NOT NULL,
            PRIMARY KEY (post_id, tag_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create post_tags table");

	// Query with missing primary key field
	let queryset = QuerySet::<PostTag>::new();
	let mut pk_values = HashMap::new();
	pk_values.insert("post_id".to_string(), PkValue::Int(1));
	// Missing tag_id

	let result = queryset.get_composite(&pk_values).await;
	assert!(result.is_err(), "Query should fail with missing PK field");

	let error = result.unwrap_err();
	assert!(
		error.to_string().contains("validation failed") || error.to_string().contains("missing"),
		"Error should indicate validation failure: {}",
		error
	);
}

#[rstest]
#[serial(composite_pk_query)]
#[tokio::test]
async fn test_get_composite_with_optional_field(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify get_composite() correctly handles optional fields
	// with both NULL and non-NULL values in database records
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize reinhardt_orm manager
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize database");

	// Create user_roles table
	sqlx::query(
		"CREATE TABLE user_roles (
            user_id BIGINT NOT NULL,
            role_id BIGINT NOT NULL,
            granted_by VARCHAR(100),
            PRIMARY KEY (user_id, role_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create user_roles table");

	// Insert test data
	sqlx::query("INSERT INTO user_roles (user_id, role_id, granted_by) VALUES (1, 5, 'admin')")
		.execute(&*pool)
		.await
		.expect("Failed to insert test data");

	sqlx::query("INSERT INTO user_roles (user_id, role_id, granted_by) VALUES (2, 5, NULL)")
		.execute(&*pool)
		.await
		.expect("Failed to insert test data with NULL");

	// Query record with optional field present
	let queryset = QuerySet::<UserRole>::new();
	let mut pk_values = HashMap::new();
	pk_values.insert("user_id".to_string(), PkValue::Int(1));
	pk_values.insert("role_id".to_string(), PkValue::Int(5));

	let result = queryset.get_composite(&pk_values).await;
	assert!(result.is_ok(), "Query should succeed");

	let user_role = result.unwrap();
	assert_eq!(user_role.user_id, 1);
	assert_eq!(user_role.role_id, 5);
	assert_eq!(user_role.granted_by, Some("admin".to_string()));

	// Query record with optional field NULL
	let mut pk_values_null = HashMap::new();
	pk_values_null.insert("user_id".to_string(), PkValue::Int(2));
	pk_values_null.insert("role_id".to_string(), PkValue::Int(5));

	let result_null = queryset.get_composite(&pk_values_null).await;
	assert!(result_null.is_ok(), "Query should succeed for NULL field");

	let user_role_null = result_null.unwrap();
	assert_eq!(user_role_null.user_id, 2);
	assert_eq!(user_role_null.role_id, 5);
	assert_eq!(user_role_null.granted_by, None);
}

#[rstest]
#[serial(composite_pk_query)]
#[tokio::test]
async fn test_get_composite_multiple_records(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify get_composite() successfully retrieves correct record
	// when multiple records exist with different composite keys
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize reinhardt_orm manager
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize database");

	// Create test table
	sqlx::query(
		"CREATE TABLE post_tags (
            post_id BIGINT NOT NULL,
            tag_id BIGINT NOT NULL,
            description VARCHAR(200) NOT NULL,
            PRIMARY KEY (post_id, tag_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create post_tags table");

	// Insert multiple records with same partial key (shouldn't happen with proper PK)
	// This tests the error handling for unexpected database states
	sqlx::query("INSERT INTO post_tags (post_id, tag_id, description) VALUES (10, 20, 'First')")
		.execute(&*pool)
		.await
		.expect("Failed to insert test data");

	sqlx::query("INSERT INTO post_tags (post_id, tag_id, description) VALUES (10, 21, 'Second')")
		.execute(&*pool)
		.await
		.expect("Failed to insert test data");

	// Query should succeed for unique composite key
	let queryset = QuerySet::<PostTag>::new();
	let mut pk_values = HashMap::new();
	pk_values.insert("post_id".to_string(), PkValue::Int(10));
	pk_values.insert("tag_id".to_string(), PkValue::Int(20));

	let result = queryset.get_composite(&pk_values).await;
	assert!(
		result.is_ok(),
		"Query should succeed for unique composite PK"
	);

	let post_tag = result.unwrap();
	assert_eq!(post_tag.post_id, 10);
	assert_eq!(post_tag.tag_id, 20);
	assert_eq!(post_tag.description, "First");
}

#[rstest]
#[serial(composite_pk_query)]
#[tokio::test]
async fn test_get_composite_string_pk(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify get_composite() supports string type as part of
	// composite primary key alongside integer types
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize reinhardt_orm manager
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize database");

	// Create table with string PK component
	sqlx::query(
		"CREATE TABLE string_composite (
            category VARCHAR(50) NOT NULL,
            item_id BIGINT NOT NULL,
            value TEXT,
            PRIMARY KEY (category, item_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create string_composite table");

	// Insert test data
	sqlx::query(
		"INSERT INTO string_composite (category, item_id, value) VALUES ('electronics', 100, 'Laptop')",
	)
	.execute(&*pool)
	.await
	.expect("Failed to insert test data");

	// Define temporary model
	#[derive(Serialize, Deserialize)]
	#[model(app_label = "test_app", table_name = "string_composite")]
	struct StringComposite {
		#[field(primary_key = true, max_length = 50)]
		category: String,

		#[field(primary_key = true)]
		item_id: i64,

		#[field(null = true, max_length = 1000)]
		value: Option<String>,
	}

	impl std::fmt::Display for StringComposite {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			write!(f, "StringComposite({}, {})", self.category, self.item_id)
		}
	}

	// Query with string PK
	let queryset = QuerySet::<StringComposite>::new();
	let mut pk_values = HashMap::new();
	pk_values.insert(
		"category".to_string(),
		PkValue::String("electronics".to_string()),
	);
	pk_values.insert("item_id".to_string(), PkValue::Int(100));

	let result = queryset.get_composite(&pk_values).await;
	assert!(result.is_ok(), "Query should succeed with string PK");

	let record = result.unwrap();
	assert_eq!(record.category, "electronics");
	assert_eq!(record.item_id, 100);
	assert_eq!(record.value, Some("Laptop".to_string()));
}
