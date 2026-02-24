//! GenericForeignKey with database constraints support
//!
//! This module provides enhanced GenericForeignKey functionality with optional
//! database-level foreign key constraints, similar to Django's contenttypes framework.
//!
//! ## Features
//!
//! - Type-safe generic foreign keys
//! - Optional database constraints validation
//! - Integration with ContentType persistence
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
//! use reinhardt_db::contenttypes::ContentType;
//!
//! let mut gfk = GenericForeignKeyField::new();
//! let ct = ContentType::new("blog", "Post").with_id(1);
//!
//! gfk.set(&ct, 42);
//! assert!(gfk.is_set());
//! assert_eq!(gfk.object_id(), Some(42));
//! ```

use serde::{Deserialize, Serialize};

use super::ContentType;

/// Enhanced GenericForeignKey field with constraint support
///
/// Provides a generic foreign key implementation with optional database constraint
/// validation. When database feature is enabled, can validate references against
/// actual database records.
///
/// ## Example
///
/// ```rust
/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
/// use reinhardt_db::contenttypes::ContentType;
///
/// let mut gfk = GenericForeignKeyField::new();
/// let ct = ContentType::new("auth", "User").with_id(5);
///
/// gfk.set(&ct, 123);
/// assert_eq!(gfk.content_type_id(), Some(5));
/// assert_eq!(gfk.object_id(), Some(123));
///
/// gfk.clear();
/// assert!(!gfk.is_set());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenericForeignKeyField {
	content_type_id: Option<i64>,
	object_id: Option<i64>,
}

impl GenericForeignKeyField {
	/// Create a new unset GenericForeignKey field
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	///
	/// let gfk = GenericForeignKeyField::new();
	/// assert!(!gfk.is_set());
	/// ```
	pub fn new() -> Self {
		Self {
			content_type_id: None,
			object_id: None,
		}
	}

	/// Create a GenericForeignKey with values
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	///
	/// let gfk = GenericForeignKeyField::with_values(Some(1), Some(42));
	/// assert!(gfk.is_set());
	/// assert_eq!(gfk.content_type_id(), Some(1));
	/// assert_eq!(gfk.object_id(), Some(42));
	/// ```
	pub fn with_values(content_type_id: Option<i64>, object_id: Option<i64>) -> Self {
		Self {
			content_type_id,
			object_id,
		}
	}

	/// Set the GenericForeignKey to reference a specific object
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	/// use reinhardt_db::contenttypes::ContentType;
	///
	/// let mut gfk = GenericForeignKeyField::new();
	/// let ct = ContentType::new("shop", "Product").with_id(3);
	///
	/// gfk.set(&ct, 99);
	/// assert!(gfk.is_set());
	/// ```
	pub fn set(&mut self, content_type: &ContentType, object_id: i64) {
		self.content_type_id = content_type.id;
		self.object_id = Some(object_id);
	}

	/// Get the content type ID
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	///
	/// let gfk = GenericForeignKeyField::with_values(Some(5), Some(10));
	/// assert_eq!(gfk.content_type_id(), Some(5));
	/// ```
	pub fn content_type_id(&self) -> Option<i64> {
		self.content_type_id
	}

	/// Get the object ID
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	///
	/// let gfk = GenericForeignKeyField::with_values(Some(5), Some(10));
	/// assert_eq!(gfk.object_id(), Some(10));
	/// ```
	pub fn object_id(&self) -> Option<i64> {
		self.object_id
	}

	/// Check if the GenericForeignKey is set
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	///
	/// let gfk = GenericForeignKeyField::new();
	/// assert!(!gfk.is_set());
	///
	/// let gfk = GenericForeignKeyField::with_values(Some(1), Some(1));
	/// assert!(gfk.is_set());
	/// ```
	pub fn is_set(&self) -> bool {
		self.content_type_id.is_some() && self.object_id.is_some()
	}

	/// Clear the GenericForeignKey
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	///
	/// let mut gfk = GenericForeignKeyField::with_values(Some(1), Some(1));
	/// assert!(gfk.is_set());
	///
	/// gfk.clear();
	/// assert!(!gfk.is_set());
	/// ```
	pub fn clear(&mut self) {
		self.content_type_id = None;
		self.object_id = None;
	}

	/// Set content type ID directly
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	///
	/// let mut gfk = GenericForeignKeyField::new();
	/// gfk.set_content_type_id(Some(7));
	/// assert_eq!(gfk.content_type_id(), Some(7));
	/// ```
	pub fn set_content_type_id(&mut self, id: Option<i64>) {
		self.content_type_id = id;
	}

	/// Set object ID directly
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	///
	/// let mut gfk = GenericForeignKeyField::new();
	/// gfk.set_object_id(Some(42));
	/// assert_eq!(gfk.object_id(), Some(42));
	/// ```
	pub fn set_object_id(&mut self, id: Option<i64>) {
		self.object_id = id;
	}

	/// Get the content type from registry
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::generic_fk::GenericForeignKeyField;
	/// use reinhardt_db::contenttypes::{ContentType, CONTENT_TYPE_REGISTRY};
	///
	/// // Register a content type
	/// let ct = CONTENT_TYPE_REGISTRY.register(ContentType::new("blog", "Comment"));
	///
	/// let mut gfk = GenericForeignKeyField::new();
	/// gfk.set(&ct, 100);
	///
	/// let retrieved_ct = gfk.get_content_type();
	/// assert!(retrieved_ct.is_some());
	/// assert_eq!(retrieved_ct.unwrap().model, "Comment");
	/// ```
	pub fn get_content_type(&self) -> Option<ContentType> {
		self.content_type_id
			.and_then(|id| super::CONTENT_TYPE_REGISTRY.get_by_id(id))
	}
}

impl Default for GenericForeignKeyField {
	fn default() -> Self {
		Self::new()
	}
}

/// Database constraint validation for GenericForeignKey
///
/// When the database feature is enabled, this trait provides methods to validate
/// that GenericForeignKey references point to existing database records.
#[cfg(feature = "database")]
pub mod constraints {
	use super::*;
	use crate::contenttypes::persistence::{
		ContentTypePersistence, ContentTypePersistenceBackend, PersistenceError,
	};

	/// Trait for validating GenericForeignKey constraints
	#[async_trait::async_trait]
	pub trait GenericForeignKeyConstraints {
		/// Validate that the GenericForeignKey references an existing content type
		async fn validate_content_type(
			&self,
			persistence: &ContentTypePersistence,
		) -> Result<bool, PersistenceError>;

		/// Get the validated content type from database
		async fn get_validated_content_type(
			&self,
			persistence: &ContentTypePersistence,
		) -> Result<Option<ContentType>, PersistenceError>;
	}

	#[async_trait::async_trait]
	impl GenericForeignKeyConstraints for GenericForeignKeyField {
		async fn validate_content_type(
			&self,
			persistence: &ContentTypePersistence,
		) -> Result<bool, PersistenceError> {
			if let Some(ct_id) = self.content_type_id {
				let ct = persistence.get_by_id(ct_id).await?;
				Ok(ct.is_some())
			} else {
				Ok(false)
			}
		}

		async fn get_validated_content_type(
			&self,
			persistence: &ContentTypePersistence,
		) -> Result<Option<ContentType>, PersistenceError> {
			if let Some(ct_id) = self.content_type_id {
				persistence.get_by_id(ct_id).await
			} else {
				Ok(None)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_generic_fk_field_new() {
		let gfk = GenericForeignKeyField::new();
		assert!(!gfk.is_set());
		assert_eq!(gfk.content_type_id(), None);
		assert_eq!(gfk.object_id(), None);
	}

	#[test]
	fn test_generic_fk_field_with_values() {
		let gfk = GenericForeignKeyField::with_values(Some(5), Some(42));
		assert!(gfk.is_set());
		assert_eq!(gfk.content_type_id(), Some(5));
		assert_eq!(gfk.object_id(), Some(42));
	}

	#[test]
	fn test_generic_fk_field_set() {
		let mut gfk = GenericForeignKeyField::new();
		let ct = ContentType::new("blog", "Post").with_id(3);

		gfk.set(&ct, 99);
		assert!(gfk.is_set());
		assert_eq!(gfk.content_type_id(), Some(3));
		assert_eq!(gfk.object_id(), Some(99));
	}

	#[test]
	fn test_generic_fk_field_clear() {
		let mut gfk = GenericForeignKeyField::with_values(Some(1), Some(1));
		assert!(gfk.is_set());

		gfk.clear();
		assert!(!gfk.is_set());
		assert_eq!(gfk.content_type_id(), None);
		assert_eq!(gfk.object_id(), None);
	}

	#[test]
	fn test_generic_fk_field_setters() {
		let mut gfk = GenericForeignKeyField::new();

		gfk.set_content_type_id(Some(10));
		assert_eq!(gfk.content_type_id(), Some(10));
		assert!(!gfk.is_set()); // Not fully set yet

		gfk.set_object_id(Some(20));
		assert_eq!(gfk.object_id(), Some(20));
		assert!(gfk.is_set()); // Now fully set
	}

	#[test]
	fn test_generic_fk_field_default() {
		let gfk = GenericForeignKeyField::default();
		assert!(!gfk.is_set());
	}

	#[test]
	fn test_generic_fk_field_get_content_type() {
		use crate::contenttypes::CONTENT_TYPE_REGISTRY;

		// Clear registry first
		CONTENT_TYPE_REGISTRY.clear();

		let ct = CONTENT_TYPE_REGISTRY.register(ContentType::new("test", "Model"));
		let mut gfk = GenericForeignKeyField::new();

		gfk.set(&ct, 42);
		let retrieved = gfk.get_content_type();

		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().model, "Model");

		// Clean up
		CONTENT_TYPE_REGISTRY.clear();
	}

	#[test]
	fn test_generic_fk_field_partial_set() {
		let mut gfk = GenericForeignKeyField::new();

		// Only content type ID set
		gfk.set_content_type_id(Some(1));
		assert!(!gfk.is_set());

		// Only object ID set
		let mut gfk2 = GenericForeignKeyField::new();
		gfk2.set_object_id(Some(1));
		assert!(!gfk2.is_set());

		// Both set
		gfk.set_object_id(Some(1));
		assert!(gfk.is_set());
	}

	#[test]
	fn test_generic_fk_field_serialization() {
		let gfk = GenericForeignKeyField::with_values(Some(5), Some(10));

		// Serialize
		let serialized = serde_json::to_string(&gfk).unwrap();
		assert!(serialized.contains("5"));
		assert!(serialized.contains("10"));

		// Deserialize
		let deserialized: GenericForeignKeyField = serde_json::from_str(&serialized).unwrap();
		assert_eq!(deserialized, gfk);
	}

	#[test]
	fn test_generic_fk_field_equality() {
		let gfk1 = GenericForeignKeyField::with_values(Some(1), Some(2));
		let gfk2 = GenericForeignKeyField::with_values(Some(1), Some(2));
		let gfk3 = GenericForeignKeyField::with_values(Some(1), Some(3));

		assert_eq!(gfk1, gfk2);
		assert_ne!(gfk1, gfk3);
	}
}

#[cfg(all(test, feature = "database"))]
mod database_tests {
	use super::*;
	use crate::contenttypes::persistence::{ContentTypePersistence, ContentTypePersistenceBackend};
	use constraints::GenericForeignKeyConstraints;
	use std::sync::{Arc, Once};

	static INIT_DRIVERS: Once = Once::new();

	fn init_drivers() {
		INIT_DRIVERS.call_once(|| {
			sqlx::any::install_default_drivers();
		});
	}

	async fn create_test_persistence() -> ContentTypePersistence {
		init_drivers();

		// Use in-memory SQLite with shared cache mode and single connection
		let db_url = "sqlite::memory:?mode=rwc&cache=shared";

		// Create persistence with minimal connection pool for tests
		use sqlx::pool::PoolOptions;
		let pool = PoolOptions::new()
			.min_connections(1)
			.max_connections(1)
			.connect(db_url)
			.await
			.expect("Failed to connect to test database");

		let persistence = ContentTypePersistence::from_pool(Arc::new(pool), db_url);

		persistence
			.create_table()
			.await
			.expect("Failed to create table");
		persistence
	}

	#[tokio::test]
	async fn test_validate_content_type() {
		let persistence = create_test_persistence().await;

		// Create a content type in database
		let ct = persistence
			.save(&ContentType::new("blog", "Post"))
			.await
			.expect("Failed to save");
		let ct_id = ct.id.unwrap();

		// Create GFK pointing to it
		let gfk = GenericForeignKeyField::with_values(Some(ct_id), Some(42));

		// Should validate successfully
		let valid = gfk
			.validate_content_type(&persistence)
			.await
			.expect("Failed to validate");
		assert!(valid);

		// Create GFK with non-existent content type
		let invalid_gfk = GenericForeignKeyField::with_values(Some(9999), Some(42));
		let valid = invalid_gfk
			.validate_content_type(&persistence)
			.await
			.expect("Failed to validate");
		assert!(!valid);
	}

	#[tokio::test]
	async fn test_get_validated_content_type() {
		let persistence = create_test_persistence().await;

		// Create a content type in database
		let ct = persistence
			.save(&ContentType::new("auth", "User"))
			.await
			.expect("Failed to save");
		let ct_id = ct.id.unwrap();

		// Create GFK pointing to it
		let gfk = GenericForeignKeyField::with_values(Some(ct_id), Some(123));

		// Should retrieve the content type
		let validated_ct = gfk
			.get_validated_content_type(&persistence)
			.await
			.expect("Failed to get validated content type");

		let validated_ct = validated_ct.unwrap();
		assert_eq!(validated_ct.app_label, "auth");
		assert_eq!(validated_ct.model, "User");
	}

	#[tokio::test]
	async fn test_validate_unset_gfk() {
		let persistence = create_test_persistence().await;

		let gfk = GenericForeignKeyField::new();

		// Unset GFK should not validate
		let valid = gfk
			.validate_content_type(&persistence)
			.await
			.expect("Failed to validate");
		assert!(!valid);

		// Should return None
		let ct = gfk
			.get_validated_content_type(&persistence)
			.await
			.expect("Failed to get");
		assert!(ct.is_none());
	}
}
