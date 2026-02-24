//! Aggregation functions for database queries
//!
//! This module provides Django-inspired aggregation functionality.

use reinhardt_query::prelude::{Alias, Iden};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Aggregate function types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregateFunc {
	/// COUNT aggregation
	Count,
	/// COUNT DISTINCT aggregation
	CountDistinct,
	/// SUM aggregation
	Sum,
	/// AVG aggregation
	Avg,
	/// MAX aggregation
	Max,
	/// MIN aggregation
	Min,
}

impl fmt::Display for AggregateFunc {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			AggregateFunc::Count => write!(f, "COUNT"),
			AggregateFunc::CountDistinct => write!(f, "COUNT"),
			AggregateFunc::Sum => write!(f, "SUM"),
			AggregateFunc::Avg => write!(f, "AVG"),
			AggregateFunc::Max => write!(f, "MAX"),
			AggregateFunc::Min => write!(f, "MIN"),
		}
	}
}

/// Aggregate expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregate {
	/// The aggregate function
	pub func: AggregateFunc,
	/// The field to aggregate (None for COUNT(*))
	pub field: Option<String>,
	/// Alias for the result
	pub alias: Option<String>,
	/// Whether this is a DISTINCT aggregation
	pub distinct: bool,
}

/// Validates an SQL identifier (column name, alias, etc.)
///
/// This function checks that the identifier follows safe SQL naming conventions:
/// - Non-empty
/// - Contains only alphanumeric characters and underscores
/// - Does not start with a number
///
/// # Arguments
/// * `name` - The identifier to validate
///
/// # Returns
/// * `Ok(())` if the identifier is valid
/// * `Err(String)` with error message if invalid
///
/// # Examples
/// ```
/// # use reinhardt_db::orm::aggregation::validate_identifier;
/// assert!(validate_identifier("user_id").is_ok());
/// assert!(validate_identifier("name123").is_ok());
/// assert!(validate_identifier("123invalid").is_err()); // Starts with number
/// assert!(validate_identifier("user-id").is_err());     // Contains hyphen
/// assert!(validate_identifier("user; DROP TABLE").is_err()); // SQL injection attempt
/// ```
pub fn validate_identifier(name: &str) -> Result<(), String> {
	// Check for empty string
	if name.is_empty() {
		return Err("Identifier cannot be empty".to_string());
	}

	// Check for wildcard (special case - allowed)
	if name == "*" {
		return Ok(());
	}

	// Check that all characters are alphanumeric or underscore
	if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
		return Err(format!(
			"Identifier '{}' contains invalid characters. Only alphanumeric characters and underscores are allowed",
			name
		));
	}

	// Check that it doesn't start with a number
	if let Some(first_char) = name.chars().next()
		&& first_char.is_numeric()
	{
		return Err(format!("Identifier '{}' cannot start with a number", name));
	}

	Ok(())
}

impl Aggregate {
	/// Create a COUNT aggregate
	///
	/// # Panics
	/// Panics if the field name contains invalid characters
	pub fn count(field: Option<&str>) -> Self {
		if let Some(f) = field {
			validate_identifier(f).expect("Invalid field name for COUNT aggregate");
		}
		Self {
			func: AggregateFunc::Count,
			field: field.map(|s| s.to_string()),
			alias: None,
			distinct: false,
		}
	}

	/// Create a COUNT(*) aggregate
	pub fn count_all() -> Self {
		Self {
			func: AggregateFunc::Count,
			field: None,
			alias: None,
			distinct: false,
		}
	}

	/// Create a COUNT DISTINCT aggregate
	///
	/// # Panics
	/// Panics if the field name contains invalid characters
	pub fn count_distinct(field: &str) -> Self {
		validate_identifier(field).expect("Invalid field name for COUNT DISTINCT aggregate");
		Self {
			func: AggregateFunc::CountDistinct,
			field: Some(field.to_string()),
			alias: None,
			distinct: true,
		}
	}

	/// Create a SUM aggregate
	///
	/// # Panics
	/// Panics if the field name contains invalid characters
	pub fn sum(field: &str) -> Self {
		validate_identifier(field).expect("Invalid field name for SUM aggregate");
		Self {
			func: AggregateFunc::Sum,
			field: Some(field.to_string()),
			alias: None,
			distinct: false,
		}
	}

	/// Create an AVG aggregate
	///
	/// # Panics
	/// Panics if the field name contains invalid characters
	pub fn avg(field: &str) -> Self {
		validate_identifier(field).expect("Invalid field name for AVG aggregate");
		Self {
			func: AggregateFunc::Avg,
			field: Some(field.to_string()),
			alias: None,
			distinct: false,
		}
	}

	/// Create a MAX aggregate
	///
	/// # Panics
	/// Panics if the field name contains invalid characters
	pub fn max(field: &str) -> Self {
		validate_identifier(field).expect("Invalid field name for MAX aggregate");
		Self {
			func: AggregateFunc::Max,
			field: Some(field.to_string()),
			alias: None,
			distinct: false,
		}
	}

	/// Create a MIN aggregate
	///
	/// # Panics
	/// Panics if the field name contains invalid characters
	pub fn min(field: &str) -> Self {
		validate_identifier(field).expect("Invalid field name for MIN aggregate");
		Self {
			func: AggregateFunc::Min,
			field: Some(field.to_string()),
			alias: None,
			distinct: false,
		}
	}

	/// Set an alias for this aggregate
	///
	/// # Panics
	/// Panics if the alias contains invalid characters
	pub fn with_alias(mut self, alias: &str) -> Self {
		validate_identifier(alias).expect("Invalid alias name");
		self.alias = Some(alias.to_string());
		self
	}

	/// Convert to SQL string using reinhardt-query for safe identifier escaping
	pub fn to_sql(&self) -> String {
		let mut parts = Vec::new();

		// Build aggregate expression
		parts.push(self.func.to_string());
		parts.push("(".to_string());

		if self.distinct && self.field.is_some() {
			parts.push("DISTINCT ".to_string());
		}

		match &self.field {
			Some(field) => {
				// Use reinhardt-query's Alias to safely escape the identifier
				let iden = Alias::new(field);
				parts.push(iden.to_string());
			}
			None => parts.push("*".to_string()),
		}

		parts.push(")".to_string());

		if let Some(alias) = &self.alias {
			parts.push(" AS ".to_string());
			// Safely escape the alias identifier
			let alias_iden = Alias::new(alias);
			parts.push(alias_iden.to_string());
		}

		parts.join("")
	}

	/// Convert to SQL string without alias (for use in SELECT expressions with expr_as)
	/// Uses reinhardt-query for safe identifier escaping
	pub fn to_sql_expr(&self) -> String {
		let mut parts = Vec::new();

		parts.push(self.func.to_string());
		parts.push("(".to_string());

		if self.distinct && self.field.is_some() {
			parts.push("DISTINCT ".to_string());
		}

		match &self.field {
			Some(field) => {
				// Use reinhardt-query's Alias to safely escape the identifier
				let iden = Alias::new(field);
				parts.push(iden.to_string());
			}
			None => parts.push("*".to_string()),
		}

		parts.push(")".to_string());

		parts.join("")
	}
}

/// Result of an aggregation
#[derive(Debug, Clone)]
pub enum AggregateValue {
	/// Integer value
	Int(i64),
	/// Float value
	Float(f64),
	/// Null value
	Null,
}

/// Result container for aggregation queries
#[derive(Debug, Clone)]
pub struct AggregateResult {
	/// Map of alias to value
	pub values: std::collections::HashMap<String, AggregateValue>,
}

impl AggregateResult {
	/// Create a new empty result
	pub fn new() -> Self {
		Self {
			values: std::collections::HashMap::new(),
		}
	}

	/// Get a value by alias
	pub fn get(&self, alias: &str) -> Option<&AggregateValue> {
		self.values.get(alias)
	}

	/// Insert a value
	pub fn insert(&mut self, alias: String, value: AggregateValue) {
		self.values.insert(alias, value);
	}
}

impl Default for AggregateResult {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_validate_identifier_valid() {
		assert!(validate_identifier("user_id").is_ok());
		assert!(validate_identifier("name123").is_ok());
		assert!(validate_identifier("_internal").is_ok());
		assert!(validate_identifier("CamelCase").is_ok());
		assert!(validate_identifier("*").is_ok()); // Wildcard is allowed
	}

	#[test]
	fn test_validate_identifier_invalid() {
		// Starts with number
		assert!(validate_identifier("123invalid").is_err());

		// Contains invalid characters
		assert!(validate_identifier("user-id").is_err());
		assert!(validate_identifier("user.name").is_err());
		assert!(validate_identifier("user name").is_err());

		// SQL injection attempts
		assert!(validate_identifier("user; DROP TABLE").is_err());
		assert!(validate_identifier("id' OR '1'='1").is_err());
		assert!(validate_identifier("id); DELETE FROM users; --").is_err());

		// Empty string
		assert!(validate_identifier("").is_err());
	}

	#[test]
	#[should_panic(expected = "Invalid field name")]
	fn test_aggregate_rejects_invalid_field() {
		// Should panic when trying to create aggregate with SQL injection attempt
		Aggregate::sum("amount; DROP TABLE users");
	}

	#[test]
	#[should_panic(expected = "Invalid alias")]
	fn test_aggregate_rejects_invalid_alias() {
		// Should panic when trying to use invalid alias
		Aggregate::sum("amount").with_alias("total; DROP TABLE");
	}

	#[test]
	fn test_aggregate_escapes_identifiers() {
		// Test that identifiers are properly escaped using reinhardt-query
		let agg = Aggregate::sum("user_id");
		let sql = agg.to_sql();

		// The identifier should be in the output
		assert!(sql.contains("user_id"));
		// But it should be properly formatted
		assert_eq!(sql, "SUM(user_id)");
	}

	#[test]
	fn test_count_aggregate() {
		let agg = Aggregate::count(Some("id"));
		assert_eq!(agg.to_sql(), "COUNT(id)");
	}

	#[test]
	fn test_count_all_aggregate() {
		let agg = Aggregate::count_all();
		assert_eq!(agg.to_sql(), "COUNT(*)");
	}

	#[test]
	fn test_count_distinct_aggregate() {
		let agg = Aggregate::count_distinct("user_id");
		assert_eq!(agg.to_sql(), "COUNT(DISTINCT user_id)");
	}

	#[test]
	fn test_sum_aggregate() {
		let agg = Aggregate::sum("amount");
		assert_eq!(agg.to_sql(), "SUM(amount)");
	}

	#[test]
	fn test_avg_aggregate() {
		let agg = Aggregate::avg("score");
		assert_eq!(agg.to_sql(), "AVG(score)");
	}

	#[test]
	fn test_max_aggregate() {
		let agg = Aggregate::max("price");
		assert_eq!(agg.to_sql(), "MAX(price)");
	}

	#[test]
	fn test_min_aggregate() {
		let agg = Aggregate::min("age");
		assert_eq!(agg.to_sql(), "MIN(age)");
	}

	#[test]
	fn test_aggregate_with_alias() {
		let agg = Aggregate::sum("amount").with_alias("total_amount");
		assert_eq!(agg.to_sql(), "SUM(amount) AS total_amount");
	}
}
