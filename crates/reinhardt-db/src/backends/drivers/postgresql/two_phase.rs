//! PostgreSQL-specific Two-Phase Commit implementation
//!
//! This module implements the `TwoPhaseParticipant` trait for PostgreSQL using
//! the PREPARE TRANSACTION, COMMIT PREPARED, and ROLLBACK PREPARED SQL commands.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, pool::PoolConnection};
use std::sync::Arc;

use crate::backends::error::{DatabaseError, Result};

/// Session for a PostgreSQL two-phase commit transaction
///
/// Owns a dedicated database connection for the entire lifecycle of the transaction.
/// This ensures that BEGIN, all data modifications, and PREPARE TRANSACTION execute
/// on the same connection, which is required for correct 2PC semantics.
pub struct PgSession {
	/// The dedicated PostgreSQL connection for this transaction
	pub connection: PoolConnection<Postgres>,
	/// The transaction identifier
	pub xid: String,
	/// Current state of the transaction
	pub state: PgTwoPhaseState,
}

/// State of a PostgreSQL two-phase transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PgTwoPhaseState {
	/// Transaction is active (after BEGIN)
	Active,
	/// Transaction has been prepared (after PREPARE TRANSACTION)
	Prepared,
}

/// PostgreSQL Two-Phase Commit participant
///
/// Manages two-phase commit transactions using PostgreSQL's PREPARE TRANSACTION feature.
///
/// # Requirements
///
/// PostgreSQL must be configured with `max_prepared_transactions > 0` (default is 0).
///
/// # Two-Phase Transaction Flow
///
/// 1. BEGIN - Start a transaction
/// 2. ... perform operations ...
/// 3. PREPARE TRANSACTION 'xid' - Prepare the transaction
/// 4. COMMIT PREPARED 'xid' or ROLLBACK PREPARED 'xid' - Finalize
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
/// use sqlx::PgPool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = PgPool::connect("postgres://localhost/mydb").await?;
/// let participant = PostgresTwoPhaseParticipant::new(pool);
///
/// // Start a transaction and get a session
/// let mut session = participant.begin("txn_001").await?;
///
/// // ... perform operations using session.connection ...
///
/// // Prepare the transaction
/// participant.prepare(&mut session).await?;
///
/// // Commit the prepared transaction
/// participant.commit(session).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct PostgresTwoPhaseParticipant {
	pool: Arc<PgPool>,
	// Internal session storage for ORM layer compatibility
	// XID -> Session mapping for managing active transactions
	sessions: Arc<std::sync::Mutex<std::collections::HashMap<String, PgSession>>>,
}

impl PostgresTwoPhaseParticipant {
	/// Create a new PostgreSQL two-phase commit participant
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// let participant = PostgresTwoPhaseParticipant::new(pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(pool: PgPool) -> Self {
		Self {
			pool: Arc::new(pool),
			sessions: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
		}
	}

	/// Create from an `Arc<PgPool>`
	pub fn from_pool_arc(pool: Arc<PgPool>) -> Self {
		Self {
			pool,
			sessions: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
		}
	}

	/// Get a reference to the underlying PgPool
	///
	/// This method is useful for tests and advanced use cases where direct
	/// access to the pool is required.
	pub fn pool(&self) -> &PgPool {
		self.pool.as_ref()
	}

	/// Begin a transaction and return a session
	///
	/// Acquires a dedicated connection from the pool and starts a transaction.
	/// The returned session owns this connection for the entire transaction lifetime.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// let mut session = participant.begin("txn_001").await?;
	/// // ... perform operations using session.connection ...
	/// # Ok(())
	/// # }
	/// ```
	pub async fn begin(&self, xid: &str) -> Result<PgSession> {
		// Acquire dedicated connection for the transaction
		let mut connection = self.pool.acquire().await.map_err(DatabaseError::from)?;

		// Start transaction on the dedicated connection
		sqlx::query("BEGIN")
			.execute(&mut *connection)
			.await
			.map_err(DatabaseError::from)?;

		Ok(PgSession {
			connection,
			xid: xid.to_string(),
			state: PgTwoPhaseState::Active,
		})
	}

	/// Prepare a transaction for two-phase commit
	///
	/// This executes `PREPARE TRANSACTION 'xid'` in PostgreSQL.
	/// Transitions the session state from Active to Prepared.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The transaction is not active
	/// - `max_prepared_transactions` is set to 0
	/// - The transaction ID already exists
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// let mut session = participant.begin("txn_001").await?;
	/// // ... perform operations using session.connection ...
	/// participant.prepare(&mut session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn prepare(&self, session: &mut PgSession) -> Result<()> {
		// Use Box::leak to convert String to &'static str for sqlx compatibility
		// This is acceptable as prepared transaction IDs are typically short-lived
		let xid_escaped = pg_escape::quote_literal(&session.xid);
		let sql = format!("PREPARE TRANSACTION {}", xid_escaped);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		session.state = PgTwoPhaseState::Prepared;
		Ok(())
	}

	/// Commit a prepared transaction
	///
	/// This executes `COMMIT PREPARED 'xid'` in PostgreSQL. Consumes the session.
	///
	/// # Errors
	///
	/// Returns an error if the prepared transaction does not exist.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// let mut session = participant.begin("txn_001").await?;
	/// participant.prepare(&mut session).await?;
	/// participant.commit(session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn commit(&self, mut session: PgSession) -> Result<()> {
		// Use Box::leak to convert String to &'static str for sqlx compatibility
		let xid_escaped = pg_escape::quote_literal(&session.xid);
		let sql = format!("COMMIT PREPARED {}", xid_escaped);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Session is consumed and connection is dropped
		Ok(())
	}

	/// Commit a prepared transaction by XID (for recovery scenarios)
	///
	/// This executes `COMMIT PREPARED 'xid'` in PostgreSQL using a new connection.
	/// Use this for recovery scenarios where you don't have the original session.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// // Recovery scenario: commit by XID directly
	/// participant.commit_by_xid("txn_001").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn commit_by_xid(&self, xid: &str) -> Result<()> {
		let mut conn = self.pool.acquire().await.map_err(DatabaseError::from)?;
		let xid_escaped = pg_escape::quote_literal(xid);
		let sql = format!("COMMIT PREPARED {}", xid_escaped);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *conn)
			.await
			.map_err(DatabaseError::from)?;
		Ok(())
	}

	/// Rollback a prepared transaction
	///
	/// This executes `ROLLBACK PREPARED 'xid'` in PostgreSQL. Consumes the session.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// let mut session = participant.begin("txn_001").await?;
	/// participant.prepare(&mut session).await?;
	/// participant.rollback(session).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rollback(&self, mut session: PgSession) -> Result<()> {
		// Use Box::leak to convert String to &'static str for sqlx compatibility
		let xid_escaped = pg_escape::quote_literal(&session.xid);
		let sql = format!("ROLLBACK PREPARED {}", xid_escaped);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Session is consumed and connection is dropped
		Ok(())
	}

	/// Rollback a prepared transaction by XID (for recovery scenarios)
	///
	/// This executes `ROLLBACK PREPARED 'xid'` in PostgreSQL using a new connection.
	/// Use this for recovery scenarios where you don't have the original session.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// // Recovery scenario: rollback by XID directly
	/// participant.rollback_by_xid("txn_001").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rollback_by_xid(&self, xid: &str) -> Result<()> {
		let mut conn = self.pool.acquire().await.map_err(DatabaseError::from)?;
		let xid_escaped = pg_escape::quote_literal(xid);
		let sql = format!("ROLLBACK PREPARED {}", xid_escaped);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *conn)
			.await
			.map_err(DatabaseError::from)?;
		Ok(())
	}

	/// Query all prepared transactions from pg_prepared_xacts
	///
	/// Returns a list of prepared transaction IDs. This is useful for recovery
	/// scenarios where you need to find orphaned prepared transactions.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// let prepared_txns = participant.list_prepared_transactions().await?;
	/// for txn_info in prepared_txns {
	///     println!("Prepared transaction: {}", txn_info.gid);
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn list_prepared_transactions(&self) -> Result<Vec<PreparedTransactionInfo>> {
		let rows = sqlx::query(
			"SELECT gid, prepared, owner, database FROM pg_prepared_xacts ORDER BY prepared",
		)
		.fetch_all(self.pool.as_ref())
		.await
		.map_err(DatabaseError::from)?;

		let mut transactions = Vec::new();
		for row in rows {
			transactions.push(PreparedTransactionInfo {
				gid: row.try_get("gid").map_err(DatabaseError::from)?,
				prepared: row.try_get("prepared").map_err(DatabaseError::from)?,
				owner: row.try_get("owner").map_err(DatabaseError::from)?,
				database: row.try_get("database").map_err(DatabaseError::from)?,
			});
		}

		Ok(transactions)
	}

	/// Recover a specific prepared transaction
	///
	/// This is a convenience method that checks if a transaction is prepared
	/// and returns its information.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// if let Some(info) = participant.find_prepared_transaction("txn_001").await? {
	///     println!("Found prepared transaction: {:?}", info);
	///     // Decide whether to commit or rollback
	///     participant.commit_by_xid("txn_001").await?;
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn find_prepared_transaction(
		&self,
		xid: &str,
	) -> Result<Option<PreparedTransactionInfo>> {
		let row = sqlx::query(
			"SELECT gid, prepared, owner, database FROM pg_prepared_xacts WHERE gid = $1",
		)
		.bind(xid)
		.fetch_optional(self.pool.as_ref())
		.await
		.map_err(DatabaseError::from)?;

		if let Some(row) = row {
			Ok(Some(PreparedTransactionInfo {
				gid: row.try_get("gid").map_err(DatabaseError::from)?,
				prepared: row.try_get("prepared").map_err(DatabaseError::from)?,
				owner: row.try_get("owner").map_err(DatabaseError::from)?,
				database: row.try_get("database").map_err(DatabaseError::from)?,
			}))
		} else {
			Ok(None)
		}
	}

	/// Cleanup stale prepared transactions older than the specified duration
	///
	/// This is useful for recovering from failures where prepared transactions
	/// were abandoned. Use with caution as this may affect in-progress distributed
	/// transactions.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::postgresql::two_phase::PostgresTwoPhaseParticipant;
	/// # use sqlx::PgPool;
	/// # use std::time::Duration;
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// # let participant = PostgresTwoPhaseParticipant::new(pool);
	/// // Rollback transactions older than 1 hour
	/// let cleaned = participant.cleanup_stale_transactions(Duration::from_secs(3600)).await?;
	/// println!("Cleaned up {} stale transactions", cleaned);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn cleanup_stale_transactions(&self, max_age: std::time::Duration) -> Result<usize> {
		let max_age_secs = max_age.as_secs() as i32;
		let rows = sqlx::query(
			"SELECT gid FROM pg_prepared_xacts
             WHERE EXTRACT(EPOCH FROM (NOW() - prepared)) > $1",
		)
		.bind(max_age_secs)
		.fetch_all(self.pool.as_ref())
		.await
		.map_err(DatabaseError::from)?;

		let mut cleaned = 0;
		for row in rows {
			let gid: String = row.try_get("gid").map_err(DatabaseError::from)?;
			if self.rollback_by_xid(&gid).await.is_ok() {
				cleaned += 1;
			}
		}

		Ok(cleaned)
	}

	// XID-based wrapper methods for ORM layer compatibility
	// These methods manage sessions internally using the sessions HashMap

	/// Begin a transaction by XID (ORM layer wrapper)
	///
	/// Creates a session and stores it internally for later use.
	pub async fn begin_by_xid(&self, xid: &str) -> Result<()> {
		let session = self.begin(xid).await?;
		self.sessions
			.lock()
			.unwrap()
			.insert(xid.to_string(), session);
		Ok(())
	}

	/// Prepare a transaction by XID (ORM layer wrapper)
	///
	/// Executes PREPARE TRANSACTION without exposing the session to the caller.
	pub async fn prepare_by_xid(&self, xid: &str) -> Result<()> {
		// Extract the session temporarily to avoid holding the lock across await
		let mut session = {
			let mut sessions = self.sessions.lock().unwrap();
			sessions.remove(xid).ok_or_else(|| {
				DatabaseError::QueryError(format!("No active session for XID: {}", xid))
			})?
		};

		// Perform the prepare operation directly without calling self.prepare()
		let xid_escaped = pg_escape::quote_literal(xid);
		let sql = format!("PREPARE TRANSACTION {}", xid_escaped);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Update state and re-insert
		session.state = PgTwoPhaseState::Prepared;
		self.sessions
			.lock()
			.unwrap()
			.insert(xid.to_string(), session);

		Ok(())
	}

	/// Commit a transaction by XID (ORM layer wrapper)
	///
	/// Removes the session from internal storage, executes COMMIT PREPARED, and consumes the session.
	pub async fn commit_managed(&self, xid: &str) -> Result<()> {
		let mut session = self.sessions.lock().unwrap().remove(xid).ok_or_else(|| {
			DatabaseError::QueryError(format!("No active session for XID: {}", xid))
		})?;

		// Execute commit directly without calling self.commit()
		let xid_escaped = pg_escape::quote_literal(xid);
		let sql = format!("COMMIT PREPARED {}", xid_escaped);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Session is consumed and connection is dropped
		Ok(())
	}

	/// Rollback a transaction by XID (ORM layer wrapper)
	///
	/// Removes the session from internal storage, executes ROLLBACK PREPARED, and consumes the session.
	pub async fn rollback_managed(&self, xid: &str) -> Result<()> {
		let mut session = self.sessions.lock().unwrap().remove(xid).ok_or_else(|| {
			DatabaseError::QueryError(format!("No active session for XID: {}", xid))
		})?;

		// Execute rollback directly without calling self.rollback()
		let xid_escaped = pg_escape::quote_literal(xid);
		let sql = format!("ROLLBACK PREPARED {}", xid_escaped);
		let sql_static: &'static str = Box::leak(sql.into_boxed_str());
		sqlx::query(sql_static)
			.execute(&mut *session.connection)
			.await
			.map_err(DatabaseError::from)?;

		// Session is consumed and connection is dropped
		Ok(())
	}
}

/// Information about a prepared transaction
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTransactionInfo {
	/// Global transaction identifier
	pub gid: String,
	/// Timestamp when the transaction was prepared (with timezone)
	pub prepared: DateTime<Utc>,
	/// Owner (role) of the transaction
	pub owner: String,
	/// Database name
	pub database: String,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_prepared_transaction_info_creation() {
		let info = PreparedTransactionInfo {
			gid: "txn_001".to_string(),
			prepared: DateTime::UNIX_EPOCH,
			owner: "postgres".to_string(),
			database: "testdb".to_string(),
		};

		assert_eq!(info.gid, "txn_001");
		assert_eq!(info.owner, "postgres");
		assert_eq!(info.database, "testdb");
	}

	#[tokio::test]
	async fn test_participant_clone() {
		// Test that PostgresTwoPhaseParticipant can be cloned
		// This is important for use in multi-threaded contexts
		let pool = Arc::new(
			PgPool::connect_lazy("postgresql://localhost/testdb")
				.expect("Failed to create lazy pool"),
		);
		let participant1 = PostgresTwoPhaseParticipant::from_pool_arc(pool.clone());
		let participant2 = participant1.clone();

		// Both should reference the same pool
		assert!(Arc::ptr_eq(&participant1.pool, &participant2.pool));
	}
}
