//! SQL compiler for field lookups
//!
//! Converts Field and Lookup types into SQL WHERE clauses
//! Also compiles field comparisons for JOIN conditions and aggregate expressions for HAVING clauses

use super::comparison::{ComparisonOperator, FieldComparison, FieldRef};
use super::lookup::{Lookup, LookupType, LookupValue};
use crate::orm::Model;
use reinhardt_query::prelude::SimpleExpr;

/// Compiles field lookups into SQL
pub struct QueryFieldCompiler;

impl QueryFieldCompiler {
	/// Compile a lookup into a SQL WHERE clause
	/// Use SQLite-compatible syntax by default
	///
	pub fn compile<M: Model>(lookup: &Lookup<M>) -> String {
		Self::compile_for_sqlite(lookup)
	}

	/// Compile a lookup into a reinhardt-query `SimpleExpr`
	///
	/// This wraps the compiled SQL in `Expr::cust()` for integration with reinhardt-query conditions.
	pub fn compile_to_expr<M: Model>(lookup: &Lookup<M>) -> SimpleExpr {
		let sql = Self::compile(lookup);
		reinhardt_query::prelude::Expr::cust(sql).into_simple_expr()
	}

	/// Compile for SQLite (uses LIKE with LOWER() for case-insensitive)
	///
	pub fn compile_for_sqlite<M: Model>(lookup: &Lookup<M>) -> String {
		let lookup_type = lookup.lookup_type();

		// For case-insensitive lookups, wrap field in LOWER()
		let field_sql = match lookup_type {
			LookupType::IExact
			| LookupType::IContains
			| LookupType::IStartsWith
			| LookupType::IEndsWith => {
				let field_path = lookup.field_path();
				format!("LOWER({})", Self::compile_field_path_raw(field_path))
			}
			_ => Self::compile_field_path(lookup.field_path()),
		};

		let operator = Self::lookup_type_to_operator_sqlite(lookup_type);
		let value_sql = Self::compile_value_sqlite(lookup.value(), lookup_type);

		match lookup_type {
			LookupType::IsNull | LookupType::IsNotNull => {
				format!(
					"{} {}",
					Self::compile_field_path(lookup.field_path()),
					operator
				)
			}
			LookupType::Range => {
				if let LookupValue::Range(start, end) = &lookup.value() {
					let start_sql = Self::value_to_sql(start);
					let end_sql = Self::value_to_sql(end);
					format!(
						"{} BETWEEN {} AND {}",
						Self::compile_field_path(lookup.field_path()),
						start_sql,
						end_sql
					)
				} else {
					panic!("Range lookup requires Range value");
				}
			}
			LookupType::In | LookupType::NotIn => {
				format!(
					"{} {} ({})",
					Self::compile_field_path(lookup.field_path()),
					operator,
					value_sql
				)
			}
			_ => {
				format!("{} {} {}", field_sql, operator, value_sql)
			}
		}
	}

	/// Compile field path without transforms (raw)
	fn compile_field_path_raw(path: &[String]) -> String {
		path.iter()
			.filter(|segment| !Self::is_transform(segment))
			.map(|s| s.as_str())
			.collect::<Vec<_>>()
			.join(".")
	}

	/// Check if a path segment is a transform
	fn is_transform(segment: &str) -> bool {
		matches!(
			segment,
			"lower"
				| "upper" | "trim"
				| "length" | "year"
				| "month" | "day"
				| "week" | "weekday"
				| "quarter" | "hour"
				| "minute" | "second"
				| "date" | "abs"
				| "ceil" | "floor"
				| "round"
		)
	}

	/// Compile field path into SQL
	fn compile_field_path(path: &[String]) -> String {
		let mut transforms = Vec::new();
		let mut field_name = String::new();

		for segment in path {
			match segment.as_str() {
				// String transforms
				"lower" => transforms.push("LOWER"),
				"upper" => transforms.push("UPPER"),
				"trim" => transforms.push("TRIM"),
				"length" => transforms.push("LENGTH"),

				// DateTime transforms
				"year" => transforms.push("EXTRACT(YEAR FROM"),
				"month" => transforms.push("EXTRACT(MONTH FROM"),
				"day" => transforms.push("EXTRACT(DAY FROM"),
				"week" => transforms.push("EXTRACT(WEEK FROM"),
				"weekday" => transforms.push("EXTRACT(DOW FROM"),
				"quarter" => transforms.push("EXTRACT(QUARTER FROM"),
				"hour" => transforms.push("EXTRACT(HOUR FROM"),
				"minute" => transforms.push("EXTRACT(MINUTE FROM"),
				"second" => transforms.push("EXTRACT(SECOND FROM"),
				"date" => transforms.push("DATE"),

				// Numeric transforms
				"abs" => transforms.push("ABS"),
				"ceil" => transforms.push("CEIL"),
				"floor" => transforms.push("FLOOR"),
				"round" => transforms.push("ROUND"),

				// Otherwise, it's a field name or relation
				_ => {
					if !field_name.is_empty() {
						field_name.push('.');
					}
					field_name.push_str(segment);
				}
			}
		}

		// Apply transforms from innermost to outermost
		let mut result = field_name;
		for transform in &transforms {
			if transform.starts_with("EXTRACT") {
				result = format!("{} {})", transform, result);
			} else {
				result = format!("{}({})", transform, result);
			}
		}

		result
	}

	/// Convert lookup type to SQL operator (SQLite-compatible)
	fn lookup_type_to_operator_sqlite(lookup_type: &LookupType) -> &'static str {
		match lookup_type {
			LookupType::Exact => "=",
			LookupType::IExact => "=", // Use = with LOWER() wrapper
			LookupType::Ne => "!=",
			LookupType::Contains | LookupType::StartsWith | LookupType::EndsWith => "LIKE",
			LookupType::IContains | LookupType::IStartsWith | LookupType::IEndsWith => "LIKE", // Use LIKE with LOWER() wrapper
			LookupType::Regex => "REGEXP",  // SQLite needs regex extension
			LookupType::IRegex => "REGEXP", // Case-insensitive regex
			LookupType::Gt => ">",
			LookupType::Gte => ">=",
			LookupType::Lt => "<",
			LookupType::Lte => "<=",
			LookupType::Range => "BETWEEN",
			LookupType::In => "IN",
			LookupType::NotIn => "NOT IN",
			LookupType::IsNull => "IS NULL",
			LookupType::IsNotNull => "IS NOT NULL",
		}
	}

	/// Convert lookup type to SQL operator (PostgreSQL)
	// Allow dead_code: PostgreSQL-specific operator mapping used when PostgreSQL backend is active
	#[allow(dead_code)]
	fn lookup_type_to_operator(lookup_type: &LookupType) -> &'static str {
		match lookup_type {
			LookupType::Exact => "=",
			LookupType::IExact => "ILIKE",
			LookupType::Ne => "!=",
			LookupType::Contains | LookupType::StartsWith | LookupType::EndsWith => "LIKE",
			LookupType::IContains | LookupType::IStartsWith | LookupType::IEndsWith => "ILIKE",
			LookupType::Regex => "~",
			LookupType::IRegex => "~*",
			LookupType::Gt => ">",
			LookupType::Gte => ">=",
			LookupType::Lt => "<",
			LookupType::Lte => "<=",
			LookupType::Range => "BETWEEN",
			LookupType::In => "IN",
			LookupType::NotIn => "NOT IN",
			LookupType::IsNull => "IS NULL",
			LookupType::IsNotNull => "IS NOT NULL",
		}
	}

	/// Compile lookup value to SQL (SQLite-compatible)
	fn compile_value_sqlite(value: &LookupValue, lookup_type: &LookupType) -> String {
		match lookup_type {
			LookupType::IExact => {
				// For case-insensitive equality, lowercase the value too
				if let LookupValue::String(s) = value {
					format!("LOWER('{}')", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::Contains => {
				if let LookupValue::String(s) = value {
					format!("'%{}%'", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::IContains => {
				if let LookupValue::String(s) = value {
					format!("LOWER('%{}%')", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::StartsWith => {
				if let LookupValue::String(s) = value {
					format!("'{}%'", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::IStartsWith => {
				if let LookupValue::String(s) = value {
					format!("LOWER('{}%')", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::EndsWith => {
				if let LookupValue::String(s) = value {
					format!("'%{}'", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::IEndsWith => {
				if let LookupValue::String(s) = value {
					format!("LOWER('%{}')", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::In | LookupType::NotIn => {
				if let LookupValue::Array(items) = value {
					items
						.iter()
						.map(Self::value_to_sql)
						.collect::<Vec<_>>()
						.join(", ")
				} else {
					Self::value_to_sql(value)
				}
			}
			_ => Self::value_to_sql(value),
		}
	}

	/// Compile lookup value to SQL
	// Allow dead_code: lookup value compilation used by backend-specific query field compilers
	#[allow(dead_code)]
	fn compile_value(value: &LookupValue, lookup_type: &LookupType) -> String {
		match lookup_type {
			LookupType::Contains => {
				if let LookupValue::String(s) = value {
					format!("'%{}%'", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::IContains => {
				if let LookupValue::String(s) = value {
					format!("'%{}%'", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::StartsWith | LookupType::IStartsWith => {
				if let LookupValue::String(s) = value {
					format!("'{}%'", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::EndsWith | LookupType::IEndsWith => {
				if let LookupValue::String(s) = value {
					format!("'%{}'", Self::escape_sql_string(s))
				} else {
					Self::value_to_sql(value)
				}
			}
			LookupType::In | LookupType::NotIn => {
				if let LookupValue::Array(items) = value {
					items
						.iter()
						.map(Self::value_to_sql)
						.collect::<Vec<_>>()
						.join(", ")
				} else {
					Self::value_to_sql(value)
				}
			}
			_ => Self::value_to_sql(value),
		}
	}

	/// Convert value to SQL literal
	fn value_to_sql(value: &LookupValue) -> String {
		match value {
			LookupValue::String(s) => format!("'{}'", Self::escape_sql_string(s)),
			LookupValue::Int(i) => i.to_string(),
			LookupValue::Float(f) => f.to_string(),
			LookupValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
			LookupValue::Array(items) => {
				let values: Vec<String> = items.iter().map(Self::value_to_sql).collect();
				values.join(", ")
			}
			LookupValue::Range(_, _) => {
				// Handled specially in compile()
				String::new()
			}
			LookupValue::Null => "NULL".to_string(),
		}
	}

	/// Escape SQL string to prevent injection
	fn escape_sql_string(s: &str) -> String {
		s.replace('\'', "''")
	}

	// ========================================
	// Compile JOIN conditions and HAVING clauses
	// ========================================

	/// Convert field comparison expression to SQL
	///
	/// Converts field-to-field comparison expressions used in JOIN conditions (ON clause) to SQL.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::query_fields::comparison::*;
	/// use reinhardt_db::orm::query_fields::compiler::QueryFieldCompiler;
	///
	/// // u1.id < u2.id
	/// let comparison = FieldComparison::new(
	///     FieldRef::field_with_alias("u1".to_string(), vec!["id".to_string()]),
	///     FieldRef::field_with_alias("u2".to_string(), vec!["id".to_string()]),
	///     ComparisonOperator::Lt,
	/// );
	///
	/// let sql = QueryFieldCompiler::compile_field_comparison(&comparison);
	/// assert_eq!(sql, "u1.id < u2.id");
	/// ```
	pub fn compile_field_comparison(comparison: &FieldComparison) -> String {
		let left = Self::compile_field_ref(&comparison.left);
		let right = Self::compile_field_ref(&comparison.right);
		let op = Self::comparison_operator_to_sql(comparison.op);

		format!("{} {} {}", left, op, right)
	}

	/// Convert field reference to SQL
	fn compile_field_ref(field_ref: &FieldRef) -> String {
		match field_ref {
			FieldRef::Field {
				table_alias,
				field_path,
			} => {
				let path = field_path.join(".");
				if let Some(alias) = table_alias {
					format!("{}.{}", alias, path)
				} else {
					path
				}
			}
			FieldRef::Value(v) => v.clone(),
		}
	}

	/// Convert comparison operator to SQL
	fn comparison_operator_to_sql(op: ComparisonOperator) -> &'static str {
		match op {
			ComparisonOperator::Eq => "=",
			ComparisonOperator::Ne => "!=",
			ComparisonOperator::Gt => ">",
			ComparisonOperator::Gte => ">=",
			ComparisonOperator::Lt => "<",
			ComparisonOperator::Lte => "<=",
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::query_fields::comparison::{ComparisonOperator, FieldComparison, FieldRef};
	use crate::orm::query_fields::field::Field;
	use crate::orm::query_fields::lookup::{Lookup, LookupType, LookupValue};
	use crate::orm::query_fields::traits::{Date, DateTime};
	use crate::orm::{Manager, Model};
	use reinhardt_core::validators::TableName;

	#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	struct TestUser {
		id: i64,
		email: String,
		age: i32,
		score: f64,
		is_active: bool,
		created_at: DateTime,
	}

	const TEST_USER_TABLE: TableName = TableName::new_const("test_user");

	#[derive(Debug, Clone)]
	struct TestUserFields;

	impl crate::orm::model::FieldSelector for TestUserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestUser {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;
		fn table_name() -> &'static str {
			TEST_USER_TABLE.as_str()
		}
		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.id)
		}
		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}
		fn primary_key_field() -> &'static str {
			"id"
		}
		fn new_fields() -> Self::Fields {
			TestUserFields
		}
	}

	#[test]
	fn test_compile_simple_equality() {
		let lookup =
			Field::<TestUser, String>::new(vec!["email"]).eq("test@example.com".to_string());
		let sql = QueryFieldCompiler::compile(&lookup);
		assert_eq!(sql, "email = 'test@example.com'");
	}

	#[test]
	fn test_compile_contains() {
		let lookup = Field::<TestUser, String>::new(vec!["email"]).contains("example");
		let sql = QueryFieldCompiler::compile(&lookup);
		assert_eq!(sql, "email LIKE '%example%'");
	}

	#[test]
	fn test_compile_lower_contains() {
		let lookup = Field::<TestUser, String>::new(vec!["email"])
			.lower()
			.contains("example");
		let sql = QueryFieldCompiler::compile(&lookup);
		assert_eq!(sql, "LOWER(email) LIKE '%example%'");
	}

	#[test]
	fn test_compile_numeric_comparison() {
		let lookup = Field::<TestUser, i32>::new(vec!["age"]).gte(18);
		let sql = QueryFieldCompiler::compile(&lookup);
		assert_eq!(sql, "age >= 18");
	}

	#[test]
	fn test_compile_range() {
		let lookup = Field::<TestUser, i32>::new(vec!["age"]).in_range(18, 65);
		let sql = QueryFieldCompiler::compile(&lookup);
		assert_eq!(sql, "age BETWEEN 18 AND 65");
	}

	#[test]
	fn test_compile_is_null() {
		let lookup = Field::<TestUser, Option<String>>::new(vec!["email"]).is_null();
		let sql = QueryFieldCompiler::compile(&lookup);
		assert_eq!(sql, "email IS NULL");
	}

	#[test]
	fn test_sql_injection_prevention() {
		let lookup = Field::<TestUser, String>::new(vec!["email"])
			.eq("test'; DROP TABLE users; --".to_string());
		assert_eq!(
			QueryFieldCompiler::compile(&lookup),
			"email = 'test''; DROP TABLE users; --'"
		);
	}

	#[rstest::rstest]
	#[case("email", LookupType::Exact, LookupValue::String("alice@example.com".to_string()), "email = 'alice@example.com'")]
	#[case("email", LookupType::IExact, LookupValue::String("ADMIN".to_string()), "LOWER(email) = LOWER('ADMIN')")]
	#[case("email", LookupType::Ne, LookupValue::String("guest".to_string()), "email != 'guest'")]
	#[case("age", LookupType::Gt, LookupValue::Int(21), "age > 21")]
	#[case("age", LookupType::Gte, LookupValue::Int(21), "age >= 21")]
	#[case("age", LookupType::Lt, LookupValue::Int(65), "age < 65")]
	#[case("age", LookupType::Lte, LookupValue::Int(65), "age <= 65")]
	#[case("email", LookupType::Contains, LookupValue::String("o'reilly".to_string()), "email LIKE '%o''reilly%'")]
	#[case("email", LookupType::IContains, LookupValue::String("ADMIN".to_string()), "LOWER(email) LIKE LOWER('%ADMIN%')")]
	#[case("email", LookupType::StartsWith, LookupValue::String("admin".to_string()), "email LIKE 'admin%'")]
	#[case("email", LookupType::IStartsWith, LookupValue::String("ADMIN".to_string()), "LOWER(email) LIKE LOWER('ADMIN%')")]
	#[case("email", LookupType::EndsWith, LookupValue::String("@example.com".to_string()), "email LIKE '%@example.com'")]
	#[case("email", LookupType::IEndsWith, LookupValue::String("@EXAMPLE.COM".to_string()), "LOWER(email) LIKE LOWER('%@EXAMPLE.COM')")]
	#[case("email", LookupType::Regex, LookupValue::String("^[a-z]+$".to_string()), "email REGEXP '^[a-z]+$'")]
	#[case("email", LookupType::IRegex, LookupValue::String("^[A-Z]+$".to_string()), "email REGEXP '^[A-Z]+$'")]
	#[case(
		"age",
		LookupType::Range,
		LookupValue::Range(Box::new(LookupValue::Int(18)), Box::new(LookupValue::Int(65))),
		"age BETWEEN 18 AND 65"
	)]
	#[case("age", LookupType::In, LookupValue::Array(vec![LookupValue::Int(1), LookupValue::Int(3)]), "age IN (1, 3)")]
	#[case("email", LookupType::NotIn, LookupValue::Array(vec![LookupValue::String("o'reilly".to_string()), LookupValue::String("admin".to_string())]), "email NOT IN ('o''reilly', 'admin')")]
	#[case("score", LookupType::Exact, LookupValue::Float(9.5), "score = 9.5")]
	#[case(
		"is_active",
		LookupType::Exact,
		LookupValue::Bool(true),
		"is_active = TRUE"
	)]
	#[case("email", LookupType::IsNull, LookupValue::Null, "email IS NULL")]
	#[case("email", LookupType::IsNotNull, LookupValue::Null, "email IS NOT NULL")]
	fn sqlite_lookup_matrix(
		#[case] field: &str,
		#[case] lookup_type: LookupType,
		#[case] value: LookupValue,
		#[case] expected: &str,
	) {
		let lookup = Lookup::<TestUser>::new(vec![field.to_string()], lookup_type, value);
		assert_eq!(QueryFieldCompiler::compile_for_sqlite(&lookup), expected);
	}

	#[test]
	fn test_compile_string_transforms() {
		let upper = Field::<TestUser, String>::new(vec!["email"])
			.upper()
			.eq("ADMIN".to_string());
		assert_eq!(
			QueryFieldCompiler::compile(&upper),
			"UPPER(email) = 'ADMIN'"
		);

		let trimmed = Field::<TestUser, String>::new(vec!["email"])
			.trim()
			.contains("example");
		assert_eq!(
			QueryFieldCompiler::compile(&trimmed),
			"TRIM(email) LIKE '%example%'"
		);

		let length = Lookup::<TestUser>::new(
			vec!["email".to_string(), "length".to_string()],
			LookupType::Gte,
			LookupValue::Int(10),
		);
		assert_eq!(QueryFieldCompiler::compile(&length), "LENGTH(email) >= 10");
	}

	#[test]
	fn test_compile_datetime_transforms() {
		let year = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.year()
			.eq(2024);
		assert_eq!(
			QueryFieldCompiler::compile(&year),
			"EXTRACT(YEAR FROM created_at) = 2024"
		);

		let month = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.month()
			.eq(8);
		assert_eq!(
			QueryFieldCompiler::compile(&month),
			"EXTRACT(MONTH FROM created_at) = 8"
		);

		let day = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.day()
			.eq(6);
		assert_eq!(
			QueryFieldCompiler::compile(&day),
			"EXTRACT(DAY FROM created_at) = 6"
		);

		let week = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.week()
			.eq(32);
		assert_eq!(
			QueryFieldCompiler::compile(&week),
			"EXTRACT(WEEK FROM created_at) = 32"
		);

		let weekday = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.weekday()
			.eq(3);
		assert_eq!(
			QueryFieldCompiler::compile(&weekday),
			"EXTRACT(DOW FROM created_at) = 3"
		);

		let quarter = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.quarter()
			.eq(3);
		assert_eq!(
			QueryFieldCompiler::compile(&quarter),
			"EXTRACT(QUARTER FROM created_at) = 3"
		);

		let hour = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.hour()
			.eq(14);
		assert_eq!(
			QueryFieldCompiler::compile(&hour),
			"EXTRACT(HOUR FROM created_at) = 14"
		);

		let minute = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.minute()
			.eq(30);
		assert_eq!(
			QueryFieldCompiler::compile(&minute),
			"EXTRACT(MINUTE FROM created_at) = 30"
		);

		let second = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.second()
			.eq(45);
		assert_eq!(
			QueryFieldCompiler::compile(&second),
			"EXTRACT(SECOND FROM created_at) = 45"
		);

		let date = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.date()
			.eq(Date {
				year: 2024,
				month: 8,
				day: 6,
			});
		assert_eq!(
			QueryFieldCompiler::compile(&date),
			"DATE(created_at) = 20240806"
		);
	}

	#[test]
	fn test_compile_numeric_transforms() {
		let absolute = Field::<TestUser, f64>::new(vec!["score"]).abs().eq(2.5);
		assert_eq!(QueryFieldCompiler::compile(&absolute), "ABS(score) = 2.5");

		let ceiling = Field::<TestUser, f64>::new(vec!["score"]).ceil().eq(3.0);
		assert_eq!(QueryFieldCompiler::compile(&ceiling), "CEIL(score) = 3");

		let floor = Field::<TestUser, f64>::new(vec!["score"]).floor().eq(2.0);
		assert_eq!(QueryFieldCompiler::compile(&floor), "FLOOR(score) = 2");

		let rounded = Field::<TestUser, f64>::new(vec!["score"])
			.abs()
			.round()
			.gte(10.0);
		assert_eq!(
			QueryFieldCompiler::compile(&rounded),
			"ROUND(ABS(score)) >= 10"
		);
	}

	#[rstest::rstest]
	#[case(FieldRef::field(vec!["id".to_string()]), FieldRef::field(vec!["owner_id".to_string()]), ComparisonOperator::Eq, "id = owner_id")]
	#[case(FieldRef::field(vec!["id".to_string()]), FieldRef::field(vec!["owner_id".to_string()]), ComparisonOperator::Ne, "id != owner_id")]
	#[case(FieldRef::field(vec!["id".to_string()]), FieldRef::field(vec!["owner_id".to_string()]), ComparisonOperator::Gt, "id > owner_id")]
	#[case(FieldRef::field(vec!["id".to_string()]), FieldRef::field(vec!["owner_id".to_string()]), ComparisonOperator::Gte, "id >= owner_id")]
	#[case(FieldRef::field(vec!["id".to_string()]), FieldRef::field(vec!["owner_id".to_string()]), ComparisonOperator::Lt, "id < owner_id")]
	#[case(FieldRef::field(vec!["id".to_string()]), FieldRef::field(vec!["owner_id".to_string()]), ComparisonOperator::Lte, "id <= owner_id")]
	#[case(FieldRef::field_with_alias("left".to_string(), vec!["id".to_string()]), FieldRef::field_with_alias("right".to_string(), vec!["owner_id".to_string()]), ComparisonOperator::Eq, "left.id = right.owner_id")]
	#[case(FieldRef::field_with_alias("left".to_string(), vec!["id".to_string()]), FieldRef::field_with_alias("right".to_string(), vec!["owner_id".to_string()]), ComparisonOperator::Ne, "left.id != right.owner_id")]
	#[case(FieldRef::field_with_alias("left".to_string(), vec!["id".to_string()]), FieldRef::field_with_alias("right".to_string(), vec!["owner_id".to_string()]), ComparisonOperator::Gt, "left.id > right.owner_id")]
	#[case(FieldRef::field_with_alias("left".to_string(), vec!["id".to_string()]), FieldRef::field_with_alias("right".to_string(), vec!["owner_id".to_string()]), ComparisonOperator::Gte, "left.id >= right.owner_id")]
	#[case(FieldRef::field_with_alias("left".to_string(), vec!["id".to_string()]), FieldRef::field_with_alias("right".to_string(), vec!["owner_id".to_string()]), ComparisonOperator::Lt, "left.id < right.owner_id")]
	#[case(FieldRef::field_with_alias("left".to_string(), vec!["id".to_string()]), FieldRef::field_with_alias("right".to_string(), vec!["owner_id".to_string()]), ComparisonOperator::Lte, "left.id <= right.owner_id")]
	fn field_comparison_matrix(
		#[case] left: FieldRef,
		#[case] right: FieldRef,
		#[case] operator: ComparisonOperator,
		#[case] expected: &str,
	) {
		let comparison = FieldComparison::new(left, right, operator);
		assert_eq!(
			QueryFieldCompiler::compile_field_comparison(&comparison),
			expected
		);
	}
}
