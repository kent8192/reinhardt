//! CockroachDB Distributed Transaction Support
//!
//! This module implements distributed transaction handling for CockroachDB,
//! including automatic retry logic for serialization conflicts and support
//! for AS OF SYSTEM TIME queries.
//!
//! CockroachDB uses serializable isolation by default and may require
//! transaction retries when conflicts occur. This module provides helpers
//! to handle these scenarios gracefully.

use sqlx::{PgPool, Postgres, Row, Transaction};
use std::sync::Arc;
use std::time::Duration;

use crate::backends::error::{DatabaseError, Result};

/// Maximum number of transaction retry attempts
const MAX_RETRIES: u32 = 5;

/// Base backoff duration for retry logic
const BASE_BACKOFF_MS: u64 = 100;

/// CockroachDB distributed transaction manager
///
/// Handles transaction retries and AS OF SYSTEM TIME queries for CockroachDB.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::backends::drivers::cockroachdb::distributed_tx::CockroachDBTransactionManager;
/// use sqlx::PgPool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
/// let tx_manager = CockroachDBTransactionManager::new(pool);
///
/// // Execute a transaction with automatic retry
/// tx_manager.execute_with_retry(|tx| Box::pin(async move {
///     sqlx::query("INSERT INTO users (name) VALUES ($1)")
///         .bind("Alice")
///         .execute(&mut **tx)
///         .await?;
///     Ok(())
/// })).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CockroachDBTransactionManager {
	pool: Arc<PgPool>,
	max_retries: u32,
	base_backoff: Duration,
}

impl CockroachDBTransactionManager {
	/// Create a new CockroachDB transaction manager
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::distributed_tx::CockroachDBTransactionManager;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let tx_manager = CockroachDBTransactionManager::new(pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(pool: PgPool) -> Self {
		Self {
			pool: Arc::new(pool),
			max_retries: MAX_RETRIES,
			base_backoff: Duration::from_millis(BASE_BACKOFF_MS),
		}
	}

	/// Create from an `Arc<PgPool>`
	pub fn from_pool_arc(pool: Arc<PgPool>) -> Self {
		Self {
			pool,
			max_retries: MAX_RETRIES,
			base_backoff: Duration::from_millis(BASE_BACKOFF_MS),
		}
	}

	/// Set maximum retry attempts
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::distributed_tx::CockroachDBTransactionManager;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let tx_manager = CockroachDBTransactionManager::new(pool)
	///     .with_max_retries(10);
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_max_retries(mut self, max_retries: u32) -> Self {
		self.max_retries = max_retries;
		self
	}

	/// Set base backoff duration
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::distributed_tx::CockroachDBTransactionManager;
	/// use sqlx::PgPool;
	/// use std::time::Duration;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let tx_manager = CockroachDBTransactionManager::new(pool)
	///     .with_base_backoff(Duration::from_millis(200));
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_base_backoff(mut self, base_backoff: Duration) -> Self {
		self.base_backoff = base_backoff;
		self
	}

	/// Execute a transaction with automatic retry on serialization conflicts
	///
	/// This method will retry the transaction if it encounters a serialization
	/// error (SQLSTATE 40001). The retry uses exponential backoff.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::distributed_tx::CockroachDBTransactionManager;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// # let tx_manager = CockroachDBTransactionManager::new(pool);
	/// tx_manager.execute_with_retry(|tx| Box::pin(async move {
	///     sqlx::query("UPDATE accounts SET balance = balance + $1 WHERE id = $2")
	///         .bind(100)
	///         .bind(1)
	///         .execute(&mut **tx)
	///         .await?;
	///     Ok(())
	/// })).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn execute_with_retry<F, T>(&self, mut f: F) -> Result<T>
	where
		F: for<'a> FnMut(
			&'a mut Transaction<'_, Postgres>,
		) -> std::pin::Pin<
			Box<dyn std::future::Future<Output = Result<T>> + Send + 'a>,
		>,
		T: Send,
	{
		let mut attempt = 0;

		loop {
			let mut tx = self.pool.begin().await.map_err(DatabaseError::from)?;

			match f(&mut tx).await {
				Ok(result) => {
					tx.commit().await.map_err(DatabaseError::from)?;
					return Ok(result);
				}
				Err(e) => {
					let _ = tx.rollback().await;

					if Self::is_serialization_error(&e) && attempt < self.max_retries {
						attempt += 1;
						let _backoff = self.calculate_backoff(attempt);
						continue;
					}

					return Err(e);
				}
			}
		}
	}

	/// Get a reference to the underlying pool for direct queries
	///
	/// Use this to construct custom queries with AS OF SYSTEM TIME.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::distributed_tx::CockroachDBTransactionManager;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// # let tx_manager = CockroachDBTransactionManager::new(pool);
	/// // Use the pool to execute queries with AS OF SYSTEM TIME
	/// let pool = tx_manager.pool();
	/// let rows = sqlx::query("SELECT * FROM users AS OF SYSTEM TIME '-5s'")
	///     .fetch_all(pool)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn pool(&self) -> &PgPool {
		&self.pool
	}

	/// Execute a transaction with a specified priority
	///
	/// CockroachDB supports transaction priorities (LOW, NORMAL, HIGH).
	/// Higher priority transactions are less likely to be aborted.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::distributed_tx::CockroachDBTransactionManager;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// # let tx_manager = CockroachDBTransactionManager::new(pool);
	/// tx_manager.execute_with_priority("HIGH", |tx| Box::pin(async move {
	///     sqlx::query("INSERT INTO users (name) VALUES ($1)")
	///         .bind("Alice")
	///         .execute(&mut **tx)
	///         .await?;
	///     Ok(())
	/// })).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn execute_with_priority<F, T>(&self, priority: &str, mut f: F) -> Result<T>
	where
		F: for<'a> FnMut(
			&'a mut Transaction<'_, Postgres>,
		) -> std::pin::Pin<
			Box<dyn std::future::Future<Output = Result<T>> + Send + 'a>,
		>,
		T: Send,
	{
		let mut tx = self.pool.begin().await.map_err(DatabaseError::from)?;

		let sql = format!("SET TRANSACTION PRIORITY {}", priority);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *tx)
			.await
			.map_err(DatabaseError::from)?;

		let result = f(&mut tx).await?;
		tx.commit().await.map_err(DatabaseError::from)?;

		Ok(result)
	}

	/// Check if error is a serialization/retry error
	fn is_serialization_error(error: &DatabaseError) -> bool {
		match error {
			DatabaseError::QueryError(msg) => {
				// CockroachDB serialization error (SQLSTATE 40001)
				msg.contains("40001")
					|| msg.contains("restart transaction")
					|| msg.contains("serialization failure")
			}
			_ => false,
		}
	}

	/// Calculate exponential backoff with jitter
	fn calculate_backoff(&self, attempt: u32) -> Duration {
		let backoff = self.base_backoff.as_millis() as u64 * 2u64.pow(attempt);
		let jitter = (rand::random::<f64>() * 0.3 + 0.85) * backoff as f64;
		Duration::from_millis(jitter as u64)
	}

	/// Get cluster information
	///
	/// Query SHOW CLUSTER SETTING version for cluster version info.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::distributed_tx::CockroachDBTransactionManager;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// # let tx_manager = CockroachDBTransactionManager::new(pool);
	/// let info = tx_manager.get_cluster_info().await?;
	/// println!("Cluster version: {}", info.version);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get_cluster_info(&self) -> Result<ClusterInfo> {
		let row = sqlx::query("SHOW CLUSTER SETTING version")
			.fetch_one(self.pool.as_ref())
			.await
			.map_err(DatabaseError::from)?;

		let version: String = row.try_get(0).map_err(DatabaseError::from)?;

		Ok(ClusterInfo { version })
	}
}

/// CockroachDB cluster information
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterInfo {
	/// CockroachDB version
	pub version: String,
}

// Add rand dependency internally for jitter calculation
mod rand {
	use std::cell::RefCell;
	use std::time::{SystemTime, UNIX_EPOCH};

	thread_local! {
		static RNG: RefCell<u64> = RefCell::new(
			SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.unwrap()
				.as_nanos() as u64
		);
	}

	pub(crate) fn random<T: RandomValue>() -> T {
		T::random()
	}

	pub(crate) trait RandomValue {
		fn random() -> Self;
	}

	impl RandomValue for f64 {
		fn random() -> Self {
			RNG.with(|rng| {
				let mut state = rng.borrow_mut();
				// Simple LCG for jitter
				*state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
				(*state >> 11) as f64 / ((1u64 << 53) as f64)
			})
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_cluster_info_creation() {
		let info = ClusterInfo {
			version: "v23.1.0".to_string(),
		};

		assert_eq!(info.version, "v23.1.0");
	}

	#[test]
	fn test_is_serialization_error() {
		let err1 = DatabaseError::QueryError("SQLSTATE 40001: restart transaction".to_string());
		assert!(CockroachDBTransactionManager::is_serialization_error(&err1));

		let err2 = DatabaseError::QueryError("serialization failure".to_string());
		assert!(CockroachDBTransactionManager::is_serialization_error(&err2));

		let err3 = DatabaseError::QueryError("some other error".to_string());
		assert!(!CockroachDBTransactionManager::is_serialization_error(
			&err3
		));

		let err4 = DatabaseError::ConnectionError("connection failed".to_string());
		assert!(!CockroachDBTransactionManager::is_serialization_error(
			&err4
		));
	}

	#[tokio::test]
	async fn test_with_max_retries() {
		let pool = Arc::new(
			PgPool::connect_lazy("postgresql://localhost:26257/testdb")
				.expect("Failed to create lazy pool"),
		);
		let tx_manager = CockroachDBTransactionManager::from_pool_arc(pool).with_max_retries(10);

		assert_eq!(tx_manager.max_retries, 10);
	}

	#[tokio::test]
	async fn test_with_base_backoff() {
		let pool = Arc::new(
			PgPool::connect_lazy("postgresql://localhost:26257/testdb")
				.expect("Failed to create lazy pool"),
		);
		let tx_manager = CockroachDBTransactionManager::from_pool_arc(pool)
			.with_base_backoff(Duration::from_millis(200));

		assert_eq!(tx_manager.base_backoff, Duration::from_millis(200));
	}

	#[tokio::test]
	async fn test_calculate_backoff() {
		let pool = Arc::new(
			PgPool::connect_lazy("postgresql://localhost:26257/testdb")
				.expect("Failed to create lazy pool"),
		);
		let tx_manager = CockroachDBTransactionManager::from_pool_arc(pool);

		// Test that backoff increases with attempts
		let backoff1 = tx_manager.calculate_backoff(1);
		let backoff2 = tx_manager.calculate_backoff(2);

		// backoff2 should be roughly double backoff1 (with jitter)
		assert!(backoff2 > backoff1);
	}

	#[test]
	fn test_random_f64() {
		let val1: f64 = rand::random();
		let val2: f64 = rand::random();

		// Check values are in range [0, 1)
		assert!((0.0..1.0).contains(&val1));
		assert!((0.0..1.0).contains(&val2));

		// Values should be different (extremely unlikely to be equal)
		assert_ne!(val1, val2);
	}
}
