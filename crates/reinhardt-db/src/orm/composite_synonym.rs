//! Composite synonym functionality
//!
//! Allows creating aliases (synonyms) from multiple field combinations.
//! Similar to SQLAlchemy's synonym but for composite fields.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Error type for composite synonym operations
#[non_exhaustive]
#[derive(Debug)]
pub enum SynonymError {
	FieldNotFound(String),
	InvalidCombination(String),
	ComputationError(String),
}

impl fmt::Display for SynonymError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SynonymError::FieldNotFound(msg) => write!(f, "Field not found: {}", msg),
			SynonymError::InvalidCombination(msg) => {
				write!(f, "Invalid field combination: {}", msg)
			}
			SynonymError::ComputationError(msg) => write!(f, "Computation error: {}", msg),
		}
	}
}

impl std::error::Error for SynonymError {}

/// Value type for field values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
	Integer(i64),
	Float(f64),
	String(String),
	Boolean(bool),
	Null,
}

impl std::fmt::Display for FieldValue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FieldValue::Integer(i) => write!(f, "{}", i),
			FieldValue::Float(fl) => write!(f, "{}", fl),
			FieldValue::String(s) => write!(f, "{}", s),
			FieldValue::Boolean(b) => write!(f, "{}", b),
			FieldValue::Null => Ok(()),
		}
	}
}

/// Represents a composite synonym (alias) for multiple fields
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::composite_synonym::CompositeSynonym;
///
/// let full_name = CompositeSynonym::new(
///     "full_name".to_string(),
///     vec!["first_name".to_string(), "last_name".to_string()],
/// );
///
/// assert_eq!(full_name.name(), "full_name");
/// assert_eq!(full_name.fields().len(), 2);
/// ```
pub struct CompositeSynonym {
	/// Name of the synonym
	name: String,
	/// Fields that compose this synonym
	fields: Vec<String>,
	/// Separator for joining field values
	separator: String,
}

impl CompositeSynonym {
	/// Creates a new CompositeSynonym with default separator (space)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_synonym::CompositeSynonym;
	///
	/// let address = CompositeSynonym::new(
	///     "address".to_string(),
	///     vec!["street".to_string(), "city".to_string(), "zip".to_string()],
	/// );
	///
	/// assert_eq!(address.separator(), " ");
	/// ```
	pub fn new(name: String, fields: Vec<String>) -> Self {
		Self {
			name,
			fields,
			separator: " ".to_string(),
		}
	}

	/// Sets a custom separator for the synonym
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_synonym::CompositeSynonym;
	///
	/// let csv_fields = CompositeSynonym::new(
	///     "csv_data".to_string(),
	///     vec!["field1".to_string(), "field2".to_string()],
	/// )
	/// .with_separator(",");
	///
	/// assert_eq!(csv_fields.separator(), ",");
	/// ```
	pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
		self.separator = separator.into();
		self
	}

	/// Gets the name of the synonym
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Gets the fields that compose this synonym
	pub fn fields(&self) -> &[String] {
		&self.fields
	}

	/// Gets the separator
	pub fn separator(&self) -> &str {
		&self.separator
	}

	/// Generates SQL expression for this synonym
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_synonym::CompositeSynonym;
	///
	/// let full_name = CompositeSynonym::new(
	///     "full_name".to_string(),
	///     vec!["first_name".to_string(), "last_name".to_string()],
	/// );
	///
	/// let sql = full_name.to_sql();
	/// assert!(sql.contains("CONCAT"));
	/// assert!(sql.contains("first_name"));
	/// assert!(sql.contains("last_name"));
	/// ```
	pub fn to_sql(&self) -> String {
		if self.fields.is_empty() {
			return String::from("NULL");
		}

		if self.fields.len() == 1 {
			return self.fields[0].clone();
		}

		let field_refs: Vec<String> = self.fields.iter().map(|f| format!("'{}'", f)).collect();

		format!("CONCAT_WS('{}', {})", self.separator, field_refs.join(", "))
	}

	/// Computes the synonym value from an object's field values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::composite_synonym::{CompositeSynonym, FieldValue};
	/// use std::collections::HashMap;
	///
	/// let full_name = CompositeSynonym::new(
	///     "full_name".to_string(),
	///     vec!["first_name".to_string(), "last_name".to_string()],
	/// );
	///
	/// let mut object = HashMap::new();
	/// object.insert("first_name".to_string(), FieldValue::String("John".to_string()));
	/// object.insert("last_name".to_string(), FieldValue::String("Doe".to_string()));
	///
	/// let result = full_name.compute_value(&object);
	/// assert_eq!(result, "John Doe");
	/// ```
	pub fn compute_value(&self, object: &HashMap<String, FieldValue>) -> String {
		self.fields
			.iter()
			.filter_map(|field| object.get(field))
			.map(|value| value.to_string())
			.filter(|s| !s.is_empty())
			.collect::<Vec<_>>()
			.join(&self.separator)
	}

	/// Validates that all required fields exist in the object
	pub fn validate_fields(
		&self,
		object: &HashMap<String, FieldValue>,
	) -> Result<(), SynonymError> {
		for field in &self.fields {
			if !object.contains_key(field) {
				return Err(SynonymError::FieldNotFound(field.clone()));
			}
		}
		Ok(())
	}

	/// Computes the synonym value with strict validation
	pub fn compute_value_strict(
		&self,
		object: &HashMap<String, FieldValue>,
	) -> Result<String, SynonymError> {
		self.validate_fields(object)?;
		Ok(self.compute_value(object))
	}

	/// Creates a synonym for concatenating fields without separator
	pub fn concat(name: String, fields: Vec<String>) -> Self {
		Self {
			name,
			fields,
			separator: String::new(),
		}
	}

	/// Creates a synonym for comma-separated values
	pub fn csv(name: String, fields: Vec<String>) -> Self {
		Self {
			name,
			fields,
			separator: ",".to_string(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_composite_synonym_creation() {
		let synonym = CompositeSynonym::new(
			"full_name".to_string(),
			vec!["first_name".to_string(), "last_name".to_string()],
		);

		assert_eq!(synonym.name(), "full_name");
		assert_eq!(synonym.fields().len(), 2);
		assert_eq!(synonym.separator(), " ");
	}

	#[test]
	fn test_composite_synonym_with_custom_separator() {
		let synonym = CompositeSynonym::new(
			"csv_data".to_string(),
			vec!["field1".to_string(), "field2".to_string()],
		)
		.with_separator(", ");

		assert_eq!(synonym.separator(), ", ");
	}

	#[test]
	fn test_to_sql() {
		let synonym = CompositeSynonym::new(
			"full_name".to_string(),
			vec!["first_name".to_string(), "last_name".to_string()],
		);

		let sql = synonym.to_sql();
		assert!(sql.contains("CONCAT_WS"));
		assert!(sql.contains("' '"));
	}

	#[test]
	fn test_to_sql_single_field() {
		let synonym = CompositeSynonym::new("alias".to_string(), vec!["field".to_string()]);

		let sql = synonym.to_sql();
		assert_eq!(sql, "field");
	}

	#[test]
	fn test_to_sql_empty_fields() {
		let synonym = CompositeSynonym::new("empty".to_string(), vec![]);

		let sql = synonym.to_sql();
		assert_eq!(sql, "NULL");
	}

	#[test]
	fn test_compute_value() {
		let synonym = CompositeSynonym::new(
			"full_name".to_string(),
			vec!["first_name".to_string(), "last_name".to_string()],
		);

		let mut object = HashMap::new();
		object.insert(
			"first_name".to_string(),
			FieldValue::String("Alice".to_string()),
		);
		object.insert(
			"last_name".to_string(),
			FieldValue::String("Smith".to_string()),
		);

		let result = synonym.compute_value(&object);
		assert_eq!(result, "Alice Smith");
	}

	#[test]
	fn test_compute_value_with_custom_separator() {
		let synonym = CompositeSynonym::new(
			"address".to_string(),
			vec!["street".to_string(), "city".to_string()],
		)
		.with_separator(", ");

		let mut object = HashMap::new();
		object.insert(
			"street".to_string(),
			FieldValue::String("123 Main St".to_string()),
		);
		object.insert(
			"city".to_string(),
			FieldValue::String("Springfield".to_string()),
		);

		let result = synonym.compute_value(&object);
		assert_eq!(result, "123 Main St, Springfield");
	}

	#[test]
	fn test_compute_value_with_missing_fields() {
		let synonym = CompositeSynonym::new(
			"full_name".to_string(),
			vec!["first_name".to_string(), "last_name".to_string()],
		);

		let mut object = HashMap::new();
		object.insert(
			"first_name".to_string(),
			FieldValue::String("Alice".to_string()),
		);

		let result = synonym.compute_value(&object);
		assert_eq!(result, "Alice");
	}

	#[test]
	fn test_validate_fields() {
		let synonym = CompositeSynonym::new(
			"full_name".to_string(),
			vec!["first_name".to_string(), "last_name".to_string()],
		);

		let mut object = HashMap::new();
		object.insert(
			"first_name".to_string(),
			FieldValue::String("Alice".to_string()),
		);
		object.insert(
			"last_name".to_string(),
			FieldValue::String("Smith".to_string()),
		);

		assert!(synonym.validate_fields(&object).is_ok());

		let mut incomplete_object = HashMap::new();
		incomplete_object.insert(
			"first_name".to_string(),
			FieldValue::String("Alice".to_string()),
		);

		assert!(synonym.validate_fields(&incomplete_object).is_err());
	}

	#[test]
	fn test_compute_value_strict() {
		let synonym = CompositeSynonym::new(
			"full_name".to_string(),
			vec!["first_name".to_string(), "last_name".to_string()],
		);

		let mut object = HashMap::new();
		object.insert(
			"first_name".to_string(),
			FieldValue::String("Alice".to_string()),
		);
		object.insert(
			"last_name".to_string(),
			FieldValue::String("Smith".to_string()),
		);

		let result = synonym.compute_value_strict(&object);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "Alice Smith");

		let mut incomplete_object = HashMap::new();
		incomplete_object.insert(
			"first_name".to_string(),
			FieldValue::String("Alice".to_string()),
		);

		let result = synonym.compute_value_strict(&incomplete_object);
		assert!(result.is_err());
	}

	#[test]
	fn test_concat_synonym() {
		let synonym = CompositeSynonym::concat(
			"code".to_string(),
			vec!["prefix".to_string(), "number".to_string()],
		);

		let mut object = HashMap::new();
		object.insert("prefix".to_string(), FieldValue::String("ABC".to_string()));
		object.insert("number".to_string(), FieldValue::String("123".to_string()));

		let result = synonym.compute_value(&object);
		assert_eq!(result, "ABC123");
		assert_eq!(synonym.separator(), "");
	}

	#[test]
	fn test_csv_synonym() {
		let synonym = CompositeSynonym::csv(
			"tags".to_string(),
			vec!["tag1".to_string(), "tag2".to_string()],
		);

		let mut object = HashMap::new();
		object.insert("tag1".to_string(), FieldValue::String("rust".to_string()));
		object.insert("tag2".to_string(), FieldValue::String("orm".to_string()));

		let result = synonym.compute_value(&object);
		assert_eq!(result, "rust,orm");
		assert_eq!(synonym.separator(), ",");
	}

	#[test]
	fn test_field_value_to_string() {
		assert_eq!(FieldValue::Integer(42).to_string(), "42");
		assert_eq!(FieldValue::Float(3.15).to_string(), "3.15");
		assert_eq!(FieldValue::String("test".to_string()).to_string(), "test");
		assert_eq!(FieldValue::Boolean(true).to_string(), "true");
		assert_eq!(FieldValue::Null.to_string(), "");
	}
}
