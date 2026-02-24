//! MySQL-specific Two-Phase Commit implementation with type-safe state transitions
//!
//! This module implements the `TwoPhaseParticipant` trait for MySQL using
//! the XA transaction protocol (XA START, XA END, XA PREPARE, XA COMMIT, XA ROLLBACK).
//!
//! # Type-Safe State Transitions
//!
//! The API uses the type system to enforce correct XA transaction state transitions at compile time:
//!
//! ```text
//! Idle --begin()--> Started --end()--> Ended --prepare()--> Prepared
//!                                                               |
//!                                                   commit() ---+--- rollback()
//!                                                               |
//!                                                          Completed
//! ```
//!
//! Invalid state transitions are caught at compile time. For example, you cannot call
//! `prepare()` on a `XaSessionStarted` - it requires `XaSessionEnded`.

use sqlx::pool::PoolConnection;
use sqlx::{MySql, MySqlPool, Row};
use std::sync::Arc;

use crate::backends::error::{DatabaseError, Result};

/// XA transaction session in Started state
///
/// After calling `begin()`, the transaction is in Started state.
/// You can perform database operations on the connection.
pub struct XaSessionStarted {
	/// The dedicated MySQL connection for this XA transaction
	pub connection: PoolConnection<MySql>,
	/// The XA transaction identifier
	pub xid: String,
}

/// XA transaction session in Ended state
///
/// After calling `end()` on a Started session, the transaction is in Ended state.
/// From here, you can either `prepare()` for two-phase commit or use `commit_one_phase()`.
pub struct XaSessionEnded {
	/// The dedicated MySQL connection for this XA transaction
	pub connection: PoolConnection<MySql>,
	/// The XA transaction identifier
	pub xid: String,
}

/// XA transaction session in Prepared state
///
/// After calling `prepare()` on an Ended session, the transaction is in Prepared state.
/// From here, you must either `commit()` or `rollback()` the transaction.
pub struct XaSessionPrepared {
	/// The dedicated MySQL connection for this XA transaction
	pub connection: PoolConnection<MySql>,
	/// The XA transaction identifier
	pub xid: String,
}

/// MySQL Two-Phase Commit participant using XA transactions
///
/// Manages two-phase commit transactions using MySQL's XA transaction protocol.
/// XA transactions in MySQL follow the X/Open XA standard for distributed
/// transaction processing.
///
/// # Type-Safe State Transitions
///
/// The API enforces correct state transitions at compile time:
///
/// | State      | Valid Methods           | Transitions To     |
/// |------------|-------------------------|--------------------|
/// | Idle       | `begin()`               | `XaSessionStarted` |
/// | Started    | `end()`, `rollback()`   | `XaSessionEnded`   |
/// | Ended      | `prepare()`, `commit_one_phase()` | `XaSessionPrepared` |
/// | Prepared   | `commit()`, `rollback()` | Completed         |
///
/// # Example: Complete Two-Phase Commit Flow
///
/// ```no_run
/// use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
/// use sqlx::MySqlPool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
/// let participant = MySqlTwoPhaseParticipant::new(pool);
///
/// // 1. Start XA transaction - returns XaSessionStarted
/// let session = participant.begin("txn_001").await?;
///
/// // 2. Perform database operations
/// // sqlx::query("INSERT INTO ...").execute(&mut *session.connection).await?;
///
/// // 3. End the XA transaction - consumes Started, returns Ended
/// let session = participant.end(session).await?;
///
/// // 4. Prepare for two-phase commit - consumes Ended, returns Prepared
/// let session = participant.prepare(session).await?;
///
/// // 5. Commit the prepared transaction - consumes Prepared
/// participant.commit(session).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Example: One-Phase Commit Optimization
///
/// ```no_run
/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
/// # use sqlx::MySqlPool;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
/// # let participant = MySqlTwoPhaseParticipant::new(pool);
/// let session = participant.begin("txn_002").await?;
/// // ... perform operations ...
/// let session = participant.end(session).await?;
///
/// // Skip prepare and use one-phase commit
/// participant.commit_one_phase(session).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Example: Rollback from Any State
///
/// ```no_run
/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
/// # use sqlx::MySqlPool;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
/// # let participant = MySqlTwoPhaseParticipant::new(pool);
/// let session = participant.begin("txn_003").await?;
///
/// // Can rollback from Started state
/// participant.rollback_started(session).await?;
///
/// // Or from Prepared state
/// let session = participant.begin("txn_004").await?;
/// let session = participant.end(session).await?;
/// let session = participant.prepare(session).await?;
/// participant.rollback_prepared(session).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct MySqlTwoPhaseParticipant {
	pool: Arc<MySqlPool>,
}

impl MySqlTwoPhaseParticipant {
	/// Create a new MySQL two-phase commit participant
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// use sqlx::MySqlPool;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// let participant = MySqlTwoPhaseParticipant::new(pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(pool: MySqlPool) -> Self {
		Self {
			pool: Arc::new(pool),
		}
	}

	/// Create from an `Arc<MySqlPool>`
	pub fn from_pool_arc(pool: Arc<MySqlPool>) -> Self {
		Self { pool }
	}

	/// Get a reference to the underlying MySqlPool
	///
	/// This method is useful for tests and advanced use cases where direct
	/// access to the pool is required.
	pub fn pool(&self) -> &MySqlPool {
		self.pool.as_ref()
	}

	/// Start an XA transaction
	///
	/// Executes `XA START 'xid'` in MySQL and returns an `XaSessionStarted` that owns
	/// the connection. All subsequent XA operations must use this session.
	///
	/// # Returns
	///
	/// Returns `XaSessionStarted` which can be used to perform database operations
	/// and must be passed to `end()` to transition to the next state.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// let session = participant.begin("txn_001").await?;
	/// // Perform database operations on session.connection
	/// # Ok(())
	/// # }
	/// ```
	pub async fn begin(&self, xid: impl Into<String>) -> Result<XaSessionStarted> {
		// Acquire new connection for XA transaction
		let mut connection = self.pool.acquire().await.map_err(DatabaseError::from)?;
		let xid = xid.into();

		let sql = format!("XA START '{}'", Self::escape_xid(&xid));
		// MySQL XA commands are not supported in prepared statement protocol
		sqlx::raw_sql(&sql)
			.execute(&mut *connection)
			.await
			.map_err(DatabaseError::from)?;

		Ok(XaSessionStarted { connection, xid })
	}

	/// End an XA transaction
	///
	/// Executes `XA END 'xid'` in MySQL. Must be called before `prepare()`.
	/// Consumes `XaSessionStarted` and returns `XaSessionEnded`.
	///
	/// # Type Safety
	///
	/// This method enforces at compile time that:
	/// - The transaction is in Started state (accepts only `XaSessionStarted`)
	/// - The session transitions to Ended state (returns `XaSessionEnded`)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// let session = participant.begin("txn_001").await?;
	/// // ... perform operations ...
	/// let session = participant.end(session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn end(&self, mut session: XaSessionStarted) -> Result<XaSessionEnded> {
		let sql = format!("XA END '{}'", Self::escape_xid(&session.xid));
		sqlx::raw_sql(&sql)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		Ok(XaSessionEnded {
			connection: session.connection,
			xid: session.xid,
		})
	}

	/// Prepare an XA transaction for two-phase commit
	///
	/// Executes `XA PREPARE 'xid'` in MySQL.
	/// Consumes `XaSessionEnded` and returns `XaSessionPrepared`.
	///
	/// # Type Safety
	///
	/// This method enforces at compile time that:
	/// - The transaction is in Ended state (accepts only `XaSessionEnded`)
	/// - Cannot be called before `end()` (compile error if you try)
	/// - The session transitions to Prepared state (returns `XaSessionPrepared`)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// let session = participant.begin("txn_001").await?;
	/// // ... perform operations ...
	/// let session = participant.end(session).await?;
	/// let session = participant.prepare(session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn prepare(&self, mut session: XaSessionEnded) -> Result<XaSessionPrepared> {
		let sql = format!("XA PREPARE '{}'", Self::escape_xid(&session.xid));
		sqlx::raw_sql(&sql)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		Ok(XaSessionPrepared {
			connection: session.connection,
			xid: session.xid,
		})
	}

	/// Commit a prepared XA transaction
	///
	/// Executes `XA COMMIT 'xid'` in MySQL. Consumes `XaSessionPrepared` to complete
	/// the two-phase commit.
	///
	/// # Type Safety
	///
	/// This method enforces at compile time that:
	/// - The transaction is in Prepared state (accepts only `XaSessionPrepared`)
	/// - Cannot be called before `prepare()` (compile error if you try)
	/// - The transaction is completed (session is consumed)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// let session = participant.begin("txn_001").await?;
	/// let session = participant.end(session).await?;
	/// let session = participant.prepare(session).await?;
	/// participant.commit(session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn commit(&self, mut session: XaSessionPrepared) -> Result<()> {
		let sql = format!("XA COMMIT '{}'", Self::escape_xid(&session.xid));
		sqlx::raw_sql(&sql)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Session is consumed and connection is dropped
		Ok(())
	}

	/// Commit a prepared XA transaction by XID (for recovery scenarios)
	///
	/// This executes `XA COMMIT 'xid'` in MySQL using a new connection.
	/// Use this for recovery scenarios where you don't have the original session.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// // Recovery scenario: commit by XID directly
	/// participant.commit_by_xid("txn_001").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn commit_by_xid(&self, xid: &str) -> Result<()> {
		let mut conn = self.pool.acquire().await.map_err(DatabaseError::from)?;
		let sql = format!("XA COMMIT '{}'", Self::escape_xid(xid));
		sqlx::raw_sql(&sql)
			.execute(&mut *conn)
			.await
			.map_err(DatabaseError::from)?;
		Ok(())
	}

	/// Commit an XA transaction with one-phase optimization
	///
	/// Executes `XA COMMIT 'xid' ONE PHASE` in MySQL. This is an optimization
	/// for single-phase commit that skips the PREPARE step. Consumes `XaSessionEnded`.
	///
	/// # Type Safety
	///
	/// This method enforces at compile time that:
	/// - The transaction is in Ended state (accepts only `XaSessionEnded`)
	/// - The PREPARE step is not required for one-phase commit
	/// - The transaction is completed (session is consumed)
	///
	/// # When to Use
	///
	/// Use this method when you don't need the full two-phase commit protocol,
	/// such as when there's only one participant in the distributed transaction.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// let session = participant.begin("txn_001").await?;
	/// // ... perform operations ...
	/// let session = participant.end(session).await?;
	/// participant.commit_one_phase(session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn commit_one_phase(&self, mut session: XaSessionEnded) -> Result<()> {
		let sql = format!("XA COMMIT '{}' ONE PHASE", Self::escape_xid(&session.xid));
		sqlx::raw_sql(&sql)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Session is consumed and connection is dropped
		Ok(())
	}

	/// Rollback an XA transaction from Started state
	///
	/// Executes `XA ROLLBACK 'xid'` in MySQL from the Started state.
	/// Consumes `XaSessionStarted`.
	///
	/// # Type Safety
	///
	/// This method enforces at compile time that:
	/// - The transaction is in Started state (accepts only `XaSessionStarted`)
	/// - The transaction is aborted (session is consumed)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// let session = participant.begin("txn_001").await?;
	/// // Error occurred, rollback from Started state
	/// participant.rollback_started(session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rollback_started(&self, mut session: XaSessionStarted) -> Result<()> {
		let sql = format!("XA ROLLBACK '{}'", Self::escape_xid(&session.xid));
		sqlx::raw_sql(&sql)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Session is consumed and connection is dropped
		Ok(())
	}

	/// Rollback an XA transaction from Prepared state
	///
	/// Executes `XA ROLLBACK 'xid'` in MySQL from the Prepared state.
	/// Consumes `XaSessionPrepared`.
	///
	/// # Type Safety
	///
	/// This method enforces at compile time that:
	/// - The transaction is in Prepared state (accepts only `XaSessionPrepared`)
	/// - The transaction is aborted (session is consumed)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// let session = participant.begin("txn_001").await?;
	/// let session = participant.end(session).await?;
	/// let session = participant.prepare(session).await?;
	/// // Decide to rollback instead of commit
	/// participant.rollback_prepared(session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rollback_prepared(&self, mut session: XaSessionPrepared) -> Result<()> {
		let sql = format!("XA ROLLBACK '{}'", Self::escape_xid(&session.xid));
		sqlx::raw_sql(&sql)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Session is consumed and connection is dropped
		Ok(())
	}

	/// Rollback a prepared XA transaction by XID (for recovery scenarios)
	///
	/// This executes `XA ROLLBACK 'xid'` in MySQL using a new connection.
	/// Use this for recovery scenarios where you don't have the original session.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// // Recovery scenario: rollback by XID directly
	/// participant.rollback_by_xid("txn_001").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rollback_by_xid(&self, xid: &str) -> Result<()> {
		let mut conn = self.pool.acquire().await.map_err(DatabaseError::from)?;
		let sql = format!("XA ROLLBACK '{}'", Self::escape_xid(xid));
		sqlx::raw_sql(&sql)
			.execute(&mut *conn)
			.await
			.map_err(DatabaseError::from)?;
		Ok(())
	}

	/// Query all prepared XA transactions using XA RECOVER
	///
	/// Returns a list of prepared transaction IDs. This is useful for recovery
	/// scenarios where you need to find orphaned prepared transactions.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// let prepared_txns = participant.list_prepared_transactions().await?;
	/// for txn_info in prepared_txns {
	///     println!("Prepared XA transaction: {:?}", txn_info);
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn list_prepared_transactions(&self) -> Result<Vec<XaTransactionInfo>> {
		// MySQL XA RECOVER is not supported in prepared statement protocol
		let rows = sqlx::raw_sql("XA RECOVER")
			.fetch_all(self.pool.as_ref())
			.await
			.map_err(DatabaseError::from)?;

		let mut transactions = Vec::new();
		for row in rows {
			// XA RECOVER returns: formatID, gtrid_length, bqual_length, data
			let format_id: i32 = row.try_get("formatID").map_err(DatabaseError::from)?;
			let gtrid_length: i32 = row.try_get("gtrid_length").map_err(DatabaseError::from)?;
			let bqual_length: i32 = row.try_get("bqual_length").map_err(DatabaseError::from)?;
			let data: Vec<u8> = row.try_get("data").map_err(DatabaseError::from)?;

			transactions.push(XaTransactionInfo {
				format_id,
				gtrid_length,
				bqual_length,
				data: data.clone(),
				xid: String::from_utf8_lossy(&data).to_string(),
			});
		}

		Ok(transactions)
	}

	/// Find a specific prepared XA transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// if let Some(info) = participant.find_prepared_transaction("txn_001").await? {
	///     println!("Found prepared XA transaction: {:?}", info);
	///     // Decide whether to commit or rollback using XID-based methods
	///     participant.commit_by_xid("txn_001").await?;
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn find_prepared_transaction(&self, xid: &str) -> Result<Option<XaTransactionInfo>> {
		let all_txns = self.list_prepared_transactions().await?;
		Ok(all_txns.into_iter().find(|txn| txn.xid == xid))
	}

	/// Cleanup stale prepared XA transactions
	///
	/// This method queries all prepared transactions and attempts to rollback
	/// those matching a specific pattern. Use with caution as this may affect
	/// in-progress distributed transactions.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::mysql::two_phase::MySqlTwoPhaseParticipant;
	/// # use sqlx::MySqlPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// # let participant = MySqlTwoPhaseParticipant::new(pool);
	/// // Cleanup all transactions starting with "stale_"
	/// let cleaned = participant.cleanup_stale_transactions("stale_").await?;
	/// println!("Cleaned up {} stale XA transactions", cleaned);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn cleanup_stale_transactions(&self, prefix: &str) -> Result<usize> {
		let all_txns = self.list_prepared_transactions().await?;
		let mut cleaned = 0;

		for txn in all_txns {
			if txn.xid.starts_with(prefix) && self.rollback_by_xid(&txn.xid).await.is_ok() {
				cleaned += 1;
			}
		}

		Ok(cleaned)
	}

	/// Escape XID to prevent SQL injection
	///
	/// Note: MySQL XA transaction IDs have specific format requirements.
	/// This is a simple escaping mechanism that removes single quotes.
	fn escape_xid(xid: &str) -> String {
		xid.replace('\'', "''")
	}
}

/// Information about an XA transaction from XA RECOVER
#[derive(Debug, Clone, PartialEq)]
pub struct XaTransactionInfo {
	/// Format identifier
	pub format_id: i32,
	/// Global transaction ID length
	pub gtrid_length: i32,
	/// Branch qualifier length
	pub bqual_length: i32,
	/// Raw transaction data
	pub data: Vec<u8>,
	/// String representation of the XID
	pub xid: String,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_xa_transaction_info_creation() {
		let info = XaTransactionInfo {
			format_id: 1,
			gtrid_length: 7,
			bqual_length: 0,
			data: b"txn_001".to_vec(),
			xid: "txn_001".to_string(),
		};

		assert_eq!(info.format_id, 1);
		assert_eq!(info.gtrid_length, 7);
		assert_eq!(info.bqual_length, 0);
		assert_eq!(info.xid, "txn_001");
	}

	#[test]
	fn test_escape_xid() {
		assert_eq!(MySqlTwoPhaseParticipant::escape_xid("simple"), "simple");
		assert_eq!(MySqlTwoPhaseParticipant::escape_xid("it's"), "it''s");
		assert_eq!(MySqlTwoPhaseParticipant::escape_xid("a'b'c"), "a''b''c");
	}

	#[tokio::test]
	async fn test_participant_clone() {
		// Test that MySqlTwoPhaseParticipant can be cloned
		let pool = Arc::new(
			MySqlPool::connect_lazy("mysql://localhost/testdb")
				.expect("Failed to create lazy pool"),
		);
		let participant1 = MySqlTwoPhaseParticipant::from_pool_arc(pool.clone());
		let participant2 = participant1.clone();

		// Both should reference the same pool
		assert!(Arc::ptr_eq(&participant1.pool, &participant2.pool));
	}
}
