//! Integration tests for validators with ORM
//!
//! Based on SQLAlchemy test/orm/test_validators.py
//!
//! **USES TESTCONTAINERS**: These tests use TestContainers for PostgreSQL database.
//! Docker Desktop must be running before executing these tests.

use reinhardt_core::validators::{
	MaxLengthValidator, MaxValueValidator, MinLengthValidator, MinValueValidator, RangeValidator,
	ValidationError, Validator,
};
use reinhardt_db::DatabaseConnection;
use reinhardt_db::orm::manager::reinitialize_database;
use reinhardt_integration_tests::{
	migrations::apply_basic_test_migrations, validator_test_common::*,
};
use reinhardt_test::fixtures::postgres_container;
use reinhardt_test::fixtures::validator::{ValidatorDbGuard, validator_db_guard};
use reinhardt_test::resource::TeardownGuard;
use rstest::*;
use serial_test::serial;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

/// Dedicated fixture for Validator ORM integration tests
///
/// Uses postgres_container to obtain a container and
/// applies basic test migrations
#[fixture]
async fn validator_orm_test_db(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, TestDatabase, u16, String) {
	let (container, _pool, port, url) = postgres_container.await;

	// Create ORM DatabaseConnection from URL
	let connection = DatabaseConnection::connect(&url).await.unwrap();

	// Apply basic test migrations using inner BackendsConnection
	apply_basic_test_migrations(connection.inner())
		.await
		.unwrap();

	// Initialize global database connection for ORM Manager API
	reinitialize_database(&url)
		.await
		.expect("Failed to reinitialize database");

	// Create TestDatabase with the connection
	let test_db = TestDatabase {
		connection: Arc::new(connection),
		_container: None, // Container is managed by fixture
	};

	(container, test_db, port, url)
}

#[cfg(test)]
mod scalar_validation_tests {
	use super::*;

	// Based on SQLAlchemy: ValidatorTest::test_scalar
	#[rstest]
	#[tokio::test]
	#[serial(validator_orm_db)]
	async fn test_scalar_field_validation(
		#[future] validator_orm_test_db: (ContainerAsync<GenericImage>, TestDatabase, u16, String),
		_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
	) {
		let (_container, test_db, _port, _database_url) = validator_orm_test_db.await;

		// No need to setup tables - migrations already applied

		// Test username length validation
		let min_validator = MinLengthValidator::new(3);
		let max_validator = MaxLengthValidator::new(100);

		// Valid username
		let valid_username = "john_doe";
		assert!(min_validator.validate(valid_username).is_ok());
		assert!(max_validator.validate(valid_username).is_ok());

		// Insert user with valid username using TestDatabase method
		let user_id = test_db
			.insert_user(valid_username, "john@example.com")
			.await
			.unwrap();
		assert!(user_id > 0);

		// Cleanup is automatic via TeardownGuard and container drop
	}

	// Based on SQLAlchemy: test_validators_dict
	#[test]
	fn test_multiple_field_validators() {
		// Simulate validating multiple fields like SQLAlchemy's validator dict
		let username_min = MinLengthValidator::new(3);
		let username_max = MaxLengthValidator::new(30);
		let email_min = MinLengthValidator::new(5);
		let email_max = MaxLengthValidator::new(255);

		// Valid values
		let username = "testuser";
		let email = "test@example.com";

		assert!(username_min.validate(username).is_ok());
		assert!(username_max.validate(username).is_ok());
		assert!(email_min.validate(email).is_ok());
		assert!(email_max.validate(email).is_ok());

		// Invalid values
		let short_username = "ab";
		assert!(username_min.validate(short_username).is_err());

		let short_email = "a@b";
		assert!(email_min.validate(short_email).is_err());
	}
}

#[cfg(test)]
mod numeric_validation_tests {
	use super::*;

	// Test numeric validators with database constraints
	#[test]
	fn test_price_validation() {
		let min_price = MinValueValidator::new(0.0);
		let max_price = MaxValueValidator::new(999999.99);

		// Valid prices
		assert!(min_price.validate(&0.0).is_ok());
		assert!(min_price.validate(&99.99).is_ok());
		assert!(max_price.validate(&500.0).is_ok());

		// Invalid prices
		assert!(min_price.validate(&-10.0).is_err());
		assert!(max_price.validate(&1000000.0).is_err());
	}

	#[test]
	fn test_stock_quantity_validation() {
		let stock_validator = RangeValidator::new(0, 10000);

		// Valid stock quantities
		assert!(stock_validator.validate(&0).is_ok());
		assert!(stock_validator.validate(&100).is_ok());
		assert!(stock_validator.validate(&10000).is_ok());

		// Invalid stock quantities
		assert!(stock_validator.validate(&-1).is_err());
		assert!(stock_validator.validate(&10001).is_err());
	}

	#[test]
	fn test_quantity_validation() {
		// Order quantity must be positive
		let min_quantity = MinValueValidator::new(1);
		let max_quantity = MaxValueValidator::new(100);

		// Valid quantities
		assert!(min_quantity.validate(&1).is_ok());
		assert!(min_quantity.validate(&50).is_ok());
		assert!(max_quantity.validate(&100).is_ok());

		// Invalid quantities
		assert!(min_quantity.validate(&0).is_err());
		assert!(min_quantity.validate(&-5).is_err());
		assert!(max_quantity.validate(&101).is_err());
	}
}

#[cfg(test)]
mod bulk_operation_validation_tests {
	use super::*;

	// Based on SQLAlchemy: test_validator_bulk_collection_set
	#[test]
	fn test_bulk_validation() {
		let username_validator = MinLengthValidator::new(3);

		// Simulate bulk validation
		let usernames = ["alice", "bob", "charlie", "david"];

		let results: Vec<_> = usernames
			.iter()
			.map(|name| username_validator.validate(*name))
			.collect();

		// All should pass
		for (i, result) in results.iter().enumerate() {
			assert!(
				result.is_ok(),
				"Username '{}' failed validation",
				usernames[i]
			);
		}

		// Test with invalid usernames
		let invalid_usernames = ["ab", "x", ""];
		let invalid_results: Vec<_> = invalid_usernames
			.iter()
			.map(|name| username_validator.validate(*name))
			.collect();

		// All should fail
		for (i, result) in invalid_results.iter().enumerate() {
			assert!(
				result.is_err(),
				"Username '{}' should have failed validation",
				invalid_usernames[i]
			);
		}
	}

	#[test]
	fn test_bulk_email_validation() {
		use reinhardt_core::validators::EmailValidator;

		let email_validator = EmailValidator::new();

		// Valid emails
		let valid_emails = vec!["user1@example.com", "user2@test.org", "admin@company.co.uk"];

		for email in &valid_emails {
			assert!(
				email_validator.validate(*email).is_ok(),
				"Email '{}' should be valid",
				email
			);
		}

		// Invalid emails
		let invalid_emails = vec![
			"notanemail",
			"@example.com",
			"user@",
			"user space@example.com",
		];

		for email in &invalid_emails {
			assert!(
				email_validator.validate(*email).is_err(),
				"Email '{}' should be invalid",
				email
			);
		}
	}
}

#[cfg(test)]
mod constraint_validation_tests {
	use super::*;

	// Test validators that mirror database constraints
	#[test]
	fn test_not_null_with_min_length() {
		let validator = MinLengthValidator::new(1);

		// Empty string should fail
		assert_validation_result(validator.validate(""), false, Some("too short"));

		// Non-empty should pass
		assert!(validator.validate("a").is_ok());
	}

	#[test]
	fn test_check_constraint_validation() {
		// Simulate CHECK (stock >= 0) constraint
		let stock_validator = MinValueValidator::new(0);

		assert!(stock_validator.validate(&0).is_ok());
		assert!(stock_validator.validate(&100).is_ok());
		assert!(stock_validator.validate(&-1).is_err());

		// Simulate CHECK (quantity > 0) constraint
		let quantity_validator = MinValueValidator::new(1);

		assert!(quantity_validator.validate(&1).is_ok());
		assert!(quantity_validator.validate(&100).is_ok());
		assert!(quantity_validator.validate(&0).is_err());
	}

	#[test]
	fn test_unique_constraint_simulation() {
		// This test verifies in-memory uniqueness checking
		// For actual database UNIQUE constraint validation, see test_unique_constraint_database_violation
		let existing_codes = ["PROD001", "PROD002", "PROD003"];

		let new_code = "PROD004";
		assert!(!existing_codes.contains(&new_code));

		let duplicate_code = "PROD001";
		assert!(existing_codes.contains(&duplicate_code));
	}

	#[rstest]
	#[tokio::test]
	#[serial(validator_orm_db)]
	async fn test_unique_constraint_database_violation(
		#[future] validator_orm_test_db: (ContainerAsync<GenericImage>, TestDatabase, u16, String),
		_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
	) {
		use reinhardt_core::validators::UniqueValidator;

		let (_container, test_db, _port, _database_url) = validator_orm_test_db.await;

		// Insert first user successfully using TestDatabase method
		let user1_id = test_db
			.insert_user("alice", "alice@example.com")
			.await
			.unwrap();
		assert!(user1_id > 0);

		// Create a UniqueValidator for username field using pool from connection
		let pool = test_db.connection.inner().into_postgres().unwrap().clone();
		let pool_clone = pool.clone();
		let username_validator = UniqueValidator::new(
			"username",
			Box::new(move |value, exclude_id| {
				let pool = pool_clone.clone();
				Box::pin(async move {
					let query = if let Some(id) = exclude_id {
						sqlx::query_as::<_, (i64,)>(
							"SELECT COUNT(*) FROM test_users WHERE username = $1 AND id != $2",
						)
						.bind(&value)
						.bind(id)
					} else {
						sqlx::query_as::<_, (i64,)>(
							"SELECT COUNT(*) FROM test_users WHERE username = $1",
						)
						.bind(&value)
					};

					let result = query.fetch_one(&pool).await;
					result.map(|(count,)| count > 0).unwrap_or(false)
				})
			}),
		);

		// Validate new username (should pass)
		let result = username_validator.validate_async("bob", None).await;
		assert!(result.is_ok());

		// Validate existing username (should fail with NotUnique error)
		let result = username_validator.validate_async("alice", None).await;
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert_eq!(
			error.to_string(),
			"Field 'username' must be unique. Value 'alice' already exists"
		);

		// Validate updating with same username (should pass when excluding current ID)
		let result = username_validator
			.validate_async("alice", Some(user1_id))
			.await;
		assert!(result.is_ok());

		// Test actual database UNIQUE constraint violation
		// Direct sqlx query to catch UNIQUE constraint error
		let insert_result =
			sqlx::query("INSERT INTO test_users (username, email) VALUES ($1, $2) RETURNING id")
				.bind("alice")
				.bind("different@example.com")
				.fetch_one(&pool)
				.await;
		assert!(insert_result.is_err());

		// Verify error message contains UNIQUE constraint violation
		let error_message = insert_result.unwrap_err().to_string();
		assert!(
			error_message.contains("unique") || error_message.contains("duplicate"),
			"Expected UNIQUE constraint error, got: {}",
			error_message
		);

		// Cleanup handled automatically by TeardownGuard
	}
}

#[cfg(test)]
mod relationship_validation_tests {
	use super::*;

	// Based on SQLAlchemy: test_validator_backrefs
	#[rstest]
	#[tokio::test]
	#[serial(validator_orm_db)]
	async fn test_foreign_key_validation(
		#[future] validator_orm_test_db: (ContainerAsync<GenericImage>, TestDatabase, u16, String),
		_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
	) {
		let (_container, test_db, _port, _database_url) = validator_orm_test_db.await;

		// Insert user and verify it exists using TestDatabase method
		let user_id = test_db
			.insert_user("testuser", "test@example.com")
			.await
			.unwrap();

		// Simple positivity check (legacy validation)
		assert!(user_id > 0);
		let id_validator = MinValueValidator::new(1);
		assert!(id_validator.validate(&user_id).is_ok());

		// Cleanup handled automatically by TeardownGuard
	}

	#[rstest]
	#[tokio::test]
	#[serial(validator_orm_db)]
	async fn test_foreign_key_existence_validation(
		#[future] validator_orm_test_db: (ContainerAsync<GenericImage>, TestDatabase, u16, String),
		_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
	) {
		use reinhardt_core::validators::ExistsValidator;

		let (_container, test_db, _port, _database_url) = validator_orm_test_db.await;

		// Insert user and product for valid FK references using TestDatabase methods
		let user_id = test_db
			.insert_user("alice", "alice@example.com")
			.await
			.unwrap();
		let product_id = test_db
			.insert_product("Laptop", "PROD001", 999.99, 10)
			.await
			.unwrap();

		// Get pool from DatabaseConnection for raw SQL queries
		let pool = test_db.connection.inner().into_postgres().unwrap().clone();

		// Create ExistsValidator for user_id
		let user_pool = pool.clone();
		let user_validator = ExistsValidator::new(
			"user_id",
			"users",
			Box::new(move |value| {
				let pool = user_pool.clone();
				Box::pin(async move {
					if let Ok(id) = value.parse::<i32>() {
						let result = sqlx::query_as::<_, (i64,)>(
							"SELECT COUNT(*) FROM test_users WHERE id = $1",
						)
						.bind(id)
						.fetch_one(&pool)
						.await;
						result.map(|(count,)| count > 0).unwrap_or(false)
					} else {
						false
					}
				})
			}),
		);

		// Create ExistsValidator for product_id
		let product_pool = pool.clone();
		let product_validator = ExistsValidator::new(
			"product_id",
			"products",
			Box::new(move |value| {
				let pool = product_pool.clone();
				Box::pin(async move {
					if let Ok(id) = value.parse::<i32>() {
						let result = sqlx::query_as::<_, (i64,)>(
							"SELECT COUNT(*) FROM test_products WHERE id = $1",
						)
						.bind(id)
						.fetch_one(&pool)
						.await;
						result.map(|(count,)| count > 0).unwrap_or(false)
					} else {
						false
					}
				})
			}),
		);

		// Validate existing user_id (should pass)
		let result = user_validator.validate_async(user_id.to_string()).await;
		assert!(result.is_ok());

		// Validate non-existing user_id (should fail)
		let result = user_validator.validate_async("99999").await;
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert_eq!(
			error.to_string(),
			"Foreign key reference not found: user_id with value 99999 does not exist in users"
		);

		// Validate existing product_id (should pass)
		let result = product_validator
			.validate_async(product_id.to_string())
			.await;
		assert!(result.is_ok());

		// Validate non-existing product_id (should fail)
		let result = product_validator.validate_async("88888").await;
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert!(
			error
				.to_string()
				.contains("Foreign key reference not found")
		);
		assert!(error.to_string().contains("product_id"));
		assert!(error.to_string().contains("88888"));

		// Test actual database FK constraint violation (insert with invalid FK)
		let insert_result: Result<sqlx::postgres::PgQueryResult, sqlx::Error> = sqlx::query(
			"INSERT INTO test_orders (user_id, product_id, quantity) VALUES ($1, $2, $3)",
		)
		.bind(99999) // non-existent user_id
		.bind(product_id)
		.bind(1)
		.execute(&pool)
		.await;

		assert!(insert_result.is_err());

		// Verify error message indicates FK constraint violation
		let error_message = insert_result.unwrap_err().to_string();
		assert!(
			error_message.contains("foreign key") || error_message.contains("violates"),
			"Expected FK constraint error, got: {}",
			error_message
		);

		// Cleanup handled automatically by TeardownGuard
	}

	#[rstest]
	#[tokio::test]
	#[serial(validator_orm_db)]
	async fn test_foreign_key_update_violation(
		#[future] validator_orm_test_db: (ContainerAsync<GenericImage>, TestDatabase, u16, String),
		_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
	) {
		let (_container, test_db, _port, _database_url) = validator_orm_test_db.await;

		// Insert user and product
		let user_id = test_db.insert_user("bob", "bob@example.com").await.unwrap();
		let product_id = test_db
			.insert_product("Mouse", "PROD002", 29.99, 50)
			.await
			.unwrap();

		// Insert valid order
		let order_id = test_db.insert_order(user_id, product_id, 2).await.unwrap();
		assert!(order_id > 0);

		// Attempt to update order with non-existent user_id (should fail)
		let pool = test_db.connection.inner().into_postgres().unwrap().clone();
		let update_result: Result<sqlx::postgres::PgQueryResult, sqlx::Error> =
			sqlx::query("UPDATE test_orders SET user_id = $1 WHERE id = $2")
				.bind(99999)
				.bind(order_id)
				.execute(&pool)
				.await;

		assert!(update_result.is_err());
		let error_message = update_result.unwrap_err().to_string();
		assert!(
			error_message.contains("foreign key") || error_message.contains("violates"),
			"Expected FK constraint error on update, got: {}",
			error_message
		);

		// Cleanup handled automatically by TeardownGuard
	}

	#[rstest]
	#[tokio::test]
	#[serial(validator_orm_db)]
	async fn test_foreign_key_cascade_delete(
		#[future] validator_orm_test_db: (ContainerAsync<GenericImage>, TestDatabase, u16, String),
		_validator_db_guard: TeardownGuard<ValidatorDbGuard>,
	) {
		let (_container, test_db, _port, _database_url) = validator_orm_test_db.await;

		// Insert user and product
		let user_id = test_db
			.insert_user("charlie", "charlie@example.com")
			.await
			.unwrap();
		let product_id = test_db
			.insert_product("Keyboard", "PROD003", 79.99, 30)
			.await
			.unwrap();

		// Insert order referencing both
		let order_id = test_db.insert_order(user_id, product_id, 1).await.unwrap();
		assert!(order_id > 0);

		// Attempt to delete user (should fail because of FK constraint)
		// Note: test_orders table does NOT have ON DELETE CASCADE
		let pool = test_db.connection.inner().into_postgres().unwrap().clone();
		let delete_result: Result<sqlx::postgres::PgQueryResult, sqlx::Error> =
			sqlx::query("DELETE FROM test_users WHERE id = $1")
				.bind(user_id)
				.execute(&pool)
				.await;

		assert!(delete_result.is_err());
		let error_message = delete_result.unwrap_err().to_string();
		assert!(
			error_message.contains("foreign key")
				|| error_message.contains("violates")
				|| error_message.contains("still referenced"),
			"Expected FK constraint error on delete, got: {}",
			error_message
		);

		// Verify user still exists (delete was prevented)
		let user_exists = test_db.user_exists(user_id).await.unwrap();
		assert!(user_exists);

		// Cleanup handled automatically by TeardownGuard
	}
}

#[cfg(test)]
mod validator_composition_with_orm_tests {
	use super::*;

	#[test]
	fn test_combined_validators_for_model_field() {
		// Simulate validating a model field with multiple validators
		struct UserValidation {
			username_min: MinLengthValidator,
			username_max: MaxLengthValidator,
		}

		impl UserValidation {
			fn new() -> Self {
				Self {
					username_min: MinLengthValidator::new(3),
					username_max: MaxLengthValidator::new(30),
				}
			}

			fn validate_username(&self, username: &str) -> Result<(), Vec<ValidationError>> {
				let mut errors = Vec::new();

				if let Err(e) = self.username_min.validate(username) {
					errors.push(e);
				}
				if let Err(e) = self.username_max.validate(username) {
					errors.push(e);
				}

				if errors.is_empty() {
					Ok(())
				} else {
					Err(errors)
				}
			}
		}

		let validation = UserValidation::new();

		// Valid username
		assert!(validation.validate_username("john_doe").is_ok());

		// Too short
		let result = validation.validate_username("ab");
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().len(), 1);

		// Too long
		let long_username = "a".repeat(50);
		let result = validation.validate_username(&long_username);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().len(), 1);
	}
}
