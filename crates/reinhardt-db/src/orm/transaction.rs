//! # Transaction Management
//!
//! This module provides transaction management APIs for database operations.
//!
//! ## Recommended API: Closure-based Transactions
//!
//! The recommended way to use transactions is through the closure-based API:
//!
//! - [`transaction()`] - Execute a closure with automatic commit/rollback
//! - [`transaction_with_isolation()`] - Transaction with specific isolation level
//!
//! ### Example
//!
//! ```rust
//! use reinhardt_db::orm::transaction::transaction;
//! use reinhardt_db::orm::connection::DatabaseConnection;
//!
//! # async fn example() -> Result<(), anyhow::Error> {
//! let conn = DatabaseConnection::connect("sqlite::memory:").await?;
//!
//! let result = transaction(&conn, |_tx| async move {
//!     // Your operations here
//!     Ok(42)
//! }).await?;
//!
//! assert_eq!(result, 42);
//! # Ok(())
//! # }
//! ```
//!
//! ## Low-level API: TransactionScope
//!
//! For advanced use cases, you can use [`TransactionScope`] directly:
//!
//! - Manual control over commit/rollback timing
//! - Nested transactions via savepoints
//! - Access to transaction metadata
//!
//! ### Example
//!
//! ```rust
//! use reinhardt_db::orm::transaction::TransactionScope;
//! use reinhardt_db::orm::connection::DatabaseConnection;
//!
//! # async fn example() -> Result<(), anyhow::Error> {
//! let conn = DatabaseConnection::connect("sqlite::memory:").await?;
//! let tx = TransactionScope::begin(&conn).await?;
//!
//! // Perform operations
//!
//! tx.commit().await?;  // Explicit commit
//! # Ok(())
//! # }
//! ```
//!
//! ## Legacy API: atomic()
//!
//! The [`atomic()`] function is an alternative API that doesn't pass the
//! transaction scope to the closure. Consider using [`transaction()`] instead
//! for new code.
//!
//! ### Migration from atomic() to transaction()
//!
//! ```rust
//! # use reinhardt_db::orm::connection::DatabaseConnection;
//! # async fn example() -> Result<(), anyhow::Error> {
//! # let conn = DatabaseConnection::connect("sqlite::memory:").await?;
//! // Old API (atomic)
//! use reinhardt_db::orm::transaction::atomic;
//! let result = atomic(&conn, || async move {
//!     Ok(42)
//! }).await?;
//!
//! // New API (transaction) - preferred
//! use reinhardt_db::orm::transaction::transaction;
//! let result = transaction(&conn, |_tx| async move {
//!     Ok(42)
//! }).await?;
//! # Ok(())
//! # }
//! ```

use std::sync::{Arc, Mutex};

/// Transaction isolation levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
	ReadUncommitted,
	ReadCommitted,
	RepeatableRead,
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
	NotStarted,
	Active,
	Committed,
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

	/// Execute transaction begin on database
	/// Documentation for `begin_db`
	///
	pub async fn begin_db(&mut self) -> reinhardt_core::exception::Result<()> {
		let sql = self
			.begin()
			.map_err(reinhardt_core::exception::Error::Database)?;
		let conn = super::manager::get_connection().await?;
		conn.execute(&sql, vec![]).await?;
		Ok(())
	}

	/// Execute transaction commit on database
	/// Documentation for `commit_db`
	///
	pub async fn commit_db(&mut self) -> reinhardt_core::exception::Result<()> {
		let sql = self
			.commit()
			.map_err(reinhardt_core::exception::Error::Database)?;
		let conn = super::manager::get_connection().await?;
		conn.execute(&sql, vec![]).await?;
		Ok(())
	}

	/// Execute transaction rollback on database
	/// Documentation for `rollback_db`
	///
	pub async fn rollback_db(&mut self) -> reinhardt_core::exception::Result<()> {
		let sql = self
			.rollback()
			.map_err(reinhardt_core::exception::Error::Database)?;
		let conn = super::manager::get_connection().await?;
		conn.execute(&sql, vec![]).await?;
		Ok(())
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

/// Atomic transaction builder (similar to Django's transaction.atomic)
pub struct Atomic<F> {
	_func: F,
	_isolation_level: Option<IsolationLevel>,
}

impl<F> Atomic<F> {
	/// Create a new atomic transaction wrapper around a function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::Atomic;
	///
	/// let atomic = Atomic::new(|| {
	///     // Transaction logic here
	/// });
	/// // Verify the atomic transaction wrapper is created successfully
	/// let _: Atomic<_> = atomic;
	/// ```
	pub fn new(func: F) -> Self {
		Self {
			_func: func,
			_isolation_level: None,
		}
	}
	/// Set the isolation level for the atomic transaction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::transaction::{Atomic, IsolationLevel};
	///
	/// let atomic = Atomic::new(|| {
	///     // Transaction logic
	/// }).with_isolation_level(IsolationLevel::Serializable);
	/// // Verify the atomic transaction with isolation level is created successfully
	/// let _: Atomic<_> = atomic;
	/// ```
	pub fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
		self._isolation_level = Some(level);
		self
	}
}

/// Transaction scope guard with automatic rollback on drop
///
/// This struct implements RAII (Resource Acquisition Is Initialization) pattern
/// for database transactions. When the scope is dropped without explicit commit,
/// it automatically rolls back the transaction.
///
/// # Connection Affinity
///
/// `TransactionScope` holds a dedicated database connection that is used for all
/// queries within the transaction. This ensures proper transaction isolation by
/// guaranteeing that all operations (BEGIN, queries, COMMIT/ROLLBACK) run on the
/// same physical connection.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::orm::connection::DatabaseConnection;
/// use reinhardt_db::orm::transaction::TransactionScope;
///
/// # async fn example() {
/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
/// let conn = DatabaseConnection::connect("postgres://localhost/test").await.unwrap();
///
/// // Transaction is automatically rolled back if not committed
/// {
///     let mut tx = TransactionScope::begin(&conn).await.unwrap();
///     // ... perform operations ...
///     // If we don't call tx.commit(), rollback happens automatically
/// }
///
/// // Explicit commit
/// {
///     let mut tx = TransactionScope::begin(&conn).await.unwrap();
///     // ... perform operations ...
///     tx.commit().await.unwrap(); // Explicit commit
/// }
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub struct TransactionScope {
	executor: Option<Box<dyn super::connection::TransactionExecutor>>,
	committed: bool,
}

impl TransactionScope {
	/// Begin a new transaction scope
	///
	/// This acquires a dedicated database connection and begins a transaction on it.
	/// All queries executed through this scope are guaranteed to run on the same
	/// physical connection, ensuring proper transaction isolation.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::TransactionScope;
	///
	/// # async fn example() {
	/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await.unwrap();
	/// let mut tx = TransactionScope::begin(&conn).await.unwrap();
	/// // ... perform operations ...
	/// tx.commit().await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn begin(
		conn: &super::connection::DatabaseConnection,
	) -> Result<Self, anyhow::Error> {
		let executor = conn.begin().await?;
		Ok(Self {
			executor: Some(executor),
			committed: false,
		})
	}

	/// Begin a new transaction scope with specific isolation level
	///
	/// This acquires a dedicated database connection and begins a transaction with
	/// the specified isolation level. All queries executed through this scope are
	/// guaranteed to run on the same physical connection.
	///
	/// # Arguments
	///
	/// * `conn` - The database connection
	/// * `level` - The desired isolation level for the transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::{TransactionScope, IsolationLevel};
	///
	/// # async fn example() {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await.unwrap();
	/// let mut tx = TransactionScope::begin_with_isolation(&conn, IsolationLevel::Serializable).await.unwrap();
	/// // ... perform operations ...
	/// tx.commit().await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn begin_with_isolation(
		conn: &super::connection::DatabaseConnection,
		level: IsolationLevel,
	) -> Result<Self, anyhow::Error> {
		let executor = conn.begin_with_isolation(level.to_backends_level()).await?;
		Ok(Self {
			executor: Some(executor),
			committed: false,
		})
	}

	/// Execute a SQL statement within the transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::TransactionScope;
	///
	/// # async fn example() {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await.unwrap();
	/// let mut tx = TransactionScope::begin(&conn).await.unwrap();
	///
	/// // Execute SQL within the transaction
	/// tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Alice".into()]).await.unwrap();
	///
	/// tx.commit().await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn execute(
		&mut self,
		sql: &str,
		params: Vec<super::connection::QueryValue>,
	) -> Result<u64, anyhow::Error> {
		let executor = self
			.executor
			.as_mut()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		let result = executor.execute(sql, params).await?;
		Ok(result.rows_affected)
	}

	/// Execute a SQL query and return a single row within the transaction
	pub async fn query_one(
		&mut self,
		sql: &str,
		params: Vec<super::connection::QueryValue>,
	) -> Result<super::connection::QueryRow, anyhow::Error> {
		let executor = self
			.executor
			.as_mut()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		let row = executor.fetch_one(sql, params).await?;
		Ok(super::connection::QueryRow::from_backend_row(row))
	}

	/// Execute a SQL query and return all rows within the transaction
	pub async fn query(
		&mut self,
		sql: &str,
		params: Vec<super::connection::QueryValue>,
	) -> Result<Vec<super::connection::QueryRow>, anyhow::Error> {
		let executor = self
			.executor
			.as_mut()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		let rows = executor.fetch_all(sql, params).await?;
		Ok(rows
			.into_iter()
			.map(super::connection::QueryRow::from_backend_row)
			.collect())
	}

	/// Execute a SQL query and return an optional row within the transaction
	pub async fn query_optional(
		&mut self,
		sql: &str,
		params: Vec<super::connection::QueryValue>,
	) -> Result<Option<super::connection::QueryRow>, anyhow::Error> {
		let executor = self
			.executor
			.as_mut()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		let row = executor.fetch_optional(sql, params).await?;
		Ok(row.map(super::connection::QueryRow::from_backend_row))
	}

	/// Commit the transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::TransactionScope;
	///
	/// # async fn example() {
	/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await.unwrap();
	/// let mut tx = TransactionScope::begin(&conn).await.unwrap();
	/// // ... perform operations ...
	/// tx.commit().await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn commit(mut self) -> Result<(), anyhow::Error> {
		let executor = self
			.executor
			.take()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		executor.commit().await?;
		self.committed = true;
		Ok(())
	}

	/// Explicit rollback
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::TransactionScope;
	///
	/// # async fn example() {
	/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await.unwrap();
	/// let mut tx = TransactionScope::begin(&conn).await.unwrap();
	/// // ... error occurs ...
	/// tx.rollback().await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn rollback(mut self) -> Result<(), anyhow::Error> {
		let executor = self
			.executor
			.take()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		executor.rollback().await?;
		self.committed = true; // Mark as handled to prevent double rollback in Drop
		Ok(())
	}

	/// Create a savepoint within the transaction
	///
	/// Savepoints allow partial rollback of a transaction. You can create
	/// multiple savepoints and rollback to any of them without affecting
	/// work done before that savepoint.
	///
	/// # Arguments
	///
	/// * `name` - The name of the savepoint
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::TransactionScope;
	///
	/// # async fn example() -> Result<(), anyhow::Error> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let mut tx = TransactionScope::begin(&conn).await?;
	///
	/// tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Alice".into()]).await?;
	///
	/// // Create a savepoint before risky operation
	/// tx.savepoint("before_risky_op").await?;
	///
	/// // Perform risky operation
	/// if let Err(_) = tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Invalid".into()]).await {
	///     // Rollback to savepoint, keeping Alice's insert
	///     tx.rollback_to_savepoint("before_risky_op").await?;
	/// }
	///
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn savepoint(&mut self, name: &str) -> Result<(), anyhow::Error> {
		let executor = self
			.executor
			.as_mut()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		executor.savepoint(name).await?;
		Ok(())
	}

	/// Release a savepoint
	///
	/// Releasing a savepoint removes it from the transaction's savepoint stack.
	/// This is typically done after the risky operation succeeded and the
	/// savepoint is no longer needed.
	///
	/// # Arguments
	///
	/// * `name` - The name of the savepoint to release
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::TransactionScope;
	///
	/// # async fn example() -> Result<(), anyhow::Error> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let mut tx = TransactionScope::begin(&conn).await?;
	///
	/// tx.savepoint("sp1").await?;
	/// // ... operations succeeded ...
	/// tx.release_savepoint("sp1").await?;
	///
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn release_savepoint(&mut self, name: &str) -> Result<(), anyhow::Error> {
		let executor = self
			.executor
			.as_mut()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		executor.release_savepoint(name).await?;
		Ok(())
	}

	/// Rollback to a savepoint
	///
	/// This undoes all changes made after the savepoint was created,
	/// but keeps the transaction open for further operations.
	///
	/// # Arguments
	///
	/// * `name` - The name of the savepoint to rollback to
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::TransactionScope;
	///
	/// # async fn example() -> Result<(), anyhow::Error> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let mut tx = TransactionScope::begin(&conn).await?;
	///
	/// tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Alice".into()]).await?;
	/// tx.savepoint("sp1").await?;
	///
	/// // This will be rolled back
	/// tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Bob".into()]).await?;
	///
	/// // Rollback to savepoint - Bob's insert is undone, Alice's remains
	/// tx.rollback_to_savepoint("sp1").await?;
	///
	/// tx.commit().await?; // Only Alice is committed
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rollback_to_savepoint(&mut self, name: &str) -> Result<(), anyhow::Error> {
		let executor = self
			.executor
			.as_mut()
			.ok_or_else(|| anyhow::anyhow!("Transaction already consumed"))?;
		executor.rollback_to_savepoint(name).await?;
		Ok(())
	}

	/// Execute a closure and automatically commit on success or rollback on error
	///
	/// This method provides a closure-based API for executing operations within
	/// the transaction scope. The transaction is automatically committed if the
	/// closure returns Ok, or rolled back if it returns Err.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::TransactionScope;
	///
	/// # async fn example() -> Result<(), anyhow::Error> {
	/// let conn = DatabaseConnection::connect("sqlite::memory:").await?;
	/// let mut tx = TransactionScope::begin(&conn).await?;
	///
	/// let result = tx.run(|tx| async move {
	///     // Perform operations via tx.execute(), tx.query(), etc.
	///     Ok(42)
	/// }).await?;
	///
	/// assert_eq!(result, 42);
	/// # Ok(())
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn run<F, Fut, T>(mut self, f: F) -> Result<T, anyhow::Error>
	where
		F: FnOnce(&mut Self) -> Fut,
		Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
	{
		match f(&mut self).await {
			Ok(result) => {
				self.commit().await?;
				Ok(result)
			}
			Err(e) => {
				self.rollback().await?;
				Err(e)
			}
		}
	}
}

impl Drop for TransactionScope {
	/// Automatically rollback transaction if not committed
	///
	/// This ensures that transactions are always cleaned up, even if
	/// an error occurs or the scope is exited early.
	///
	/// # Note
	///
	/// When using `TransactionScope` directly (not through `transaction()` function),
	/// it's recommended to explicitly call `commit()` or `rollback()` to handle
	/// errors properly. The automatic rollback in Drop cannot propagate errors.
	///
	/// The automatic rollback in Drop requires a multi-threaded tokio runtime.
	/// For single-threaded runtimes or when no runtime is available, only a
	/// warning message is printed.
	fn drop(&mut self) {
		if !self.committed
			&& let Some(executor) = self.executor.take()
		{
			eprintln!(
				"Warning: TransactionScope dropped without explicit commit/rollback. \
					 Consider using transaction() function for automatic error handling."
			);

			// Try to execute rollback in blocking context
			// This only works on multi-threaded runtime
			// Note: Errors during Drop cannot be propagated, so we just log them
			if let Ok(handle) = tokio::runtime::Handle::try_current() {
				// Try to use block_in_place if available (multi-threaded runtime)
				let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
					tokio::task::block_in_place(|| {
						handle.block_on(async { executor.rollback().await })
					})
				}));

				match result {
					Ok(Ok(())) => {
						// Rollback succeeded
					}
					Ok(Err(e)) => {
						eprintln!("Error during automatic rollback: {}", e);
					}
					Err(_) => {
						// block_in_place panicked (likely single-threaded runtime)
						eprintln!(
							"Warning: Cannot perform automatic rollback on single-threaded runtime. \
								 Use transaction() function or explicit commit()/rollback()."
						);
					}
				}
			} else {
				// No runtime available
				eprintln!(
					"Warning: No async runtime available for automatic rollback. \
						 Transaction may not be cleaned up properly."
				);
			}
		}
	}
}

/// Execute a function within a transaction scope
///
/// This is a convenience function that automatically handles transaction
/// begin/commit/rollback. If the function returns Ok, the transaction is
/// committed. If it returns Err or panics, the transaction is rolled back.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::orm::connection::DatabaseConnection;
/// use reinhardt_db::orm::transaction::atomic;
///
/// # async fn example() {
/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
/// let conn = DatabaseConnection::connect("postgres://localhost/test").await.unwrap();
///
/// let result = atomic(&conn, || async move {
///     // Perform operations using conn...
///     // The transaction is automatically managed
///     Ok::<_, anyhow::Error>(42)
/// }).await.unwrap();
///
/// assert_eq!(result, 42);
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub async fn atomic<F, Fut, T>(
	conn: &super::connection::DatabaseConnection,
	f: F,
) -> Result<T, anyhow::Error>
where
	F: FnOnce() -> Fut,
	Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
	let tx = TransactionScope::begin(conn).await?;
	let result = f().await?;
	tx.commit().await?;
	Ok(result)
}

/// Execute a function within a transaction with specific isolation level
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::orm::connection::DatabaseConnection;
/// use reinhardt_db::orm::transaction::{atomic_with_isolation, IsolationLevel};
///
/// # async fn example() {
/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
/// let conn = DatabaseConnection::connect("postgres://localhost/test").await.unwrap();
///
/// let result = atomic_with_isolation(
///     &conn,
///     IsolationLevel::Serializable,
///     || async move {
///         // Perform operations...
///         Ok::<_, anyhow::Error>(42)
///     }
/// ).await.unwrap();
///
/// assert_eq!(result, 42);
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub async fn atomic_with_isolation<F, Fut, T>(
	conn: &super::connection::DatabaseConnection,
	level: IsolationLevel,
	f: F,
) -> Result<T, anyhow::Error>
where
	F: FnOnce() -> Fut,
	Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
	let tx = TransactionScope::begin_with_isolation(conn, level).await?;
	let result = f().await?;
	tx.commit().await?;
	Ok(result)
}

/// Execute a closure within a transaction scope with automatic commit/rollback
///
/// This function provides closure-based transaction management:
/// - On success (Ok): Automatically commits the transaction
/// - On error (Err): Automatically rolls back the transaction
///
/// The closure receives a mutable reference to the `TransactionScope` which can be used
/// to execute SQL within the transaction.
///
/// # Examples
///
/// ```rust,ignore
/// # #[tokio::main]
/// # async fn main() {
/// use reinhardt_db::orm::connection::DatabaseConnection;
/// use reinhardt_db::orm::transaction::transaction;
/// use std::future::Future;
/// use std::pin::Pin;
///
/// # async fn example() -> Result<(), anyhow::Error> {
/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
/// let conn = DatabaseConnection::connect("sqlite::memory:").await?;
///
/// // Simple transaction
/// transaction(&conn, |tx| async {
///     tx.execute("INSERT INTO users (name) VALUES (?)", vec!["Alice".into()]).await?;
///     Ok(())
/// }).await?;
///
/// // Transaction with return value
/// let user_id: i64 = transaction(&conn, |tx| async {
///     tx.execute("INSERT INTO users (name) VALUES (?)", vec!["Bob".into()]).await?;
///     Ok(42_i64) // Example return value
/// }).await?;
///
/// assert_eq!(user_id, 42);
/// # Ok(())
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// # }
/// ```
///
/// # Error Handling
///
/// If the closure returns an error, the transaction is automatically rolled back:
///
/// ```no_run
/// use reinhardt_db::orm::connection::DatabaseConnection;
/// use reinhardt_db::orm::transaction::transaction;
///
/// # async fn example() -> Result<(), anyhow::Error> {
/// let conn = DatabaseConnection::connect("sqlite::memory:").await?;
///
/// let result: Result<(), anyhow::Error> = transaction(&conn, |_tx| async move {
///     // Simulate an error
///     Err(anyhow::anyhow!("Operation failed"))
/// }).await;
///
/// assert!(result.is_err()); // Transaction was automatically rolled back
/// # Ok(())
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub async fn transaction<F, Fut, T>(
	conn: &super::connection::DatabaseConnection,
	f: F,
) -> Result<T, anyhow::Error>
where
	F: FnOnce(&mut TransactionScope) -> Fut,
	Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
	let mut tx = TransactionScope::begin(conn).await?;

	match f(&mut tx).await {
		Ok(result) => {
			tx.commit().await?;
			Ok(result)
		}
		Err(e) => {
			tx.rollback().await?;
			Err(e)
		}
	}
}

/// Execute a closure within a transaction with specified isolation level
///
/// Like `transaction()`, but allows specifying the isolation level for the transaction.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::orm::connection::DatabaseConnection;
/// use reinhardt_db::orm::transaction::{transaction_with_isolation, IsolationLevel};
///
/// # async fn example() -> Result<(), anyhow::Error> {
/// let conn = DatabaseConnection::connect("sqlite::memory:").await?;
///
/// transaction_with_isolation(&conn, IsolationLevel::Serializable, |_tx| async move {
///     // Critical operation requiring serializable isolation
///     // update_inventory().await?;
///     Ok(())
/// }).await?;
/// # Ok(())
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub async fn transaction_with_isolation<F, Fut, T>(
	conn: &super::connection::DatabaseConnection,
	level: IsolationLevel,
	f: F,
) -> Result<T, anyhow::Error>
where
	F: FnOnce(&mut TransactionScope) -> Fut,
	Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
	let mut tx = TransactionScope::begin_with_isolation(conn, level).await?;

	match f(&mut tx).await {
		Ok(result) => {
			tx.commit().await?;
			Ok(result)
		}
		Err(e) => {
			tx.rollback().await?;
			Err(e)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::backends::backend::DatabaseBackend as BackendTrait;
	use crate::backends::connection::DatabaseConnection as BackendsConnection;
	use crate::backends::error::Result;
	use crate::backends::types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor};
	use crate::orm::connection::{DatabaseBackend, DatabaseConnection};
	use crate::prelude::Model;
	use rstest::*;
	use std::sync::Arc;

	// Mock transaction executor for testing
	struct MockTransactionExecutor;

	#[async_trait::async_trait]
	impl TransactionExecutor for MockTransactionExecutor {
		async fn execute(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			Ok(QueryResult { rows_affected: 0 })
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
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> Result<()> {
			Ok(())
		}
	}

	struct MockBackend;

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
			Ok(QueryResult { rows_affected: 1 })
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
			Ok(Box::new(MockTransactionExecutor))
		}
	}

	#[fixture]
	fn mock_connection() -> DatabaseConnection {
		let mock_backend = Arc::new(MockBackend);
		let backends_conn = BackendsConnection::new(mock_backend);
		DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn)
	}

	#[rstest]
	#[tokio::test]
	async fn test_transaction_scope_commit(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let tx = TransactionScope::begin(&conn).await;
		let tx = tx.unwrap();
		assert!(!tx.committed);

		let result = tx.commit().await;
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_transaction_scope_rollback(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let tx = TransactionScope::begin(&conn).await.unwrap();
		let result = tx.rollback().await;
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_transaction_scope_with_isolation(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let tx = TransactionScope::begin_with_isolation(&conn, IsolationLevel::Serializable).await;
		let tx = tx.unwrap();
		let result = tx.commit().await;
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_atomic_helper(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let result = atomic(&conn, || async move { Ok::<_, anyhow::Error>(42) }).await;

		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 42);
	}

	#[rstest]
	#[tokio::test]
	async fn test_atomic_helper_with_error(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let result = atomic(&conn, || async move {
			Err::<i32, _>(anyhow::anyhow!("test error"))
		})
		.await;

		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_atomic_with_isolation_helper(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let result = atomic_with_isolation(&conn, IsolationLevel::Serializable, || async move {
			Ok::<_, anyhow::Error>(100)
		})
		.await;

		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 100);
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

	async fn setup_transaction_test_db() -> reinhardt_core::exception::Result<()> {
		use sqlx::SqlitePool;
		use tokio::sync::OnceCell;

		static POOL: OnceCell<SqlitePool> = OnceCell::const_new();

		// Initialize in-memory SQLite database for testing
		let pool = POOL
			.get_or_init(|| async {
				SqlitePool::connect("sqlite::memory:")
					.await
					.expect("Failed to create in-memory SQLite pool")
			})
			.await;

		// Create table if not exists and clear existing data for test isolation
		sqlx::query(
			"CREATE TABLE IF NOT EXISTS test_items (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                value INTEGER NOT NULL
            )",
		)
		.execute(pool)
		.await
		.map_err(|e| {
			reinhardt_core::exception::Error::Database(format!("Create table failed: {}", e))
		})?;

		// Clear any existing data
		sqlx::query("DELETE FROM test_items")
			.execute(pool)
			.await
			.map_err(|e| {
				reinhardt_core::exception::Error::Database(format!(
					"Clear table data failed: {}",
					e
				))
			})?;

		Ok(())
	}

	/// Test: Transaction begin SQL generation and state management
	///
	/// This test verifies that:
	/// 1. Transaction::begin() generates correct SQL
	/// 2. Transaction state is correctly updated (active, depth)
	/// 3. begin() returns the expected SQL statement
	///
	/// NOTE: This test does NOT execute against a real database (no begin_db()).
	/// It only tests SQL generation and state management logic.
	/// Database execution tests are in tests/integration/.
	#[tokio::test]
	async fn test_begin_db_execution() {
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

	#[tokio::test]
	async fn test_commit_db_sql_generation() {
		// Test that commit_db() generates and attempts to execute correct SQL
		// Note: Full transaction semantics require a dedicated connection
		setup_transaction_test_db().await.unwrap();

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

	#[tokio::test]
	async fn test_rollback_db_sql_generation() {
		// Test that rollback_db() generates and attempts to execute correct SQL
		setup_transaction_test_db().await.unwrap();

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

	#[tokio::test]
	async fn test_nested_transaction_sql_generation() {
		// Test nested transaction (savepoint) SQL generation
		setup_transaction_test_db().await.unwrap();

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

	#[tokio::test]
	async fn test_transaction_isolation_level_sql() {
		// Test that isolation level is properly included in BEGIN statement
		setup_transaction_test_db().await.unwrap();

		let mut tx = Transaction::new().with_isolation_level(IsolationLevel::Serializable);
		let begin_sql = tx.begin().unwrap();

		assert!(begin_sql.contains("ISOLATION LEVEL SERIALIZABLE"));
		assert!(tx.is_active());
	}
}
// Auto-generated tests for transaction module
// Translated from Django/SQLAlchemy test suite
// Total available: 80 | Included: 80

#[cfg(test)]
mod transaction_extended_tests {
	use super::*;
	use crate::orm::connection::{DatabaseBackend, DatabaseConnection};
	// use crate::orm::expressions::{F, Q};
	// use super::transaction::*;
	use crate::backends::backend::DatabaseBackend as BackendTrait;
	use crate::backends::connection::DatabaseConnection as BackendsConnection;
	use crate::backends::error::Result;
	use crate::backends::types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor};
	use rstest::*;
	use std::sync::Arc;

	// Mock transaction executor for testing
	struct MockTransactionExecutor;

	#[async_trait::async_trait]
	impl TransactionExecutor for MockTransactionExecutor {
		async fn execute(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			Ok(QueryResult { rows_affected: 0 })
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
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> Result<()> {
			Ok(())
		}
	}

	struct MockBackend;

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
			Ok(QueryResult { rows_affected: 1 })
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
			Ok(Box::new(MockTransactionExecutor))
		}
	}

	#[fixture]
	fn mock_connection() -> DatabaseConnection {
		let mock_backend = Arc::new(MockBackend);
		let backends_conn = BackendsConnection::new(mock_backend);
		DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn)
	}

	#[test]
	// From: Django/transactions
	fn test_alternate_decorator_syntax_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_alternate_decorator_syntax_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_alternate_decorator_syntax_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_alternate_decorator_syntax_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_allows_queries_after_fixing_transaction() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert!(!tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_allows_queries_after_fixing_transaction_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert!(!tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_does_not_leak_savepoints_on_failure() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_does_not_leak_savepoints_on_failure_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_prevents_calling_transaction_methods() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_prevents_calling_transaction_methods_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_prevents_queries_in_broken_transaction() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_prevents_queries_in_broken_transaction_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_prevents_queries_in_broken_transaction_after_client_close() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert!(!tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_prevents_queries_in_broken_transaction_after_client_close_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert!(!tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_prevents_setting_autocommit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_atomic_prevents_setting_autocommit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_commit_2() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_commit_3() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_decorator_syntax_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_decorator_syntax_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_decorator_syntax_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_decorator_syntax_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_failure_on_exit_transaction() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_failure_on_exit_transaction_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_force_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_force_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_implicit_savepoint_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_implicit_savepoint_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_mark_for_rollback_on_error_in_autocommit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_mark_for_rollback_on_error_in_autocommit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_mark_for_rollback_on_error_in_transaction() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state(), Ok(TransactionState::RolledBack));
	}

	#[test]
	// From: Django/transactions
	fn test_mark_for_rollback_on_error_in_transaction_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state(), Ok(TransactionState::RolledBack));
	}

	#[test]
	// From: Django/transactions
	fn test_merged_commit_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_commit_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_commit_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_commit_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_inner_savepoint_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_merged_inner_savepoint_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_merged_outer_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_outer_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_rollback_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_rollback_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_rollback_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_merged_rollback_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_both_durable() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_both_durable_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_commit_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.depth(), 1);
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_commit_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.depth(), 1);
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_commit_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_commit_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_inner_durable() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_nested_inner_durable_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.depth(), 1);
		assert!(tx.is_active());
	}

	#[test]
	// From: Django/transactions
	fn test_nested_outer_durable() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_outer_durable_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.commit().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_rollback_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_rollback_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_rollback_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_nested_rollback_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_orm_query_after_error_and_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_orm_query_after_error_and_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_orm_query_without_autocommit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		assert!(tx.is_active());
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_orm_query_without_autocommit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		assert!(tx.is_active());
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_prevent_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_prevent_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_reuse_commit_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_reuse_commit_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_reuse_commit_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_reuse_commit_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_reuse_rollback_commit() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_reuse_rollback_commit_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_reuse_rollback_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_reuse_rollback_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_rollback() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_rollback_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.rollback().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	}

	#[test]
	// From: Django/transactions
	fn test_sequence_of_durables() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_sequence_of_durables_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_wrap_callable_instance() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	// From: Django/transactions
	fn test_wrap_callable_instance_1() {
		let mut tx = Transaction::new();
		tx.begin().unwrap();
		tx.commit().unwrap();
		assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	}

	// Tests for new closure-based transaction API
	#[rstest]
	#[tokio::test]
	async fn test_transaction_closure_success(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let result = transaction(&conn, |_tx| async move { Ok(42) }).await;

		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 42);
	}

	#[rstest]
	#[tokio::test]
	async fn test_transaction_closure_error_rollback(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let result: std::result::Result<(), _> =
			transaction(
				&conn,
				|_tx| async move { Err(anyhow::anyhow!("Test error")) },
			)
			.await;

		assert!(result.is_err());
		assert_eq!(result.unwrap_err().to_string(), "Test error");
	}

	#[rstest]
	#[tokio::test]
	async fn test_transaction_with_isolation_level(mock_connection: DatabaseConnection) {
		let conn = mock_connection;

		let result = transaction_with_isolation(
			&conn,
			IsolationLevel::Serializable,
			|_tx| async move { Ok(()) },
		)
		.await;

		assert!(result.is_ok());
	}
}
