use std::sync::Arc;

/// Driver-independent classification for database failures.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseErrorKind {
	/// A database connection could not be established or was lost.
	Connection,
	/// The injected database handle outlived its owning DI scope.
	ConnectionHandleExpired,
	/// A database operation or connection-pool acquisition timed out.
	Timeout,
	/// A unique constraint was violated.
	UniqueViolation,
	/// A foreign key constraint was violated.
	ForeignKeyViolation,
	/// A non-null constraint was violated.
	NotNullViolation,
	/// A check constraint was violated.
	CheckViolation,
	/// A query contained invalid database syntax.
	Syntax,
	/// A value or expression had an incompatible database type.
	Type,
	/// A referenced database column was not found.
	ColumnNotFound,
	/// A database transaction failed.
	Transaction,
	/// Database configuration was invalid or incomplete.
	Configuration,
	/// Database serialization or deserialization failed.
	Serialization,
	/// The requested database operation is not supported.
	Unsupported,
	/// A database query failed for a reason not covered by a more specific kind.
	Query,
}

/// Structured database failure retained by the framework error boundary.
#[derive(Clone)]
pub struct DatabaseError {
	kind: DatabaseErrorKind,
	message: String,
	code: Option<String>,
	metadata: Option<Box<DatabaseErrorMetadata>>,
	source: Option<Arc<dyn std::error::Error + Send + Sync>>,
}

#[derive(Clone, Default)]
struct DatabaseErrorMetadata {
	constraint: Option<String>,
	table: Option<String>,
	columns: Vec<String>,
}

impl DatabaseError {
	/// Creates a database error with the specified classification and message.
	pub fn new(kind: DatabaseErrorKind, message: impl Into<String>) -> Self {
		Self {
			kind,
			message: message.into(),
			code: None,
			metadata: None,
			source: None,
		}
	}

	/// Associates a driver- or database-specific error code with this error.
	pub fn with_code(mut self, code: impl Into<String>) -> Self {
		self.code = Some(code.into());
		self
	}

	/// Associates the physical constraint name reported by the database.
	pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
		self.metadata_mut().constraint = Some(constraint.into());
		self
	}

	/// Associates the physical table name reported by the database.
	pub fn with_table(mut self, table: impl Into<String>) -> Self {
		self.metadata_mut().table = Some(table.into());
		self
	}

	/// Replaces the ordered physical columns reported by the database.
	pub fn with_columns<I, S>(mut self, columns: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<String>,
	{
		self.metadata_mut().columns = columns.into_iter().map(Into::into).collect();
		self
	}

	/// Replaces the diagnostic message while retaining classification metadata.
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = message.into();
		self
	}

	/// Retains the typed error that caused this database failure.
	pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
		self.source = Some(Arc::new(source));
		self
	}

	/// Retains an already boxed typed database error source.
	pub fn with_boxed_source(mut self, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
		self.source = Some(Arc::from(source));
		self
	}

	/// Returns the driver-independent classification of this error.
	pub fn kind(&self) -> DatabaseErrorKind {
		self.kind
	}

	/// Returns the diagnostic message retained for this error.
	pub fn message(&self) -> &str {
		&self.message
	}

	/// Returns the driver- or database-specific error code, if available.
	pub fn code(&self) -> Option<&str> {
		self.code.as_deref()
	}

	/// Returns the physical constraint name, when supplied by the backend.
	pub fn constraint(&self) -> Option<&str> {
		self.metadata
			.as_deref()
			.and_then(|metadata| metadata.constraint.as_deref())
	}

	/// Returns the physical table name, when supplied by the backend.
	pub fn table(&self) -> Option<&str> {
		self.metadata
			.as_deref()
			.and_then(|metadata| metadata.table.as_deref())
	}

	/// Returns the ordered physical columns supplied by the backend.
	pub fn columns(&self) -> &[String] {
		self.metadata
			.as_deref()
			.map_or(&[], |metadata| metadata.columns.as_slice())
	}

	fn metadata_mut(&mut self) -> &mut DatabaseErrorMetadata {
		self.metadata
			.get_or_insert_with(|| Box::new(DatabaseErrorMetadata::default()))
	}
}

impl std::fmt::Debug for DatabaseError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("DatabaseError")
			.field("kind", &self.kind)
			.field("message", &self.message)
			.field("code", &self.code)
			.field("constraint", &self.constraint())
			.field("table", &self.table())
			.field("columns", &self.columns())
			.field("source", &self.source)
			.finish()
	}
}

impl std::fmt::Display for DatabaseError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str(&self.message)
	}
}

impl std::error::Error for DatabaseError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		self.source
			.as_deref()
			.map(|source| source as &(dyn std::error::Error + 'static))
	}
}

impl PartialEq for DatabaseError {
	fn eq(&self, other: &Self) -> bool {
		self.kind == other.kind
			&& self.message == other.message
			&& self.code == other.code
			&& self.constraint() == other.constraint()
			&& self.table() == other.table()
			&& self.columns() == other.columns()
	}
}

impl Eq for DatabaseError {}

#[cfg(test)]
mod tests {
	use std::error::Error as _;
	use std::io;
	use std::mem::size_of;

	use super::{DatabaseError, DatabaseErrorKind};

	#[test]
	fn connection_handle_expired_has_stable_display_and_server_status() {
		let error = DatabaseError::new(
			DatabaseErrorKind::ConnectionHandleExpired,
			"The injected database connection is no longer available because its DI scope has ended",
		);

		assert_eq!(
			error.to_string(),
			"The injected database connection is no longer available because its DI scope has ended"
		);
		assert_eq!(crate::exception::Error::from(error).status_code(), 500);
	}

	#[test]
	fn cloned_database_error_preserves_typed_source() {
		let error = DatabaseError::new(DatabaseErrorKind::Query, "query failed")
			.with_source(io::Error::other("driver failure"));

		let cloned = error.clone();

		assert_eq!(cloned.kind(), DatabaseErrorKind::Query);
		assert!(
			cloned
				.source()
				.and_then(|source| source.downcast_ref::<io::Error>())
				.is_some()
		);
	}

	#[test]
	fn database_error_retains_structured_object_metadata() {
		let error = DatabaseError::new(DatabaseErrorKind::UniqueViolation, "duplicate")
			.with_constraint("users_email_key")
			.with_table("users")
			.with_columns(["email", "tenant_id"]);

		assert_eq!(error.constraint(), Some("users_email_key"));
		assert_eq!(error.table(), Some("users"));
		assert_eq!(error.columns(), ["email", "tenant_id"]);
		assert!(format!("{error:?}").contains("users_email_key"));
	}

	#[test]
	fn database_error_remains_small_enough_for_result_values() {
		assert!(size_of::<DatabaseError>() < 128);
	}

	#[test]
	fn cloned_database_error_retains_metadata_and_source() {
		let error = DatabaseError::new(DatabaseErrorKind::NotNullViolation, "missing")
			.with_table("users")
			.with_columns(["email"])
			.with_source(io::Error::other("driver failure"));

		let cloned = error.clone();

		assert_eq!(cloned.table(), Some("users"));
		assert_eq!(cloned.columns(), ["email"]);
		assert!(
			cloned
				.source()
				.and_then(|source| source.downcast_ref::<io::Error>())
				.is_some()
		);
	}

	#[test]
	fn database_error_equality_includes_metadata_but_not_source_identity() {
		let left = DatabaseError::new(DatabaseErrorKind::UniqueViolation, "duplicate")
			.with_constraint("users_email_key")
			.with_columns(["email"])
			.with_source(io::Error::other("left"));
		let same = DatabaseError::new(DatabaseErrorKind::UniqueViolation, "duplicate")
			.with_constraint("users_email_key")
			.with_columns(["email"])
			.with_source(io::Error::other("right"));
		let different = DatabaseError::new(DatabaseErrorKind::UniqueViolation, "duplicate")
			.with_constraint("users_username_key")
			.with_columns(["username"]);

		assert_eq!(left, same);
		assert_ne!(left, different);
	}
}
