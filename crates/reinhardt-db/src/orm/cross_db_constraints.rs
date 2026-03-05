//! Cross-database constraint validation
//!
//! This module provides validation for foreign key relationships across different databases,
//! detecting potential issues that cannot be enforced at the database level.
//!
//! ## Django Limitation
//!
//! Django does not support foreign keys or many-to-many relationships across different databases.
//! If you attempt to use such relationships, Django will not enforce referential integrity.
//!
//! ## Reinhardt Approach
//!
//! Reinhardt provides explicit validation and clear error messages when cross-database
//! relationships are detected, helping developers understand and work around database limitations.

use super::database_routing::DatabaseRouter;
use std::sync::Arc;
use thiserror::Error;

/// Errors related to cross-database constraints
#[non_exhaustive]
#[derive(Debug, Error, Clone, PartialEq)]
pub enum CrossDbError {
	/// Foreign key relationship spans multiple databases
	#[error(
		"Foreign key '{field}' from {source_model} ({source_db}) to {target_model} ({target_db}) \
         crosses database boundaries. Cross-database foreign keys are not supported by most databases."
	)]
	ForeignKeyAcrossDatabase {
		source_model: String,
		target_model: String,
		field: String,
		source_db: String,
		target_db: String,
	},

	/// Many-to-many relationship spans multiple databases
	#[error(
		"Many-to-many relationship '{field}' between {source_model} ({source_db}) and \
         {target_model} ({target_db}) crosses database boundaries. Cross-database many-to-many \
         relationships are not supported."
	)]
	ManyToManyAcrossDatabase {
		source_model: String,
		target_model: String,
		field: String,
		source_db: String,
		target_db: String,
	},

	/// One-to-one relationship spans multiple databases
	#[error(
		"One-to-one relationship '{field}' between {source_model} ({source_db}) and \
         {target_model} ({target_db}) crosses database boundaries."
	)]
	OneToOneAcrossDatabase {
		source_model: String,
		target_model: String,
		field: String,
		source_db: String,
		target_db: String,
	},
}

/// Validation mode for cross-database constraints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
	/// Raise an error when cross-database constraints are detected
	Strict,
	/// Log a warning but allow the relationship
	Warn,
	/// Silently allow cross-database relationships (not recommended)
	Allow,
}

/// Validator for cross-database constraints
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::cross_db_constraints::{CrossDbConstraintValidator, ValidationMode};
/// use reinhardt_db::orm::database_routing::DatabaseRouter;
/// use std::sync::Arc;
///
/// let router = DatabaseRouter::new("default")
///     .add_rule("User", "users_db")
///     .add_rule("Order", "orders_db");
///
/// let validator = CrossDbConstraintValidator::new(Arc::new(router))
///     .with_mode(ValidationMode::Strict);
///
/// // This will return an error because User and Order are in different databases
/// let result = validator.validate_foreign_key("Order", "User", "user_id");
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct CrossDbConstraintValidator {
	router: Arc<DatabaseRouter>,
	mode: ValidationMode,
}

impl CrossDbConstraintValidator {
	/// Create a new validator with the given database router
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cross_db_constraints::CrossDbConstraintValidator;
	/// use reinhardt_db::orm::database_routing::DatabaseRouter;
	/// use std::sync::Arc;
	///
	/// let router = DatabaseRouter::new("default");
	/// let validator = CrossDbConstraintValidator::new(Arc::new(router));
	/// ```
	pub fn new(router: Arc<DatabaseRouter>) -> Self {
		Self {
			router,
			mode: ValidationMode::Strict,
		}
	}

	/// Set the validation mode
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cross_db_constraints::{CrossDbConstraintValidator, ValidationMode};
	/// use reinhardt_db::orm::database_routing::DatabaseRouter;
	/// use std::sync::Arc;
	///
	/// let router = DatabaseRouter::new("default");
	/// let validator = CrossDbConstraintValidator::new(Arc::new(router))
	///     .with_mode(ValidationMode::Warn);
	/// ```
	pub fn with_mode(mut self, mode: ValidationMode) -> Self {
		self.mode = mode;
		self
	}

	/// Get the current validation mode
	pub fn mode(&self) -> ValidationMode {
		self.mode
	}

	/// Validate a foreign key relationship
	///
	/// Checks if the source and target models are in the same database.
	/// Returns an error if they are in different databases and validation mode is Strict.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cross_db_constraints::{CrossDbConstraintValidator, ValidationMode};
	/// use reinhardt_db::orm::database_routing::DatabaseRouter;
	/// use std::sync::Arc;
	///
	/// let router = DatabaseRouter::new("default")
	///     .add_rule("User", "db1")
	///     .add_rule("Post", "db1");  // Same database
	///
	/// let validator = CrossDbConstraintValidator::new(Arc::new(router));
	///
	/// // This is OK - both in db1
	/// assert!(validator.validate_foreign_key("Post", "User", "author_id").is_ok());
	/// ```
	pub fn validate_foreign_key(
		&self,
		source_model: &str,
		target_model: &str,
		field: &str,
	) -> Result<(), CrossDbError> {
		let source_db = self.router.db_for_write(source_model);
		let target_db = self.router.db_for_read(target_model);

		if source_db != target_db {
			let error = CrossDbError::ForeignKeyAcrossDatabase {
				source_model: source_model.to_string(),
				target_model: target_model.to_string(),
				field: field.to_string(),
				source_db,
				target_db,
			};

			match self.mode {
				ValidationMode::Strict => Err(error),
				ValidationMode::Warn => {
					eprintln!("WARNING: {}", error);
					Ok(())
				}
				ValidationMode::Allow => Ok(()),
			}
		} else {
			Ok(())
		}
	}

	/// Validate a many-to-many relationship
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cross_db_constraints::CrossDbConstraintValidator;
	/// use reinhardt_db::orm::database_routing::DatabaseRouter;
	/// use std::sync::Arc;
	///
	/// let router = DatabaseRouter::new("default")
	///     .add_rule("User", "db1")
	///     .add_rule("Group", "db2");  // Different database!
	///
	/// let validator = CrossDbConstraintValidator::new(Arc::new(router));
	///
	/// // This will error - different databases
	/// let result = validator.validate_many_to_many("User", "Group", "groups");
	/// assert!(result.is_err());
	/// ```
	pub fn validate_many_to_many(
		&self,
		source_model: &str,
		target_model: &str,
		field: &str,
	) -> Result<(), CrossDbError> {
		let source_db = self.router.db_for_write(source_model);
		let target_db = self.router.db_for_write(target_model);

		if source_db != target_db {
			let error = CrossDbError::ManyToManyAcrossDatabase {
				source_model: source_model.to_string(),
				target_model: target_model.to_string(),
				field: field.to_string(),
				source_db,
				target_db,
			};

			match self.mode {
				ValidationMode::Strict => Err(error),
				ValidationMode::Warn => {
					eprintln!("WARNING: {}", error);
					Ok(())
				}
				ValidationMode::Allow => Ok(()),
			}
		} else {
			Ok(())
		}
	}

	/// Validate a one-to-one relationship
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cross_db_constraints::CrossDbConstraintValidator;
	/// use reinhardt_db::orm::database_routing::DatabaseRouter;
	/// use std::sync::Arc;
	///
	/// let router = DatabaseRouter::new("default");
	/// let validator = CrossDbConstraintValidator::new(Arc::new(router));
	///
	/// // Both use default database
	/// assert!(validator.validate_one_to_one("User", "Profile", "profile").is_ok());
	/// ```
	pub fn validate_one_to_one(
		&self,
		source_model: &str,
		target_model: &str,
		field: &str,
	) -> Result<(), CrossDbError> {
		let source_db = self.router.db_for_write(source_model);
		let target_db = self.router.db_for_write(target_model);

		if source_db != target_db {
			let error = CrossDbError::OneToOneAcrossDatabase {
				source_model: source_model.to_string(),
				target_model: target_model.to_string(),
				field: field.to_string(),
				source_db,
				target_db,
			};

			match self.mode {
				ValidationMode::Strict => Err(error),
				ValidationMode::Warn => {
					eprintln!("WARNING: {}", error);
					Ok(())
				}
				ValidationMode::Allow => Ok(()),
			}
		} else {
			Ok(())
		}
	}

	/// Batch validate multiple relationships
	///
	/// Returns all errors encountered during validation.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cross_db_constraints::{CrossDbConstraintValidator, RelationshipType};
	/// use reinhardt_db::orm::database_routing::DatabaseRouter;
	/// use std::sync::Arc;
	///
	/// let router = DatabaseRouter::new("default")
	///     .add_rule("User", "db1")
	///     .add_rule("Post", "db2")
	///     .add_rule("Comment", "db3");
	///
	/// let validator = CrossDbConstraintValidator::new(Arc::new(router));
	///
	/// let relationships = vec![
	///     ("Post", "User", "author_id", RelationshipType::ForeignKey),
	///     ("Comment", "Post", "post_id", RelationshipType::ForeignKey),
	/// ];
	///
	/// let errors = validator.validate_batch(&relationships);
	/// assert_eq!(errors.len(), 2);  // Both cross database boundaries
	/// ```
	pub fn validate_batch(
		&self,
		relationships: &[(&str, &str, &str, RelationshipType)],
	) -> Vec<CrossDbError> {
		relationships
			.iter()
			.filter_map(|(source, target, field, rel_type)| {
				let result = match rel_type {
					RelationshipType::ForeignKey => {
						self.validate_foreign_key(source, target, field)
					}
					RelationshipType::ManyToMany => {
						self.validate_many_to_many(source, target, field)
					}
					RelationshipType::OneToOne => self.validate_one_to_one(source, target, field),
				};
				result.err()
			})
			.collect()
	}
}

/// Type of relationship between models
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipType {
	/// Foreign key relationship
	ForeignKey,
	/// Many-to-many relationship
	ManyToMany,
	/// One-to-one relationship
	OneToOne,
}

#[cfg(test)]
mod tests {
	use super::*;

	fn create_router() -> Arc<DatabaseRouter> {
		Arc::new(
			DatabaseRouter::new("default")
				.add_rule("User", "users_db")
				.add_rule("Post", "posts_db")
				.add_rule("Comment", "default"),
		)
	}

	#[test]
	fn test_foreign_key_same_database() {
		let router = Arc::new(DatabaseRouter::new("default"));
		let validator = CrossDbConstraintValidator::new(router);

		let result = validator.validate_foreign_key("Post", "User", "author_id");
		assert!(result.is_ok());
	}

	#[test]
	fn test_foreign_key_different_databases_strict() {
		let router = create_router();
		let validator = CrossDbConstraintValidator::new(router).with_mode(ValidationMode::Strict);

		let result = validator.validate_foreign_key("Post", "User", "author_id");
		assert!(result.is_err());
		match result.unwrap_err() {
			CrossDbError::ForeignKeyAcrossDatabase {
				source_model,
				target_model,
				field,
				source_db,
				target_db,
			} => {
				assert_eq!(source_model, "Post");
				assert_eq!(target_model, "User");
				assert_eq!(field, "author_id");
				assert_eq!(source_db, "posts_db");
				assert_eq!(target_db, "users_db");
			}
			_ => panic!("Expected ForeignKeyAcrossDatabase error"),
		}
	}

	#[test]
	fn test_foreign_key_different_databases_warn() {
		let router = create_router();
		let validator = CrossDbConstraintValidator::new(router).with_mode(ValidationMode::Warn);

		let result = validator.validate_foreign_key("Post", "User", "author_id");
		assert!(result.is_ok()); // Warn mode allows the relationship
	}

	#[test]
	fn test_foreign_key_different_databases_allow() {
		let router = create_router();
		let validator = CrossDbConstraintValidator::new(router).with_mode(ValidationMode::Allow);

		let result = validator.validate_foreign_key("Post", "User", "author_id");
		assert!(result.is_ok());
	}

	#[test]
	fn test_many_to_many_different_databases() {
		let router = create_router();
		let validator = CrossDbConstraintValidator::new(router);

		let result = validator.validate_many_to_many("User", "Post", "favorite_posts");
		assert!(result.is_err());
		match result.unwrap_err() {
			CrossDbError::ManyToManyAcrossDatabase { .. } => {}
			_ => panic!("Expected ManyToManyAcrossDatabase error"),
		}
	}

	#[test]
	fn test_one_to_one_same_database() {
		let router = Arc::new(DatabaseRouter::new("default"));
		let validator = CrossDbConstraintValidator::new(router);

		let result = validator.validate_one_to_one("User", "Profile", "profile");
		assert!(result.is_ok());
	}

	#[test]
	fn test_one_to_one_different_databases() {
		let router = create_router();
		let validator = CrossDbConstraintValidator::new(router);

		let result = validator.validate_one_to_one("User", "Post", "featured_post");
		assert!(result.is_err());
	}

	#[test]
	fn test_batch_validation() {
		let router = create_router();
		let validator = CrossDbConstraintValidator::new(router);

		let relationships = vec![
			("Post", "User", "author_id", RelationshipType::ForeignKey), // posts_db -> users_db (cross)
			("Comment", "Post", "post_id", RelationshipType::ForeignKey), // default -> posts_db (cross)
			(
				"User",
				"Post",
				"favorite_posts",
				RelationshipType::ManyToMany,
			), // users_db -> posts_db (cross)
		];

		let errors = validator.validate_batch(&relationships);
		assert_eq!(errors.len(), 3); // All three relationships cross database boundaries
	}

	#[test]
	fn test_error_display() {
		let error = CrossDbError::ForeignKeyAcrossDatabase {
			source_model: "Post".to_string(),
			target_model: "User".to_string(),
			field: "author_id".to_string(),
			source_db: "posts_db".to_string(),
			target_db: "users_db".to_string(),
		};

		let message = error.to_string();
		assert!(message.contains("Post"));
		assert!(message.contains("User"));
		assert!(message.contains("author_id"));
		assert!(message.contains("posts_db"));
		assert!(message.contains("users_db"));
	}

	#[test]
	fn test_validation_mode_getter() {
		let router = Arc::new(DatabaseRouter::new("default"));
		let validator = CrossDbConstraintValidator::new(router).with_mode(ValidationMode::Warn);

		assert_eq!(validator.mode(), ValidationMode::Warn);
	}
}
