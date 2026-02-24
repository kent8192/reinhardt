//! Table reference types for SQL queries.
//!
//! This module provides types for referencing tables:
//!
//! - [`TableRef`]: Reference to a table (simple, qualified, aliased, subquery)
//! - [`IntoTableRef`]: Conversion trait for table references

use super::iden::{DynIden, IntoIden};

/// Reference to a table in a SQL query.
///
/// This enum represents different ways to reference a table,
/// from simple table names to subqueries.
#[derive(Debug, Clone)]
pub enum TableRef {
	/// Simple table reference (e.g., `users`)
	Table(DynIden),
	/// Schema-qualified table reference (e.g., `public.users`)
	SchemaTable(DynIden, DynIden),
	/// Database, schema, and table reference (e.g., `mydb.public.users`)
	DatabaseSchemaTable(DynIden, DynIden, DynIden),
	/// Table with alias (e.g., `users AS u`)
	TableAlias(DynIden, DynIden),
	/// Schema-qualified table with alias (e.g., `public.users AS u`)
	SchemaTableAlias(DynIden, DynIden, DynIden),
	/// Subquery with alias (e.g., `(SELECT ...) AS alias`)
	SubQuery(Box<crate::query::SelectStatement>, DynIden),
}

impl TableRef {
	/// Create a simple table reference.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::TableRef;
	///
	/// let table = TableRef::table("users");
	/// ```
	pub fn table<I: IntoIden>(table: I) -> Self {
		Self::Table(table.into_iden())
	}

	/// Create a schema-qualified table reference.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::TableRef;
	///
	/// let table = TableRef::schema_table("public", "users");
	/// ```
	pub fn schema_table<S: IntoIden, T: IntoIden>(schema: S, table: T) -> Self {
		Self::SchemaTable(schema.into_iden(), table.into_iden())
	}

	/// Create a database, schema, and table reference.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::TableRef;
	///
	/// let table = TableRef::database_schema_table("mydb", "public", "users");
	/// ```
	pub fn database_schema_table<D: IntoIden, S: IntoIden, T: IntoIden>(
		database: D,
		schema: S,
		table: T,
	) -> Self {
		Self::DatabaseSchemaTable(database.into_iden(), schema.into_iden(), table.into_iden())
	}

	/// Create a table reference with an alias.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::TableRef;
	///
	/// let table = TableRef::table_alias("users", "u");
	/// ```
	pub fn table_alias<T: IntoIden, A: IntoIden>(table: T, alias: A) -> Self {
		Self::TableAlias(table.into_iden(), alias.into_iden())
	}

	/// Create a schema-qualified table reference with an alias.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::TableRef;
	///
	/// let table = TableRef::schema_table_alias("public", "users", "u");
	/// ```
	pub fn schema_table_alias<S: IntoIden, T: IntoIden, A: IntoIden>(
		schema: S,
		table: T,
		alias: A,
	) -> Self {
		Self::SchemaTableAlias(schema.into_iden(), table.into_iden(), alias.into_iden())
	}
}

/// Conversion trait for table references.
///
/// This trait allows various types to be converted into `TableRef`.
pub trait IntoTableRef {
	/// Convert this type into a `TableRef`.
	fn into_table_ref(self) -> TableRef;
}

// Implementation for TableRef itself
impl IntoTableRef for TableRef {
	fn into_table_ref(self) -> TableRef {
		self
	}
}

// Implementation for DynIden (simple table reference)
impl IntoTableRef for DynIden {
	fn into_table_ref(self) -> TableRef {
		TableRef::Table(self)
	}
}

// Implementation for &'static str (simple table reference)
impl IntoTableRef for &'static str {
	fn into_table_ref(self) -> TableRef {
		TableRef::Table(self.into_iden())
	}
}

// Implementation for String (simple table reference)
impl IntoTableRef for String {
	fn into_table_ref(self) -> TableRef {
		TableRef::Table(self.into_iden())
	}
}

// Implementation for tuple (schema, table)
impl<S, T> IntoTableRef for (S, T)
where
	S: IntoIden,
	T: IntoIden,
{
	fn into_table_ref(self) -> TableRef {
		TableRef::SchemaTable(self.0.into_iden(), self.1.into_iden())
	}
}

// Implementation for triple (database, schema, table)
impl<D, S, T> IntoTableRef for (D, S, T)
where
	D: IntoIden,
	S: IntoIden,
	T: IntoIden,
{
	fn into_table_ref(self) -> TableRef {
		TableRef::DatabaseSchemaTable(self.0.into_iden(), self.1.into_iden(), self.2.into_iden())
	}
}

// Implementation for Alias
impl IntoTableRef for super::alias::Alias {
	fn into_table_ref(self) -> TableRef {
		TableRef::Table(self.into_iden())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::Alias;
	use rstest::rstest;

	#[rstest]
	fn test_table_ref_simple() {
		let table = TableRef::table("users");
		if let TableRef::Table(iden) = table {
			assert_eq!(iden.to_string(), "users");
		} else {
			panic!("Expected Table variant");
		}
	}

	#[rstest]
	fn test_table_ref_schema_qualified() {
		let table = TableRef::schema_table("public", "users");
		if let TableRef::SchemaTable(schema, tbl) = table {
			assert_eq!(schema.to_string(), "public");
			assert_eq!(tbl.to_string(), "users");
		} else {
			panic!("Expected SchemaTable variant");
		}
	}

	#[rstest]
	fn test_table_ref_database_schema_qualified() {
		let table = TableRef::database_schema_table("mydb", "public", "users");
		if let TableRef::DatabaseSchemaTable(db, schema, tbl) = table {
			assert_eq!(db.to_string(), "mydb");
			assert_eq!(schema.to_string(), "public");
			assert_eq!(tbl.to_string(), "users");
		} else {
			panic!("Expected DatabaseSchemaTable variant");
		}
	}

	#[rstest]
	fn test_table_ref_with_alias() {
		let table = TableRef::table_alias("users", "u");
		if let TableRef::TableAlias(tbl, alias) = table {
			assert_eq!(tbl.to_string(), "users");
			assert_eq!(alias.to_string(), "u");
		} else {
			panic!("Expected TableAlias variant");
		}
	}

	#[rstest]
	fn test_table_ref_schema_with_alias() {
		let table = TableRef::schema_table_alias("public", "users", "u");
		if let TableRef::SchemaTableAlias(schema, tbl, alias) = table {
			assert_eq!(schema.to_string(), "public");
			assert_eq!(tbl.to_string(), "users");
			assert_eq!(alias.to_string(), "u");
		} else {
			panic!("Expected SchemaTableAlias variant");
		}
	}

	#[rstest]
	fn test_into_table_ref_from_str() {
		let table: TableRef = "users".into_table_ref();
		if let TableRef::Table(iden) = table {
			assert_eq!(iden.to_string(), "users");
		} else {
			panic!("Expected Table variant");
		}
	}

	#[rstest]
	fn test_into_table_ref_from_alias() {
		let alias = Alias::new("my_table");
		let table: TableRef = alias.into_table_ref();
		if let TableRef::Table(iden) = table {
			assert_eq!(iden.to_string(), "my_table");
		} else {
			panic!("Expected Table variant");
		}
	}
}
