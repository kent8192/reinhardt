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

/// A timezone-aware or timezone-naive timestamp returned by an aggregate.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateDateTime {
	/// A timestamp normalized to UTC by the database driver.
	Utc(chrono::DateTime<chrono::Utc>),
	/// A timestamp whose database type does not carry timezone information.
	Naive(chrono::NaiveDateTime),
}

/// A backend-neutral value returned by a terminal aggregate query.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateValue {
	/// A signed integer value.
	Integer(i64),
	/// A floating-point value.
	Float(f64),
	/// A fixed-precision decimal value.
	Decimal(rust_decimal::Decimal),
	/// A UTF-8 string value.
	String(String),
	/// A boolean value.
	Bool(bool),
	/// A binary value.
	Bytes(Vec<u8>),
	/// A native JSON value.
	Json(serde_json::Value),
	/// A UUID value.
	Uuid(uuid::Uuid),
	/// A calendar date.
	Date(chrono::NaiveDate),
	/// A time of day.
	Time(chrono::NaiveTime),
	/// A timestamp value.
	DateTime(AggregateDateTime),
	/// A SQL NULL value.
	Null,
}

impl AggregateValue {
	/// Returns the stable name used in checked-accessor diagnostics.
	pub(crate) fn variant_name(&self) -> &'static str {
		match self {
			Self::Integer(_) => "Integer",
			Self::Float(_) => "Float",
			Self::Decimal(_) => "Decimal",
			Self::String(_) => "String",
			Self::Bool(_) => "Bool",
			Self::Bytes(_) => "Bytes",
			Self::Json(_) => "Json",
			Self::Uuid(_) => "Uuid",
			Self::Date(_) => "Date",
			Self::Time(_) => "Time",
			Self::DateTime(_) => "DateTime",
			Self::Null => "Null",
		}
	}
}

/// Values returned by a terminal aggregate query in deterministic label order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AggregateResult {
	values: std::collections::BTreeMap<String, AggregateValue>,
}

impl AggregateResult {
	/// Creates an empty result container.
	pub fn new() -> Self {
		Self::default()
	}

	/// Inserts a decoded aggregate value under a validated label.
	pub fn insert(&mut self, label: impl Into<String>, value: AggregateValue) {
		self.values.insert(label.into(), value);
	}

	/// Returns a checked value by label.
	pub fn get(&self, label: &str) -> reinhardt_core::exception::Result<&AggregateValue> {
		self.values.get(label).ok_or_else(|| {
			reinhardt_core::exception::Error::Serialization(format!(
				"aggregate result does not contain label '{label}'"
			))
		})
	}

	/// Iterates values in lexical label order.
	pub fn iter(&self) -> impl Iterator<Item = (&str, &AggregateValue)> {
		self.values
			.iter()
			.map(|(label, value)| (label.as_str(), value))
	}

	/// Returns an integer value, failing if the label is absent or has another type.
	pub fn get_i64(&self, label: &str) -> reinhardt_core::exception::Result<i64> {
		match self.get(label)? {
			AggregateValue::Integer(value) => Ok(*value),
			value => Err(accessor_type_error(label, value, "Integer")),
		}
	}

	/// Returns a floating-point value, failing if the label is absent or has another type.
	pub fn get_f64(&self, label: &str) -> reinhardt_core::exception::Result<f64> {
		match self.get(label)? {
			AggregateValue::Float(value) => Ok(*value),
			value => Err(accessor_type_error(label, value, "Float")),
		}
	}

	/// Returns a decimal value, failing if the label is absent or has another type.
	pub fn get_decimal(
		&self,
		label: &str,
	) -> reinhardt_core::exception::Result<rust_decimal::Decimal> {
		match self.get(label)? {
			AggregateValue::Decimal(value) => Ok(*value),
			value => Err(accessor_type_error(label, value, "Decimal")),
		}
	}

	/// Returns the number of decoded labels.
	pub fn len(&self) -> usize {
		self.values.len()
	}

	/// Returns whether no aggregate values were decoded.
	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}
}

fn accessor_type_error(
	label: &str,
	value: &AggregateValue,
	expected: &str,
) -> reinhardt_core::exception::Error {
	reinhardt_core::exception::Error::Serialization(format!(
		"aggregate label '{label}' contains {}, expected {expected}",
		value.variant_name()
	))
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::exception::Error;

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

	#[test]
	fn aggregate_result_iterates_labels_lexically() {
		let mut result = AggregateResult::new();
		result.insert("zeta", AggregateValue::String("z".to_owned()));
		result.insert("alpha", AggregateValue::Integer(1));

		let labels = result.iter().map(|(label, _)| label).collect::<Vec<_>>();
		assert_eq!(labels, vec!["alpha", "zeta"]);
	}

	#[test]
	fn aggregate_result_reports_missing_label_and_checked_types() {
		let mut result = AggregateResult::new();
		result.insert(
			"total",
			AggregateValue::Decimal(rust_decimal::Decimal::new(425, 2)),
		);

		let missing = result
			.get_i64("missing")
			.expect_err("missing labels must fail");
		assert!(matches!(
			missing,
			Error::Serialization(message)
				if message == "aggregate result does not contain label 'missing'"
		));
		let wrong = result
			.get_i64("total")
			.expect_err("wrong variants must fail");
		assert!(matches!(
			wrong,
			Error::Serialization(message)
				if message == "aggregate label 'total' contains Decimal, expected Integer"
		));
	}

	#[test]
	fn aggregate_result_checked_numeric_accessors_return_values() {
		let mut result = AggregateResult::new();
		result.insert("count", AggregateValue::Integer(4));
		result.insert("average", AggregateValue::Float(4.25));
		result.insert(
			"total",
			AggregateValue::Decimal(rust_decimal::Decimal::new(425, 2)),
		);

		assert_eq!(result.get_i64("count").expect("integer value"), 4);
		assert_eq!(result.get_f64("average").expect("float value"), 4.25);
		assert_eq!(
			result.get_decimal("total").expect("decimal value"),
			rust_decimal::Decimal::new(425, 2)
		);
	}
}
