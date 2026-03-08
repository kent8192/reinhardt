//! Composite primary key support for database models
//!
//! This module provides functionality for defining and working with composite primary keys,
//! which consist of multiple fields combined to form a unique identifier for a database record.

use super::constraints::Constraint;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Error types for composite primary key operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum CompositePkError {
	/// Empty fields list provided
	EmptyFields,
	/// Missing required field value
	MissingField(String),
	/// Invalid field value type
	InvalidFieldType {
		/// The field name.
		field: String,
		/// The expected type name.
		expected: String,
	},
	/// Duplicate field name in definition
	DuplicateField(String),
}

impl std::fmt::Display for CompositePkError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			CompositePkError::EmptyFields => {
				write!(f, "Composite primary key must have at least one field")
			}
			CompositePkError::MissingField(field) => write!(f, "Missing required field: {}", field),
			CompositePkError::InvalidFieldType { field, expected } => {
				write!(
					f,
					"Invalid type for field '{}': expected {}",
					field, expected
				)
			}
			CompositePkError::DuplicateField(field) => write!(f, "Duplicate field name: {}", field),
		}
	}
}

impl std::error::Error for CompositePkError {}

/// Value types supported in composite primary keys
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PkValue {
	/// String variant.
	String(String),
	/// Int variant.
	Int(i64),
	/// Uint variant.
	Uint(u64),
	/// Bool variant.
	Bool(bool),
}

impl PkValue {
	/// Convert the value to a string representation for SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::PkValue;
	///
	/// let value = PkValue::Int(42);
	/// assert_eq!(value.to_sql_string(), "42");
	///
	/// let value = PkValue::String("test".to_string());
	/// assert_eq!(value.to_sql_string(), "'test'");
	/// ```
	pub fn to_sql_string(&self) -> String {
		match self {
			PkValue::String(s) => format!("'{}'", s.replace('\'', "''")),
			PkValue::Int(i) => i.to_string(),
			PkValue::Uint(u) => u.to_string(),
			PkValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
		}
	}
}

// Conversion implementations for common types
impl From<String> for PkValue {
	fn from(value: String) -> Self {
		PkValue::String(value)
	}
}

impl From<&str> for PkValue {
	fn from(value: &str) -> Self {
		PkValue::String(value.to_string())
	}
}

impl From<i32> for PkValue {
	fn from(value: i32) -> Self {
		PkValue::Int(value as i64)
	}
}

impl From<i64> for PkValue {
	fn from(value: i64) -> Self {
		PkValue::Int(value)
	}
}

impl From<u32> for PkValue {
	fn from(value: u32) -> Self {
		PkValue::Uint(value as u64)
	}
}

impl From<u64> for PkValue {
	fn from(value: u64) -> Self {
		PkValue::Uint(value)
	}
}

impl From<bool> for PkValue {
	fn from(value: bool) -> Self {
		PkValue::Bool(value)
	}
}

impl From<&i32> for PkValue {
	fn from(value: &i32) -> Self {
		PkValue::Int(*value as i64)
	}
}

impl From<&i64> for PkValue {
	fn from(value: &i64) -> Self {
		PkValue::Int(*value)
	}
}

impl From<&u32> for PkValue {
	fn from(value: &u32) -> Self {
		PkValue::Uint(*value as u64)
	}
}

impl From<&u64> for PkValue {
	fn from(value: &u64) -> Self {
		PkValue::Uint(*value)
	}
}

impl From<&bool> for PkValue {
	fn from(value: &bool) -> Self {
		PkValue::Bool(*value)
	}
}

impl From<&String> for PkValue {
	fn from(value: &String) -> Self {
		PkValue::String(value.clone())
	}
}

/// Quote a SQL identifier using ANSI SQL double-quote escaping.
///
/// Embedded double quotes are escaped by doubling them.
fn quote_identifier(ident: &str) -> String {
	format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Composite primary key definition consisting of multiple fields
///
/// A composite primary key is a primary key that spans multiple columns in a database table.
/// This is useful when no single field can uniquely identify a record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositePrimaryKey {
	/// Field names that compose the primary key
	fields: Vec<String>,
	/// Optional constraint name
	name: Option<String>,
}

impl CompositePrimaryKey {
	/// Create a new composite primary key with the specified fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::CompositePrimaryKey;
	///
	/// let pk = CompositePrimaryKey::new(vec!["user_id".to_string(), "role_id".to_string()]);
	/// assert!(pk.is_ok());
	/// assert_eq!(pk.unwrap().fields().len(), 2);
	/// ```
	pub fn new(fields: Vec<String>) -> Result<Self, CompositePkError> {
		if fields.is_empty() {
			return Err(CompositePkError::EmptyFields);
		}

		let mut seen = HashMap::new();
		for field in &fields {
			if seen.contains_key(field) {
				return Err(CompositePkError::DuplicateField(field.clone()));
			}
			seen.insert(field.clone(), true);
		}

		Ok(Self { fields, name: None })
	}

	/// Create a composite primary key with a custom constraint name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::CompositePrimaryKey;
	///
	/// let pk = CompositePrimaryKey::with_name(
	///     vec!["user_id".to_string(), "group_id".to_string()],
	///     "user_group_pk"
	/// );
	/// assert!(pk.is_ok());
	/// ```
	pub fn with_name(
		fields: Vec<String>,
		name: impl Into<String>,
	) -> Result<Self, CompositePkError> {
		let mut pk = Self::new(fields)?;
		pk.name = Some(name.into());
		Ok(pk)
	}

	/// Get the list of field names in the composite key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::CompositePrimaryKey;
	///
	/// let pk = CompositePrimaryKey::new(vec!["a".to_string(), "b".to_string()]).unwrap();
	/// assert_eq!(pk.fields(), &["a", "b"]);
	/// ```
	pub fn fields(&self) -> &[String] {
		&self.fields
	}

	/// Get the constraint name if set
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// Generate SQL PRIMARY KEY constraint definition
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::CompositePrimaryKey;
	///
	/// let pk = CompositePrimaryKey::new(vec!["user_id".to_string(), "order_id".to_string()]).unwrap();
	/// let sql = pk.to_sql();
	/// assert!(sql.contains("PRIMARY KEY"));
	/// assert!(sql.contains("user_id"));
	/// assert!(sql.contains("order_id"));
	/// ```
	pub fn to_sql(&self) -> String {
		let fields: Vec<String> = self.fields.iter().map(|f| quote_identifier(f)).collect();
		let fields = fields.join(", ");
		if let Some(ref name) = self.name {
			format!(
				"CONSTRAINT {} PRIMARY KEY ({})",
				quote_identifier(name),
				fields
			)
		} else {
			format!("PRIMARY KEY ({})", fields)
		}
	}

	/// Validate that all required fields are present in the provided values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::{CompositePrimaryKey, PkValue};
	/// use std::collections::HashMap;
	///
	/// let pk = CompositePrimaryKey::new(vec!["id".to_string(), "type".to_string()]).unwrap();
	/// let mut values = HashMap::new();
	/// values.insert("id".to_string(), PkValue::Int(1));
	/// values.insert("type".to_string(), PkValue::String("admin".to_string()));
	///
	/// assert!(pk.validate(&values).is_ok());
	/// ```
	pub fn validate(&self, values: &HashMap<String, PkValue>) -> Result<(), CompositePkError> {
		for field in &self.fields {
			if !values.contains_key(field) {
				return Err(CompositePkError::MissingField(field.clone()));
			}
		}
		Ok(())
	}

	/// Generate a WHERE clause for querying by this composite key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::{CompositePrimaryKey, PkValue};
	/// use std::collections::HashMap;
	///
	/// let pk = CompositePrimaryKey::new(vec!["user_id".to_string(), "role_id".to_string()]).unwrap();
	/// let mut values = HashMap::new();
	/// values.insert("user_id".to_string(), PkValue::Int(100));
	/// values.insert("role_id".to_string(), PkValue::Int(5));
	///
	/// let where_clause = pk.to_where_clause(&values);
	/// assert!(where_clause.is_ok());
	/// assert!(where_clause.unwrap().contains("\"user_id\" = 100"));
	/// ```
	pub fn to_where_clause(
		&self,
		values: &HashMap<String, PkValue>,
	) -> Result<String, CompositePkError> {
		self.validate(values)?;

		let conditions: Vec<String> = self
			.fields
			.iter()
			.map(|field| {
				let value = values.get(field).unwrap();
				format!("{} = {}", quote_identifier(field), value.to_sql_string())
			})
			.collect();

		Ok(conditions.join(" AND "))
	}

	/// Check if this composite key contains a specific field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::CompositePrimaryKey;
	///
	/// let pk = CompositePrimaryKey::new(vec!["a".to_string(), "b".to_string()]).unwrap();
	/// assert!(pk.contains_field("a"));
	/// assert!(pk.contains_field("b"));
	/// assert!(!pk.contains_field("c"));
	/// ```
	pub fn contains_field(&self, field: &str) -> bool {
		self.fields.iter().any(|f| f == field)
	}

	/// Get the number of fields in this composite key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_pk::CompositePrimaryKey;
	///
	/// let pk = CompositePrimaryKey::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]).unwrap();
	/// assert_eq!(pk.field_count(), 3);
	/// ```
	pub fn field_count(&self) -> usize {
		self.fields.len()
	}
}

impl Constraint for CompositePrimaryKey {
	fn to_sql(&self) -> String {
		CompositePrimaryKey::to_sql(self)
	}

	fn name(&self) -> &str {
		self.name.as_deref().unwrap_or("composite_pk")
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_composite_pk_new_valid() {
		let pk = CompositePrimaryKey::new(vec!["user_id".to_string(), "role_id".to_string()]);
		let pk = pk.unwrap();
		assert_eq!(pk.fields().len(), 2);
		assert_eq!(pk.fields()[0], "user_id");
		assert_eq!(pk.fields()[1], "role_id");
	}

	#[test]
	fn test_composite_pk_new_empty_fields() {
		let pk = CompositePrimaryKey::new(vec![]);
		assert!(pk.is_err());
		assert_eq!(pk.unwrap_err(), CompositePkError::EmptyFields);
	}

	#[test]
	fn test_composite_pk_duplicate_fields() {
		let pk =
			CompositePrimaryKey::new(vec!["id".to_string(), "type".to_string(), "id".to_string()]);
		assert!(pk.is_err());
		match pk.unwrap_err() {
			CompositePkError::DuplicateField(field) => assert_eq!(field, "id"),
			_ => panic!("Expected DuplicateField error"),
		}
	}

	#[test]
	fn test_composite_pk_with_name() {
		let pk =
			CompositePrimaryKey::with_name(vec!["a".to_string(), "b".to_string()], "custom_pk");
		let pk = pk.unwrap();
		assert_eq!(pk.name(), Some("custom_pk"));
	}

	#[test]
	fn test_composite_pk_to_sql_without_name() {
		let pk =
			CompositePrimaryKey::new(vec!["user_id".to_string(), "order_id".to_string()]).unwrap();
		let sql = pk.to_sql();
		assert_eq!(sql, "PRIMARY KEY (\"user_id\", \"order_id\")");
	}

	#[test]
	fn test_composite_pk_to_sql_with_name() {
		let pk = CompositePrimaryKey::with_name(
			vec!["user_id".to_string(), "order_id".to_string()],
			"user_order_pk",
		)
		.unwrap();
		let sql = pk.to_sql();
		assert_eq!(
			sql,
			"CONSTRAINT \"user_order_pk\" PRIMARY KEY (\"user_id\", \"order_id\")"
		);
	}

	#[test]
	fn test_pk_value_to_sql_string() {
		assert_eq!(PkValue::Int(42).to_sql_string(), "42");
		assert_eq!(PkValue::Uint(100).to_sql_string(), "100");
		assert_eq!(PkValue::Bool(true).to_sql_string(), "TRUE");
		assert_eq!(PkValue::Bool(false).to_sql_string(), "FALSE");
		assert_eq!(
			PkValue::String("test".to_string()).to_sql_string(),
			"'test'"
		);
	}

	#[test]
	fn test_pk_value_sql_string_escapes_quotes() {
		let value = PkValue::String("O'Brien".to_string());
		assert_eq!(value.to_sql_string(), "'O''Brien'");
	}

	#[test]
	fn test_validate_success() {
		let pk = CompositePrimaryKey::new(vec!["id".to_string(), "type".to_string()]).unwrap();
		let mut values = HashMap::new();
		values.insert("id".to_string(), PkValue::Int(1));
		values.insert("type".to_string(), PkValue::String("admin".to_string()));

		assert!(pk.validate(&values).is_ok());
	}

	#[test]
	fn test_validate_missing_field() {
		let pk = CompositePrimaryKey::new(vec!["id".to_string(), "type".to_string()]).unwrap();
		let mut values = HashMap::new();
		values.insert("id".to_string(), PkValue::Int(1));

		let result = pk.validate(&values);
		assert!(result.is_err());
		match result.unwrap_err() {
			CompositePkError::MissingField(field) => assert_eq!(field, "type"),
			_ => panic!("Expected MissingField error"),
		}
	}

	#[test]
	fn test_to_where_clause_success() {
		let pk =
			CompositePrimaryKey::new(vec!["user_id".to_string(), "role_id".to_string()]).unwrap();
		let mut values = HashMap::new();
		values.insert("user_id".to_string(), PkValue::Int(100));
		values.insert("role_id".to_string(), PkValue::Int(5));

		let where_clause = pk.to_where_clause(&values);
		let clause = where_clause.unwrap();
		assert!(clause.contains("\"user_id\" = 100"));
		assert!(clause.contains("\"role_id\" = 5"));
		assert!(clause.contains(" AND "));
	}

	#[test]
	fn test_to_where_clause_missing_field() {
		let pk =
			CompositePrimaryKey::new(vec!["user_id".to_string(), "role_id".to_string()]).unwrap();
		let mut values = HashMap::new();
		values.insert("user_id".to_string(), PkValue::Int(100));

		let result = pk.to_where_clause(&values);
		assert!(result.is_err());
	}

	#[test]
	fn test_contains_field() {
		let pk = CompositePrimaryKey::new(vec!["a".to_string(), "b".to_string()]).unwrap();
		assert!(pk.contains_field("a"));
		assert!(pk.contains_field("b"));
		assert!(!pk.contains_field("c"));
	}

	#[test]
	fn test_field_count() {
		let pk = CompositePrimaryKey::new(vec!["a".to_string(), "b".to_string(), "c".to_string()])
			.unwrap();
		assert_eq!(pk.field_count(), 3);
	}

	#[test]
	fn test_constraint_trait_implementation() {
		let pk = CompositePrimaryKey::with_name(vec!["id".to_string()], "test_pk").unwrap();
		assert_eq!(pk.name(), Some("test_pk"));
		assert!(pk.to_sql().contains("PRIMARY KEY"));
	}

	#[test]
	fn test_composite_pk_three_fields() {
		let pk = CompositePrimaryKey::new(vec![
			"org_id".to_string(),
			"user_id".to_string(),
			"project_id".to_string(),
		])
		.unwrap();
		assert_eq!(pk.field_count(), 3);
		let sql = pk.to_sql();
		assert!(sql.contains("org_id"));
		assert!(sql.contains("user_id"));
		assert!(sql.contains("project_id"));
	}

	#[test]
	fn test_where_clause_with_string_values() {
		let pk = CompositePrimaryKey::new(vec!["country".to_string(), "city".to_string()]).unwrap();
		let mut values = HashMap::new();
		values.insert("country".to_string(), PkValue::String("USA".to_string()));
		values.insert("city".to_string(), PkValue::String("New York".to_string()));

		let where_clause = pk.to_where_clause(&values).unwrap();
		assert!(where_clause.contains("\"country\" = 'USA'"));
		assert!(where_clause.contains("\"city\" = 'New York'"));
	}

	#[test]
	fn test_where_clause_with_mixed_types() {
		let pk = CompositePrimaryKey::new(vec!["id".to_string(), "active".to_string()]).unwrap();
		let mut values = HashMap::new();
		values.insert("id".to_string(), PkValue::Uint(42));
		values.insert("active".to_string(), PkValue::Bool(true));

		let where_clause = pk.to_where_clause(&values).unwrap();
		assert!(where_clause.contains("\"id\" = 42"));
		assert!(where_clause.contains("\"active\" = TRUE"));
	}

	#[test]
	fn test_error_display() {
		let err = CompositePkError::EmptyFields;
		assert_eq!(
			err.to_string(),
			"Composite primary key must have at least one field"
		);

		let err = CompositePkError::MissingField("user_id".to_string());
		assert_eq!(err.to_string(), "Missing required field: user_id");

		let err = CompositePkError::DuplicateField("id".to_string());
		assert_eq!(err.to_string(), "Duplicate field name: id");
	}

	#[test]
	fn test_pk_value_serialization() {
		let value = PkValue::Int(42);
		let serialized = serde_json::to_string(&value).unwrap();
		assert_eq!(serialized, "42");

		let value = PkValue::String("test".to_string());
		let serialized = serde_json::to_string(&value).unwrap();
		assert_eq!(serialized, "\"test\"");
	}

	#[rstest]
	fn test_to_sql_quotes_normal_field_names() {
		// Arrange
		let pk =
			CompositePrimaryKey::new(vec!["user_id".to_string(), "role_id".to_string()]).unwrap();

		// Act
		let sql = pk.to_sql();

		// Assert
		assert_eq!(sql, "PRIMARY KEY (\"user_id\", \"role_id\")");
	}

	#[rstest]
	fn test_to_sql_escapes_special_characters_in_field_names() {
		// Arrange
		let pk =
			CompositePrimaryKey::new(vec!["field\"; DROP TABLE users; --".to_string()]).unwrap();

		// Act
		let sql = pk.to_sql();

		// Assert
		// The embedded double quote is escaped by doubling, preventing identifier breakout
		assert_eq!(sql, "PRIMARY KEY (\"field\"\"; DROP TABLE users; --\")");
	}

	#[rstest]
	fn test_to_sql_escapes_special_characters_in_constraint_name() {
		// Arrange
		let pk =
			CompositePrimaryKey::with_name(vec!["id".to_string()], "pk\"; DROP TABLE users; --")
				.unwrap();

		// Act
		let sql = pk.to_sql();

		// Assert
		assert_eq!(
			sql,
			"CONSTRAINT \"pk\"\"; DROP TABLE users; --\" PRIMARY KEY (\"id\")"
		);
	}

	#[rstest]
	fn test_to_sql_escapes_embedded_double_quotes_in_field() {
		// Arrange
		let pk = CompositePrimaryKey::new(vec!["field\"name".to_string()]).unwrap();

		// Act
		let sql = pk.to_sql();

		// Assert
		assert_eq!(sql, "PRIMARY KEY (\"field\"\"name\")");
	}

	#[rstest]
	fn test_where_clause_quotes_field_names_with_special_characters() {
		// Arrange
		let malicious_field = "id\"; DROP TABLE users; --".to_string();
		let pk = CompositePrimaryKey::new(vec![malicious_field.clone()]).unwrap();
		let mut values = HashMap::new();
		values.insert(malicious_field, PkValue::Int(1));

		// Act
		let clause = pk.to_where_clause(&values).unwrap();

		// Assert
		assert_eq!(clause, "\"id\"\"; DROP TABLE users; --\" = 1");
	}

	#[rstest]
	fn test_where_clause_quotes_normal_field_names() {
		// Arrange
		let pk = CompositePrimaryKey::new(vec!["user_id".to_string()]).unwrap();
		let mut values = HashMap::new();
		values.insert("user_id".to_string(), PkValue::Int(42));

		// Act
		let clause = pk.to_where_clause(&values).unwrap();

		// Assert
		assert_eq!(clause, "\"user_id\" = 42");
	}
}
