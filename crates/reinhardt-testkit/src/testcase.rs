//! Base test case with common setup and assertions
//!
//! Similar to DRF's APITestCase

use crate::client::APIClient;
use crate::resource::AsyncTestResource;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Error types that can occur during test teardown
#[derive(Debug, Error)]
pub enum TeardownError {
	/// Failed to rollback one or more active transactions
	#[error("Failed to rollback transactions: {0}")]
	TransactionRollbackFailed(String),

	/// Failed to close database connection
	#[error("Failed to close database connection: {0}")]
	ConnectionCloseFailed(String),

	/// Failed to cleanup client state
	#[error("Failed to cleanup client state: {0}")]
	ClientCleanupFailed(String),
}

/// Handle for tracking active test transactions
///
/// This struct tracks transaction state for monitoring and cleanup purposes.
/// Actual transaction management is handled by sqlx's Transaction type,
/// which automatically rolls back uncommitted transactions when dropped.
#[cfg(feature = "testcontainers")]
#[derive(Debug, Clone)]
pub struct TransactionHandle {
	/// Unique identifier for the transaction
	id: String,
	/// Whether the transaction has been committed
	committed: bool,
}

#[cfg(feature = "testcontainers")]
impl TransactionHandle {
	/// Create a new transaction handle with a unique ID
	pub fn new() -> Self {
		Self {
			id: uuid::Uuid::now_v7().to_string(),
			committed: false,
		}
	}

	/// Get the transaction ID
	pub fn id(&self) -> &str {
		&self.id
	}

	/// Check if the transaction has been committed
	pub fn is_committed(&self) -> bool {
		self.committed
	}

	/// Mark the transaction as committed
	pub fn mark_committed(&mut self) {
		self.committed = true;
	}
}

#[cfg(feature = "testcontainers")]
impl Default for TransactionHandle {
	fn default() -> Self {
		Self::new()
	}
}

/// Base test case for API testing
///
/// Provides:
/// - Pre-configured APIClient
/// - Automatic setup/teardown via AsyncTestResource
/// - Assertion helpers
/// - Optional TestContainer database integration
///
/// # Example
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() {
/// use reinhardt_testkit::testcase::APITestCase;
/// use reinhardt_testkit::resource::AsyncTeardownGuard;
/// use rstest::*;
///
/// #[fixture]
/// async fn api_test() -> AsyncTeardownGuard<APITestCase> {
///     AsyncTeardownGuard::new().await
/// }
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_list_users(#[future] api_test: AsyncTeardownGuard<APITestCase>) {
///     let case = api_test.await;
///     let response = case.client().await.get("/api/users/").await.unwrap();
///     response.assert_ok();
/// }
/// # }
/// ```
pub struct APITestCase {
	client: Arc<RwLock<APIClient>>,
	#[cfg(feature = "testcontainers")]
	database_url: Arc<RwLock<Option<String>>>,
	#[cfg(feature = "testcontainers")]
	db_connection: Arc<RwLock<Option<sqlx::AnyPool>>>,
	#[cfg(feature = "testcontainers")]
	active_transactions: Arc<RwLock<Vec<TransactionHandle>>>,
}

impl APITestCase {
	/// Get the database connection URL (if configured)
	#[cfg(feature = "testcontainers")]
	pub async fn database_url(&self) -> Option<String> {
		self.database_url.read().await.clone()
	}

	/// Get the test client
	pub async fn client(&self) -> tokio::sync::RwLockReadGuard<'_, APIClient> {
		self.client.read().await
	}

	/// Get mutable access to the test client
	pub async fn client_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, APIClient> {
		self.client.write().await
	}

	/// Set the database URL (useful for TestContainers integration)
	#[cfg(feature = "testcontainers")]
	pub async fn set_database_url(&self, url: String) {
		let mut db_url = self.database_url.write().await;
		*db_url = Some(url);
	}

	/// Set the database connection pool
	///
	/// This method allows setting a pre-configured database connection pool
	/// for use in tests. The pool will be properly closed during teardown.
	///
	/// # Example
	/// ```rust,ignore
	/// use sqlx::AnyPool;
	///
	/// let pool = AnyPool::connect("postgres://localhost/test").await?;
	/// test_case.set_database_connection(pool).await;
	/// ```
	#[cfg(feature = "testcontainers")]
	pub async fn set_database_connection(&self, pool: sqlx::AnyPool) {
		let mut conn = self.db_connection.write().await;
		*conn = Some(pool);
	}

	/// Get the database connection pool (if configured)
	#[cfg(feature = "testcontainers")]
	pub async fn db_connection(&self) -> Option<sqlx::AnyPool> {
		self.db_connection.read().await.clone()
	}

	/// Begin a new tracked transaction
	///
	/// This method registers a new transaction handle for tracking purposes.
	/// The actual sqlx::Transaction should be obtained from the pool directly.
	/// The handle is used to track whether transactions are properly committed
	/// or rolled back during teardown.
	///
	/// # Returns
	/// A TransactionHandle that can be used to track the transaction state.
	///
	/// # Example
	/// ```rust,ignore
	/// let handle = test_case.begin_transaction().await;
	/// // ... perform database operations with sqlx::Transaction ...
	/// handle.mark_committed(); // Mark as committed if successful
	/// ```
	#[cfg(feature = "testcontainers")]
	pub async fn begin_transaction(&self) -> TransactionHandle {
		let handle = TransactionHandle::new();
		let mut transactions = self.active_transactions.write().await;
		transactions.push(handle.clone());
		handle
	}

	/// Mark a transaction as committed by its ID
	///
	/// This removes the transaction from the active list, indicating
	/// it was successfully committed and doesn't need rollback.
	#[cfg(feature = "testcontainers")]
	pub async fn commit_transaction(&self, transaction_id: &str) {
		let mut transactions = self.active_transactions.write().await;
		if let Some(pos) = transactions.iter().position(|t| t.id() == transaction_id) {
			let mut handle = transactions.remove(pos);
			handle.mark_committed();
		}
	}

	/// Get the count of active (uncommitted) transactions
	#[cfg(feature = "testcontainers")]
	pub async fn active_transaction_count(&self) -> usize {
		self.active_transactions.read().await.len()
	}
}

#[async_trait::async_trait]
impl AsyncTestResource for APITestCase {
	async fn setup() -> Self {
		Self {
			client: Arc::new(RwLock::new(APIClient::new())),
			#[cfg(feature = "testcontainers")]
			database_url: Arc::new(RwLock::new(None)),
			#[cfg(feature = "testcontainers")]
			db_connection: Arc::new(RwLock::new(None)),
			#[cfg(feature = "testcontainers")]
			active_transactions: Arc::new(RwLock::new(Vec::new())),
		}
	}

	async fn teardown(self) {
		// Step 1: Clean up HTTP client state
		{
			let client = self.client.write().await;
			client.cleanup().await;
		}

		// Step 2: Handle database cleanup (testcontainers feature only)
		#[cfg(feature = "testcontainers")]
		{
			// Log any uncommitted transactions (they will be rolled back when pool closes)
			let transactions = self.active_transactions.read().await;
			let uncommitted_count = transactions.iter().filter(|t| !t.is_committed()).count();
			if uncommitted_count > 0 {
				// Uncommitted transactions will be automatically rolled back by sqlx
				// when the pool is closed
				tracing::debug!(
					"Rolling back {} uncommitted transaction(s) during teardown",
					uncommitted_count
				);
			}
			drop(transactions);

			// Close the database connection pool
			let mut pool_guard = self.db_connection.write().await;
			if let Some(pool) = pool_guard.take() {
				// Close the pool gracefully - this will rollback any uncommitted transactions
				pool.close().await;
			}
		}

		// Step 3: Drop the client
		drop(self.client);
	}
}

/// Helper macro for defining test cases with automatic setup/teardown
///
/// # Example
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() {
/// # use reinhardt_testkit::test_case;
/// test_case! {
///     async fn test_get_users(case: &APITestCase) {
///         let client = case.client().await;
///         let response = client.get("/api/users/").await.unwrap();
///         response.assert_ok();
///     }
/// }
/// # }
/// ```
#[macro_export]
macro_rules! test_case {
	(
        async fn $name:ident($case:ident: &APITestCase) $body:block
    ) => {
		#[rstest::rstest]
		#[tokio::test]
		async fn $name() {
			use $crate::resource::AsyncTeardownGuard;
			use $crate::testcase::APITestCase;

			let guard = AsyncTeardownGuard::<APITestCase>::new().await;
			let $case = &*guard;

			// Run test
			$body

			// guard is dropped here, teardown() is automatically called
		}
	};
}

/// Helper macro for defining authenticated test cases
#[macro_export]
macro_rules! authenticated_test_case {
    (
        async fn $name:ident($case:ident: &APITestCase, $user:ident: serde_json::Value) $body:block
    ) => {
        #[rstest::rstest]
        #[tokio::test]
        async fn $name() {
            use $crate::resource::AsyncTeardownGuard;
            use $crate::testcase::APITestCase;

            let guard = AsyncTeardownGuard::<APITestCase>::new().await;
            let $case = &*guard;

            // Setup authentication
            let $user = serde_json::json!({
                "id": 1,
                "username": "testuser",
            });
            {
                let client = $case.client().await;
                #[allow(deprecated)]
                client.force_authenticate(Some($user.clone())).await;
            }

            // Run test
            $body

            // guard is dropped here, teardown() is automatically called
        }
    };
}

/// Helper macro for defining test cases with database containers
///
/// Requires `testcontainers` feature to be enabled.
///
/// This macro automatically sets up a PostgreSQL or MySQL container via TestContainers,
/// initializes an `APITestCase` with the database URL, and ensures proper cleanup.
///
/// # Examples
///
/// ## PostgreSQL Example
///
/// ```rust,ignore
/// use reinhardt_testkit::test_case_with_db;
/// use reinhardt_testkit::testcase::APITestCase;
///
/// test_case_with_db! {
///     postgres,
///     async fn test_users_with_postgres(case: &APITestCase) {
///         let db_url = case.database_url().await.unwrap();
///         // Database URL is automatically set
///         assert!(db_url.starts_with("postgres://"));
///
///         // Perform database operations...
///     }
/// }
/// ```
///
/// ## MySQL Example
///
/// ```rust,ignore
/// use reinhardt_testkit::test_case_with_db;
/// use reinhardt_testkit::testcase::APITestCase;
///
/// test_case_with_db! {
///     mysql,
///     async fn test_users_with_mysql(case: &APITestCase) {
///         let db_url = case.database_url().await.unwrap();
///         // Database URL is automatically set
///         assert!(db_url.starts_with("mysql://"));
///
///         // Perform database operations...
///     }
/// }
/// ```
#[cfg(feature = "testcontainers")]
#[macro_export]
macro_rules! test_case_with_db {
    (
        postgres,
        async fn $name:ident($case:ident: &APITestCase) $body:block
    ) => {
        #[rstest::rstest]
        #[tokio::test]
        async fn $name() {
            use $crate::containers::{with_postgres, PostgresContainer};
            use $crate::resource::AsyncTeardownGuard;
            use $crate::testcase::APITestCase;

            with_postgres(|db| async move {
                let guard = AsyncTeardownGuard::<APITestCase>::new().await;
                let $case = &*guard;
                $case.set_database_url(db.connection_url()).await;

                // Run test
                $body

                // guard is dropped here, teardown() is automatically called
                Ok(())
            })
            .await
            .unwrap();
        }
    };
    (
        mysql,
        async fn $name:ident($case:ident: &APITestCase) $body:block
    ) => {
        #[rstest::rstest]
        #[tokio::test]
        async fn $name() {
            use $crate::containers::{with_mysql, MySqlContainer};
            use $crate::resource::AsyncTeardownGuard;
            use $crate::testcase::APITestCase;

            with_mysql(|db| async move {
                let guard = AsyncTeardownGuard::<APITestCase>::new().await;
                let $case = &*guard;
                $case.set_database_url(db.connection_url()).await;

                // Run test
                $body

                // guard is dropped here, teardown() is automatically called
                Ok(())
            })
            .await
            .unwrap();
        }
    };
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ========================================================================
	// TeardownError Display tests
	// ========================================================================

	#[rstest]
	fn test_teardown_error_transaction_rollback_display() {
		// Arrange
		let error = TeardownError::TransactionRollbackFailed("tx-123 failed".to_string());

		// Act
		let display = format!("{}", error);

		// Assert
		assert_eq!(display, "Failed to rollback transactions: tx-123 failed");
	}

	#[rstest]
	fn test_teardown_error_connection_close_display() {
		// Arrange
		let error = TeardownError::ConnectionCloseFailed("connection refused".to_string());

		// Act
		let display = format!("{}", error);

		// Assert
		assert_eq!(
			display,
			"Failed to close database connection: connection refused"
		);
	}

	#[rstest]
	fn test_teardown_error_client_cleanup_display() {
		// Arrange
		let error = TeardownError::ClientCleanupFailed("timeout".to_string());

		// Act
		let display = format!("{}", error);

		// Assert
		assert_eq!(display, "Failed to cleanup client state: timeout");
	}

	#[rstest]
	fn test_teardown_error_debug() {
		// Arrange
		let error = TeardownError::TransactionRollbackFailed("debug test".to_string());

		// Act
		let debug = format!("{:?}", error);

		// Assert
		assert!(
			debug.contains("debug test"),
			"Debug output should contain the message, got: {}",
			debug
		);
	}

	// ========================================================================
	// APITestCase tests
	// ========================================================================

	#[rstest]
	#[tokio::test]
	async fn test_api_test_case_setup_creates_client() {
		// Arrange & Act
		let test_case = APITestCase::setup().await;

		// Assert
		let client = test_case.client().await;
		// Verify we can access the client (read guard obtained successfully)
		drop(client);
	}

	#[rstest]
	#[tokio::test]
	async fn test_api_test_case_client_read_access() {
		// Arrange
		let test_case = APITestCase::setup().await;

		// Act
		let client = test_case.client().await;

		// Assert
		// Successfully obtained read guard - the client is accessible
		assert!(
			std::mem::size_of_val(&*client) > 0,
			"Client should have non-zero size"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_api_test_case_teardown_completes() {
		// Arrange
		let test_case = APITestCase::setup().await;

		// Act & Assert
		// teardown should complete without panicking
		test_case.teardown().await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_api_test_case_multiple_reads() {
		// Arrange
		let test_case = APITestCase::setup().await;

		// Act
		let client1 = test_case.client().await;
		let client2 = test_case.client().await;

		// Assert
		// Both read guards should be held concurrently without deadlock
		assert!(
			std::mem::size_of_val(&*client1) > 0,
			"First client read should succeed"
		);
		assert!(
			std::mem::size_of_val(&*client2) > 0,
			"Second client read should succeed"
		);
	}

	// ========================================================================
	// TransactionHandle tests (testcontainers feature)
	// ========================================================================

	#[cfg(feature = "testcontainers")]
	mod testcontainers_tests {
		use super::*;
		use rstest::rstest;

		#[rstest]
		fn test_transaction_handle_new() {
			// Arrange & Act
			let handle = TransactionHandle::new();

			// Assert
			assert!(!handle.id().is_empty(), "ID should not be empty");
			assert!(!handle.is_committed(), "New handle should not be committed");
		}

		#[rstest]
		fn test_transaction_handle_mark_committed() {
			// Arrange
			let mut handle = TransactionHandle::new();

			// Act
			handle.mark_committed();

			// Assert
			assert!(handle.is_committed());
		}

		#[rstest]
		fn test_transaction_handle_default() {
			// Arrange & Act
			let handle = TransactionHandle::default();

			// Assert
			assert!(!handle.id().is_empty(), "Default ID should not be empty");
			assert!(
				!handle.is_committed(),
				"Default handle should not be committed"
			);
		}

		#[rstest]
		fn test_transaction_handle_id_is_uuid() {
			// Arrange & Act
			let handle = TransactionHandle::new();

			// Assert
			let id = handle.id();
			// UUID v4 format: 8-4-4-4-12 hex characters
			let parts: Vec<&str> = id.split('-').collect();
			assert_eq!(
				parts.len(),
				5,
				"UUID should have 5 parts separated by hyphens, got: {}",
				id
			);
			assert_eq!(parts[0].len(), 8);
			assert_eq!(parts[1].len(), 4);
			assert_eq!(parts[2].len(), 4);
			assert_eq!(parts[3].len(), 4);
			assert_eq!(parts[4].len(), 12);
		}
	}
}
