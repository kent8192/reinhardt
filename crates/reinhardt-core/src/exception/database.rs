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
	source: Option<Arc<dyn std::error::Error + Send + Sync>>,
}

impl DatabaseError {
	/// Creates a database error with the specified classification and message.
	pub fn new(kind: DatabaseErrorKind, message: impl Into<String>) -> Self {
		Self {
			kind,
			message: message.into(),
			code: None,
			source: None,
		}
	}

	/// Associates a driver- or database-specific error code with this error.
	pub fn with_code(mut self, code: impl Into<String>) -> Self {
		self.code = Some(code.into());
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
}

impl std::fmt::Debug for DatabaseError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("DatabaseError")
			.field("kind", &self.kind)
			.field("message", &self.message)
			.field("code", &self.code)
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
		self.kind == other.kind && self.message == other.message && self.code == other.code
	}
}

impl Eq for DatabaseError {}

#[cfg(test)]
mod tests {
	use std::error::Error as _;
	use std::io;

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
}
