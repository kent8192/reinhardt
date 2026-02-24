use mockall::mock;
use reinhardt_db::backends::{
	Result,
	backend::DatabaseBackend as BackendTrait,
	connection::DatabaseConnection as BackendsConnection,
	types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor},
};
use reinhardt_db::orm::{DatabaseBackend, DatabaseConnection};
use rstest::*;
use std::sync::Arc;

// ============================================================================
// mockall-based Database Backend Mock
// ============================================================================

mock! {
	/// Mock implementation of DatabaseBackend trait using mockall
	///
	/// This mock provides automatic verification of method calls and arguments.
	///
	/// # Usage with rstest Fixtures
	///
	/// For complete examples using rstest fixtures, see the unit tests in this module:
	/// - `test_mock_execute_with_verification()` - Demonstrates strict argument verification
	/// - `test_with_mock_database()` - Shows usage with the `mock_database` fixture
	/// - `test_with_mock_connection()` - Shows usage with the `mock_connection` fixture
	///
	/// # Direct Usage Example
	///
	/// ```rust
	/// use reinhardt_test::fixtures::MockDatabaseBackend;
	/// use reinhardt_db::backends::types::{QueryResult, QueryValue};
	/// use reinhardt_db::backends::backend::DatabaseBackend as BackendTrait;
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut mock = MockDatabaseBackend::new();
	///
	///     // Set expectations with strict argument verification
	///     mock.expect_execute()
	///         .withf(|sql, params| {
	///             sql.contains("INSERT INTO users") && params.len() == 2
	///         })
	///         .times(1)
	///         .returning(|_, _| Ok(QueryResult { rows_affected: 1 }));
	///
	///     // Execute the query (must call to satisfy .times(1) expectation)
	///     let result = mock.execute(
	///         "INSERT INTO users (name, email) VALUES ($1, $2)",
	///         vec![
	///             QueryValue::String("Alice".to_string()),
	///             QueryValue::String("alice@example.com".to_string()),
	///         ],
	///     ).await;
	///
	///     assert!(result.is_ok());
	///     // Mock automatically verifies expectations on drop
	/// }
	/// ```
	pub DatabaseBackend {}

	#[async_trait::async_trait]
	impl BackendTrait for DatabaseBackend {
		fn database_type(&self) -> DatabaseType;
		fn placeholder(&self, index: usize) -> String;
		fn supports_returning(&self) -> bool;
		fn supports_on_conflict(&self) -> bool;

		async fn execute(&self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult>;
		async fn fetch_one(&self, sql: &str, params: Vec<QueryValue>) -> Result<Row>;
		async fn fetch_all(&self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>>;
		async fn fetch_optional(&self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>>;
		async fn begin(&self) -> Result<Box<dyn TransactionExecutor>>;

		fn as_any(&self) -> &dyn std::any::Any;
	}
}

// SAFETY: MockDatabaseBackend is Send-safe because all internal state
// (mockall expectations) is stored in thread-safe containers. The mock
// is designed for single-threaded test usage with tokio's async runtime,
// and expectations are set before any concurrent access occurs.
unsafe impl Send for MockDatabaseBackend {}
// SAFETY: MockDatabaseBackend is Sync-safe because all expectation
// matching in mockall uses internal synchronization. The mock backend
// is accessed through Arc<MockDatabaseBackend> in test fixtures, and
// concurrent read access to expectations is safe.
unsafe impl Sync for MockDatabaseBackend {}

// ============================================================================
// rstest Fixtures
// ============================================================================

/// Fixture providing a mock database backend with default expectations
///
/// This fixture creates a MockDatabaseBackend with basic default behaviors:
/// - PostgreSQL database type
/// - Standard $N placeholder format
/// - All optional features supported (RETURNING, ON CONFLICT)
///
/// # Usage with rstest
///
/// This fixture is designed to be used with rstest's `#[rstest]` attribute.
/// See the unit tests in this module for complete examples:
/// - `test_mock_database_default_expectations()` - Verifies default expectations
/// - `test_mock_execute_with_verification()` - Shows strict argument verification
///
/// Note: Doctests cannot use rstest fixtures directly due to Rust's doctest limitations.
/// For runnable examples, refer to the unit tests in the `#[cfg(test)]` section below.
#[fixture]
pub fn mock_database() -> MockDatabaseBackend {
	let mut mock = MockDatabaseBackend::new();

	// Default expectations
	mock.expect_database_type()
		.return_const(DatabaseType::Postgres);

	mock.expect_placeholder()
		.returning(|idx| format!("${}", idx));

	mock.expect_supports_returning().return_const(true);

	mock.expect_supports_on_conflict().return_const(true);

	// Note: as_any() expectation intentionally not set
	// It will panic if called, which is the desired behavior for tests

	mock
}

/// Fixture providing a complete DatabaseConnection with mock backend
///
/// This fixture creates a fully configured DatabaseConnection with a mock backend
/// that returns empty results by default. Suitable for unit tests that only need
/// to verify connection-level behavior without actual database operations.
///
/// # Usage with rstest
///
/// This fixture is designed to be used with rstest's `#[rstest]` attribute.
/// See the unit test `test_mock_connection_fixture()` for a complete example.
///
/// Note: Doctests cannot use rstest fixtures directly due to Rust's doctest limitations.
/// For runnable examples, refer to the unit tests in the `#[cfg(test)]` section below.
#[fixture]
pub fn mock_connection() -> DatabaseConnection {
	let mut mock = MockDatabaseBackend::new();

	// Basic configuration
	mock.expect_database_type()
		.return_const(DatabaseType::Postgres);

	mock.expect_placeholder()
		.returning(|idx| format!("${}", idx));

	mock.expect_supports_returning().return_const(true);

	mock.expect_supports_on_conflict().return_const(true);

	// Note: as_any() expectation intentionally not set
	// It will panic if called, which is the desired behavior for tests

	// Default query behavior: return empty results
	mock.expect_execute()
		.returning(|_, _| Ok(QueryResult { rows_affected: 0 }));

	mock.expect_fetch_all().returning(|_, _| Ok(Vec::new()));

	mock.expect_fetch_one().returning(|_, _| {
		let mut row = Row::new();
		row.data.insert("count".to_string(), QueryValue::Int(0));
		Ok(row)
	});

	mock.expect_fetch_optional().returning(|_, _| Ok(None));

	let backends_conn = BackendsConnection::new(Arc::new(mock));
	DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_mock_database_default_expectations() {
		let mock = mock_database();

		assert_eq!(mock.database_type(), DatabaseType::Postgres);
		assert_eq!(mock.placeholder(1), "$1");
		assert!(mock.supports_returning());
		assert!(mock.supports_on_conflict());
	}

	#[test]
	fn test_mock_database_custom_expectations() {
		let mut mock = MockDatabaseBackend::new();

		mock.expect_database_type()
			.return_const(DatabaseType::Mysql);

		mock.expect_placeholder().returning(|_| "?".to_string());

		assert_eq!(mock.database_type(), DatabaseType::Mysql);
		assert_eq!(mock.placeholder(1), "?");
	}

	#[tokio::test]
	async fn test_mock_execute_with_verification() {
		let mut mock = MockDatabaseBackend::new();

		// Strict verification: exact SQL and param count
		mock.expect_execute()
			.withf(|sql, params| sql.contains("INSERT INTO users") && params.len() == 2)
			.times(1)
			.returning(|_, _| Ok(QueryResult { rows_affected: 1 }));

		let result = mock
			.execute(
				"INSERT INTO users (name, email) VALUES ($1, $2)",
				vec![
					QueryValue::String("Alice".to_string()),
					QueryValue::String("alice@example.com".to_string()),
				],
			)
			.await;

		assert!(result.is_ok());
		assert_eq!(result.unwrap().rows_affected, 1);

		// Mock automatically verifies that .times(1) expectation was met on drop
	}

	#[rstest]
	fn test_mock_connection_fixture(mock_connection: DatabaseConnection) {
		// Verify connection is usable
		assert!(matches!(
			mock_connection.backend(),
			DatabaseBackend::Postgres
		));
	}
}
