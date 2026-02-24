//! Nested serializer ORM integration
//!
//! This module provides ORM integration for nested serializers, enabling:
//! - Nested instance creation with transactions
//! - Foreign key constraint handling
//! - Many-to-many relationship management
//! - Cascade operations

use super::{SerializerError, ValidatorError};
use async_trait::async_trait;
use reinhardt_db::orm::{
	Model,
	transaction::{Transaction, TransactionScope, transaction},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Context for nested serializer save operations
///
/// Tracks transaction state, parent relationships, and provides
/// utilities for managing nested instance creation.
#[derive(Debug)]
pub struct NestedSaveContext {
	/// Current transaction (if any)
	pub transaction: Option<Transaction>,
	/// Parent instance data for foreign key resolution
	pub parent_data: HashMap<String, Value>,
	/// Depth of nesting (for preventing infinite recursion)
	pub depth: usize,
	/// Maximum allowed depth
	pub max_depth: usize,
}

impl NestedSaveContext {
	/// Create a new nested save context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_orm::NestedSaveContext;
	///
	/// let context = NestedSaveContext::new();
	/// // Verify context is initialized with default depth settings
	/// assert_eq!(context.depth, 0);
	/// assert_eq!(context.max_depth, 10);
	/// ```
	pub fn new() -> Self {
		Self {
			transaction: None,
			parent_data: HashMap::new(),
			depth: 0,
			max_depth: 10,
		}
	}

	/// Create context with transaction
	pub fn with_transaction(mut self, transaction: Transaction) -> Self {
		self.transaction = Some(transaction);
		self
	}

	/// Add parent data for foreign key resolution
	pub fn with_parent_data(mut self, key: String, value: Value) -> Self {
		self.parent_data.insert(key, value);
		self
	}

	/// Set maximum nesting depth
	pub fn with_max_depth(mut self, max_depth: usize) -> Self {
		self.max_depth = max_depth;
		self
	}

	/// Increment depth and check limit
	pub fn increment_depth(&mut self) -> Result<(), SerializerError> {
		self.depth += 1;
		if self.depth > self.max_depth {
			return Err(SerializerError::Validation(ValidatorError::Custom {
				message: format!("Maximum nesting depth {} exceeded", self.max_depth),
			}));
		}
		Ok(())
	}

	/// Create a child context with incremented depth
	pub fn child_context(&self) -> Result<Self, SerializerError> {
		let child = Self {
			transaction: None, // Transaction is managed at top level
			parent_data: self.parent_data.clone(),
			depth: self.depth + 1,
			max_depth: self.max_depth,
		};

		if child.depth > child.max_depth {
			return Err(SerializerError::Validation(ValidatorError::Custom {
				message: format!("Maximum nesting depth {} exceeded", self.max_depth),
			}));
		}

		Ok(child)
	}

	/// Get parent field value
	pub fn get_parent_value(&self, key: &str) -> Option<&Value> {
		self.parent_data.get(key)
	}

	/// Execute nested operation within transaction scope
	///
	/// Automatically chooses between top-level transaction and nested savepoint
	/// based on current depth.
	///
	/// # Examples
	///
	/// ```ignore
	/// let context = NestedSaveContext::new();
	/// // Verify transaction scope handling
	/// let result = context.with_scope(|_tx| async move {
	///     // Perform operations
	///     Ok(value)
	/// }).await?;
	/// ```
	pub async fn with_scope<F, Fut, T>(&self, f: F) -> Result<T, SerializerError>
	where
		F: FnOnce(&mut TransactionScope) -> Fut,
		Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
	{
		if self.depth == 0 {
			// Top-level transaction
			TransactionHelper::with_transaction(f).await
		} else {
			// Nested transaction (savepoint)
			TransactionHelper::savepoint(self.depth, f).await
		}
	}
}

impl Default for NestedSaveContext {
	fn default() -> Self {
		Self::new()
	}
}

/// Trait for nested serializers that can save to database
///
/// Provides methods for creating and updating nested instances
/// within transactions.
#[async_trait]
pub trait NestedSerializerSave
where
	Self: Sized,
{
	/// The model type this nested serializer works with
	type Model: Model + Serialize + DeserializeOwned + Clone + Send + Sync;
	/// The parent model type (for foreign key resolution)
	type ParentModel: Model + Send + Sync;

	/// Create nested instance with transaction support
	///
	/// This method:
	/// 1. Validates nested data
	/// 2. Resolves foreign keys to parent
	/// 3. Creates instance within transaction
	/// 4. Returns created instance
	///
	/// # Errors
	///
	/// Returns `SerializerError` if:
	/// - Validation fails
	/// - Foreign key resolution fails
	/// - Database operation fails
	/// - Maximum depth exceeded
	async fn create_nested(
		data: Value,
		context: &mut NestedSaveContext,
	) -> Result<Self::Model, SerializerError>;

	/// Update nested instance with transaction support
	///
	/// This method:
	/// 1. Validates update data
	/// 2. Merges with existing instance
	/// 3. Updates within transaction
	/// 4. Returns updated instance
	async fn update_nested(
		instance: Self::Model,
		data: Value,
		context: &mut NestedSaveContext,
	) -> Result<Self::Model, SerializerError>;

	/// Resolve foreign key to parent instance
	///
	/// Extracts parent primary key from context and sets it
	/// on the nested instance data.
	fn resolve_parent_fk(
		data: &mut Value,
		context: &NestedSaveContext,
		fk_field: &str,
		parent_pk_field: &str,
	) -> Result<(), SerializerError> {
		if let Some(parent_pk) = context.get_parent_value(parent_pk_field) {
			if let Value::Object(map) = data {
				map.insert(fk_field.to_string(), parent_pk.clone());
			}
		} else {
			return Err(SerializerError::Validation(ValidatorError::RequiredField {
				field_name: parent_pk_field.to_string(),
				message: format!(
					"Parent primary key '{}' not found in context",
					parent_pk_field
				),
			}));
		}
		Ok(())
	}
}

/// Many-to-many relationship manager
///
/// Handles creation and updates of many-to-many relationships
/// through junction tables.
#[derive(Debug, Clone)]
pub struct ManyToManyManager<T, R>
where
	T: Model,
	R: Model,
{
	/// Source model type
	_source: PhantomData<T>,
	/// Target model type
	_target: PhantomData<R>,
	/// Junction table name
	pub junction_table: String,
	/// Source foreign key field name
	pub source_fk: String,
	/// Target foreign key field name
	pub target_fk: String,
}

impl<T, R> ManyToManyManager<T, R>
where
	T: Model,
	R: Model,
{
	/// Create a new many-to-many manager
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_rest::serializers::nested_orm_integration::ManyToManyManager;
	///
	/// // Verify manager is created with junction table configuration
	/// let manager = ManyToManyManager::<User, Group>::new(
	///     "user_groups",
	///     "user_id",
	///     "group_id",
	/// );
	/// ```
	pub fn new(
		junction_table: impl Into<String>,
		source_fk: impl Into<String>,
		target_fk: impl Into<String>,
	) -> Self {
		Self {
			_source: PhantomData,
			_target: PhantomData,
			junction_table: junction_table.into(),
			source_fk: source_fk.into(),
			target_fk: target_fk.into(),
		}
	}

	/// Add relationships in bulk
	///
	/// Creates junction table entries for all provided target IDs.
	///
	/// # Examples
	///
	/// ```ignore
	/// // Verify bulk relationship creation
	/// manager.add_bulk(&user_id, vec![group1_id, group2_id]).await?;
	/// ```
	pub async fn add_bulk(
		&self,
		source_id: &T::PrimaryKey,
		target_ids: Vec<R::PrimaryKey>,
	) -> Result<(), SerializerError>
	where
		T::PrimaryKey: std::fmt::Display,
		R::PrimaryKey: std::fmt::Display,
	{
		use reinhardt_db::orm::manager::get_connection;

		if target_ids.is_empty() {
			return Ok(());
		}

		// Build bulk INSERT query
		let values: Vec<String> = target_ids
			.iter()
			.map(|target_id| format!("({}, {})", source_id, target_id))
			.collect();

		let query = format!(
			"INSERT INTO {} ({}, {}) VALUES {}",
			self.junction_table,
			self.source_fk,
			self.target_fk,
			values.join(", ")
		);

		let conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;

		conn.execute(&query, vec![])
			.await
			.map_err(|e| SerializerError::Other {
				message: format!("Failed to add M2M relationships: {}", e),
			})?;

		Ok(())
	}

	/// Remove relationships in bulk
	///
	/// Deletes junction table entries for provided target IDs.
	pub async fn remove_bulk(
		&self,
		source_id: &T::PrimaryKey,
		target_ids: Vec<R::PrimaryKey>,
	) -> Result<(), SerializerError>
	where
		T::PrimaryKey: std::fmt::Display,
		R::PrimaryKey: std::fmt::Display,
	{
		use reinhardt_db::orm::manager::get_connection;

		if target_ids.is_empty() {
			return Ok(());
		}

		// Build DELETE query with IN clause
		let target_ids_str: Vec<String> = target_ids.iter().map(|id| id.to_string()).collect();

		let query = format!(
			"DELETE FROM {} WHERE {} = {} AND {} IN ({})",
			self.junction_table,
			self.source_fk,
			source_id,
			self.target_fk,
			target_ids_str.join(", ")
		);

		let conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;

		conn.execute(&query, vec![])
			.await
			.map_err(|e| SerializerError::Other {
				message: format!("Failed to remove M2M relationships: {}", e),
			})?;

		Ok(())
	}

	/// Set relationships (replace all existing)
	///
	/// Removes all existing relationships and creates new ones in a single operation.
	///
	/// # Examples
	///
	/// ```ignore
	/// // Verify replacing user's groups atomically
	/// manager.set(&user_id, vec![group1_id, group2_id]).await?;
	/// ```
	pub async fn set(
		&self,
		source_id: &T::PrimaryKey,
		target_ids: Vec<R::PrimaryKey>,
	) -> Result<(), SerializerError>
	where
		T::PrimaryKey: std::fmt::Display,
		R::PrimaryKey: std::fmt::Display,
	{
		// Clear existing relationships, then add new ones
		self.clear(source_id).await?;
		self.add_bulk(source_id, target_ids).await?;
		Ok(())
	}

	/// Clear all relationships for source instance
	///
	/// Deletes all junction table entries for the given source ID.
	///
	/// # Examples
	///
	/// ```ignore
	/// // Verify clearing all user's group memberships
	/// manager.clear(&user_id).await?;
	/// ```
	pub async fn clear(&self, source_id: &T::PrimaryKey) -> Result<(), SerializerError>
	where
		T::PrimaryKey: std::fmt::Display,
	{
		use reinhardt_db::orm::manager::get_connection;

		let query = format!(
			"DELETE FROM {} WHERE {} = {}",
			self.junction_table, self.source_fk, source_id
		);

		let conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;

		conn.execute(&query, vec![])
			.await
			.map_err(|e| SerializerError::Other {
				message: format!("Failed to clear M2M relationships: {}", e),
			})?;

		Ok(())
	}
}

/// Transaction helper for nested operations
///
/// Provides utilities for managing transactions during nested
/// instance creation.
pub struct TransactionHelper;

impl TransactionHelper {
	/// Execute operations within a transaction
	///
	/// Creates a transaction, executes the provided function,
	/// and commits or rolls back based on the result.
	///
	/// # Examples
	///
	/// Execute function within a database transaction using RAII pattern
	///
	/// This method uses `TransactionScope` which automatically handles:
	/// - BEGIN on creation
	/// - COMMIT on explicit commit
	/// - ROLLBACK on drop (if not committed)
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_rest::serializers::nested_orm_integration::TransactionHelper;
	///
	/// // Verify transaction scope with automatic commit/rollback
	/// let result = TransactionHelper::with_transaction(|_tx| async move {
	///     // Perform database operations within transaction
	///     Ok(created_instance)
	/// }).await?;
	/// ```
	pub async fn with_transaction<F, Fut, T>(f: F) -> Result<T, SerializerError>
	where
		F: FnOnce(&mut TransactionScope) -> Fut,
		Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
	{
		use reinhardt_db::orm::manager::get_connection;

		// Get database connection
		let conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;

		// Wrap the closure to convert Box<dyn Error> to anyhow::Error
		let wrapped_f = |tx: &mut TransactionScope| {
			let fut = f(tx);
			async move {
				match fut.await {
					Ok(value) => Ok(value),
					Err(e) => Err(anyhow::anyhow!("{}", e)),
				}
			}
		};

		// Use the transaction helper function for automatic commit/rollback
		transaction(&conn, wrapped_f)
			.await
			.map_err(|e| SerializerError::Other {
				message: format!("Transaction failed: {}", e),
			})
	}

	/// Create a savepoint for nested transaction using RAII pattern
	///
	/// This method uses `TransactionScope::begin_nested()` which automatically handles:
	/// - SAVEPOINT creation on begin
	/// - RELEASE SAVEPOINT on explicit commit
	/// - ROLLBACK TO SAVEPOINT on drop (if not committed)
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_rest::serializers::nested_orm_integration::TransactionHelper;
	///
	/// // Verify nested savepoint creation and rollback handling
	/// let result = TransactionHelper::savepoint(2, |_tx| async move {
	///     // Perform nested operations
	///     Ok(result)
	/// }).await?;
	/// ```
	pub async fn savepoint<F, Fut, T>(depth: usize, f: F) -> Result<T, SerializerError>
	where
		F: FnOnce(&mut TransactionScope) -> Fut,
		Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
	{
		use reinhardt_db::orm::manager::get_connection;

		// Get database connection
		let conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;

		// Create a new transaction scope
		let mut tx = TransactionScope::begin(&conn)
			.await
			.map_err(|e| SerializerError::Other {
				message: format!("Failed to begin transaction: {}", e),
			})?;

		// Create savepoint with unique name based on depth
		let savepoint_name = format!("nested_save_sp_{}", depth);
		tx.savepoint(&savepoint_name)
			.await
			.map_err(|e| SerializerError::Other {
				message: format!("Failed to create savepoint: {}", e),
			})?;

		// Execute the closure
		match f(&mut tx).await {
			Ok(result) => {
				// Success - release savepoint and commit
				tx.release_savepoint(&savepoint_name).await.map_err(|e| {
					SerializerError::Other {
						message: format!("Failed to release savepoint: {}", e),
					}
				})?;
				tx.commit().await.map_err(|e| SerializerError::Other {
					message: format!("Failed to commit transaction: {}", e),
				})?;
				Ok(result)
			}
			Err(e) => {
				// Error - rollback to savepoint and then rollback transaction
				let _ = tx.rollback_to_savepoint(&savepoint_name).await;
				let _ = tx.rollback().await;
				Err(SerializerError::Other {
					message: format!("Savepoint operation failed: {}", e),
				})
			}
		}
	}
}
