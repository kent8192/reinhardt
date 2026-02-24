//! PostgreSQL-specific extension and advanced feature helpers
//!
//! This module provides helper functions for PostgreSQL-specific features that are not
//! supported by reinhardt-query or other database backends.

/// PostgreSQL extension management helpers
pub mod extension {
	/// Generate CREATE EXTENSION statement
	///
	/// # Arguments
	///
	/// * `extension_name` - Name of the extension (e.g., "postgis", "uuid-ossp")
	/// * `if_not_exists` - Whether to add IF NOT EXISTS clause
	/// * `schema` - Optional schema to install extension in
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::extension;
	///
	/// let sql = extension::create_extension("postgis", true, None);
	/// assert_eq!(sql, "CREATE EXTENSION IF NOT EXISTS \"postgis\"");
	///
	/// let sql_with_schema = extension::create_extension("postgis", true, Some("public"));
	/// assert_eq!(sql_with_schema, "CREATE EXTENSION IF NOT EXISTS \"postgis\" SCHEMA \"public\"");
	/// ```
	pub fn create_extension(
		extension_name: &str,
		if_not_exists: bool,
		schema: Option<&str>,
	) -> String {
		let if_not_exists_clause = if if_not_exists { " IF NOT EXISTS" } else { "" };

		let schema_clause = if let Some(s) = schema {
			format!(" SCHEMA \"{}\"", s)
		} else {
			String::new()
		};

		format!(
			"CREATE EXTENSION{} \"{}\"{}",
			if_not_exists_clause, extension_name, schema_clause
		)
	}

	/// Generate DROP EXTENSION statement
	///
	/// # Arguments
	///
	/// * `extension_name` - Name of the extension
	/// * `if_exists` - Whether to add IF EXISTS clause
	/// * `cascade` - Whether to add CASCADE clause
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::extension;
	///
	/// let sql = extension::drop_extension("postgis", true, true);
	/// assert_eq!(sql, "DROP EXTENSION IF EXISTS \"postgis\" CASCADE");
	/// ```
	pub fn drop_extension(extension_name: &str, if_exists: bool, cascade: bool) -> String {
		let if_exists_clause = if if_exists { " IF EXISTS" } else { "" };
		let cascade_clause = if cascade { " CASCADE" } else { "" };

		format!(
			"DROP EXTENSION{} \"{}\"{}",
			if_exists_clause, extension_name, cascade_clause
		)
	}
}

/// PostgreSQL advanced index helpers (GIN, GiST, etc.)
pub mod index {
	/// Generate CREATE INDEX USING GIN statement
	///
	/// # Arguments
	///
	/// * `index_name` - Name of the index
	/// * `table` - Table name
	/// * `expression` - Index expression (e.g., "to_tsvector('english', content)")
	/// * `where_clause` - Optional WHERE clause for partial index
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::index;
	///
	/// let sql = index::create_gin_index("idx_content_search", "articles", "to_tsvector('english', content)", None);
	/// assert!(sql.contains("USING GIN"));
	/// ```
	pub fn create_gin_index(
		index_name: &str,
		table: &str,
		expression: &str,
		where_clause: Option<&str>,
	) -> String {
		let where_part = if let Some(w) = where_clause {
			format!(" WHERE {}", w)
		} else {
			String::new()
		};

		format!(
			"CREATE INDEX \"{}\" ON \"{}\" USING GIN ({}){}",
			index_name, table, expression, where_part
		)
	}

	/// Generate CREATE INDEX USING GiST statement
	///
	/// # Arguments
	///
	/// * `index_name` - Name of the index
	/// * `table` - Table name
	/// * `expression` - Index expression (e.g., "location" for geometry column)
	/// * `where_clause` - Optional WHERE clause for partial index
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::index;
	///
	/// let sql = index::create_gist_index("idx_location_spatial", "locations", "point", None);
	/// assert!(sql.contains("USING GIST"));
	/// ```
	pub fn create_gist_index(
		index_name: &str,
		table: &str,
		expression: &str,
		where_clause: Option<&str>,
	) -> String {
		let where_part = if let Some(w) = where_clause {
			format!(" WHERE {}", w)
		} else {
			String::new()
		};

		format!(
			"CREATE INDEX \"{}\" ON \"{}\" USING GIST ({}){}",
			index_name, table, expression, where_part
		)
	}

	/// Generate CREATE INDEX USING BRIN statement (Block Range Index)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::index;
	///
	/// let sql = index::create_brin_index("idx_created_at_brin", "events", "created_at", None);
	/// assert!(sql.contains("USING BRIN"));
	/// ```
	pub fn create_brin_index(
		index_name: &str,
		table: &str,
		expression: &str,
		where_clause: Option<&str>,
	) -> String {
		let where_part = if let Some(w) = where_clause {
			format!(" WHERE {}", w)
		} else {
			String::new()
		};

		format!(
			"CREATE INDEX \"{}\" ON \"{}\" USING BRIN ({}){}",
			index_name, table, expression, where_part
		)
	}
}

/// PostgreSQL ENUM type helpers
pub mod enum_type {
	/// Generate CREATE TYPE ... AS ENUM statement
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::enum_type;
	///
	/// let sql = enum_type::create_enum("user_status", &["active", "inactive", "pending"]);
	/// assert!(sql.contains("CREATE TYPE"));
	/// assert!(sql.contains("AS ENUM"));
	/// ```
	pub fn create_enum(type_name: &str, values: &[&str]) -> String {
		let values_str = values
			.iter()
			.map(|v| format!("'{}'", v))
			.collect::<Vec<_>>()
			.join(", ");

		format!("CREATE TYPE \"{}\" AS ENUM ({})", type_name, values_str)
	}

	/// Generate DROP TYPE statement
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::enum_type;
	///
	/// let sql = enum_type::drop_enum("user_status", true, true);
	/// assert_eq!(sql, "DROP TYPE IF EXISTS \"user_status\" CASCADE");
	/// ```
	pub fn drop_enum(type_name: &str, if_exists: bool, cascade: bool) -> String {
		let if_exists_clause = if if_exists { " IF EXISTS" } else { "" };
		let cascade_clause = if cascade { " CASCADE" } else { "" };

		format!(
			"DROP TYPE{} \"{}\"{}",
			if_exists_clause, type_name, cascade_clause
		)
	}

	/// Generate ALTER TYPE ... ADD VALUE statement
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::enum_type;
	///
	/// let sql = enum_type::add_enum_value("user_status", "suspended", None);
	/// assert_eq!(sql, "ALTER TYPE \"user_status\" ADD VALUE 'suspended'");
	///
	/// let sql_after = enum_type::add_enum_value("user_status", "suspended", Some("pending"));
	/// assert_eq!(sql_after, "ALTER TYPE \"user_status\" ADD VALUE 'suspended' AFTER 'pending'");
	/// ```
	pub fn add_enum_value(type_name: &str, new_value: &str, after: Option<&str>) -> String {
		let after_clause = if let Some(a) = after {
			format!(" AFTER '{}'", a)
		} else {
			String::new()
		};

		format!(
			"ALTER TYPE \"{}\" ADD VALUE '{}'{}",
			type_name, new_value, after_clause
		)
	}
}

/// PostgreSQL sequence helpers
pub mod sequence {
	/// Generate setval() function call for sequence
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::sequence;
	///
	/// let sql = sequence::set_sequence_value("users_id_seq", "SELECT MAX(id) FROM users");
	/// assert!(sql.contains("SELECT setval"));
	/// ```
	pub fn set_sequence_value(sequence_name: &str, value_expression: &str) -> String {
		format!("SELECT setval('{}', ({}))", sequence_name, value_expression)
	}

	/// Generate nextval() function call for sequence
	pub fn next_sequence_value(sequence_name: &str) -> String {
		format!("SELECT nextval('{}')", sequence_name)
	}

	/// Generate currval() function call for sequence
	pub fn current_sequence_value(sequence_name: &str) -> String {
		format!("SELECT currval('{}')", sequence_name)
	}
}

/// PostgreSQL row locking helpers
pub mod locking {
	/// Row lock mode
	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub enum LockMode {
		/// FOR UPDATE - strongest lock, prevents other transactions from reading
		ForUpdate,
		/// FOR NO KEY UPDATE - allows concurrent reads
		ForNoKeyUpdate,
		/// FOR SHARE - allows concurrent reads but prevents updates
		ForShare,
		/// FOR KEY SHARE - weakest lock, only prevents key updates
		ForKeyShare,
	}

	impl LockMode {
		/// Get the SQL clause for this lock mode
		pub fn as_sql(&self) -> &'static str {
			match self {
				LockMode::ForUpdate => "FOR UPDATE",
				LockMode::ForNoKeyUpdate => "FOR NO KEY UPDATE",
				LockMode::ForShare => "FOR SHARE",
				LockMode::ForKeyShare => "FOR KEY SHARE",
			}
		}
	}

	/// Append row lock clause to SELECT statement
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::backends::drivers::postgresql::extensions::locking::{append_row_lock, LockMode};
	///
	/// let sql = "SELECT * FROM users WHERE id = 1";
	/// let locked_sql = append_row_lock(sql, LockMode::ForUpdate, false);
	/// assert_eq!(locked_sql, "SELECT * FROM users WHERE id = 1 FOR UPDATE");
	///
	/// let nowait_sql = append_row_lock(sql, LockMode::ForUpdate, true);
	/// assert_eq!(nowait_sql, "SELECT * FROM users WHERE id = 1 FOR UPDATE NOWAIT");
	/// ```
	pub fn append_row_lock(sql: &str, lock_mode: LockMode, nowait: bool) -> String {
		let nowait_clause = if nowait { " NOWAIT" } else { "" };

		format!("{} {}{}", sql, lock_mode.as_sql(), nowait_clause)
	}
}
