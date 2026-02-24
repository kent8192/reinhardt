//! Column reference types for SQL queries.
//!
//! This module provides types for referencing columns:
//!
//! - [`ColumnRef`]: Reference to a column (simple, qualified, asterisk)
//! - [`IntoColumnRef`]: Conversion trait for column references

use super::iden::{DynIden, IntoIden};

/// Reference to a column in a SQL query.
///
/// This enum represents different ways to reference a column,
/// from simple column names to fully qualified references.
#[derive(Debug, Clone)]
pub enum ColumnRef {
	/// Simple column reference (e.g., `name`)
	Column(DynIden),
	/// Table-qualified column reference (e.g., `users.name`)
	TableColumn(DynIden, DynIden),
	/// Schema and table-qualified column reference (e.g., `public.users.name`)
	SchemaTableColumn(DynIden, DynIden, DynIden),
	/// Asterisk for selecting all columns (e.g., `*`)
	Asterisk,
	/// Table-qualified asterisk (e.g., `users.*`)
	TableAsterisk(DynIden),
}

impl ColumnRef {
	/// Create a simple column reference.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::ColumnRef;
	///
	/// let col = ColumnRef::column("name");
	/// ```
	pub fn column<I: IntoIden>(column: I) -> Self {
		Self::Column(column.into_iden())
	}

	/// Create a table-qualified column reference.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::ColumnRef;
	///
	/// let col = ColumnRef::table_column("users", "name");
	/// ```
	pub fn table_column<T: IntoIden, C: IntoIden>(table: T, column: C) -> Self {
		Self::TableColumn(table.into_iden(), column.into_iden())
	}

	/// Create a schema and table-qualified column reference.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::ColumnRef;
	///
	/// let col = ColumnRef::schema_table_column("public", "users", "name");
	/// ```
	pub fn schema_table_column<S: IntoIden, T: IntoIden, C: IntoIden>(
		schema: S,
		table: T,
		column: C,
	) -> Self {
		Self::SchemaTableColumn(schema.into_iden(), table.into_iden(), column.into_iden())
	}

	/// Create an asterisk reference for all columns.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::ColumnRef;
	///
	/// let col = ColumnRef::asterisk();
	/// ```
	pub fn asterisk() -> Self {
		Self::Asterisk
	}

	/// Create a table-qualified asterisk reference.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::ColumnRef;
	///
	/// let col = ColumnRef::table_asterisk("users");
	/// ```
	pub fn table_asterisk<T: IntoIden>(table: T) -> Self {
		Self::TableAsterisk(table.into_iden())
	}
}

/// Conversion trait for column references.
///
/// This trait allows various types to be converted into `ColumnRef`.
pub trait IntoColumnRef {
	/// Convert this type into a `ColumnRef`.
	fn into_column_ref(self) -> ColumnRef;
}

// Implementation for ColumnRef itself
impl IntoColumnRef for ColumnRef {
	fn into_column_ref(self) -> ColumnRef {
		self
	}
}

// Blanket implementation for all types that can be converted to an identifier.
// This covers DynIden, &'static str, String, Alias, and any #[derive(Iden)] enum.
impl<T: IntoIden> IntoColumnRef for T {
	fn into_column_ref(self) -> ColumnRef {
		ColumnRef::Column(self.into_iden())
	}
}

// Implementation for tuple (table, column)
impl<T, C> IntoColumnRef for (T, C)
where
	T: IntoIden,
	C: IntoIden,
{
	fn into_column_ref(self) -> ColumnRef {
		ColumnRef::TableColumn(self.0.into_iden(), self.1.into_iden())
	}
}

// Implementation for triple (schema, table, column)
impl<S, T, C> IntoColumnRef for (S, T, C)
where
	S: IntoIden,
	T: IntoIden,
	C: IntoIden,
{
	fn into_column_ref(self) -> ColumnRef {
		ColumnRef::SchemaTableColumn(self.0.into_iden(), self.1.into_iden(), self.2.into_iden())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::Alias;
	use rstest::rstest;

	#[rstest]
	fn test_column_ref_simple() {
		let col = ColumnRef::column("name");
		if let ColumnRef::Column(iden) = col {
			assert_eq!(iden.to_string(), "name");
		} else {
			panic!("Expected Column variant");
		}
	}

	#[rstest]
	fn test_column_ref_table_qualified() {
		let col = ColumnRef::table_column("users", "name");
		if let ColumnRef::TableColumn(table, column) = col {
			assert_eq!(table.to_string(), "users");
			assert_eq!(column.to_string(), "name");
		} else {
			panic!("Expected TableColumn variant");
		}
	}

	#[rstest]
	fn test_column_ref_schema_qualified() {
		let col = ColumnRef::schema_table_column("public", "users", "name");
		if let ColumnRef::SchemaTableColumn(schema, table, column) = col {
			assert_eq!(schema.to_string(), "public");
			assert_eq!(table.to_string(), "users");
			assert_eq!(column.to_string(), "name");
		} else {
			panic!("Expected SchemaTableColumn variant");
		}
	}

	#[rstest]
	fn test_column_ref_asterisk() {
		let col = ColumnRef::asterisk();
		assert!(matches!(col, ColumnRef::Asterisk));
	}

	#[rstest]
	fn test_column_ref_table_asterisk() {
		let col = ColumnRef::table_asterisk("users");
		if let ColumnRef::TableAsterisk(table) = col {
			assert_eq!(table.to_string(), "users");
		} else {
			panic!("Expected TableAsterisk variant");
		}
	}

	#[rstest]
	fn test_into_column_ref_from_str() {
		let col: ColumnRef = "name".into_column_ref();
		if let ColumnRef::Column(iden) = col {
			assert_eq!(iden.to_string(), "name");
		} else {
			panic!("Expected Column variant");
		}
	}

	#[rstest]
	fn test_into_column_ref_from_alias() {
		let alias = Alias::new("my_column");
		let col: ColumnRef = alias.into_column_ref();
		if let ColumnRef::Column(iden) = col {
			assert_eq!(iden.to_string(), "my_column");
		} else {
			panic!("Expected Column variant");
		}
	}
}
