//! Error types for database operations

use std::error::Error as _;

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
	/// Multiple pgvector features used by one database operation.
	Multiple(PgvectorOperationSet),
}

/// Compact set of structural pgvector operation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PgvectorOperationSet(u8);

impl PgvectorOperationSet {
	const COLUMN_TYPE: u8 = 1 << 0;
	const DISTANCE_OPERATOR: u8 = 1 << 1;
	const APPROXIMATE_INDEX: u8 = 1 << 2;
	const VECTOR_VALUE: u8 = 1 << 3;

	const fn from_kind(kind: PgvectorOperationKind) -> Self {
		Self(match kind {
			PgvectorOperationKind::ColumnType => Self::COLUMN_TYPE,
			PgvectorOperationKind::DistanceOperator => Self::DISTANCE_OPERATOR,
			PgvectorOperationKind::ApproximateIndex => Self::APPROXIMATE_INDEX,
			PgvectorOperationKind::VectorValue => Self::VECTOR_VALUE,
			PgvectorOperationKind::Multiple(set) => set.0,
		})
	}

	const fn into_kind(self) -> PgvectorOperationKind {
		match self.0 {
			Self::COLUMN_TYPE => PgvectorOperationKind::ColumnType,
			Self::DISTANCE_OPERATOR => PgvectorOperationKind::DistanceOperator,
			Self::APPROXIMATE_INDEX => PgvectorOperationKind::ApproximateIndex,
			Self::VECTOR_VALUE => PgvectorOperationKind::VectorValue,
			_ => PgvectorOperationKind::Multiple(self),
		}
	}
}

impl PgvectorOperationKind {
	/// Returns the union of two structural pgvector contexts.
	pub const fn union(self, other: Self) -> Self {
		PgvectorOperationSet(
			PgvectorOperationSet::from_kind(self).0 | PgvectorOperationSet::from_kind(other).0,
		)
		.into_kind()
	}

	/// Returns whether this context contains an operation kind.
	pub const fn contains(self, kind: Self) -> bool {
		let current = PgvectorOperationSet::from_kind(self).0;
		let expected = PgvectorOperationSet::from_kind(kind).0;
		current & expected == expected
	}
}

fn strip_ascii_case_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
	let candidate = value.get(..prefix.len())?;
	candidate
		.eq_ignore_ascii_case(prefix)
		.then(|| &value[prefix.len()..])
}

fn strip_ascii_case_suffix<'a>(value: &'a str, suffix: &str) -> Option<&'a str> {
	let start = value.len().checked_sub(suffix.len())?;
	value[start..]
		.eq_ignore_ascii_case(suffix)
		.then(|| &value[..start])
}

fn split_once_ascii_case<'a>(value: &'a str, delimiter: &str) -> Option<(&'a str, &'a str)> {
	value
		.as_bytes()
		.windows(delimiter.len())
		.position(|candidate| candidate.eq_ignore_ascii_case(delimiter.as_bytes()))
		.map(|index| (&value[..index], &value[index + delimiter.len()..]))
}

fn postgres_unquoted_identifier_start(character: char) -> bool {
	matches!(character, 'a'..='z' | '_')
}

fn postgres_unquoted_identifier_continue(character: char) -> bool {
	matches!(character, 'a'..='z' | '0'..='9' | '_')
}

fn postgres_unquoted_identifier_is_canonical(identifier: &str) -> bool {
	!identifier.is_empty() && pg_escape::quote_identifier(identifier).as_ref() == identifier
}

fn parse_postgres_qualified_identifier(identifier: &str) -> Option<Vec<String>> {
	let mut characters = identifier.chars().peekable();
	let mut segments = Vec::new();
	loop {
		let mut segment = String::new();
		if characters.next_if_eq(&'"').is_some() {
			let mut closed = false;
			while let Some(character) = characters.next() {
				if character == '\0' {
					return None;
				}
				if character == '"' {
					if characters.next_if_eq(&'"').is_some() {
						segment.push('"');
					} else {
						closed = true;
						break;
					}
				} else {
					segment.push(character);
				}
			}
			if !closed || segment.is_empty() {
				return None;
			}
		} else {
			let first = characters.next()?;
			if !postgres_unquoted_identifier_start(first) {
				return None;
			}
			segment.push(first);
			while characters
				.peek()
				.is_some_and(|character| postgres_unquoted_identifier_continue(*character))
			{
				segment.push(characters.next().expect("peeked identifier character"));
			}
			if !postgres_unquoted_identifier_is_canonical(&segment) {
				return None;
			}
		}
		segments.push(segment);
		match characters.next() {
			None => break,
			Some('.') if characters.peek().is_some() => {}
			Some(_) => return None,
		}
	}
	Some(segments)
}

#[doc(hidden)]
pub fn database_table_matches_model(model_table: &str, database_table: &str) -> bool {
	if model_table.is_empty() || model_table.contains('\0') {
		return false;
	}
	if !model_table.contains(['.', '"']) {
		return model_table == database_table;
	}

	parse_postgres_qualified_identifier(model_table)
		.and_then(|segments| segments.into_iter().last())
		.is_some_and(|table| table == database_table)
}

fn postgres_display_name_final_segment_is(display_name: &str, expected: &str) -> bool {
	// PostgreSQL's TypeNameToString and NameListToString join parsed name components with dots
	// before the diagnostic adds its outer quotes. A quoted identifier containing a dot is
	// therefore indistinguishable from a qualified name here; keep the recoverable final
	// component constrained to the exact pgvector allowlist supplied by the caller.
	let Some(display_name) = display_name
		.strip_prefix('"')
		.and_then(|name| name.strip_suffix('"'))
	else {
		return false;
	};
	if display_name.contains('\0') {
		return false;
	}
	match display_name.rsplit_once('.') {
		Some((prefix, final_segment)) => !prefix.is_empty() && final_segment == expected,
		None => display_name == expected,
	}
}

fn postgres_operand_type_is_vector(identifier: &str) -> bool {
	parse_postgres_qualified_identifier(identifier).is_some_and(|segments| {
		matches!(segments.as_slice(), [final_segment] if final_segment == "vector")
			|| matches!(segments.as_slice(), [_, final_segment] if final_segment == "vector")
	})
}

fn qualified_operator_is(operator: &str, expected: &str) -> bool {
	if operator == expected {
		return true;
	}
	// NameListToString has already discarded the operator schema's original quoting.
	// Only an identifier that quote_identifier would emit unchanged remains
	// unambiguous enough to use as installation evidence.
	operator
		.strip_suffix(expected)
		.and_then(|prefix| prefix.strip_suffix('.'))
		.is_some_and(postgres_unquoted_identifier_is_canonical)
}

fn split_distance_signature(signature: &str) -> Option<(&str, &str, &str)> {
	let bytes = signature.as_bytes();
	let mut separators = Vec::with_capacity(2);
	let mut index = 0;
	let mut quoted = false;
	while index < bytes.len() {
		match bytes[index] {
			b'"' if quoted && bytes.get(index + 1) == Some(&b'"') => {
				index += 2;
				continue;
			}
			b'"' => quoted = !quoted,
			b' ' if !quoted => separators.push(index),
			_ => {}
		}
		index += 1;
	}
	if quoted || separators.len() != 2 {
		return None;
	}
	let first = separators[0];
	let second = separators[1];
	if first == 0 || second == first + 1 || second + 1 == signature.len() {
		return None;
	}
	Some((
		&signature[..first],
		&signature[first + 1..second],
		&signature[second + 1..],
	))
}

fn missing_vector_type(message: &str) -> bool {
	strip_ascii_case_prefix(message, "type ")
		.and_then(|identifier| strip_ascii_case_suffix(identifier, " does not exist"))
		.is_some_and(|identifier| postgres_display_name_final_segment_is(identifier, "vector"))
}

fn missing_vector_access_method(message: &str) -> bool {
	strip_ascii_case_prefix(message, "access method ")
		.and_then(|identifier| strip_ascii_case_suffix(identifier, " does not exist"))
		.is_some_and(|identifier| matches!(identifier, "\"hnsw\"" | "\"ivfflat\""))
}

fn missing_vector_operator_class(message: &str) -> bool {
	let Some(message) = strip_ascii_case_prefix(message, "operator class ") else {
		return false;
	};
	let Some((operator_class, access_method)) =
		split_once_ascii_case(message, " does not exist for access method ")
	else {
		return false;
	};
	["vector_l2_ops", "vector_ip_ops", "vector_cosine_ops"]
		.iter()
		.any(|expected| postgres_display_name_final_segment_is(operator_class, expected))
		&& matches!(access_method, "\"hnsw\"" | "\"ivfflat\"")
}

fn missing_vector_distance_operator(message: &str) -> bool {
	let Some(signature) = strip_ascii_case_prefix(message, "operator does not exist: ") else {
		return false;
	};
	let Some((left, operator, right)) = split_distance_signature(signature) else {
		return false;
	};
	postgres_operand_type_is_vector(left)
		&& ["<->", "<#>", "<=>"]
			.iter()
			.any(|expected| qualified_operator_is(operator, expected))
		&& postgres_operand_type_is_vector(right)
}

fn pgvector_hint_applies(
	context: Option<PgvectorOperationKind>,
	code: Option<&str>,
	message: &str,
) -> bool {
	let Some(context) = context else {
		return false;
	};
	if message.contains(['\n', '\r']) {
		return false;
	}
	match code {
		Some(POSTGRES_UNDEFINED_OBJECT) => {
			((context.contains(PgvectorOperationKind::ColumnType)
				|| context.contains(PgvectorOperationKind::VectorValue))
				&& missing_vector_type(message))
				|| (context.contains(PgvectorOperationKind::ApproximateIndex)
					&& (missing_vector_access_method(message)
						|| missing_vector_operator_class(message)))
		}
		Some(POSTGRES_UNDEFINED_FUNCTION) => {
			context.contains(PgvectorOperationKind::DistanceOperator)
				&& missing_vector_distance_operator(message)
		}
		_ => false,
	}
}

fn decorate_database_error(database_error: DatabaseError) -> DatabaseError {
	let message = format!(
		"{}. Install the pgvector extension explicitly with \
		 CreateExtension::new(\"vector\") before this operation",
		database_error.message()
	);
	database_error.with_message(message)
}

pub(crate) fn decorate_error_with_pgvector_context(
	error: reinhardt_core::exception::Error,
	context: Option<PgvectorOperationKind>,
) -> reinhardt_core::exception::Error {
	let type_not_found_for_vector_binding = context
		.is_some_and(|context| context.contains(PgvectorOperationKind::VectorValue))
		&& error
			.source()
			.and_then(|source| source.downcast_ref::<sqlx::Error>())
			.is_some_and(
				|source| matches!(source, sqlx::Error::TypeNotFound { type_name } if type_name == "vector"),
			);
	if !type_not_found_for_vector_binding
		&& !pgvector_hint_applies(
			context,
			error.database_error().and_then(DatabaseError::code),
			error.database_error().map_or("", DatabaseError::message),
		) {
		return error;
	}

	match error {
		reinhardt_core::exception::Error::Database(database_error) => {
			reinhardt_core::exception::Error::Database(decorate_database_error(database_error))
		}
		reinhardt_core::exception::Error::DatabaseWithSource {
			database_error,
			source,
		} => reinhardt_core::exception::Error::DatabaseWithSource {
			database_error: decorate_database_error(database_error),
			source,
		},
		error => error,
	}
}

pub(crate) fn into_database_error(error: reinhardt_core::exception::Error) -> DatabaseError {
	match error {
		reinhardt_core::exception::Error::Database(database_error) => database_error,
		reinhardt_core::exception::Error::DatabaseWithSource {
			database_error,
			source,
		} => database_error.with_boxed_source(source),
		error => DatabaseError::new(DatabaseErrorKind::Query, error.to_string()).with_source(error),
	}
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
	let mut database_error = DatabaseError::new(kind, error.message());
	if let Some(code) = code {
		database_error = database_error.with_code(code);
	}
	if let Some(constraint) = error.constraint() {
		database_error = database_error.with_constraint(constraint);
	}
	if let Some(table) = error.table() {
		database_error = database_error.with_table(table);
	}
	if let Some(column) = error
		.try_downcast_ref::<sqlx::postgres::PgDatabaseError>()
		.and_then(sqlx::postgres::PgDatabaseError::column)
	{
		database_error = database_error.with_columns([column]);
	}
	database_error
}

#[cfg(any(
	feature = "orm",
	feature = "postgres",
	feature = "sqlite",
	feature = "mysql",
	test
))]
pub(crate) fn map_sqlx_error(error: sqlx::Error) -> DatabaseError {
	let database_error = map_sqlx_error_ref(&error);
	database_error.with_source(error)
}

fn map_sqlx_error_ref(error: &sqlx::Error) -> DatabaseError {
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

// PostgreSQL uses this mapper in ordinary builds, while backend-only feature
// combinations retain the same internal error boundary without calling it.
#[cfg_attr(not(any(feature = "postgres", test)), allow(dead_code))]
pub(crate) fn map_sqlx_error_with_pgvector_context(
	error: sqlx::Error,
	context: Option<PgvectorOperationKind>,
) -> reinhardt_core::exception::Error {
	let mapped = reinhardt_core::exception::Error::DatabaseWithSource {
		database_error: map_sqlx_error_ref(&error),
		source: Box::new(error),
	};
	decorate_error_with_pgvector_context(mapped, context)
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
		DatabaseError, DatabaseErrorKind, PgvectorOperationKind, database_table_matches_model,
		map_sqlx_error, map_sqlx_error_with_pgvector_context,
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
			DatabaseError::new(expected_kind, DATABASE_MESSAGE)
				.with_code(DATABASE_CODE)
				.with_constraint(CONSTRAINT_NAME)
				.with_table(TABLE_NAME)
		);
		assert_eq!(error.message(), DATABASE_MESSAGE);
		assert_eq!(error.code(), Some(DATABASE_CODE));
		assert_eq!(error.constraint(), Some(CONSTRAINT_NAME));
		assert_eq!(error.table(), Some(TABLE_NAME));
		assert_eq!(error.columns(), []);
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some()
		);
		assert_eq!(error.to_string(), DATABASE_MESSAGE);
	}

	#[test]
	fn database_table_match_distinguishes_qualified_and_quoted_dotted_names() {
		assert!(database_table_matches_model("public.users", "users"));
		assert!(!database_table_matches_model(r#""public.users""#, "users"));
		assert!(database_table_matches_model(
			r#""public.users""#,
			"public.users"
		));
		assert!(!database_table_matches_model("public..users", "users"));
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

	#[test]
	fn pgvector_vector_binding_type_not_found_adds_installation_hint() {
		let error = map_sqlx_error_with_pgvector_context(
			sqlx::Error::TypeNotFound {
				type_name: "vector".to_string(),
			},
			Some(PgvectorOperationKind::VectorValue),
		);

		assert!(
			error
				.to_string()
				.contains("CreateExtension::new(\"vector\")")
		);
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some_and(
					|source| matches!(source, sqlx::Error::TypeNotFound { type_name } if type_name == "vector")
				)
		);
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

	#[rstest]
	#[case(PgvectorOperationKind::ColumnType, "42883")]
	#[case(PgvectorOperationKind::DistanceOperator, "42704")]
	#[case(PgvectorOperationKind::ApproximateIndex, "42883")]
	fn pgvector_error_hint_requires_matching_operation_and_postgres_classification(
		#[case] context: PgvectorOperationKind,
		#[case] code: &'static str,
	) {
		let error = postgres_database_error(code, "unrelated database object is missing");

		let error = map_sqlx_error_with_pgvector_context(error, Some(context));

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some(code)
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
	}

	#[rstest]
	#[case(
		PgvectorOperationKind::ColumnType,
		"42704",
		"type \"geography\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"42704",
		"access method \"brin\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"42704",
		"operator class \"jsonb_path_ops\" does not exist for access method \"gin\""
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"42883",
		"function cosine_distance(vector, vector) does not exist"
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"42883",
		"operator does not exist: integer <=> integer"
	)]
	fn pgvector_error_hint_requires_specific_missing_pgvector_evidence(
		#[case] context: PgvectorOperationKind,
		#[case] code: &'static str,
		#[case] message: &'static str,
	) {
		let error = postgres_database_error(code, message);

		let error = map_sqlx_error_with_pgvector_context(error, Some(context));

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some(code)
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
	}

	#[rstest]
	#[case(
		PgvectorOperationKind::ColumnType,
		"42704",
		"type \"extensions.vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"42883",
		"operator does not exist: extensions.vector extensions.<=> public.vector"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"42704",
		"operator class \"extensions.vector_l2_ops\" does not exist for access method \"hnsw\""
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"42704",
		"access method \"ivfflat\" does not exist"
	)]
	fn pgvector_error_hint_accepts_qualified_canonical_postgres_evidence(
		#[case] context: PgvectorOperationKind,
		#[case] code: &'static str,
		#[case] message: &'static str,
	) {
		let error = postgres_database_error(code, message);

		let error = map_sqlx_error_with_pgvector_context(error, Some(context));

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some(code)
		);
		assert!(
			error
				.to_string()
				.contains("CreateExtension::new(\"vector\")")
		);
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some()
		);
	}

	#[rstest]
	#[case("operator does not exist: \"Extensions\".vector <=> \"Extensions\".vector")]
	#[case("operator does not exist: \"my schema\".vector <=> \"my schema\".vector")]
	#[case("operator does not exist: \"my-schema\".vector <=> \"my-schema\".vector")]
	#[case("operator does not exist: \"schema.with.dot\".vector <=> \"schema.with.dot\".vector")]
	#[case("operator does not exist: \"schema\"\"quote\".vector <=> \"schema\"\"quote\".vector")]
	#[case("operator does not exist: \"拡張\".vector <=> \"拡張\".vector")]
	fn pgvector_error_hint_accepts_postgres_sql_qualified_operand_types(
		#[case] message: &'static str,
	) {
		let error = postgres_database_error("42883", message);

		let error = map_sqlx_error_with_pgvector_context(
			error,
			Some(PgvectorOperationKind::DistanceOperator),
		);

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some("42883")
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(
			error
				.to_string()
				.contains("CreateExtension::new(\"vector\")")
		);
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some()
		);
	}

	#[rstest]
	#[case("operator does not exist: vector my schema.<=> vector")]
	#[case("operator does not exist: vector my-schema.<=> vector")]
	#[case("operator does not exist: vector schema.with.dot.<=> vector")]
	#[case("operator does not exist: vector schema\"quote.<=> vector")]
	#[case("operator does not exist: vector 名前.<=> vector")]
	#[case("operator does not exist: vector select.<=> vector")]
	#[case("operator does not exist: vector Extensions.<=> vector")]
	#[case("operator does not exist: vector \"my schema\".<=> vector")]
	#[case("operator does not exist: vector \"schema.with.dot\".<=> vector")]
	#[case("operator does not exist: vector \"schema\"\"quote\".<=> vector")]
	fn pgvector_error_hint_rejects_ambiguous_postgres_operator_diagnostics(
		#[case] message: &'static str,
	) {
		let error = postgres_database_error("42883", message);

		let error = map_sqlx_error_with_pgvector_context(
			error,
			Some(PgvectorOperationKind::DistanceOperator),
		);

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some("42883")
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some(),
			"ambiguous operator evidence must retain its original SQLx source"
		);
	}

	#[rstest]
	#[case("operator does not exist: Extensions.vector <=> Extensions.vector")]
	#[case("operator does not exist: 拡張.vector <=> 拡張.vector")]
	#[case("operator does not exist: select.vector <=> select.vector")]
	#[case("operator does not exist: my$schema.vector <=> my$schema.vector")]
	fn pgvector_error_hint_rejects_noncanonical_unquoted_operand_types(
		#[case] message: &'static str,
	) {
		let error = postgres_database_error("42883", message);

		let error = map_sqlx_error_with_pgvector_context(
			error,
			Some(PgvectorOperationKind::DistanceOperator),
		);

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some("42883")
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some(),
			"noncanonical operand evidence must retain its original SQLx source"
		);
	}

	#[rstest]
	#[case(
		PgvectorOperationKind::ColumnType,
		"type \"名前.vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ColumnType,
		"type \"my schema.vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ColumnType,
		"type \"my-schema.vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ColumnType,
		"type \"schema.with.dot.vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ColumnType,
		"type \"schema\"quote.vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"operator class \"名前.vector_cosine_ops\" does not exist for access method \"hnsw\""
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"operator class \"schema.with.dot.vector_ip_ops\" does not exist for access method \"ivfflat\""
	)]
	fn pgvector_error_hint_accepts_postgres_outer_quoted_display_names(
		#[case] context: PgvectorOperationKind,
		#[case] message: &'static str,
	) {
		let error = postgres_database_error("42704", message);

		let error = map_sqlx_error_with_pgvector_context(error, Some(context));

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some("42704")
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(
			error
				.to_string()
				.contains("CreateExtension::new(\"vector\")")
		);
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some()
		);
	}

	#[rstest]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"operator class \"extensions\".\"vector_custom_ops\" does not exist for access method \"hnsw\""
	)]
	#[case(
		PgvectorOperationKind::ColumnType,
		"type \"vector\" does not exist while parsing documentation"
	)]
	#[case(
		PgvectorOperationKind::ColumnType,
		"type \"extensions\".\"vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"operator class \"extensions\".\"vector_l2_ops\" does not exist for access method \"hnsw\""
	)]
	#[case(
		PgvectorOperationKind::ColumnType,
		"type \"extensions.not_vector\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"operator class \"extensions.vector_custom_ops\" does not exist for access method \"hnsw\""
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"access method \"extensions.hnsw\" does not exist"
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"operator does not exist: extensions.vector extensions.<+> public.vector"
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"operator does not exist: extensions.vector public.extensions.<=> public.vector"
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"operator does not exist: vector <=> vector in extension documentation"
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"operator does not exist: \"extensions.vector\" <=> \"extensions.vector\""
	)]
	fn pgvector_error_hint_rejects_noncanonical_or_unapproved_identifiers(
		#[case] context: PgvectorOperationKind,
		#[case] message: &'static str,
	) {
		let code = if context.contains(PgvectorOperationKind::DistanceOperator) {
			"42883"
		} else {
			"42704"
		};
		let error = postgres_database_error(code, message);

		let error = map_sqlx_error_with_pgvector_context(error, Some(context));

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some(code)
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some(),
			"an undecorated contextual mapping must retain its original SQLx source"
		);
	}

	#[rstest]
	#[case("operator does not exist: vector\t<=> vector")]
	#[case("operator does not exist: vector  <=> vector")]
	#[case("operator does not exist:  vector <=> vector")]
	#[case("operator does not exist: vector <=> vector ")]
	fn pgvector_error_hint_rejects_noncanonical_distance_signature_spacing(
		#[case] message: &'static str,
	) {
		let error = postgres_database_error("42883", message);

		let error = map_sqlx_error_with_pgvector_context(
			error,
			Some(PgvectorOperationKind::DistanceOperator),
		);

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some("42883")
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some(),
			"rejected spacing evidence must retain its original SQLx source"
		);
	}

	#[rstest]
	#[case(
		PgvectorOperationKind::ColumnType,
		"42704",
		"type \"vector\" does not exist\nDETAIL: extension is not installed"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"42704",
		"access method \"hnsw\" does not exist\nHINT: install an extension"
	)]
	#[case(
		PgvectorOperationKind::ApproximateIndex,
		"42704",
		"operator class \"vector_l2_ops\" does not exist for access method \"hnsw\"\nDETAIL: custom failure"
	)]
	#[case(
		PgvectorOperationKind::DistanceOperator,
		"42883",
		"operator does not exist: vector <=> vector\nHINT: no operator matches"
	)]
	#[case(
		PgvectorOperationKind::ColumnType,
		"42704",
		"type \"vector\" does not exist\rDETAIL: extension is not installed"
	)]
	fn pgvector_error_hint_rejects_multiline_or_carriage_return_evidence(
		#[case] context: PgvectorOperationKind,
		#[case] code: &'static str,
		#[case] message: &'static str,
	) {
		let error = postgres_database_error(code, message);

		let error = map_sqlx_error_with_pgvector_context(error, Some(context));

		assert_eq!(
			error.database_error().and_then(DatabaseError::code),
			Some(code)
		);
		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Query)
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
		assert!(
			error
				.source()
				.and_then(|source| source.downcast_ref::<sqlx::Error>())
				.is_some(),
			"rejected multiline evidence must retain its original SQLx source"
		);
	}
}
