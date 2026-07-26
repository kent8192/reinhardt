//! # Transaction SQL and ORM Atomicity
//!
//! [`DatabaseConnection`](super::connection::DatabaseConnection) provides the ORM
//! transaction capability through
//! [`DatabaseConnection::atomic`](super::connection::DatabaseConnection::atomic). Its callback
//! receives an [`AtomicTransaction`], the only executor that may run ORM work
//! until the callback returns. A successful outer callback commits; an error
//! rolls back, and a rollback failure takes precedence over the callback error.
//!
//! Nested work uses [`AtomicTransaction::atomic`]. It creates a savepoint on
//! the same dedicated executor, releases it on success, and rolls back then
//! releases it on error. Nested callbacks never acquire another connection.
//!
//! ```no_run
//! use reinhardt_core::exception::Error;
//! use reinhardt_db::{backends::DatabaseConnection as BackendsConnection, orm::DatabaseConnectionLease};
//!
//! # async fn example() -> Result<(), Error> {
//! let owner = BackendsConnection::connect_sqlite("sqlite::memory:").await?;
//! let lease = DatabaseConnectionLease::register(owner)?;
//! let connection = lease.handle();
//! let answer = connection.atomic(async |transaction| {
//!     transaction.atomic(async |_savepoint| {
//!         Ok::<_, Error>(())
//!     }).await?;
//!     Ok::<_, Error>(42)
//! }).await?;
//! assert_eq!(answer, 42);
//! # Ok(())
//! # }
//! ```
//!
//! Mutable executor references are exclusive: pass `&mut transaction` to
//! `*_with_conn` or `*_with_db` methods and finish each await before the next
//! operation. Panics are rethrown after best-effort rollback; task cancellation
//! cannot guarantee an async rollback completes. MySQL DDL can implicitly
//! commit, so do not rely on atomicity for DDL statements.
//!
//! [`Transaction`], [`Savepoint`], and [`IsolationLevel`] remain SQL-builder
//! types. They generate SQL and are not ORM execution contexts; they cannot
//! manually begin, commit, rollback, or control a live pooled connection.
//!
//! ```
//! use reinhardt_db::orm::transaction::Transaction;
//!
//! let mut transaction = Transaction::new();
//! assert_eq!(transaction.begin().unwrap(), "BEGIN TRANSACTION");
//! assert_eq!(transaction.commit().unwrap(), "COMMIT");
//! ```

use futures::FutureExt;
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind};
use std::sync::{Arc, Mutex};

use super::connection::{
	DatabaseBackend, OrmExecutor, QueryResult, QueryValue, Row, TransactionExecutor,
};
use crate::backends::types::DatabaseType;

/// Transaction isolation levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
	/// ReadUncommitted variant.
	ReadUncommitted,
	/// ReadCommitted variant.
	ReadCommitted,
	/// RepeatableRead variant.
	RepeatableRead,
	/// Serializable variant.
	Serializable,
}

impl IsolationLevel {
	/// Convert isolation level to SQL string
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::IsolationLevel;
	///
	/// let level = IsolationLevel::Serializable;
	/// assert_eq!(level.to_sql(), "SERIALIZABLE");
	///
	/// let level = IsolationLevel::ReadCommitted;
	/// assert_eq!(level.to_sql(), "READ COMMITTED");
	/// ```
	pub fn to_sql(&self) -> &'static str {
		match self {
			IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
			IsolationLevel::ReadCommitted => "READ COMMITTED",
			IsolationLevel::RepeatableRead => "REPEATABLE READ",
			IsolationLevel::Serializable => "SERIALIZABLE",
		}
	}

	/// Convert to backends layer IsolationLevel
	pub(crate) fn to_backends_level(self) -> super::connection::IsolationLevel {
		match self {
			IsolationLevel::ReadUncommitted => super::connection::IsolationLevel::ReadUncommitted,
			IsolationLevel::ReadCommitted => super::connection::IsolationLevel::ReadCommitted,
			IsolationLevel::RepeatableRead => super::connection::IsolationLevel::RepeatableRead,
			IsolationLevel::Serializable => super::connection::IsolationLevel::Serializable,
		}
	}
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
	/// NotStarted variant.
	NotStarted,
	/// Active variant.
	Active,
	/// Committed variant.
	Committed,
	/// RolledBack variant.
	RolledBack,
}

/// Savepoint for nested transactions
///
/// Savepoint names are validated at construction to prevent SQL injection.
/// Only alphanumeric characters and underscores are allowed (must not start
/// with a digit). SQL output uses quoted identifiers for defense-in-depth.
#[derive(Debug, Clone)]
pub struct Savepoint {
	name: String,
	/// The depth.
	pub depth: usize,
}

impl Savepoint {
	/// Create a new savepoint with name and depth
	///
	/// # Panics
	///
	/// Panics if the name contains invalid characters. Only alphanumeric
	/// characters and underscores are allowed (must not start with a digit).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Savepoint;
	///
	/// let sp = Savepoint::new("my_savepoint", 1);
	/// assert_eq!(sp.name(), "my_savepoint");
	/// assert_eq!(sp.depth, 1);
	/// ```
	pub fn new(name: impl Into<String>, depth: usize) -> Self {
		let name = name.into();
		validate_savepoint_name(&name).unwrap_or_else(|e| panic!("Invalid savepoint name: {}", e));
		Self { name, depth }
	}

	/// Get the savepoint name
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Generate SQL for creating this savepoint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Savepoint;
	///
	/// let sp = Savepoint::new("checkpoint_1", 2);
	/// assert_eq!(sp.to_sql(), r#"SAVEPOINT "checkpoint_1""#);
	/// ```
	pub fn to_sql(&self) -> String {
		format!("SAVEPOINT \"{}\"", self.name.replace('"', "\"\""))
	}

	/// Generate SQL for releasing this savepoint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Savepoint;
	///
	/// let sp = Savepoint::new("checkpoint_1", 2);
	/// assert_eq!(sp.release_sql(), r#"RELEASE SAVEPOINT "checkpoint_1""#);
	/// ```
	pub fn release_sql(&self) -> String {
		format!("RELEASE SAVEPOINT \"{}\"", self.name.replace('"', "\"\""))
	}

	/// Generate SQL for rolling back to this savepoint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Savepoint;
	///
	/// let sp = Savepoint::new("checkpoint_1", 2);
	/// assert_eq!(sp.rollback_sql(), r#"ROLLBACK TO SAVEPOINT "checkpoint_1""#);
	/// ```
	pub fn rollback_sql(&self) -> String {
		format!(
			"ROLLBACK TO SAVEPOINT \"{}\"",
			self.name.replace('"', "\"\"")
		)
	}
}

/// Validate a savepoint name to prevent SQL injection.
///
/// Only alphanumeric characters and underscores are allowed.
/// The name must not be empty and must not start with a digit.
fn validate_savepoint_name(name: &str) -> Result<(), String> {
	if name.is_empty() {
		return Err("Savepoint name cannot be empty".to_string());
	}

	if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
		return Err(format!(
			"Savepoint name '{}' contains invalid characters. Only alphanumeric characters and underscores are allowed",
			name
		));
	}

	if let Some(first_char) = name.chars().next()
		&& first_char.is_numeric()
	{
		return Err(format!(
			"Savepoint name '{}' cannot start with a number",
			name
		));
	}

	Ok(())
}

/// Transaction manager
#[derive(Debug)]
pub struct Transaction {
	state: Arc<Mutex<TransactionState>>,
	isolation_level: Option<IsolationLevel>,
	savepoints: Arc<Mutex<Vec<Savepoint>>>,
	depth: usize,
}

impl Transaction {
	/// Create a new transaction in NotStarted state
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::{Transaction, TransactionState};
	///
	/// let tx = Transaction::new();
	/// assert_eq!(tx.state().unwrap(), TransactionState::NotStarted);
	/// assert_eq!(tx.depth(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			state: Arc::new(Mutex::new(TransactionState::NotStarted)),
			isolation_level: None,
			savepoints: Arc::new(Mutex::new(Vec::new())),
			depth: 0,
		}
	}
	/// Set the isolation level for this transaction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::{Transaction, IsolationLevel};
	///
	/// let mut tx = Transaction::new().with_isolation_level(IsolationLevel::Serializable);
	/// let sql = tx.begin().unwrap();
	/// assert!(sql.contains("SERIALIZABLE"));
	/// ```
	pub fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
		self.isolation_level = Some(level);
		self
	}
	/// Begin a transaction or create a savepoint for nested transactions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::{Transaction, TransactionState};
	///
	/// let mut tx = Transaction::new();
	/// let sql = tx.begin().unwrap();
	/// assert_eq!(sql, "BEGIN TRANSACTION");
	/// assert_eq!(tx.state().unwrap(), TransactionState::Active);
	///
	/// // Nested transaction creates savepoint
	/// let nested_sql = tx.begin().unwrap();
	/// assert!(nested_sql.contains("SAVEPOINT"));
	/// ```
	pub fn begin(&mut self) -> Result<String, String> {
		let mut state = self.state.lock().map_err(|e| e.to_string())?;

		match *state {
			TransactionState::NotStarted => {
				*state = TransactionState::Active;
				self.depth = 1;

				let sql = if let Some(level) = self.isolation_level {
					format!("BEGIN TRANSACTION ISOLATION LEVEL {}", level.to_sql())
				} else {
					"BEGIN TRANSACTION".to_string()
				};

				Ok(sql)
			}
			TransactionState::Active => {
				// Nested transaction - use savepoint
				self.depth += 1;
				let savepoint = Savepoint::new(format!("sp_{}", self.depth), self.depth);
				let sql = savepoint.to_sql();

				self.savepoints
					.lock()
					.map_err(|e| e.to_string())?
					.push(savepoint);

				Ok(sql)
			}
			_ => Err("Transaction already completed".to_string()),
		}
	}
	/// Commit a transaction or release a savepoint for nested transactions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::{Transaction, TransactionState};
	///
	/// let mut tx = Transaction::new();
	/// tx.begin().unwrap();
	/// let sql = tx.commit().unwrap();
	/// assert_eq!(sql, "COMMIT");
	/// assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	/// ```
	pub fn commit(&mut self) -> Result<String, String> {
		let mut state = self.state.lock().map_err(|e| e.to_string())?;

		match *state {
			TransactionState::Active => {
				if self.depth > 1 {
					// Release savepoint
					let mut savepoints = self.savepoints.lock().map_err(|e| e.to_string())?;
					if let Some(savepoint) = savepoints.pop() {
						self.depth -= 1;
						Ok(savepoint.release_sql())
					} else {
						Err("No savepoint to release".to_string())
					}
				} else {
					// Commit top-level transaction
					*state = TransactionState::Committed;
					self.depth = 0;
					Ok("COMMIT".to_string())
				}
			}
			_ => Err("No active transaction to commit".to_string()),
		}
	}
	/// Rollback a transaction or rollback to a savepoint for nested transactions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::{Transaction, TransactionState};
	///
	/// let mut tx = Transaction::new();
	/// tx.begin().unwrap();
	/// let sql = tx.rollback().unwrap();
	/// assert_eq!(sql, "ROLLBACK");
	/// assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	/// ```
	pub fn rollback(&mut self) -> Result<String, String> {
		let mut state = self.state.lock().map_err(|e| e.to_string())?;

		match *state {
			TransactionState::Active => {
				if self.depth > 1 {
					// Rollback to savepoint
					let mut savepoints = self.savepoints.lock().map_err(|e| e.to_string())?;
					if let Some(savepoint) = savepoints.pop() {
						self.depth -= 1;
						Ok(savepoint.rollback_sql())
					} else {
						Err("No savepoint to rollback to".to_string())
					}
				} else {
					// Rollback top-level transaction
					*state = TransactionState::RolledBack;
					self.depth = 0;
					self.savepoints.lock().map_err(|e| e.to_string())?.clear();
					Ok("ROLLBACK".to_string())
				}
			}
			_ => Err("No active transaction to rollback".to_string()),
		}
	}
	/// Get current transaction state
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::{Transaction, TransactionState};
	///
	/// let tx = Transaction::new();
	/// assert_eq!(tx.state().unwrap(), TransactionState::NotStarted);
	/// ```
	pub fn state(&self) -> Result<TransactionState, String> {
		self.state.lock().map(|s| *s).map_err(|e| e.to_string())
	}
	/// Get current transaction depth (0 = not started, 1 = top-level, 2+ = nested)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Transaction;
	///
	/// let mut tx = Transaction::new();
	/// assert_eq!(tx.depth(), 0);
	///
	/// tx.begin().unwrap();
	/// assert_eq!(tx.depth(), 1);
	///
	/// tx.begin().unwrap(); // Nested
	/// assert_eq!(tx.depth(), 2);
	/// ```
	pub fn depth(&self) -> usize {
		self.depth
	}

	/// Check if transaction is currently active
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Transaction;
	///
	/// let mut tx = Transaction::new();
	/// assert!(!tx.is_active());
	///
	/// tx.begin().unwrap();
	/// assert!(tx.is_active());
	///
	/// tx.commit().unwrap();
	/// assert!(!tx.is_active());
	/// ```
	pub fn is_active(&self) -> bool {
		matches!(self.state().ok(), Some(TransactionState::Active))
	}
	/// Create a named savepoint within an active transaction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Transaction;
	///
	/// let mut tx = Transaction::new();
	/// tx.begin().unwrap();
	///
	/// let sql = tx.savepoint("my_checkpoint").unwrap();
	/// assert_eq!(sql, r#"SAVEPOINT "my_checkpoint""#);
	/// ```
	pub fn savepoint(&mut self, name: impl Into<String>) -> Result<String, String> {
		let state = self.state.lock().map_err(|e| e.to_string())?;

		if *state != TransactionState::Active {
			return Err("Cannot create savepoint outside active transaction".to_string());
		}

		drop(state);

		let savepoint = Savepoint::new(name, self.depth);
		let sql = savepoint.to_sql();

		self.savepoints
			.lock()
			.map_err(|e| e.to_string())?
			.push(savepoint);

		Ok(sql)
	}
	/// Release a named savepoint, committing its changes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Transaction;
	///
	/// let mut tx = Transaction::new();
	/// tx.begin().unwrap();
	/// tx.savepoint("my_checkpoint").unwrap();
	///
	/// let sql = tx.release_savepoint("my_checkpoint").unwrap();
	/// assert_eq!(sql, r#"RELEASE SAVEPOINT "my_checkpoint""#);
	/// ```
	pub fn release_savepoint(&mut self, name: &str) -> Result<String, String> {
		let mut savepoints = self.savepoints.lock().map_err(|e| e.to_string())?;

		if let Some(pos) = savepoints.iter().position(|sp| sp.name() == name) {
			let savepoint = savepoints.remove(pos);
			Ok(savepoint.release_sql())
		} else {
			Err(format!("Savepoint '{}' not found", name))
		}
	}
	/// Rollback to a named savepoint, undoing changes after that point
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Transaction;
	///
	/// let mut tx = Transaction::new();
	/// tx.begin().unwrap();
	/// tx.savepoint("my_checkpoint").unwrap();
	///
	/// let sql = tx.rollback_to_savepoint("my_checkpoint").unwrap();
	/// assert_eq!(sql, r#"ROLLBACK TO SAVEPOINT "my_checkpoint""#);
	/// ```
	pub fn rollback_to_savepoint(&mut self, name: &str) -> Result<String, String> {
		let savepoints = self.savepoints.lock().map_err(|e| e.to_string())?;

		if let Some(savepoint) = savepoints.iter().find(|sp| sp.name() == name) {
			Ok(savepoint.rollback_sql())
		} else {
			Err(format!("Savepoint '{}' not found", name))
		}
	}
}

impl Default for Transaction {
	fn default() -> Self {
		Self::new()
	}
}

/// A closure-scoped transaction that owns one dedicated backend executor.
///
/// Instances are created only by [`super::connection::DatabaseConnection::atomic`]
/// and are finalized by its lifecycle runner. Nested calls create savepoints on
/// the same executor instead of acquiring another database connection.
pub struct AtomicTransaction {
	executor: Option<Box<dyn TransactionExecutor>>,
	backend: DatabaseBackend,
	savepoint_sequence: u64,
}

impl AtomicTransaction {
	pub(crate) fn new(executor: Box<dyn TransactionExecutor>) -> Self {
		let backend = DatabaseBackend::from(executor.backend());
		Self {
			executor: Some(executor),
			backend,
			savepoint_sequence: 0,
		}
	}

	fn executor_mut<'transaction>(
		&'transaction mut self,
	) -> reinhardt_core::exception::Result<&'transaction mut (dyn TransactionExecutor + 'static)> {
		self.executor
			.as_deref_mut()
			.ok_or_else(transaction_consumed_error)
	}

	fn executor_ref(&self) -> Option<&(dyn TransactionExecutor + 'static)> {
		self.executor.as_deref()
	}

	async fn commit(&mut self) -> reinhardt_core::exception::Result<()> {
		let executor = self
			.executor
			.take()
			.ok_or_else(transaction_consumed_error)?;
		executor.commit().await
	}

	async fn rollback(&mut self) -> reinhardt_core::exception::Result<()> {
		let executor = self
			.executor
			.take()
			.ok_or_else(transaction_consumed_error)?;
		executor.rollback().await
	}

	fn next_savepoint_name(&mut self) -> reinhardt_core::exception::Result<String> {
		let sequence = self.savepoint_sequence;
		self.savepoint_sequence = self.savepoint_sequence.checked_add(1).ok_or_else(|| {
			DatabaseError::new(
				DatabaseErrorKind::Transaction,
				"Atomic transaction savepoint sequence exhausted",
			)
		})?;
		Ok(format!("reinhardt_atomic_{sequence}"))
	}

	async fn cleanup_savepoint(
		&mut self,
		name: &str,
	) -> (
		reinhardt_core::exception::Result<()>,
		reinhardt_core::exception::Result<()>,
	) {
		let rollback_result = match self.executor_mut() {
			Ok(executor) => executor.rollback_to_savepoint(name).await,
			Err(error) => Err(error),
		};
		let release_result = match self.executor_mut() {
			Ok(executor) => executor.release_savepoint(name).await,
			Err(error) => Err(error),
		};
		(rollback_result, release_result)
	}

	pub(crate) async fn run<F, T, E>(mut self, f: F) -> std::result::Result<T, E>
	where
		F: for<'txn> std::ops::AsyncFnOnce(
				&'txn mut AtomicTransaction,
			) -> std::result::Result<T, E>,
		E: std::error::Error + From<reinhardt_core::exception::Error>,
	{
		match std::panic::AssertUnwindSafe(f(&mut self))
			.catch_unwind()
			.await
		{
			Ok(Ok(value)) => {
				self.commit().await.map_err(E::from)?;
				Ok(value)
			}
			Ok(Err(operation_error)) => match self.rollback().await {
				Ok(()) => Err(operation_error),
				Err(rollback_error) => {
					tracing::error!(
						operation_error = %operation_error,
						rollback_error = %rollback_error,
						"Atomic transaction operation and rollback both failed"
					);
					Err(E::from(rollback_error))
				}
			},
			Err(panic_payload) => {
				if let Err(rollback_error) = self.rollback().await {
					tracing::error!(
						rollback_error = %rollback_error,
						"Atomic transaction rollback failed while resuming callback panic"
					);
				}
				std::panic::resume_unwind(panic_payload)
			}
		}
	}

	/// Runs a nested closure behind a savepoint on this transaction's executor.
	pub async fn atomic<F, T, E>(&mut self, f: F) -> std::result::Result<T, E>
	where
		F: for<'txn> std::ops::AsyncFnOnce(
				&'txn mut AtomicTransaction,
			) -> std::result::Result<T, E>,
		E: std::error::Error + From<reinhardt_core::exception::Error>,
	{
		let savepoint_name = self.next_savepoint_name().map_err(E::from)?;
		self.executor_mut()?
			.savepoint(&savepoint_name)
			.await
			.map_err(E::from)?;

		match std::panic::AssertUnwindSafe(f(self)).catch_unwind().await {
			Ok(Ok(value)) => self
				.executor_mut()?
				.release_savepoint(&savepoint_name)
				.await
				.map(|()| value)
				.map_err(E::from),
			Ok(Err(operation_error)) => {
				let (rollback_result, release_result) =
					self.cleanup_savepoint(&savepoint_name).await;
				match (rollback_result, release_result) {
					(Ok(()), Ok(())) => Err(operation_error),
					(Err(rollback_error), Ok(())) => {
						tracing::error!(
							operation_error = %operation_error,
							rollback_to_savepoint_error = %rollback_error,
							"Nested atomic operation and rollback-to-savepoint both failed"
						);
						Err(E::from(rollback_error))
					}
					(Ok(()), Err(release_error)) => {
						tracing::error!(
							operation_error = %operation_error,
							release_savepoint_error = %release_error,
							"Nested atomic operation and release-savepoint both failed"
						);
						Err(E::from(release_error))
					}
					(Err(rollback_error), Err(release_error)) => {
						tracing::error!(
							operation_error = %operation_error,
							rollback_to_savepoint_error = %rollback_error,
							release_savepoint_error = %release_error,
							"Nested atomic operation and both savepoint cleanup steps failed"
						);
						Err(E::from(rollback_error))
					}
				}
			}
			Err(panic_payload) => {
				let (rollback_result, release_result) =
					self.cleanup_savepoint(&savepoint_name).await;
				if let Err(rollback_error) = rollback_result {
					tracing::error!(
						rollback_to_savepoint_error = %rollback_error,
						"Nested atomic rollback-to-savepoint failed while resuming callback panic"
					);
				}
				if let Err(release_error) = release_result {
					tracing::error!(
						release_savepoint_error = %release_error,
						"Nested atomic release-savepoint failed while resuming callback panic"
					);
				}
				std::panic::resume_unwind(panic_payload)
			}
		}
	}
}

#[async_trait::async_trait]
impl OrmExecutor for AtomicTransaction {
	fn backend(&self) -> DatabaseBackend {
		self.executor_ref()
			.map(|executor| DatabaseBackend::from(executor.backend()))
			.unwrap_or(self.backend)
	}

	fn supports_pgvector_error_hints(&self) -> bool {
		self.executor_ref()
			.is_some_and(TransactionExecutor::supports_pgvector_error_hints)
	}

	async fn execute(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		self.executor_mut()?.execute(sql, params).await
	}

	async fn execute_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		self.executor_mut()?
			.execute_with_context(sql, params, context)
			.await
	}

	async fn fetch_one(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Row> {
		self.executor_mut()?.fetch_one(sql, params).await
	}

	async fn fetch_one_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<Row> {
		self.executor_mut()?
			.fetch_one_with_context(sql, params, context)
			.await
	}

	async fn fetch_all(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		self.executor_mut()?.fetch_all(sql, params).await
	}

	async fn fetch_all_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		self.executor_mut()?
			.fetch_all_with_context(sql, params, context)
			.await
	}

	async fn fetch_optional(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		self.executor_mut()?.fetch_optional(sql, params).await
	}

	async fn fetch_optional_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		self.executor_mut()?
			.fetch_optional_with_context(sql, params, context)
			.await
	}
}

// Keep the legacy transaction executor contract available while callers migrate
// to the closure-scoped `AtomicTransaction` API.
#[async_trait::async_trait]
impl TransactionExecutor for AtomicTransaction {
	fn backend(&self) -> DatabaseType {
		self.executor_ref().map_or_else(
			|| match self.backend {
				DatabaseBackend::Postgres => DatabaseType::Postgres,
				DatabaseBackend::MySql => DatabaseType::Mysql,
				DatabaseBackend::Sqlite => DatabaseType::Sqlite,
			},
			TransactionExecutor::backend,
		)
	}

	fn supports_pgvector_error_hints(&self) -> bool {
		self.executor_ref()
			.is_some_and(TransactionExecutor::supports_pgvector_error_hints)
	}

	async fn execute(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		self.executor_mut()?.execute(sql, params).await
	}

	async fn execute_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		self.executor_mut()?
			.execute_with_context(sql, params, context)
			.await
	}

	async fn fetch_one(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Row> {
		self.executor_mut()?.fetch_one(sql, params).await
	}

	async fn fetch_one_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<Row> {
		self.executor_mut()?
			.fetch_one_with_context(sql, params, context)
			.await
	}

	async fn fetch_all(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		self.executor_mut()?.fetch_all(sql, params).await
	}

	async fn fetch_all_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		self.executor_mut()?
			.fetch_all_with_context(sql, params, context)
			.await
	}

	async fn fetch_optional(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		self.executor_mut()?.fetch_optional(sql, params).await
	}

	async fn fetch_optional_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		self.executor_mut()?
			.fetch_optional_with_context(sql, params, context)
			.await
	}

	async fn commit(self: Box<Self>) -> reinhardt_core::exception::Result<()> {
		let mut transaction = *self;
		transaction
			.executor
			.take()
			.ok_or_else(transaction_consumed_error)?
			.commit()
			.await
	}

	async fn rollback(self: Box<Self>) -> reinhardt_core::exception::Result<()> {
		let mut transaction = *self;
		transaction
			.executor
			.take()
			.ok_or_else(transaction_consumed_error)?
			.rollback()
			.await
	}

	async fn savepoint(&mut self, name: &str) -> reinhardt_core::exception::Result<()> {
		self.executor_mut()?.savepoint(name).await
	}

	async fn release_savepoint(&mut self, name: &str) -> reinhardt_core::exception::Result<()> {
		self.executor_mut()?.release_savepoint(name).await
	}

	async fn rollback_to_savepoint(&mut self, name: &str) -> reinhardt_core::exception::Result<()> {
		self.executor_mut()?.rollback_to_savepoint(name).await
	}
}

fn transaction_consumed_error() -> reinhardt_core::exception::Error {
	DatabaseError::new(
		DatabaseErrorKind::Transaction,
		"Transaction already consumed",
	)
	.into()
}
#[cfg(test)]
mod tests {
	use super::*;
	use crate::backends::backend::DatabaseBackend as BackendTrait;
	use crate::backends::connection::DatabaseConnection as BackendsConnection;
	use crate::backends::error::Result;
	use crate::backends::types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor};
	use crate::orm::Manager;
	use crate::orm::connection::{DatabaseConnection, DatabaseConnectionLease, OrmExecutor};
	use crate::prelude::Model;
	use futures::FutureExt;
	use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind};
	use rstest::*;
	use std::collections::BTreeSet;
	use std::fmt;
	use std::sync::atomic::{AtomicBool, Ordering};
	use std::sync::{Arc, Mutex};
	use tracing::field::{Field, Visit};
	use tracing_subscriber::layer::{Context, Layer};
	use tracing_subscriber::prelude::*;
	use tracing_subscriber::registry::LookupSpan;

	#[derive(Debug, thiserror::Error)]
	pub(super) enum ApplicationError {
		#[error("application rejected the operation")]
		Rejected,
		#[error(transparent)]
		Framework(#[from] reinhardt_core::exception::Error),
	}

	#[derive(Clone, Copy, Debug, Default)]
	struct FailurePlan {
		begin: bool,
		commit: bool,
		rollback: bool,
		savepoint: bool,
		release_savepoint: bool,
		rollback_to_savepoint: bool,
		unsupported_savepoints: bool,
	}

	type TransactionCalls = Arc<Mutex<Vec<String>>>;

	struct TestDatabaseConnection {
		handle: DatabaseConnection,
		_lease: DatabaseConnectionLease,
	}

	impl std::ops::Deref for TestDatabaseConnection {
		type Target = DatabaseConnection;

		fn deref(&self) -> &Self::Target {
			&self.handle
		}
	}

	// Mock transaction executor for testing
	struct MockTransactionExecutor {
		failure_plan: FailurePlan,
		calls: TransactionCalls,
	}

	#[async_trait::async_trait]
	impl TransactionExecutor for MockTransactionExecutor {
		fn backend(&self) -> DatabaseType {
			DatabaseType::Postgres
		}

		fn supports_pgvector_error_hints(&self) -> bool {
			true
		}

		async fn execute(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			self.calls.lock().unwrap().push("execute".to_string());
			Ok(QueryResult {
				rows_affected: 0,
				last_insert_id: None,
			})
		}

		async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			Ok(Row::new())
		}

		async fn fetch_all(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			Ok(Vec::new())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			Ok(None)
		}

		async fn commit(self: Box<Self>) -> Result<()> {
			self.calls.lock().unwrap().push("commit".to_string());
			if self.failure_plan.commit {
				Err(transaction_failure("commit failed"))
			} else {
				Ok(())
			}
		}

		async fn rollback(self: Box<Self>) -> Result<()> {
			self.calls.lock().unwrap().push("rollback".to_string());
			if self.failure_plan.rollback {
				Err(transaction_failure("rollback failed"))
			} else {
				Ok(())
			}
		}

		async fn savepoint(&mut self, name: &str) -> Result<()> {
			self.calls.lock().unwrap().push(format!("savepoint:{name}"));
			if self.failure_plan.savepoint {
				Err(transaction_failure("savepoint failed"))
			} else {
				Ok(())
			}
		}

		async fn release_savepoint(&mut self, name: &str) -> Result<()> {
			self.calls
				.lock()
				.unwrap()
				.push(format!("release_savepoint:{name}"));
			if self.failure_plan.release_savepoint {
				Err(transaction_failure("release savepoint failed"))
			} else {
				Ok(())
			}
		}

		async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
			self.calls
				.lock()
				.unwrap()
				.push(format!("rollback_to_savepoint:{name}"));
			if self.failure_plan.rollback_to_savepoint {
				Err(transaction_failure("rollback to savepoint failed"))
			} else {
				Ok(())
			}
		}
	}

	#[test]
	fn atomic_transaction_reports_inner_backend_and_pgvector_capability() {
		let transaction = AtomicTransaction::new(Box::new(MockTransactionExecutor {
			failure_plan: FailurePlan::default(),
			calls: Arc::new(Mutex::new(Vec::new())),
		}));

		assert_eq!(
			OrmExecutor::backend(&transaction),
			DatabaseBackend::Postgres
		);
		assert!(OrmExecutor::supports_pgvector_error_hints(&transaction));
		assert_eq!(
			TransactionExecutor::backend(&transaction),
			DatabaseType::Postgres
		);
		assert!(TransactionExecutor::supports_pgvector_error_hints(
			&transaction
		));
	}

	/// A test executor that deliberately uses the trait's default unsupported
	/// savepoint methods.
	struct UnsupportedSavepointTransactionExecutor {
		failure_plan: FailurePlan,
		calls: TransactionCalls,
	}

	#[async_trait::async_trait]
	impl TransactionExecutor for UnsupportedSavepointTransactionExecutor {
		fn backend(&self) -> DatabaseType {
			DatabaseType::Postgres
		}

		async fn execute(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			self.calls.lock().unwrap().push("execute".to_string());
			Ok(QueryResult {
				rows_affected: 0,
				last_insert_id: None,
			})
		}

		async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			Ok(Row::new())
		}

		async fn fetch_all(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			Ok(Vec::new())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			Ok(None)
		}

		async fn commit(self: Box<Self>) -> Result<()> {
			self.calls.lock().unwrap().push("commit".to_string());
			if self.failure_plan.commit {
				Err(transaction_failure("commit failed"))
			} else {
				Ok(())
			}
		}

		async fn rollback(self: Box<Self>) -> Result<()> {
			self.calls.lock().unwrap().push("rollback".to_string());
			if self.failure_plan.rollback {
				Err(transaction_failure("rollback failed"))
			} else {
				Ok(())
			}
		}
	}

	struct MockBackend {
		failure_plan: FailurePlan,
		calls: TransactionCalls,
	}

	impl MockBackend {
		fn transaction_executor(&self) -> Box<dyn TransactionExecutor> {
			if self.failure_plan.unsupported_savepoints {
				Box::new(UnsupportedSavepointTransactionExecutor {
					failure_plan: self.failure_plan,
					calls: Arc::clone(&self.calls),
				})
			} else {
				Box::new(MockTransactionExecutor {
					failure_plan: self.failure_plan,
					calls: Arc::clone(&self.calls),
				})
			}
		}
	}

	#[async_trait::async_trait]
	impl BackendTrait for MockBackend {
		fn database_type(&self) -> DatabaseType {
			DatabaseType::Postgres
		}
		fn placeholder(&self, index: usize) -> String {
			format!("${}", index)
		}
		fn supports_returning(&self) -> bool {
			true
		}
		fn supports_on_conflict(&self) -> bool {
			true
		}
		async fn execute(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})
		}
		async fn fetch_one(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			Ok(Row::new())
		}
		async fn fetch_all(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			Ok(Vec::new())
		}
		async fn fetch_optional(
			&self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			Ok(None)
		}
		fn as_any(&self) -> &dyn std::any::Any {
			self
		}
		async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
			self.calls.lock().unwrap().push("begin".to_string());
			if self.failure_plan.begin {
				Err(transaction_failure("begin failed"))
			} else {
				Ok(self.transaction_executor())
			}
		}

		async fn begin_with_isolation(
			&self,
			_isolation_level: crate::backends::types::IsolationLevel,
		) -> Result<Box<dyn TransactionExecutor>> {
			self.calls
				.lock()
				.unwrap()
				.push("begin_with_isolation".to_string());
			if self.failure_plan.begin {
				Err(transaction_failure("begin failed"))
			} else {
				Ok(self.transaction_executor())
			}
		}
	}

	fn transaction_failure(message: &str) -> reinhardt_core::exception::Error {
		DatabaseError::new(DatabaseErrorKind::Transaction, message).into()
	}

	fn mock_connection_with_failures(
		failure_plan: FailurePlan,
	) -> (TestDatabaseConnection, TransactionCalls) {
		let calls = Arc::new(Mutex::new(Vec::new()));
		let mock_backend = Arc::new(MockBackend {
			failure_plan,
			calls: Arc::clone(&calls),
		});
		let backends_conn = BackendsConnection::new(mock_backend);
		let lease = DatabaseConnectionLease::register(backends_conn).unwrap();
		let handle = lease.handle();
		(
			TestDatabaseConnection {
				handle,
				_lease: lease,
			},
			calls,
		)
	}

	fn assert_transaction_calls(calls: &TransactionCalls, expected: &[&str]) {
		let actual = calls.lock().unwrap().clone();
		let expected = expected
			.iter()
			.map(|call| (*call).to_string())
			.collect::<Vec<_>>();
		assert_eq!(actual, expected);
	}

	fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
		if let Some(message) = payload.downcast_ref::<&str>() {
			(*message).to_string()
		} else if let Some(message) = payload.downcast_ref::<String>() {
			message.clone()
		} else {
			"unknown panic payload".to_string()
		}
	}

	#[derive(Clone, Default)]
	struct EventCaptureLayer {
		events: Arc<Mutex<Vec<BTreeSet<String>>>>,
	}

	#[derive(Default)]
	struct EventFieldVisitor {
		fields: BTreeSet<String>,
	}

	impl Visit for EventFieldVisitor {
		fn record_debug(&mut self, field: &Field, _value: &dyn fmt::Debug) {
			self.fields.insert(field.name().to_string());
		}
	}

	impl<S> Layer<S> for EventCaptureLayer
	where
		S: tracing::Subscriber + for<'span> LookupSpan<'span>,
	{
		fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
			let mut visitor = EventFieldVisitor::default();
			event.record(&mut visitor);
			self.events.lock().unwrap().push(visitor.fields);
		}
	}

	#[tokio::test]
	async fn test_connection_atomic_commits_successful_callback() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan::default());

		let result: std::result::Result<u64, ApplicationError> = connection
			.atomic(async |transaction| {
				let result = OrmExecutor::execute(transaction, "SELECT 1", vec![]).await?;
				Ok(result.rows_affected)
			})
			.await;

		assert_eq!(result.unwrap(), 0);
		assert_transaction_calls(&calls, &["begin", "execute", "commit"]);
	}

	#[tokio::test]
	async fn test_connection_atomic_rolls_back_callback_error() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan::default());

		let result: std::result::Result<(), ApplicationError> = connection
			.atomic(async |_transaction| Err(ApplicationError::Rejected))
			.await;

		assert!(matches!(result, Err(ApplicationError::Rejected)));
		assert_transaction_calls(&calls, &["begin", "rollback"]);
	}

	#[tokio::test]
	async fn test_connection_atomic_returns_commit_failure() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan {
			commit: true,
			..FailurePlan::default()
		});

		let result: std::result::Result<(), ApplicationError> =
			connection.atomic(async |_transaction| Ok(())).await;

		match result {
			Err(ApplicationError::Framework(error)) => {
				assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Transaction));
			}
			other => panic!("expected a framework commit failure, got {other:?}"),
		}
		assert_transaction_calls(&calls, &["begin", "commit"]);
	}

	#[tokio::test]
	async fn test_connection_atomic_rollback_failure_wins_and_records_both_errors() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan {
			rollback: true,
			..FailurePlan::default()
		});
		let events = Arc::new(Mutex::new(Vec::new()));
		let subscriber = tracing_subscriber::registry().with(EventCaptureLayer {
			events: Arc::clone(&events),
		});
		let _subscriber_guard = tracing::subscriber::set_default(subscriber);

		let result: std::result::Result<(), ApplicationError> = connection
			.atomic(async |_transaction| Err(ApplicationError::Rejected))
			.await;

		match result {
			Err(ApplicationError::Framework(error)) => {
				assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Transaction));
			}
			other => panic!("expected the framework rollback failure, got {other:?}"),
		}
		assert_transaction_calls(&calls, &["begin", "rollback"]);
		assert!(events.lock().unwrap().iter().any(|fields| {
			fields.contains("operation_error") && fields.contains("rollback_error")
		}));
	}

	#[tokio::test]
	async fn test_connection_atomic_with_isolation_uses_same_lifecycle() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan::default());

		let result: std::result::Result<(), ApplicationError> = connection
			.atomic_with_isolation(IsolationLevel::Serializable, async |_transaction| Ok(()))
			.await;

		assert!(result.is_ok());
		assert_transaction_calls(&calls, &["begin_with_isolation", "commit"]);
	}

	#[tokio::test]
	async fn test_atomic_transaction_releases_successful_nested_savepoint() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan::default());

		let result: std::result::Result<(), ApplicationError> = connection
			.atomic(async |transaction| {
				let nested: std::result::Result<(), ApplicationError> = transaction
					.atomic(async |nested| {
						OrmExecutor::execute(nested, "SELECT 1", vec![]).await?;
						Ok(())
					})
					.await;
				nested?;
				Ok(())
			})
			.await;

		assert!(result.is_ok());
		assert_transaction_calls(
			&calls,
			&[
				"begin",
				"savepoint:reinhardt_atomic_0",
				"execute",
				"release_savepoint:reinhardt_atomic_0",
				"commit",
			],
		);
	}

	#[tokio::test]
	async fn test_atomic_transaction_allocates_distinct_savepoints_for_sibling_nested_callbacks() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan::default());

		let result: std::result::Result<(), ApplicationError> = connection
			.atomic(async |transaction| {
				let first: std::result::Result<(), ApplicationError> =
					transaction.atomic(async |_nested| Ok(())).await;
				first?;

				let second: std::result::Result<(), ApplicationError> =
					transaction.atomic(async |_nested| Ok(())).await;
				second?;
				Ok(())
			})
			.await;

		assert!(result.is_ok());
		assert_transaction_calls(
			&calls,
			&[
				"begin",
				"savepoint:reinhardt_atomic_0",
				"release_savepoint:reinhardt_atomic_0",
				"savepoint:reinhardt_atomic_1",
				"release_savepoint:reinhardt_atomic_1",
				"commit",
			],
		);
	}

	#[tokio::test]
	async fn test_atomic_transaction_rolls_back_and_releases_failed_nested_savepoint() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan::default());

		let result: std::result::Result<(), ApplicationError> = connection
			.atomic(async |transaction| {
				let nested: std::result::Result<(), ApplicationError> = transaction
					.atomic(async |_nested| Err(ApplicationError::Rejected))
					.await;
				assert!(matches!(nested, Err(ApplicationError::Rejected)));
				Ok(())
			})
			.await;

		assert!(result.is_ok());
		assert_transaction_calls(
			&calls,
			&[
				"begin",
				"savepoint:reinhardt_atomic_0",
				"rollback_to_savepoint:reinhardt_atomic_0",
				"release_savepoint:reinhardt_atomic_0",
				"commit",
			],
		);
	}

	#[tokio::test]
	async fn test_atomic_transaction_rejects_default_unsupported_savepoints_before_callback() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan {
			unsupported_savepoints: true,
			..FailurePlan::default()
		});
		let callback_was_run = Arc::new(AtomicBool::new(false));
		let callback_state = Arc::clone(&callback_was_run);

		let result: std::result::Result<(), ApplicationError> = connection
			.atomic(async move |transaction| {
				let nested: std::result::Result<(), ApplicationError> = transaction
					.atomic(async move |_nested| {
						callback_state.store(true, Ordering::SeqCst);
						Ok(())
					})
					.await;
				match nested {
					Err(ApplicationError::Framework(error)) => {
						assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Unsupported));
					}
					other => panic!("expected an unsupported savepoint error, got {other:?}"),
				}
				Ok(())
			})
			.await;

		assert!(result.is_ok());
		assert!(!callback_was_run.load(Ordering::SeqCst));
		assert_transaction_calls(&calls, &["begin", "commit"]);
	}

	#[tokio::test]
	async fn test_atomic_transaction_returns_first_nested_cleanup_failure_after_attempting_both() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan {
			rollback_to_savepoint: true,
			release_savepoint: true,
			..FailurePlan::default()
		});

		let result: std::result::Result<(), ApplicationError> = connection
			.atomic(async |transaction| {
				let nested: std::result::Result<(), ApplicationError> = transaction
					.atomic(async |_nested| Err(ApplicationError::Rejected))
					.await;
				match nested {
					Err(ApplicationError::Framework(error)) => {
						assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Transaction));
						assert_eq!(
							error
								.database_error()
								.expect("the cleanup error must retain database details")
								.message(),
							"rollback to savepoint failed"
						);
					}
					other => panic!("expected the first nested cleanup error, got {other:?}"),
				}
				Ok(())
			})
			.await;

		assert!(result.is_ok());
		assert_transaction_calls(
			&calls,
			&[
				"begin",
				"savepoint:reinhardt_atomic_0",
				"rollback_to_savepoint:reinhardt_atomic_0",
				"release_savepoint:reinhardt_atomic_0",
				"commit",
			],
		);
	}

	#[tokio::test]
	async fn test_connection_atomic_rolls_back_and_rethrows_callback_panic() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan::default());

		let panic = std::panic::AssertUnwindSafe(async {
			let _: std::result::Result<(), ApplicationError> = connection
				.atomic(async |_transaction| panic!("atomic callback panic"))
				.await;
		})
		.catch_unwind()
		.await;

		let payload = panic.expect_err("the callback panic must be rethrown");
		assert_eq!(panic_message(payload.as_ref()), "atomic callback panic");
		assert_transaction_calls(&calls, &["begin", "rollback"]);
	}

	#[tokio::test]
	async fn test_connection_atomic_rethrows_callback_panic_when_rollback_fails() {
		let (connection, calls) = mock_connection_with_failures(FailurePlan {
			rollback: true,
			..FailurePlan::default()
		});
		let events = Arc::new(Mutex::new(Vec::new()));
		let subscriber = tracing_subscriber::registry().with(EventCaptureLayer {
			events: Arc::clone(&events),
		});
		let _subscriber_guard = tracing::subscriber::set_default(subscriber);

		let panic = std::panic::AssertUnwindSafe(async {
			let _: std::result::Result<(), ApplicationError> = connection
				.atomic(async |_transaction| panic!("atomic callback panic"))
				.await;
		})
		.catch_unwind()
		.await;

		let payload = panic.expect_err("the callback panic must be rethrown");
		assert_eq!(panic_message(payload.as_ref()), "atomic callback panic");
		assert_transaction_calls(&calls, &["begin", "rollback"]);
		assert!(
			events
				.lock()
				.unwrap()
				.iter()
				.any(|fields| fields.contains("rollback_error"))
		);
	}

	#[test]
	fn test_transaction_begin() {
		let mut tx = Transaction::new();
		let sql = tx.begin().unwrap();
		assert_eq!(sql, "BEGIN TRANSACTION");
		assert_eq!(tx.state().unwrap(), TransactionState::Active);
		assert_eq!(tx.depth(), 1);
	}

	#[test]
	fn test_transaction_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		let sql = tx.commit().unwrap();
		assert_eq!(sql, "COMMIT");
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
		assert_eq!(tx.depth(), 0);
	}

	#[test]
	fn test_orm_transaction_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		let sql = tx.rollback().unwrap();
		assert_eq!(sql, "ROLLBACK");
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
		assert_eq!(tx.depth(), 0);
	}

	#[test]
	fn test_nested_transaction_begin() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		let sql = tx.begin().unwrap();
		assert!(sql.contains("SAVEPOINT \"sp_2\""));
		assert_eq!(tx.depth(), 2);
	}

	#[test]
	fn test_nested_transaction_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		let sql = tx.commit().unwrap();
		assert!(sql.contains("RELEASE SAVEPOINT"));
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	fn test_nested_transaction_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		let sql = tx.rollback().unwrap();
		assert!(sql.contains("ROLLBACK TO SAVEPOINT"));
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	fn test_isolation_level() {
		let mut tx = Transaction::new().with_isolation_level(IsolationLevel::Serializable);
		let sql = tx.begin().unwrap();
		assert!(sql.contains("ISOLATION LEVEL SERIALIZABLE"));
	}

	#[test]
	fn test_manual_savepoint() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		let sql = tx.savepoint("my_savepoint").unwrap();
		assert_eq!(sql, r#"SAVEPOINT "my_savepoint""#);
	}

	#[test]
	fn test_orm_transaction_release_savepoint() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.savepoint("my_savepoint").unwrap();
		let sql = tx.release_savepoint("my_savepoint").unwrap();
		assert_eq!(sql, r#"RELEASE SAVEPOINT "my_savepoint""#);
	}

	#[test]
	fn test_orm_transaction_rollback_savepoint() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.savepoint("my_savepoint").unwrap();
		let sql = tx.rollback_to_savepoint("my_savepoint").unwrap();
		assert_eq!(sql, r#"ROLLBACK TO SAVEPOINT "my_savepoint""#);
	}

	#[test]
	fn test_transaction_is_active() {
		let mut tx = Transaction::new();
		assert!(!tx.is_active());
		tx.begin().unwrap();
		assert!(tx.is_active());
		tx.commit().unwrap();
		assert!(!tx.is_active());
	}

	#[test]
	fn test_commit_without_begin() {
		let mut tx = Transaction::new();
		let result = tx.commit();
		assert!(result.is_err());
	}

	#[test]
	fn test_rollback_without_begin() {
		let mut tx = Transaction::new();
		let result = tx.rollback();
		assert!(result.is_err());
	}

	#[test]
	fn test_savepoint_outside_transaction() {
		let mut tx = Transaction::new();
		let result = tx.savepoint("test");
		assert!(result.is_err());
	}

	// Database execution tests
	use reinhardt_core::validators::TableName;
	use serde::{Deserialize, Serialize};

	// Allow dead_code: test model struct for transaction tests
	#[allow(dead_code)]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestItem {
		id: Option<i64>,
		name: String,
		value: i32,
	}

	#[derive(Clone)]
	struct TestItemFields;
	impl crate::orm::model::FieldSelector for TestItemFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	// Allow dead_code: test constant for transaction tests
	#[allow(dead_code)]
	const TEST_ITEM_TABLE: TableName = TableName::new_const("test_items");

	impl Model for TestItem {
		type PrimaryKey = i64;
		type Fields = TestItemFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			TEST_ITEM_TABLE.as_str()
		}

		fn new_fields() -> Self::Fields {
			TestItemFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}
	}

	/// Test: Transaction begin SQL generation and state management
	///
	/// This test verifies that:
	/// 1. Transaction::begin() generates correct SQL
	/// 2. Transaction state is correctly updated (active, depth)
	/// 3. begin() returns the expected SQL statement
	///
	#[test]
	fn test_transaction_begin_sql_generation() {
		let mut tx = Transaction::new();

		// Test SQL generation
		let sql = tx.begin().unwrap();
		assert_eq!(
			sql, "BEGIN TRANSACTION",
			"Should generate BEGIN TRANSACTION SQL"
		);

		// Test state management
		assert!(tx.is_active(), "Transaction should be active after begin()");
		assert_eq!(tx.depth(), 1, "Transaction depth should be 1");
	}

	#[test]
	fn test_transaction_commit_sql_generation() {
		let mut tx = Transaction::new();

		// Verify begin generates correct SQL and updates state
		let begin_sql = tx.begin().unwrap();
		assert_eq!(begin_sql, "BEGIN TRANSACTION");
		assert!(tx.is_active());
		assert_eq!(tx.depth(), 1);

		// Verify commit generates correct SQL and updates state
		let commit_sql = tx.commit().unwrap();
		assert_eq!(commit_sql, "COMMIT");
		assert!(!tx.is_active());
		assert_eq!(tx.depth(), 0);
	}

	#[test]
	fn test_transaction_rollback_sql_generation() {
		let mut tx = Transaction::new();

		// Verify begin generates correct SQL
		let begin_sql = tx.begin().unwrap();
		assert_eq!(begin_sql, "BEGIN TRANSACTION");
		assert!(tx.is_active());

		// Verify rollback generates correct SQL and updates state
		let rollback_sql = tx.rollback().unwrap();
		assert_eq!(rollback_sql, "ROLLBACK");
		assert!(!tx.is_active());
		assert_eq!(tx.depth(), 0);
	}

	#[test]
	fn test_nested_transaction_sql_generation() {
		// Test nested transaction (savepoint) SQL generation
		let mut tx = Transaction::new();

		// Begin outer transaction
		let begin_sql = tx.begin().unwrap();
		assert_eq!(begin_sql, "BEGIN TRANSACTION");
		assert_eq!(tx.depth(), 1);

		// Begin nested transaction (creates savepoint)
		let savepoint_sql = tx.begin().unwrap();
		assert!(savepoint_sql.contains("SAVEPOINT \"sp_2\""));
		assert_eq!(tx.depth(), 2);

		// Rollback to savepoint
		let rollback_sql = tx.rollback().unwrap();
		assert!(rollback_sql.contains("ROLLBACK TO SAVEPOINT"));
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());

		// Commit outer transaction
		let commit_sql = tx.commit().unwrap();
		assert_eq!(commit_sql, "COMMIT");
		assert_eq!(tx.depth(), 0);
		assert!(!tx.is_active());
	}

	#[test]
	fn test_transaction_isolation_level_sql() {
		// Test that isolation level is properly included in BEGIN statement
		let mut tx = Transaction::new().with_isolation_level(IsolationLevel::Serializable);
		let begin_sql = tx.begin().unwrap();

		assert!(begin_sql.contains("ISOLATION LEVEL SERIALIZABLE"));
		assert!(tx.is_active());
	}
}
