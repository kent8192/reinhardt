//! Error types for database operations

pub use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind};

/// Result type for database operations
pub type Result<T> = reinhardt_core::exception::Result<T>;

const POSTGRES_UNDEFINED_OBJECT: &str = "42704";
const POSTGRES_UNDEFINED_FUNCTION: &str = "42883";

/// Structural pgvector feature associated with a database operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PgvectorOperationKind {
	/// A schema operation that defines a vector column type.
	ColumnType,
	/// A DML operation that evaluates a vector distance operator.
	DistanceOperator,
	/// A schema operation that defines an HNSW or IVFFlat index.
	ApproximateIndex,
	/// A DML operation that binds a native vector value.
	VectorValue,
}

fn map_sqlx_database_error(error: &(dyn sqlx::error::DatabaseError + 'static)) -> DatabaseError {
	use sqlx::error::ErrorKind;

	let code = error.code().map(|code| code.into_owned());
	let kind = match code.as_deref() {
		Some("40001") => DatabaseErrorKind::Serialization,
		Some("42601") => DatabaseErrorKind::Syntax,
		_ => match error.kind() {
			ErrorKind::UniqueViolation => DatabaseErrorKind::UniqueViolation,
			ErrorKind::ForeignKeyViolation => DatabaseErrorKind::ForeignKeyViolation,
			ErrorKind::NotNullViolation => DatabaseErrorKind::NotNullViolation,
			ErrorKind::CheckViolation => DatabaseErrorKind::CheckViolation,
			ErrorKind::Other => DatabaseErrorKind::Query,
			_ => DatabaseErrorKind::Query,
		},
	};
	let database_error = DatabaseError::new(kind, error.message());
	match code {
		Some(code) => database_error.with_code(code),
		None => database_error,
	}
}

#[cfg(any(
	feature = "orm",
	feature = "postgres",
	feature = "sqlite",
	feature = "mysql",
	test
))]
pub(crate) fn map_sqlx_error(error: sqlx::Error) -> DatabaseError {
	match error {
		sqlx::Error::Database(error) => map_sqlx_database_error(error.as_ref()),
		sqlx::Error::PoolTimedOut => {
			DatabaseError::new(DatabaseErrorKind::Timeout, "Pool timed out")
		}
		sqlx::Error::Io(error) => {
			DatabaseError::new(DatabaseErrorKind::Connection, error.to_string())
		}
		sqlx::Error::Tls(error) => {
			DatabaseError::new(DatabaseErrorKind::Connection, error.to_string())
		}
		sqlx::Error::Protocol(message) => {
			DatabaseError::new(DatabaseErrorKind::Connection, message)
		}
		sqlx::Error::PoolClosed => DatabaseError::new(DatabaseErrorKind::Connection, "Pool closed"),
		sqlx::Error::WorkerCrashed => {
			DatabaseError::new(DatabaseErrorKind::Connection, "Worker crashed")
		}
		sqlx::Error::Configuration(error) => {
			DatabaseError::new(DatabaseErrorKind::Configuration, error.to_string())
		}
		sqlx::Error::TypeNotFound { type_name } => DatabaseError::new(
			DatabaseErrorKind::Type,
			format!("Type not found: {type_name}"),
		),
		sqlx::Error::ColumnIndexOutOfBounds { index, len } => DatabaseError::new(
			DatabaseErrorKind::ColumnNotFound,
			format!("Column index {index} out of bounds (len: {len})"),
		),
		sqlx::Error::ColumnNotFound(name) => DatabaseError::new(
			DatabaseErrorKind::ColumnNotFound,
			format!("Column not found: {name}"),
		),
		sqlx::Error::ColumnDecode { index, source } => DatabaseError::new(
			DatabaseErrorKind::Type,
			format!("Failed to decode column {index}: {source}"),
		),
		sqlx::Error::Decode(error) => {
			DatabaseError::new(DatabaseErrorKind::Type, error.to_string())
		}
		sqlx::Error::RowNotFound => DatabaseError::new(DatabaseErrorKind::Query, "Row not found"),
		error @ sqlx::Error::InvalidSavePointStatement => {
			DatabaseError::new(DatabaseErrorKind::Transaction, error.to_string())
		}
		error @ sqlx::Error::BeginFailed => {
			DatabaseError::new(DatabaseErrorKind::Transaction, error.to_string())
		}
		sqlx::Error::Migrate(error) => DatabaseError::new(
			DatabaseErrorKind::Query,
			format!("Migration error: {error}"),
		),
		error => DatabaseError::new(DatabaseErrorKind::Query, error.to_string()),
	}
}

pub(crate) fn map_sqlx_error_with_pgvector_context(
	error: sqlx::Error,
	context: Option<PgvectorOperationKind>,
) -> reinhardt_core::exception::Error {
	let should_add_hint = context.is_some()
		&& matches!(
			&error,
			sqlx::Error::Database(database_error)
				if matches!(
					database_error.code().as_deref(),
					Some(POSTGRES_UNDEFINED_OBJECT | POSTGRES_UNDEFINED_FUNCTION)
				)
		);

	if !should_add_hint {
		return map_sqlx_error(error).into();
	}

	let database_error = match &error {
		sqlx::Error::Database(database_error) => map_sqlx_database_error(database_error.as_ref()),
		_ => unreachable!("pgvector hints require a PostgreSQL database error"),
	};
	let message = format!(
		"{}. Install the pgvector extension explicitly with \
		 CreateExtension::new(\"vector\") before this operation",
		database_error.message()
	);
	let decorated_error = match database_error.code() {
		Some(code) => DatabaseError::new(database_error.kind(), message).with_code(code),
		None => DatabaseError::new(database_error.kind(), message),
	};

	reinhardt_core::exception::Error::DatabaseWithSource {
		database_error: decorated_error,
		source: Box::new(error),
	}
}

#[cfg(test)]
mod tests {
	use std::borrow::Cow;
	use std::error::Error as _;
	use std::fmt;
	use std::io;

	use rstest::rstest;
	use sqlx::error::{DatabaseError as SqlxDatabaseError, ErrorKind};

	use super::{
		DatabaseError, DatabaseErrorKind, PgvectorOperationKind, map_sqlx_error,
		map_sqlx_error_with_pgvector_context,
	};

	const DATABASE_MESSAGE: &str = "database operation failed";
	const DATABASE_CODE: &str = "VENDOR-CODE";
	const CONSTRAINT_NAME: &str = "private_constraint";
	const TABLE_NAME: &str = "private_table";

	#[derive(Debug)]
	struct TestSqlxDatabaseError {
		kind: fn() -> ErrorKind,
		code: &'static str,
		message: &'static str,
	}

	fn unique_violation() -> ErrorKind {
		ErrorKind::UniqueViolation
	}

	fn foreign_key_violation() -> ErrorKind {
		ErrorKind::ForeignKeyViolation
	}

	fn not_null_violation() -> ErrorKind {
		ErrorKind::NotNullViolation
	}

	fn check_violation() -> ErrorKind {
		ErrorKind::CheckViolation
	}

	fn other_database_error() -> ErrorKind {
		ErrorKind::Other
	}

	impl fmt::Display for TestSqlxDatabaseError {
		fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
			formatter.write_str(self.message)
		}
	}

	impl std::error::Error for TestSqlxDatabaseError {}

	impl SqlxDatabaseError for TestSqlxDatabaseError {
		fn message(&self) -> &str {
			self.message
		}

		fn code(&self) -> Option<Cow<'_, str>> {
			Some(Cow::Borrowed(self.code))
		}

		fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
			self
		}

		fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
			self
		}

		fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
			self
		}

		fn constraint(&self) -> Option<&str> {
			Some(CONSTRAINT_NAME)
		}

		fn table(&self) -> Option<&str> {
			Some(TABLE_NAME)
		}

		fn kind(&self) -> ErrorKind {
			(self.kind)()
		}
	}

	#[rstest]
	#[case(unique_violation, DatabaseErrorKind::UniqueViolation)]
	#[case(foreign_key_violation, DatabaseErrorKind::ForeignKeyViolation)]
	#[case(not_null_violation, DatabaseErrorKind::NotNullViolation)]
	#[case(check_violation, DatabaseErrorKind::CheckViolation)]
	#[case(other_database_error, DatabaseErrorKind::Query)]
	fn map_sqlx_error_classifies_database_errors(
		#[case] sqlx_kind: fn() -> ErrorKind,
		#[case] expected_kind: DatabaseErrorKind,
	) {
		// Arrange
		let error = sqlx::Error::Database(Box::new(TestSqlxDatabaseError {
			kind: sqlx_kind,
			code: DATABASE_CODE,
			message: DATABASE_MESSAGE,
		}));

		// Act
		let error = map_sqlx_error(error);

		// Assert
		assert_eq!(
			error,
			DatabaseError::new(expected_kind, DATABASE_MESSAGE).with_code(DATABASE_CODE)
		);
		assert_eq!(error.message(), DATABASE_MESSAGE);
		assert_eq!(error.code(), Some(DATABASE_CODE));
		assert_eq!(error.to_string(), DATABASE_MESSAGE);
	}

	#[test]
	fn map_sqlx_error_classifies_serialization_sqlstate() {
		// Arrange
		let error = sqlx::Error::Database(Box::new(TestSqlxDatabaseError {
			kind: other_database_error,
			code: "40001",
			message: DATABASE_MESSAGE,
		}));

		// Act
		let error = map_sqlx_error(error);

		// Assert
		assert_eq!(error.kind(), DatabaseErrorKind::Serialization);
		assert_eq!(error.code(), Some("40001"));
	}

	#[test]
	fn map_sqlx_error_classifies_syntax_sqlstate() {
		// Arrange
		let error = sqlx::Error::Database(Box::new(TestSqlxDatabaseError {
			kind: other_database_error,
			code: "42601",
			message: DATABASE_MESSAGE,
		}));

		// Act
		let error = map_sqlx_error(error);

		// Assert
		assert_eq!(error.kind(), DatabaseErrorKind::Syntax);
		assert_eq!(error.code(), Some("42601"));
	}

	#[rstest]
	#[case(sqlx::Error::PoolTimedOut, DatabaseErrorKind::Timeout)]
	#[case(sqlx::Error::PoolClosed, DatabaseErrorKind::Connection)]
	#[case(sqlx::Error::WorkerCrashed, DatabaseErrorKind::Connection)]
	#[case(
		sqlx::Error::Protocol("wire failure".to_string()),
		DatabaseErrorKind::Connection
	)]
	#[case(
		sqlx::Error::Configuration(Box::new(io::Error::other("invalid configuration"))),
		DatabaseErrorKind::Configuration
	)]
	#[case(
		sqlx::Error::TypeNotFound {
			type_name: "missing_type".to_string(),
		},
		DatabaseErrorKind::Type
	)]
	#[case(
		sqlx::Error::ColumnIndexOutOfBounds { index: 2, len: 1 },
		DatabaseErrorKind::ColumnNotFound
	)]
	#[case(
		sqlx::Error::ColumnNotFound("missing_column".to_string()),
		DatabaseErrorKind::ColumnNotFound
	)]
	#[case(
		sqlx::Error::Decode(Box::new(io::Error::other("decode failed"))),
		DatabaseErrorKind::Type
	)]
	#[case(sqlx::Error::RowNotFound, DatabaseErrorKind::Query)]
	#[case(sqlx::Error::InvalidSavePointStatement, DatabaseErrorKind::Transaction)]
	#[case(sqlx::Error::BeginFailed, DatabaseErrorKind::Transaction)]
	fn map_sqlx_error_classifies_non_database_errors(
		#[case] sqlx_error: sqlx::Error,
		#[case] expected_kind: DatabaseErrorKind,
	) {
		// Arrange

		// Act
		let error = map_sqlx_error(sqlx_error);

		// Assert
		assert_eq!(error.kind(), expected_kind);
		assert_eq!(error.code(), None);
	}

	fn postgres_database_error(code: &'static str, message: &'static str) -> sqlx::Error {
		sqlx::Error::Database(Box::new(TestSqlxDatabaseError {
			kind: other_database_error,
			code,
			message,
		}))
	}

	#[rstest]
	#[case(
		PgvectorOperationKind::ColumnType,
		"42704",
		"type \"vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"42883",
		"operator does not exist: vector <=> vector"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"42704",
		"access method \"hnsw\" does not exist"
	)]
	fn pgvector_error_hint_preserves_postgres_code_and_source(
		#[case] context: PgvectorOperationKind,
		#[case] code: &'static str,
		#[case] message: &'static str,
	) {
		let error = postgres_database_error(code, message);

		let error = map_sqlx_error_with_pgvector_context(error, Some(context));

		assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Query));
		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some(code)
		);
		assert!(
			error
				.to_string()
				.contains("CreateExtension::new(\"vector\")")
		);
		let sqlx_source = error
			.source()
			.expect("decorated error should retain the SQLx source");
		let sqlx_source = sqlx_source
			.downcast_ref::<sqlx::Error>()
			.expect("decorated source should be the original SQLx error");
		let database_source = sqlx_source
			.as_database_error()
			.expect("SQLx error should retain the database source")
			.as_error();
		assert!(
			database_source
				.downcast_ref::<TestSqlxDatabaseError>()
				.is_some()
		);
	}

	#[test]
	fn pgvector_error_hint_requires_structural_operation_context() {
		let error = postgres_database_error(
			"42704",
			"type \"vector\" does not exist while handling a vector-shaped message",
		);

		let error = map_sqlx_error_with_pgvector_context(error, None);

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some("42704")
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
	}

	#[test]
	fn pgvector_error_hint_ignores_unrelated_postgres_classification() {
		let error = sqlx::Error::Database(Box::new(TestSqlxDatabaseError {
			kind: unique_violation,
			code: "23505",
			message: "duplicate key value violates constraint",
		}));

		let error = map_sqlx_error_with_pgvector_context(
			error,
			Some(PgvectorOperationKind::DistanceOperator),
		);

		assert_eq!(
			error.database_kind(),
			Some(DatabaseErrorKind::UniqueViolation)
		);
		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some("23505")
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
	}
}
