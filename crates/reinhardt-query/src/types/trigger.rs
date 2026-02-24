//! Trigger-related types for DDL operations

/// Trigger event type
///
/// Specifies which DML operation triggers the trigger execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerEvent {
	/// INSERT operation
	Insert,
	/// UPDATE operation (optionally on specific columns)
	Update { columns: Option<Vec<String>> },
	/// DELETE operation
	Delete,
}

/// Trigger timing
///
/// Specifies when the trigger executes relative to the triggering event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerTiming {
	/// Execute before the triggering event
	Before,
	/// Execute after the triggering event
	After,
	/// Execute instead of the triggering event (SQLite views only)
	InsteadOf,
}

/// Trigger scope
///
/// Specifies whether the trigger executes for each row or once per statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerScope {
	/// Execute for each affected row (FOR EACH ROW)
	Row,
	/// Execute once per statement (FOR EACH STATEMENT) - PostgreSQL only
	Statement,
}

/// Trigger action timing for MySQL
///
/// MySQL-specific feature to control trigger execution order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerOrder {
	/// Execute before another trigger
	Precedes(String),
	/// Execute after another trigger
	Follows(String),
}

/// Trigger body representation
///
/// Contains the SQL statements to execute when the trigger fires.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerBody {
	/// Single SQL statement
	Single(String),
	/// Multiple SQL statements (MySQL/SQLite BEGIN...END block)
	Multiple(Vec<String>),
	/// PostgreSQL function call (EXECUTE FUNCTION function_name())
	PostgresFunction(String),
}

impl TriggerBody {
	/// Create a single-statement trigger body
	pub fn single<S: Into<String>>(statement: S) -> Self {
		Self::Single(statement.into())
	}

	/// Create a multi-statement trigger body
	pub fn multiple<I, S>(statements: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<String>,
	{
		Self::Multiple(statements.into_iter().map(|s| s.into()).collect())
	}

	/// Create a PostgreSQL function call trigger body
	pub fn postgres_function<S: Into<String>>(function_name: S) -> Self {
		Self::PostgresFunction(function_name.into())
	}
}
