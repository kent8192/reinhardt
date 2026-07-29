//! Nested serializer ORM integration
//!
//! This module provides ORM integration for nested serializers, enabling:
//! - Nested instance creation with transactions
//! - Foreign key constraint handling
//! - Many-to-many relationship management
//! - Cascade operations

use super::{SerializerError, ValidatorError};
use async_trait::async_trait;
use reinhardt_core::exception::Error as CoreError;
use reinhardt_db::orm::{
	AtomicTransaction, DatabaseBackend, DatabaseConnection, Model, OrmExecutor,
	execution::convert_values,
};
use reinhardt_query::prelude::{
	Alias, BinOper, DeleteStatement, Expr, ExprTrait, InsertStatement, IntoValue,
	MySqlQueryBuilder, PostgresQueryBuilder, Query, QueryBuilder, SqliteQueryBuilder, Values,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Builds INSERT SQL for the executor's backend.
fn build_insert_sql(statement: &InsertStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_insert(statement),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_insert(statement),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_insert(statement),
	}
}

/// Builds DELETE SQL for the executor's backend.
fn build_delete_sql(statement: &DeleteStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_delete(statement),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_delete(statement),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_delete(statement),
	}
}

/// Context for nested serializer save operations
///
/// Tracks parent relationships and nesting depth for nested instance creation.
#[derive(Debug)]
pub struct NestedSaveContext {
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
			parent_data: HashMap::new(),
			depth: 0,
			max_depth: 10,
		}
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
		T::PrimaryKey: IntoValue,
		R::PrimaryKey: IntoValue,
	{
		if target_ids.is_empty() {
			return Ok(());
		}

		use reinhardt_db::orm::manager::get_connection;

		let mut conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;
		self.add_bulk_with_conn(&mut conn, source_id, target_ids)
			.await
	}

	/// Add relationships in bulk through a caller-owned ORM executor.
	pub async fn add_bulk_with_conn<E>(
		&self,
		conn: &mut E,
		source_id: &T::PrimaryKey,
		target_ids: Vec<R::PrimaryKey>,
	) -> Result<(), SerializerError>
	where
		E: OrmExecutor,
		T::PrimaryKey: IntoValue,
		R::PrimaryKey: IntoValue,
	{
		if target_ids.is_empty() {
			return Ok(());
		}

		let mut statement = Query::insert();
		statement
			.into_table(Alias::new(&self.junction_table))
			.columns([Alias::new(&self.source_fk), Alias::new(&self.target_fk)]);
		for target_id in target_ids {
			statement.values_panic([Expr::val((*source_id).clone()), Expr::val(target_id)]);
		}

		let (query, values) = build_insert_sql(&statement, conn.backend());
		conn.execute(&query, convert_values(values))
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
		T::PrimaryKey: IntoValue,
		R::PrimaryKey: IntoValue,
	{
		if target_ids.is_empty() {
			return Ok(());
		}

		use reinhardt_db::orm::manager::get_connection;

		let mut conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;
		self.remove_bulk_with_conn(&mut conn, source_id, target_ids)
			.await
	}

	/// Remove relationships in bulk through a caller-owned ORM executor.
	pub async fn remove_bulk_with_conn<E>(
		&self,
		conn: &mut E,
		source_id: &T::PrimaryKey,
		target_ids: Vec<R::PrimaryKey>,
	) -> Result<(), SerializerError>
	where
		E: OrmExecutor,
		T::PrimaryKey: IntoValue,
		R::PrimaryKey: IntoValue,
	{
		if target_ids.is_empty() {
			return Ok(());
		}

		let mut statement = Query::delete();
		statement
			.from_table(Alias::new(&self.junction_table))
			.and_where(
				Expr::col(Alias::new(&self.source_fk))
					.binary(BinOper::Equal, Expr::val((*source_id).clone())),
			)
			.and_where(
				Expr::col(Alias::new(&self.target_fk)).is_in(target_ids.into_iter().map(Expr::val)),
			);

		let (query, values) = build_delete_sql(&statement, conn.backend());
		conn.execute(&query, convert_values(values))
			.await
			.map_err(|e| SerializerError::Other {
				message: format!("Failed to remove M2M relationships: {}", e),
			})?;

		Ok(())
	}

	/// Set relationships (replace all existing)
	///
	/// Removes all existing relationships and creates new ones. Use
	/// [`Self::set_with_conn`] with a caller-owned [`AtomicTransaction`] when
	/// the clear and add operations must be atomic. This convenience method does
	/// not start a transaction across its statements.
	///
	/// # Examples
	///
	/// ```ignore
	/// connection.atomic(async |transaction| {
	///     manager
	///         .set_with_conn(transaction, &user_id, vec![group1_id, group2_id])
	///         .await
	/// }).await?;
	/// ```
	pub async fn set(
		&self,
		source_id: &T::PrimaryKey,
		target_ids: Vec<R::PrimaryKey>,
	) -> Result<(), SerializerError>
	where
		T::PrimaryKey: IntoValue,
		R::PrimaryKey: IntoValue,
	{
		use reinhardt_db::orm::manager::get_connection;

		let mut conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;
		self.set_with_conn(&mut conn, source_id, target_ids).await
	}

	/// Replace relationships through a caller-owned ORM executor.
	pub async fn set_with_conn<E>(
		&self,
		conn: &mut E,
		source_id: &T::PrimaryKey,
		target_ids: Vec<R::PrimaryKey>,
	) -> Result<(), SerializerError>
	where
		E: OrmExecutor,
		T::PrimaryKey: IntoValue,
		R::PrimaryKey: IntoValue,
	{
		self.clear_with_conn(conn, source_id).await?;
		self.add_bulk_with_conn(conn, source_id, target_ids).await
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
		T::PrimaryKey: IntoValue,
	{
		use reinhardt_db::orm::manager::get_connection;

		let mut conn = get_connection().await.map_err(|e| SerializerError::Other {
			message: format!("Failed to get connection: {}", e),
		})?;
		self.clear_with_conn(&mut conn, source_id).await
	}

	/// Clear relationships through a caller-owned ORM executor.
	pub async fn clear_with_conn<E>(
		&self,
		conn: &mut E,
		source_id: &T::PrimaryKey,
	) -> Result<(), SerializerError>
	where
		E: OrmExecutor,
		T::PrimaryKey: IntoValue,
	{
		let mut statement = Query::delete();
		statement
			.from_table(Alias::new(&self.junction_table))
			.and_where(
				Expr::col(Alias::new(&self.source_fk))
					.binary(BinOper::Equal, Expr::val((*source_id).clone())),
			);

		let (query, values) = build_delete_sql(&statement, conn.backend());
		conn.execute(&query, convert_values(values))
			.await
			.map_err(|e| SerializerError::Other {
				message: format!("Failed to clear M2M relationships: {}", e),
			})?;

		Ok(())
	}
}

#[derive(Debug)]
enum SerializerAtomicError {
	Callback(SerializerError),
	Core(CoreError),
}

impl std::fmt::Display for SerializerAtomicError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Callback(error) => error.fmt(f),
			Self::Core(error) => error.fmt(f),
		}
	}
}

impl std::error::Error for SerializerAtomicError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Callback(error) => Some(error),
			Self::Core(error) => Some(error),
		}
	}
}

impl From<CoreError> for SerializerAtomicError {
	fn from(error: CoreError) -> Self {
		Self::Core(error)
	}
}

impl SerializerAtomicError {
	fn into_serializer_error(self) -> SerializerError {
		match self {
			Self::Callback(error) => error,
			Self::Core(error) => SerializerError::from(error),
		}
	}
}

/// Transaction helper for nested serializer operations.
pub struct TransactionHelper;

impl TransactionHelper {
	/// Execute a callback inside one outer atomic transaction.
	///
	/// The callback receives the only transaction executor it may use for ORM
	/// work. A successful callback commits; a callback error rolls back.
	pub async fn with_transaction<F, T>(
		connection: &DatabaseConnection,
		f: F,
	) -> Result<T, SerializerError>
	where
		F: for<'transaction> std::ops::AsyncFnOnce(
				&'transaction mut AtomicTransaction,
			) -> Result<T, SerializerError>,
	{
		connection
			.atomic(async |transaction| {
				f(transaction)
					.await
					.map_err(SerializerAtomicError::Callback)
			})
			.await
			.map_err(SerializerAtomicError::into_serializer_error)
	}

	/// Execute a callback behind a savepoint on the supplied atomic transaction.
	///
	/// This never acquires another connection. The supplied transaction owns the
	/// executor, so nested serializer work remains in the caller's transaction.
	pub async fn savepoint<F, T>(
		transaction: &mut AtomicTransaction,
		f: F,
	) -> Result<T, SerializerError>
	where
		F: for<'transaction> std::ops::AsyncFnOnce(
				&'transaction mut AtomicTransaction,
			) -> Result<T, SerializerError>,
	{
		transaction
			.atomic(async |nested_transaction| {
				f(nested_transaction)
					.await
					.map_err(SerializerAtomicError::Callback)
			})
			.await
			.map_err(SerializerAtomicError::into_serializer_error)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind};
	use reinhardt_db::backends::DatabaseBackend as BackendTrait;
	use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	use reinhardt_db::backends::Result as BackendResult;
	use reinhardt_db::backends::types::{
		DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor,
	};
	use reinhardt_db::orm::connection::{DatabaseConnection, DatabaseConnectionLease};
	use rstest::rstest;
	use std::sync::{Arc, Mutex};

	#[derive(Clone, Copy, Debug, Default)]
	struct FailurePlan {
		rollback_to_savepoint: bool,
		rollback: bool,
	}

	type TransactionCalls = Arc<Mutex<Vec<String>>>;

	struct RecordingTransactionExecutor {
		failure_plan: FailurePlan,
		calls: TransactionCalls,
	}

	#[async_trait]
	impl TransactionExecutor for RecordingTransactionExecutor {
		fn backend(&self) -> DatabaseType {
			DatabaseType::Postgres
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> BackendResult<QueryResult> {
			Ok(QueryResult {
				rows_affected: 0,
				last_insert_id: None,
			})
		}

		async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> BackendResult<Row> {
			Ok(Row::new())
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> BackendResult<Vec<Row>> {
			Ok(Vec::new())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> BackendResult<Option<Row>> {
			Ok(None)
		}

		async fn commit(self: Box<Self>) -> BackendResult<()> {
			self.calls.lock().unwrap().push("commit".to_string());
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> BackendResult<()> {
			self.calls.lock().unwrap().push("rollback".to_string());
			if self.failure_plan.rollback {
				Err(transaction_failure("transaction rollback failed"))
			} else {
				Ok(())
			}
		}

		async fn savepoint(&mut self, name: &str) -> BackendResult<()> {
			self.calls.lock().unwrap().push(format!("savepoint:{name}"));
			Ok(())
		}

		async fn release_savepoint(&mut self, name: &str) -> BackendResult<()> {
			self.calls
				.lock()
				.unwrap()
				.push(format!("release_savepoint:{name}"));
			Ok(())
		}

		async fn rollback_to_savepoint(&mut self, name: &str) -> BackendResult<()> {
			self.calls
				.lock()
				.unwrap()
				.push(format!("rollback_to_savepoint:{name}"));
			if self.failure_plan.rollback_to_savepoint {
				Err(transaction_failure("savepoint rollback failed"))
			} else {
				Ok(())
			}
		}
	}

	struct RecordingBackend {
		failure_plan: FailurePlan,
		calls: TransactionCalls,
	}

	#[async_trait]
	impl BackendTrait for RecordingBackend {
		fn database_type(&self) -> DatabaseType {
			DatabaseType::Postgres
		}

		fn placeholder(&self, index: usize) -> String {
			format!("${index}")
		}

		fn supports_returning(&self) -> bool {
			true
		}

		fn supports_on_conflict(&self) -> bool {
			true
		}

		async fn execute(
			&self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> BackendResult<QueryResult> {
			Ok(QueryResult {
				rows_affected: 0,
				last_insert_id: None,
			})
		}

		async fn fetch_one(&self, _sql: &str, _params: Vec<QueryValue>) -> BackendResult<Row> {
			Ok(Row::new())
		}

		async fn fetch_all(&self, _sql: &str, _params: Vec<QueryValue>) -> BackendResult<Vec<Row>> {
			Ok(Vec::new())
		}

		async fn fetch_optional(
			&self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> BackendResult<Option<Row>> {
			Ok(None)
		}

		async fn begin(&self) -> BackendResult<Box<dyn TransactionExecutor>> {
			self.calls.lock().unwrap().push("begin".to_string());
			Ok(Box::new(RecordingTransactionExecutor {
				failure_plan: self.failure_plan,
				calls: Arc::clone(&self.calls),
			}))
		}

		fn as_any(&self) -> &dyn std::any::Any {
			self
		}
	}

	fn transaction_failure(message: &str) -> reinhardt_core::exception::Error {
		DatabaseError::new(DatabaseErrorKind::Transaction, message).into()
	}

	fn recording_connection(
		failure_plan: FailurePlan,
	) -> (
		DatabaseConnection,
		DatabaseConnectionLease,
		TransactionCalls,
	) {
		let calls = Arc::new(Mutex::new(Vec::new()));
		let backend = Arc::new(RecordingBackend {
			failure_plan,
			calls: Arc::clone(&calls),
		});
		let backend_connection = BackendsConnection::new(backend);
		let lease = DatabaseConnectionLease::register(backend_connection)
			.expect("Failed to register recording database connection");
		(lease.handle(), lease, calls)
	}

	fn operation_error() -> SerializerError {
		SerializerError::Validation(ValidatorError::Custom {
			message: "nested operation failed".to_string(),
		})
	}

	fn expected_nested_cleanup_calls() -> Vec<String> {
		vec![
			"begin".to_string(),
			"savepoint:reinhardt_atomic_0".to_string(),
			"operation".to_string(),
			"rollback_to_savepoint:reinhardt_atomic_0".to_string(),
			"release_savepoint:reinhardt_atomic_0".to_string(),
			"commit".to_string(),
		]
	}

	#[rstest]
	#[tokio::test]
	async fn nested_savepoint_uses_the_supplied_atomic_transaction() {
		let (connection, _connection_lease, calls) = recording_connection(FailurePlan::default());
		let operation_calls = Arc::clone(&calls);

		let result = TransactionHelper::with_transaction(&connection, async |transaction| {
			let nested_result = TransactionHelper::savepoint(transaction, async move |_nested| {
				operation_calls
					.lock()
					.unwrap()
					.push("operation".to_string());
				Err::<(), SerializerError>(operation_error())
			})
			.await;

			assert_eq!(nested_result, Err(operation_error()));
			Ok::<(), SerializerError>(())
		})
		.await;

		assert_eq!(result, Ok(()));
		assert_eq!(*calls.lock().unwrap(), expected_nested_cleanup_calls());
	}

	#[rstest]
	#[tokio::test]
	async fn nested_savepoint_cleanup_error_maps_back_to_serializer_error() {
		let (connection, _connection_lease, calls) = recording_connection(FailurePlan {
			rollback_to_savepoint: true,
			rollback: false,
		});
		let operation_calls = Arc::clone(&calls);

		let result = TransactionHelper::with_transaction(&connection, async |transaction| {
			let nested_result = TransactionHelper::savepoint(transaction, async move |_nested| {
				operation_calls
					.lock()
					.unwrap()
					.push("operation".to_string());
				Err::<(), SerializerError>(operation_error())
			})
			.await;

			assert_eq!(
				nested_result,
				Err(SerializerError::Database(DatabaseError::new(
					DatabaseErrorKind::Transaction,
					"savepoint rollback failed",
				)))
			);
			Ok::<(), SerializerError>(())
		})
		.await;

		assert_eq!(
			result,
			Err(SerializerError::Database(DatabaseError::new(
				DatabaseErrorKind::Transaction,
				"Nested atomic operation was cancelled",
			)))
		);
		assert_eq!(
			*calls.lock().unwrap(),
			[
				"begin",
				"savepoint:reinhardt_atomic_0",
				"operation",
				"rollback_to_savepoint:reinhardt_atomic_0",
				"release_savepoint:reinhardt_atomic_0",
				"rollback",
			]
		);
	}

	#[rstest]
	#[tokio::test]
	async fn outer_transaction_rollback_error_maps_back_to_serializer_error() {
		let (connection, _connection_lease, calls) = recording_connection(FailurePlan {
			rollback_to_savepoint: false,
			rollback: true,
		});
		let operation_calls = Arc::clone(&calls);

		let result = TransactionHelper::with_transaction(&connection, async move |_transaction| {
			operation_calls
				.lock()
				.unwrap()
				.push("operation".to_string());
			Err::<(), SerializerError>(operation_error())
		})
		.await;

		assert_eq!(
			result,
			Err(SerializerError::Database(DatabaseError::new(
				DatabaseErrorKind::Transaction,
				"transaction rollback failed",
			)))
		);
		assert_eq!(
			*calls.lock().unwrap(),
			vec![
				"begin".to_string(),
				"operation".to_string(),
				"rollback".to_string(),
			]
		);
	}
}
