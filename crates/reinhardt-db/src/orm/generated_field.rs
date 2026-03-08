//! Generated field support for database-generated columns
//!
//! This module provides support for database-generated columns (computed columns),
//! similar to Django's GeneratedField and SQLAlchemy's Computed columns.
//!
//! Generated columns are automatically computed by the database based on an expression
//! and can be either STORED (persisted to disk) or VIRTUAL (computed on-the-fly).

use super::fields::{BaseField, Field, FieldDeconstruction, FieldKwarg};
use serde::{Deserialize, Serialize};

/// Storage type for generated columns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StorageType {
	/// Column value is computed and stored on disk (persistent)
	/// Provides better read performance but uses more disk space
	Stored,
	/// Column value is computed on-the-fly when queried (ephemeral)
	/// Saves disk space but requires computation on every read
	#[default]
	Virtual,
}

impl StorageType {
	/// Convert storage type to SQL keyword
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::generated_field::StorageType;
	///
	/// assert_eq!(StorageType::Stored.to_sql(), "STORED");
	/// assert_eq!(StorageType::Virtual.to_sql(), "VIRTUAL");
	/// ```
	pub fn to_sql(&self) -> &'static str {
		match self {
			StorageType::Stored => "STORED",
			StorageType::Virtual => "VIRTUAL",
		}
	}
}

/// GeneratedField - a database column with auto-computed value
///
/// Generated fields automatically compute their value based on an expression
/// and cannot be manually set. They are similar to Django's GeneratedField
/// and SQLAlchemy's computed() columns.
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::generated_field::{GeneratedField, StorageType};
///
/// // Virtual generated field (computed on-the-fly)
/// let full_name = GeneratedField::new(
///     "CONCAT(first_name, ' ', last_name)",
///     StorageType::Virtual
/// );
///
/// // Stored generated field (persisted to disk)
/// let total_price = GeneratedField::new(
///     "quantity * unit_price",
///     StorageType::Stored
/// );
/// ```
#[derive(Debug, Clone)]
pub struct GeneratedField {
	/// The base.
	pub base: BaseField,
	/// SQL expression that generates the column value
	pub expression: String,
	/// Whether the value is STORED or VIRTUAL
	pub storage_type: StorageType,
	/// Database backend specific options
	pub db_persist: bool,
}

impl GeneratedField {
	/// Create a new generated field with an expression and storage type
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::generated_field::{GeneratedField, StorageType};
	///
	/// let field = GeneratedField::new(
	///     "price * 1.1",
	///     StorageType::Virtual
	/// );
	/// assert_eq!(field.expression, "price * 1.1");
	/// assert_eq!(field.storage_type, StorageType::Virtual);
	/// ```
	pub fn new(expression: impl Into<String>, storage_type: StorageType) -> Self {
		let mut base = BaseField::new();
		// Generated fields are not editable
		base.editable = false;

		Self {
			base,
			expression: expression.into(),
			storage_type,
			db_persist: storage_type == StorageType::Stored,
		}
	}

	/// Create a virtual generated field (computed on-the-fly)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::generated_field::{GeneratedField, StorageType};
	///
	/// let field = GeneratedField::virtual_field("UPPER(name)");
	/// assert_eq!(field.storage_type, StorageType::Virtual);
	/// ```
	pub fn virtual_field(expression: impl Into<String>) -> Self {
		Self::new(expression, StorageType::Virtual)
	}

	/// Create a stored generated field (persisted to disk)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::generated_field::{GeneratedField, StorageType};
	///
	/// let field = GeneratedField::stored_field("quantity * price");
	/// assert_eq!(field.storage_type, StorageType::Stored);
	/// ```
	pub fn stored_field(expression: impl Into<String>) -> Self {
		Self::new(expression, StorageType::Stored)
	}

	/// Generate the SQL definition for this generated column
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::generated_field::{GeneratedField, StorageType};
	///
	/// let field = GeneratedField::new("price * 1.2", StorageType::Stored);
	/// assert!(field.to_sql().contains("GENERATED ALWAYS AS"));
	/// assert!(field.to_sql().contains("STORED"));
	/// ```
	pub fn to_sql(&self) -> String {
		format!(
			"GENERATED ALWAYS AS ({}) {}",
			self.expression,
			self.storage_type.to_sql()
		)
	}

	/// Generate PostgreSQL-specific SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::generated_field::{GeneratedField, StorageType};
	///
	/// let field = GeneratedField::stored_field("first_name || ' ' || last_name");
	/// assert!(field.to_postgres_sql().contains("GENERATED ALWAYS AS"));
	/// assert!(field.to_postgres_sql().contains("STORED"));
	/// ```
	pub fn to_postgres_sql(&self) -> String {
		// PostgreSQL syntax
		format!(
			"GENERATED ALWAYS AS ({}) {}",
			self.expression,
			self.storage_type.to_sql()
		)
	}

	/// Generate MySQL-specific SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::generated_field::{GeneratedField, StorageType};
	///
	/// let field = GeneratedField::virtual_field("price * 1.1");
	/// assert!(field.to_mysql_sql().contains("GENERATED ALWAYS AS"));
	/// assert!(field.to_mysql_sql().contains("VIRTUAL"));
	/// ```
	pub fn to_mysql_sql(&self) -> String {
		// MySQL 5.7+ syntax
		format!(
			"GENERATED ALWAYS AS ({}) {}",
			self.expression,
			self.storage_type.to_sql()
		)
	}

	/// Generate SQLite-specific SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::generated_field::{GeneratedField, StorageType};
	///
	/// let field = GeneratedField::stored_field("json_extract(data, '$.name')");
	/// assert!(field.to_sqlite_sql().contains("GENERATED ALWAYS AS"));
	/// assert!(field.to_sqlite_sql().contains("STORED"));
	/// ```
	pub fn to_sqlite_sql(&self) -> String {
		// SQLite 3.31.0+ syntax
		format!(
			"GENERATED ALWAYS AS ({}) {}",
			self.expression,
			self.storage_type.to_sql()
		)
	}
}

impl Field for GeneratedField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();

		// Add generated field specific kwargs
		kwargs.insert(
			"expression".to_string(),
			FieldKwarg::String(self.expression.clone()),
		);
		kwargs.insert(
			"storage_type".to_string(),
			FieldKwarg::String(self.storage_type.to_sql().to_string()),
		);
		if self.db_persist {
			kwargs.insert("db_persist".to_string(), FieldKwarg::Bool(true));
		}

		// Generated fields are always not editable
		kwargs.remove("editable");

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.GeneratedField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}

	fn is_null(&self) -> bool {
		self.base.null
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_storage_type_to_sql() {
		assert_eq!(StorageType::Stored.to_sql(), "STORED");
		assert_eq!(StorageType::Virtual.to_sql(), "VIRTUAL");
	}

	#[test]
	fn test_storage_type_default() {
		assert_eq!(StorageType::default(), StorageType::Virtual);
	}

	#[test]
	fn test_generated_field_new() {
		let field = GeneratedField::new("price * 1.1", StorageType::Stored);
		assert_eq!(field.expression, "price * 1.1");
		assert_eq!(field.storage_type, StorageType::Stored);
		assert!(!field.base.editable);
		assert!(field.db_persist);
	}

	#[test]
	fn test_generated_field_virtual() {
		let field = GeneratedField::virtual_field("UPPER(name)");
		assert_eq!(field.expression, "UPPER(name)");
		assert_eq!(field.storage_type, StorageType::Virtual);
		assert!(!field.db_persist);
	}

	#[test]
	fn test_generated_field_stored() {
		let field = GeneratedField::stored_field("quantity * price");
		assert_eq!(field.expression, "quantity * price");
		assert_eq!(field.storage_type, StorageType::Stored);
		assert!(field.db_persist);
	}

	#[test]
	fn test_generated_field_to_sql() {
		let field = GeneratedField::new("a + b", StorageType::Virtual);
		let sql = field.to_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS (a + b) VIRTUAL",
			"Expected exact generated field SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_generated_field_to_sql_stored() {
		let field = GeneratedField::stored_field("CONCAT(first_name, ' ', last_name)");
		let sql = field.to_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS (CONCAT(first_name, ' ', last_name)) STORED",
			"Expected exact stored generated field SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_generated_field_postgres_sql() {
		let field = GeneratedField::virtual_field("price * 1.2");
		let sql = field.to_postgres_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS (price * 1.2) VIRTUAL",
			"Expected exact PostgreSQL generated field SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_generated_field_mysql_sql() {
		let field = GeneratedField::stored_field("quantity * unit_price");
		let sql = field.to_mysql_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS (quantity * unit_price) STORED",
			"Expected exact MySQL generated field SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_generated_field_sqlite_sql() {
		let field = GeneratedField::virtual_field("json_extract(data, '$.name')");
		let sql = field.to_sqlite_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS (json_extract(data, '$.name')) VIRTUAL",
			"Expected exact SQLite generated field SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_generated_field_deconstruct() {
		let mut field = GeneratedField::stored_field("price + tax");
		field.set_attributes_from_name("total_price");

		let dec = field.deconstruct();
		assert_eq!(dec.name, Some("total_price".to_string()));
		assert_eq!(dec.path, "reinhardt.orm.models.GeneratedField");
		assert_eq!(
			dec.kwargs.get("expression"),
			Some(&FieldKwarg::String("price + tax".to_string()))
		);
		assert_eq!(
			dec.kwargs.get("storage_type"),
			Some(&FieldKwarg::String("STORED".to_string()))
		);
		assert_eq!(dec.kwargs.get("db_persist"), Some(&FieldKwarg::Bool(true)));
	}

	#[test]
	fn test_generated_field_not_editable() {
		let field = GeneratedField::virtual_field("col1 + col2");
		assert!(!field.base.editable);

		let dec = field.deconstruct();
		// Should not include editable=False since it's the default for generated fields
		assert!(!dec.kwargs.contains_key("editable"));
	}

	#[test]
	fn test_field_trait_implementation() {
		let mut field = GeneratedField::virtual_field("x * y");
		assert!(field.name().is_none());

		field.set_attributes_from_name("result");
		assert_eq!(field.name(), Some("result"));
	}

	#[test]
	fn test_complex_expression() {
		let field = GeneratedField::stored_field(
			"CASE WHEN status = 'active' THEN price * 0.9 ELSE price END",
		);
		let sql = field.to_sql();
		assert_eq!(
			sql,
			"GENERATED ALWAYS AS (CASE WHEN status = 'active' THEN price * 0.9 ELSE price END) STORED",
			"Expected exact complex expression SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_concat_expression() {
		let field = GeneratedField::virtual_field("CONCAT(first_name, ' ', last_name)");
		assert_eq!(field.expression, "CONCAT(first_name, ' ', last_name)");
		assert_eq!(field.storage_type, StorageType::Virtual);
	}

	#[test]
	fn test_arithmetic_expression() {
		let field = GeneratedField::stored_field("(price - discount) * quantity");
		let sql = field.to_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS ((price - discount) * quantity) STORED",
			"Expected exact arithmetic expression SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_json_extract_expression() {
		let field = GeneratedField::virtual_field("json_extract(metadata, '$.title')");
		assert_eq!(field.expression, "json_extract(metadata, '$.title')");
	}

	#[test]
	fn test_null_field() {
		let mut field = GeneratedField::virtual_field("col1 + col2");
		field.base.null = true;

		assert!(field.is_null());
		let dec = field.deconstruct();
		assert_eq!(dec.kwargs.get("null"), Some(&FieldKwarg::Bool(true)));
	}

	#[test]
	fn test_multiple_backends_sql_generation() {
		let field = GeneratedField::stored_field("price * tax_rate");

		let pg_sql = field.to_postgres_sql();
		let mysql_sql = field.to_mysql_sql();
		let sqlite_sql = field.to_sqlite_sql();

		let expected = "GENERATED ALWAYS AS (price * tax_rate) STORED";

		assert_eq!(
			pg_sql, expected,
			"Expected exact PostgreSQL SQL, got: {}",
			pg_sql
		);
		assert_eq!(
			mysql_sql, expected,
			"Expected exact MySQL SQL, got: {}",
			mysql_sql
		);
		assert_eq!(
			sqlite_sql, expected,
			"Expected exact SQLite SQL, got: {}",
			sqlite_sql
		);
	}

	#[test]
	fn test_storage_type_equality() {
		let stored1 = StorageType::Stored;
		let stored2 = StorageType::Stored;
		let virtual1 = StorageType::Virtual;

		assert_eq!(stored1, stored2);
		assert_ne!(stored1, virtual1);
	}

	#[test]
	fn test_field_name_setting() {
		let mut field = GeneratedField::virtual_field("a + b");
		assert!(field.name().is_none());

		field.set_attributes_from_name("sum_field");
		assert_eq!(field.name(), Some("sum_field"));

		field.set_attributes_from_name("total");
		assert_eq!(field.name(), Some("total"));
	}

	#[test]
	fn test_db_persist_flag() {
		let virtual_field = GeneratedField::virtual_field("col1");
		assert!(!virtual_field.db_persist);

		let stored_field = GeneratedField::stored_field("col2");
		assert!(stored_field.db_persist);
	}

	#[test]
	fn test_date_expression() {
		let field = GeneratedField::stored_field("DATE_ADD(created_at, INTERVAL 30 DAY)");
		let sql = field.to_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS (DATE_ADD(created_at, INTERVAL 30 DAY)) STORED",
			"Expected exact date expression SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_string_function_expression() {
		let field = GeneratedField::virtual_field("LOWER(TRIM(email))");
		assert_eq!(field.expression, "LOWER(TRIM(email))");
	}

	#[test]
	fn test_conditional_expression() {
		let field = GeneratedField::stored_field("IF(quantity > 10, price * 0.9, price)");
		let sql = field.to_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS (IF(quantity > 10, price * 0.9, price)) STORED",
			"Expected exact conditional expression SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_subquery_expression() {
		let field = GeneratedField::virtual_field(
			"(SELECT COUNT(*) FROM orders WHERE orders.user_id = users.id)",
		);
		assert_eq!(
			field.expression, "(SELECT COUNT(*) FROM orders WHERE orders.user_id = users.id)",
			"Expected exact subquery expression, got: {}",
			field.expression
		);
	}

	#[test]
	fn test_aggregate_expression() {
		let field = GeneratedField::stored_field("COALESCE(discount, 0) + base_price");
		let sql = field.to_sql();
		assert_eq!(
			sql, "GENERATED ALWAYS AS (COALESCE(discount, 0) + base_price) STORED",
			"Expected exact aggregate expression SQL, got: {}",
			sql
		);
	}

	#[test]
	fn test_deconstruct_virtual_field() {
		let field = GeneratedField::virtual_field("x + y");
		let dec = field.deconstruct();

		assert_eq!(
			dec.kwargs.get("storage_type"),
			Some(&FieldKwarg::String("VIRTUAL".to_string()))
		);
		assert!(!dec.kwargs.contains_key("db_persist"));
	}

	#[test]
	fn test_expression_with_special_characters() {
		let field = GeneratedField::stored_field("regexp_replace(text, '[^a-zA-Z]', '')");
		assert_eq!(field.expression, "regexp_replace(text, '[^a-zA-Z]', '')");
	}
}
