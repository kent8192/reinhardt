//! SQL Writer helper for constructing SQL strings
//!
//! This module provides `SqlWriter` type which helps build SQL strings
//! with proper formatting, spacing, and placeholder management.
//!
use crate::value::{Value, Values};

/// SQL Writer for constructing SQL strings
///
/// This struct provides a convenient API for building SQL strings with
/// proper spacing, placeholder management, and value collection.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::backend::SqlWriter;
///
/// let mut writer = SqlWriter::new();
/// writer.push("SELECT");
/// writer.push_space();
/// writer.push_identifier("id", |s| format!("\"{}\"", s));
/// writer.push(",");
/// writer.push_space();
/// writer.push_identifier("name", |s| format!("\"{}\"", s));
///
/// let sql = writer.into_string();
/// // sql: "SELECT \"id\", \"name\""
/// ```
#[derive(Debug, Clone)]
pub struct SqlWriter {
	/// The SQL string being constructed
	sql: String,
	/// Parameter values collected during construction
	values: Values,
	/// Current parameter index (1-based for PostgreSQL)
	param_index: usize,
}

impl SqlWriter {
	/// Create a new SQL writer
	pub fn new() -> Self {
		Self {
			sql: String::new(),
			values: Values::default(),
			param_index: 1,
		}
	}

	/// Push a string to SQL
	///
	/// # Arguments
	///
	/// * `s` - The string to push
	pub fn push(&mut self, s: &str) {
		self.sql.push_str(s);
	}

	/// Push a space to SQL
	pub fn push_space(&mut self) {
		if !self.sql.is_empty() && !self.sql.ends_with(' ') {
			self.sql.push(' ');
		}
	}

	/// Push an identifier (escaped)
	///
	/// # Arguments
	///
	/// * `ident` - The identifier to push
	/// * `escape_fn` - Function to escape identifier
	pub fn push_identifier<F>(&mut self, ident: &str, escape_fn: F)
	where
		F: FnOnce(&str) -> String,
	{
		self.sql.push_str(&escape_fn(ident));
	}

	/// Push a comma separator
	pub fn push_comma(&mut self) {
		self.sql.push_str(", ");
	}

	/// Push a value placeholder and collect value
	///
	/// # Arguments
	///
	/// * `value` - The value to add
	/// * `format_fn` - Function to format placeholder
	///
	/// # Returns
	///
	/// * `Some(index)` - The parameter index used for a non-NULL value
	/// * `None` - NULL value (no parameter consumed)
	///
	/// # Note on NULL Handling
	///
	/// NULL values are inlined directly as string "NULL" without consuming a parameter
	/// index. Returns `None` for NULL values to clearly indicate that no
	/// placeholder was used. This behavior is intentional to avoid type
	/// mismatches (e.g., PostgreSQL rejects `$1::int4` for TEXT columns).
	pub fn push_value<F>(&mut self, value: Value, format_fn: F) -> Option<usize>
	where
		F: FnOnce(usize) -> String,
	{
		// Write NULL directly to avoid type mismatch with parameterized queries
		// (e.g., PostgreSQL rejects `$1::int4` for TEXT columns)
		if value.is_null() {
			self.sql.push_str("NULL");
			return None;
		}

		let index = self.param_index;
		self.sql.push_str(&format_fn(index));
		self.values.push(value);
		self.param_index += 1;
		Some(index)
	}

	/// Push a keyword (with automatic spacing)
	///
	/// # Arguments
	///
	/// * `keyword` - The SQL keyword to push
	pub fn push_keyword(&mut self, keyword: &str) {
		self.push_space();
		self.sql.push_str(keyword);
	}

	/// Get current SQL string
	pub fn sql(&self) -> &str {
		&self.sql
	}

	/// Get collected values
	pub fn values(&self) -> &Values {
		&self.values
	}

	/// Get current parameter index
	pub fn param_index(&self) -> usize {
		self.param_index
	}

	/// Consume writer and return (SQL, Values)
	pub fn finish(self) -> (String, Values) {
		(self.sql, self.values)
	}

	/// Convert to string (consuming self).
	pub fn into_string(self) -> String {
		self.sql
	}

	/// Get mutable reference to SQL string
	pub fn sql_mut(&mut self) -> &mut String {
		&mut self.sql
	}

	/// Get mutable reference to values
	pub fn values_mut(&mut self) -> &mut Values {
		&mut self.values
	}

	/// Check if SQL is empty
	pub fn is_empty(&self) -> bool {
		self.sql.is_empty()
	}

	/// Get length of SQL string
	pub fn len(&self) -> usize {
		self.sql.len()
	}

	/// Push a list of items with a separator
	///
	/// # Arguments
	///
	/// * `items` - Iterator of items
	/// * `separator` - Separator string between items
	/// * `f` - Function to write each item
	pub fn push_list<I, T, F>(&mut self, items: I, separator: &str, mut f: F)
	where
		I: IntoIterator<Item = T>,
		F: FnMut(&mut Self, T),
	{
		let mut first = true;
		for item in items {
			if !first {
				self.sql.push_str(separator);
			}
			f(self, item);
			first = false;
		}
	}

	/// Append values from another Values collection
	///
	/// This is useful when combining results from multiple queries (e.g., UNION).
	/// The values are appended and parameter index is updated accordingly.
	///
	/// # Arguments
	///
	/// * `other` - The values to append
	pub fn append_values(&mut self, other: &Values) {
		for value in other.iter() {
			self.values.push(value.clone());
		}
		self.param_index += other.len();
	}
}

impl Default for SqlWriter {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sql_writer_basic() {
		let mut writer = SqlWriter::new();
		writer.push("SELECT");
		writer.push_space();
		writer.push("*");

		assert_eq!(writer.sql(), "SELECT *");
	}

	#[test]
	fn test_sql_writer_identifier() {
		let mut writer = SqlWriter::new();
		writer.push_identifier("user", |s| format!("\"{}\"", s));

		assert_eq!(writer.sql(), "\"user\"");
	}

	#[test]
	fn test_sql_writer_value_postgres() {
		let mut writer = SqlWriter::new();
		writer.push_value(Value::Int(Some(42)), |i| format!("${}", i));
		writer.push_space();
		writer.push_value(Value::String(Some(Box::new("test".to_string()))), |i| {
			format!("${}", i)
		});

		assert_eq!(writer.sql(), "$1 $2");
		assert_eq!(writer.values().len(), 2);
	}

	#[test]
	fn test_sql_writer_value_mysql() {
		let mut writer = SqlWriter::new();
		writer.push_value(Value::Int(Some(42)), |_| "?".to_string());
		writer.push_space();
		writer.push_value(Value::String(Some(Box::new("test".to_string()))), |_| {
			"?".to_string()
		});

		assert_eq!(writer.sql(), "? ?");
		assert_eq!(writer.values().len(), 2);
	}

	#[test]
	fn test_sql_writer_keyword() {
		let mut writer = SqlWriter::new();
		writer.push("SELECT");
		writer.push_keyword("FROM");
		writer.push_keyword("WHERE");

		assert_eq!(writer.sql(), "SELECT FROM WHERE");
	}

	#[test]
	fn test_sql_writer_list() {
		let mut writer = SqlWriter::new();
		writer.push_list(vec!["a", "b", "c"], ", ", |w, item| {
			w.push_identifier(item, |s| format!("\"{}\"", s));
		});

		assert_eq!(writer.sql(), "\"a\", \"b\", \"c\"");
	}

	#[test]
	fn test_sql_writer_comma() {
		let mut writer = SqlWriter::new();
		writer.push("a");
		writer.push_comma();
		writer.push("b");

		assert_eq!(writer.sql(), "a, b");
	}

	#[test]
	fn test_sql_writer_finish() {
		let mut writer = SqlWriter::new();
		writer.push("SELECT");
		writer.push_space();
		writer.push_value(Value::Int(Some(42)), |i| format!("${}", i));

		let (sql, values) = writer.finish();
		assert_eq!(sql, "SELECT $1");
		assert_eq!(values.len(), 1);
	}
}
