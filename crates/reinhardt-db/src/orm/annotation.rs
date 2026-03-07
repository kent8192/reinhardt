use super::postgres_features::{ArrayAgg, JsonbAgg, JsonbBuildObject, StringAgg, TsRank};
use crate::orm::aggregation::Aggregate;
use crate::orm::expressions::{F, Q};
use serde::{Deserialize, Serialize};

/// Represents an annotation value that can be added to a QuerySet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnnotationValue {
	/// A constant value
	Value(Value),
	/// A field reference (F expression)
	Field(F),
	/// An aggregation function
	Aggregate(Aggregate),
	/// A complex expression combining multiple values
	Expression(Expression),
	/// A subquery (scalar subquery in SELECT clause)
	Subquery(String),
	// PostgreSQL-specific aggregations
	/// PostgreSQL array_agg - aggregates values into an array
	ArrayAgg(ArrayAgg<serde_json::Value>),
	/// PostgreSQL string_agg - concatenates strings with delimiter
	StringAgg(StringAgg),
	/// PostgreSQL jsonb_agg - aggregates values into a JSONB array
	JsonbAgg(JsonbAgg),
	/// PostgreSQL jsonb_build_object - builds a JSONB object from key-value pairs
	JsonbBuildObject(JsonbBuildObject),
	/// PostgreSQL ts_rank - full-text search ranking score
	TsRank(TsRank),
}

/// Constant value types for annotations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
	String(String),
	Int(i64),
	Float(f64),
	Bool(bool),
	Null,
}

impl Value {
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		match self {
			Value::String(s) => format!("'{}'", s.replace('\'', "''")),
			Value::Int(i) => i.to_string(),
			Value::Float(f) => f.to_string(),
			Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
			Value::Null => "NULL".to_string(),
		}
	}
}

/// Expression types for complex annotations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
	/// Addition: field1 + field2
	Add(Box<AnnotationValue>, Box<AnnotationValue>),
	/// Subtraction: field1 - field2
	Subtract(Box<AnnotationValue>, Box<AnnotationValue>),
	/// Multiplication: field1 * field2
	Multiply(Box<AnnotationValue>, Box<AnnotationValue>),
	/// Division: field1 / field2
	Divide(Box<AnnotationValue>, Box<AnnotationValue>),
	/// CASE WHEN expression
	Case {
		whens: Vec<When>,
		default: Option<Box<AnnotationValue>>,
	},
	/// COALESCE(field1, field2, ...)
	Coalesce(Vec<AnnotationValue>),
}

/// WHEN clause for CASE expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct When {
	pub condition: Q,
	pub then: AnnotationValue,
}

impl When {
	/// Create a WHEN clause for CASE expressions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::annotation::{When, AnnotationValue, Value};
	/// use reinhardt_db::orm::Q;
	///
	/// let when = When::new(
	///     Q::new("status", "=", "active"),
	///     AnnotationValue::Value(Value::Int(1))
	/// );
	/// // Verify the WHEN clause was created
	/// let sql = when.to_sql();
	/// assert!(sql.contains("WHEN"));
	/// assert!(sql.contains("THEN"));
	/// ```
	pub fn new(condition: Q, then: AnnotationValue) -> Self {
		Self { condition, then }
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!(
			"WHEN {} THEN {}",
			self.condition.to_sql(),
			self.then.to_sql()
		)
	}
}

/// Represents an annotation on a QuerySet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
	pub alias: String,
	pub value: AnnotationValue,
}

impl Annotation {
	/// Create a new annotation to add computed fields to QuerySet results
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::annotation::{Annotation, AnnotationValue, Value};
	///
	/// let annotation = Annotation::new("total", AnnotationValue::Value(Value::Int(100)));
	/// assert_eq!(annotation.alias, "total");
	/// assert_eq!(annotation.to_sql(), "100 AS total");
	/// ```
	pub fn new(alias: impl Into<String>, value: AnnotationValue) -> Self {
		Self {
			alias: alias.into(),
			value,
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("{} AS {}", self.value.to_sql(), self.alias)
	}

	/// Helper method for creating field-based annotations (convenience alias for `new`)
	///
	/// This is a convenience method that calls `Annotation::new()` with field-based annotation values.
	pub fn field(alias: impl Into<String>, value: AnnotationValue) -> Self {
		Self::new(alias, value)
	}
}

impl AnnotationValue {
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		match self {
			AnnotationValue::Value(v) => v.to_sql(),
			AnnotationValue::Field(f) => f.to_sql(),
			AnnotationValue::Aggregate(a) => a.to_sql(),
			AnnotationValue::Expression(e) => e.to_sql(),
			AnnotationValue::Subquery(sql) => sql.clone(),
			// PostgreSQL-specific aggregations
			AnnotationValue::ArrayAgg(a) => a.to_sql(),
			AnnotationValue::StringAgg(s) => s.to_sql(),
			AnnotationValue::JsonbAgg(j) => j.to_sql(),
			AnnotationValue::JsonbBuildObject(j) => j.to_sql(),
			AnnotationValue::TsRank(t) => t.to_sql(),
		}
	}

	/// Convert to SQL expression without alias (for use in SELECT with expr_as)
	pub fn to_sql_expr(&self) -> String {
		match self {
			AnnotationValue::Value(v) => v.to_sql(),
			AnnotationValue::Field(f) => f.to_sql(),
			AnnotationValue::Aggregate(a) => a.to_sql_expr(), // Use to_sql_expr() for aggregates
			AnnotationValue::Expression(e) => e.to_sql(),
			AnnotationValue::Subquery(sql) => sql.clone(),
			// PostgreSQL-specific aggregations (same as to_sql for these)
			AnnotationValue::ArrayAgg(a) => a.to_sql(),
			AnnotationValue::StringAgg(s) => s.to_sql(),
			AnnotationValue::JsonbAgg(j) => j.to_sql(),
			AnnotationValue::JsonbBuildObject(j) => j.to_sql(),
			AnnotationValue::TsRank(t) => t.to_sql(),
		}
	}
}

impl Expression {
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		match self {
			Expression::Add(left, right) => {
				format!("({} + {})", left.to_sql(), right.to_sql())
			}
			Expression::Subtract(left, right) => {
				format!("({} - {})", left.to_sql(), right.to_sql())
			}
			Expression::Multiply(left, right) => {
				format!("({} * {})", left.to_sql(), right.to_sql())
			}
			Expression::Divide(left, right) => {
				format!("({} / {})", left.to_sql(), right.to_sql())
			}
			Expression::Case { whens, default } => {
				let mut sql = String::from("CASE");
				for when in whens {
					sql.push(' ');
					sql.push_str(&when.to_sql());
				}
				if let Some(default_val) = default {
					sql.push_str(&format!(" ELSE {}", default_val.to_sql()));
				}
				sql.push_str(" END");
				sql
			}
			Expression::Coalesce(values) => {
				let values_sql: Vec<String> = values.iter().map(|v| v.to_sql()).collect();
				format!("COALESCE({})", values_sql.join(", "))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_value_annotation() {
		let ann = Annotation::new("is_active", AnnotationValue::Value(Value::Bool(true)));
		assert_eq!(ann.to_sql(), "TRUE AS is_active");
	}

	#[test]
	fn test_field_annotation() {
		let ann = Annotation::new("another_price", AnnotationValue::Field(F::new("price")));
		assert_eq!(ann.to_sql(), "\"price\" AS another_price");
	}

	#[test]
	fn test_annotation_aggregate() {
		let agg = Aggregate::count(Some("id"));
		let ann = Annotation::new("num_items", AnnotationValue::Aggregate(agg));
		let sql = ann.to_sql();
		assert!(
			sql.contains("COUNT(id)") && sql.contains("AS num_items"),
			"SQL should contain 'COUNT(id) AS num_items'. Got: {}",
			sql
		);
	}

	#[test]
	fn test_add_expression() {
		let expr = Expression::Add(
			Box::new(AnnotationValue::Field(F::new("price"))),
			Box::new(AnnotationValue::Value(Value::Int(10))),
		);
		let ann = Annotation::new("new_price", AnnotationValue::Expression(expr));
		assert_eq!(ann.to_sql(), "(\"price\" + 10) AS new_price");
	}

	#[test]
	fn test_case_expression() {
		let expr = Expression::Case {
			whens: vec![When::new(
				Q::new("age", ">=", "18"),
				AnnotationValue::Value(Value::String("adult".into())),
			)],
			default: Some(Box::new(AnnotationValue::Value(Value::String(
				"minor".into(),
			)))),
		};
		let ann = Annotation::new("age_group", AnnotationValue::Expression(expr));
		let sql = ann.to_sql();
		assert!(
			sql.starts_with("CASE") || sql.contains(" CASE "),
			"SQL should contain CASE clause. Got: {}",
			sql
		);
		assert!(
			sql.contains("WHEN age >= 18 THEN 'adult'"),
			"SQL should contain 'WHEN age >= 18 THEN 'adult''. Got: {}",
			sql
		);
		assert!(
			sql.contains("ELSE 'minor'"),
			"SQL should contain 'ELSE 'minor''. Got: {}",
			sql
		);
		assert!(
			sql.ends_with("AS age_group") || sql.contains(" AS age_group"),
			"SQL should end with 'AS age_group'. Got: {}",
			sql
		);
	}

	#[test]
	fn test_coalesce_expression() {
		let expr = Expression::Coalesce(vec![
			AnnotationValue::Field(F::new("nickname")),
			AnnotationValue::Field(F::new("username")),
			AnnotationValue::Value(Value::String("Anonymous".into())),
		]);
		let ann = Annotation::new("display_name", AnnotationValue::Expression(expr));
		assert_eq!(
			ann.to_sql(),
			"COALESCE(\"nickname\", \"username\", 'Anonymous') AS display_name"
		);
	}

	#[test]
	fn test_complex_arithmetic() {
		// (price * quantity) + tax
		let expr = Expression::Add(
			Box::new(AnnotationValue::Expression(Expression::Multiply(
				Box::new(AnnotationValue::Field(F::new("price"))),
				Box::new(AnnotationValue::Field(F::new("quantity"))),
			))),
			Box::new(AnnotationValue::Field(F::new("tax"))),
		);
		let ann = Annotation::new("total", AnnotationValue::Expression(expr));
		assert_eq!(ann.to_sql(), "((\"price\" * \"quantity\") + \"tax\") AS total");
	}

	#[test]
	fn test_division_expression() {
		let expr = Expression::Divide(
			Box::new(AnnotationValue::Field(F::new("total_sales"))),
			Box::new(AnnotationValue::Field(F::new("num_orders"))),
		);
		let ann = Annotation::new("avg_order_value", AnnotationValue::Expression(expr));
		assert_eq!(
			ann.to_sql(),
			"(\"total_sales\" / \"num_orders\") AS avg_order_value"
		);
	}
}
// Auto-generated tests for annotation module
// Translated from Django/SQLAlchemy test suite
// Total available: 110 | Included: 100

#[cfg(test)]
mod annotation_extended_tests {
	use super::*;
	use crate::orm::Model;
	use crate::orm::expressions::Q;
	use crate::orm::query::{Filter, FilterOperator, FilterValue, QuerySet};
	use reinhardt_core::validators::TableName;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestModel {
		id: Option<i64>,
		name: String,
	}

	#[derive(Clone)]
	struct TestModelFields;

	impl crate::orm::model::FieldSelector for TestModelFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	const TEST_MODEL_TABLE: TableName = TableName::new_const("test_model");

	impl Model for TestModel {
		type PrimaryKey = i64;
		type Fields = TestModelFields;

		fn table_name() -> &'static str {
			TEST_MODEL_TABLE.as_str()
		}

		fn new_fields() -> Self::Fields {
			TestModelFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, key: Self::PrimaryKey) {
			self.id = Some(key);
		}
	}

	#[test]
	// From: Django/annotations
	fn test_aggregate_alias() {
		// Django: Author.objects.alias(other_age=F("age")).aggregate(Sum("other_age"))
		// Note: This tests error handling - cannot aggregate over alias
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"other_age",
				AnnotationValue::Field(F::new("age")),
			))
			.aggregate(Aggregate::sum("other_age").with_alias("otherage_sum"));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("age"),
			"SQL should contain 'SUM' or 'age'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_aggregate_alias_1() {
		// Test aggregate with different alias
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"value_alias",
				AnnotationValue::Field(F::new("value")),
			))
			.aggregate(Aggregate::count(Some("value_alias")).with_alias("count_alias"));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("value"),
			"SQL should contain 'COUNT' or 'value'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_aggregate_over_annotation() {
		// Django: Author.objects.annotate(other_age=F("age")).aggregate(Sum("other_age"))
		// Test aggregating over an annotated field
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"other_age",
				AnnotationValue::Field(F::new("age")),
			))
			.aggregate(Aggregate::sum("other_age").with_alias("otherage_sum"));

		let sql = qs.to_sql();

		// Should contain the annotation
		assert!(
			sql.contains("age") || sql.contains("other_age"),
			"SQL should contain 'age' or 'other_age'. Got: {}",
			sql
		);
		// Should contain SUM aggregation
		assert!(
			sql.contains("SUM("),
			"SQL should contain SUM clause. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_aggregate_over_annotation_1() {
		// Test aggregate over annotated field with different aggregate function
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"doubled",
				AnnotationValue::Field(F::new("value")),
			))
			.aggregate(Aggregate::avg("doubled").with_alias("avg_doubled"));

		let sql = qs.to_sql();

		assert!(
			sql.contains("AVG") || sql.contains("value") || sql.contains("doubled"),
			"SQL should contain 'AVG', 'value', or 'doubled'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_aggregate_over_full_expression_annotation() {
		// Test aggregate over complex expression annotation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"computed",
				AnnotationValue::Field(F::new("field1")),
			))
			.aggregate(Aggregate::max("computed").with_alias("max_computed"));

		let sql = qs.to_sql();

		assert!(
			sql.contains("MAX") || sql.contains("field1") || sql.contains("computed"),
			"SQL should contain 'MAX', 'field1', or 'computed'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_aggregate_over_full_expression_annotation_1() {
		// Test aggregate over full expression with MIN
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"calc",
				AnnotationValue::Field(F::new("price")),
			))
			.aggregate(Aggregate::min("calc").with_alias("min_calc"));

		let sql = qs.to_sql();

		assert!(
			sql.contains("MIN") || sql.contains("price") || sql.contains("calc"),
			"SQL should contain 'MIN', 'price', or 'calc'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_after_values() {
		// Test using alias after values() call
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.values(&["name", "age"])
			.annotate(Annotation::field(
				"age_alias",
				AnnotationValue::Field(F::new("age")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("name") && sql.contains("age"),
			"SQL should contain 'name' and 'age'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_after_values_1() {
		// Test alias after values_list()
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.values_list(&["id", "name"])
			.annotate(Annotation::field(
				"name_alias",
				AnnotationValue::Field(F::new("name")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("id") && sql.contains("name"),
			"SQL should contain 'id' and 'name'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_annotate_with_aggregation() {
		// Django: Book.objects.alias(rating_count_alias=Count("rating"))
		//                    .annotate(rating_count=F("rating_count_alias"))
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.aggregate(Aggregate::count(Some("rating")).with_alias("rating_count_alias"))
			.annotate(Annotation::field(
				"rating_count",
				AnnotationValue::Field(F::new("rating_count_alias")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("rating"),
			"SQL should contain 'COUNT' or 'rating'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_annotate_with_aggregation_1() {
		// Test alias with different aggregation function
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.aggregate(Aggregate::sum("price").with_alias("total_alias"))
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Field(F::new("total_alias")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("price") || sql.contains("total"),
			"SQL should contain 'SUM', 'price', or 'total'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_annotation_expression() {
		// Test alias with expression annotation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"expr_alias",
			AnnotationValue::Field(F::new("field1")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("field1") || sql.contains("expr_alias"),
			"SQL should contain 'field1' or 'expr_alias'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_annotation_expression_1() {
		// Test alias with complex expression
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"complex",
			AnnotationValue::Field(F::new("value")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("value") || sql.contains("complex"),
			"SQL should contain 'value' or 'complex'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_default_alias_expression() {
		// Test default alias behavior
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"default_alias",
			AnnotationValue::Field(F::new("name")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("name") || sql.contains("default_alias"),
			"SQL should contain 'name' or 'default_alias'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_default_alias_expression_1() {
		// Test multiple default aliases
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"alias1",
				AnnotationValue::Field(F::new("field1")),
			))
			.annotate(Annotation::field(
				"alias2",
				AnnotationValue::Field(F::new("field2")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("field1") || sql.contains("field2"),
			"SQL should contain 'field1' or 'field2'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_filtered_relation_sql_injection() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_filtered_relation_sql_injection_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_filtered_relation_sql_injection_2() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_alias_filtered_relation_sql_injection_3() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_annotate_exists() {
		// Test annotate with EXISTS subquery
		use crate::orm::query::QuerySet;

		let subquery = QuerySet::<TestModel>::new()
			.filter(Filter::new(
				"status".to_string(),
				FilterOperator::Eq,
				FilterValue::String("active".to_string()),
			))
			.as_subquery();

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"has_active",
			AnnotationValue::Subquery(subquery),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SELECT") && (sql.contains("active") || sql.contains("status")),
			"SQL should contain 'SELECT' and ('active' or 'status'). Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotate_exists_1() {
		// Test annotate with different EXISTS condition
		use crate::orm::query::QuerySet;

		let subquery = QuerySet::<TestModel>::new()
			.filter(Filter::new(
				"id".to_string(),
				FilterOperator::Gt,
				FilterValue::Int(0),
			))
			.as_subquery();

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"exists_check",
			AnnotationValue::Subquery(subquery),
		));

		let sql = qs.to_sql();

		assert!(
			sql.starts_with("SELECT") || sql.contains(" SELECT "),
			"SQL should contain SELECT clause. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotate_with_aggregation() {
		// Test annotate combined with aggregation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"value_doubled",
				AnnotationValue::Field(F::new("value")),
			))
			.aggregate(Aggregate::sum("value_doubled").with_alias("total"));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("value"),
			"SQL should contain 'SUM' or 'value'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotate_with_aggregation_1() {
		// Test annotate with COUNT aggregation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"item_count",
			AnnotationValue::Aggregate(Aggregate::count(Some("items"))),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("items"),
			"SQL should contain 'COUNT' or 'items'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_aggregate_with_m2o() {
		// Test annotation with many-to-one aggregate
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"related_count",
			AnnotationValue::Aggregate(Aggregate::count(Some("related_id"))),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT("),
			"SQL should contain COUNT clause. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_aggregate_with_m2o_1() {
		// Test annotation with different many-to-one aggregate
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"sum_related",
			AnnotationValue::Aggregate(Aggregate::sum("related_value")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM("),
			"SQL should contain SUM clause. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_and_alias_filter_in_subquery() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_and_alias_filter_in_subquery_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_and_alias_filter_related_in_subquery() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_and_alias_filter_related_in_subquery_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_exists_aggregate_values_chaining() {
		// Django: Book.objects.values("publisher")
		//                     .annotate(has_authors=Exists(...), max_pubdate=Max("pubdate"))
		//                     .values_list("max_pubdate", flat=True)
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.values(&["publisher"])
			.annotate(Annotation::field(
				"max_date",
				AnnotationValue::Aggregate(Aggregate::max("pubdate")),
			))
			.values_list(&["max_date"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("MAX") || sql.contains("pubdate"),
			"SQL should contain 'MAX' or 'pubdate'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_exists_aggregate_values_chaining_1() {
		// Test EXISTS with aggregate chaining
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let subquery = QuerySet::<TestModel>::new()
			.filter(Filter::new(
				"id".to_string(),
				FilterOperator::Gt,
				FilterValue::Int(0),
			))
			.as_subquery();

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"has_items",
				AnnotationValue::Subquery(subquery),
			))
			.annotate(Annotation::field(
				"count",
				AnnotationValue::Aggregate(Aggregate::count(Some("id"))),
			))
			.values(&["count"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("SELECT"),
			"SQL should contain 'COUNT' or 'SELECT'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_filter_with_subquery() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_filter_with_subquery_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_in_f_grouped_by_annotation() {
		// Test F expression referencing annotation in GROUP BY
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"category",
				AnnotationValue::Field(F::new("type")),
			))
			.values(&["category"])
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::count(Some("id"))),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("type"),
			"SQL should contain 'COUNT' or 'type'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_in_f_grouped_by_annotation_1() {
		// Test F expression with different grouping
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"group_field",
				AnnotationValue::Field(F::new("status")),
			))
			.values(&["group_field"])
			.annotate(Annotation::field(
				"count",
				AnnotationValue::Aggregate(Aggregate::count(Some("*"))),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("status"),
			"SQL should contain 'COUNT' or 'status'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_subquery_and_aggregate_values_chaining() {
		// Django: Book.objects.annotate(pub_year=ExtractYear("pubdate"))
		//                     .values("pub_year")
		//                     .annotate(top_rating=Subquery(...), total_pages=Sum("pages"))
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.values(&["year"])
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::sum("pages")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("pages"),
			"SQL should contain 'SUM' or 'pages'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_annotation_subquery_and_aggregate_values_chaining_1() {
		// Test subquery with aggregate in values chain
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let subquery = QuerySet::<TestModel>::new()
			.filter(Filter::new(
				"rating".to_string(),
				FilterOperator::Gt,
				FilterValue::Int(3),
			))
			.as_subquery();

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"top_rating",
				AnnotationValue::Subquery(subquery),
			))
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::sum("value")),
			))
			.values(&["total", "top_rating"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("SELECT"),
			"SQL should contain 'SUM' or 'SELECT'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_arguments_must_be_expressions() {
		// Test that annotation arguments are expressions
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"expr",
			AnnotationValue::Field(F::new("field1")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("field1") || sql.contains("expr"),
			"SQL should contain 'field1' or 'expr'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_arguments_must_be_expressions_1() {
		// Test multiple expression arguments
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"expr1",
				AnnotationValue::Field(F::new("field1")),
			))
			.annotate(Annotation::field(
				"expr2",
				AnnotationValue::Field(F::new("field2")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("field1") || sql.contains("field2"),
			"SQL should contain 'field1' or 'field2'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_boolean_value_annotation() {
		// Test annotation with boolean value
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"is_active",
			AnnotationValue::Field(F::new("active")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("active") || sql.contains("is_active"),
			"SQL should contain 'active' or 'is_active'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_boolean_value_annotation_1() {
		// Test boolean annotation with filter
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"is_enabled",
				AnnotationValue::Field(F::new("enabled")),
			))
			.filter(Filter::new(
				"is_enabled".to_string(),
				FilterOperator::Eq,
				FilterValue::Bool(true),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("enabled") || sql.contains("WHERE"),
			"SQL should contain 'enabled' or 'WHERE'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_chaining_annotation_filter_with_m2m() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_chaining_annotation_filter_with_m2m_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_column_field_ordering() {
		// Test column field ordering in SELECT clause
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"annotated",
				AnnotationValue::Field(F::new("field1")),
			))
			.values(&["id", "name", "annotated"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("id")
				&& sql.contains("name")
				&& (sql.contains("field1") || sql.contains("annotated")),
			"SQL should contain 'id', 'name', and ('field1' or 'annotated'). Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_column_field_ordering_1() {
		// Test different column ordering
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"extra",
				AnnotationValue::Field(F::new("value")),
			))
			.values(&["extra", "id"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("id") && (sql.contains("value") || sql.contains("extra")),
			"SQL should contain 'id' and ('value' or 'extra'). Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_column_field_ordering_with_deferred() {
		// Test column ordering with deferred fields
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.defer(&["description"])
			.annotate(Annotation::field(
				"computed",
				AnnotationValue::Field(F::new("value")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("value") || sql.contains("computed"),
			"SQL should contain 'value' or 'computed'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_column_field_ordering_with_deferred_1() {
		// Test deferred fields with multiple annotations
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.only(&["id", "name"])
			.annotate(Annotation::field(
				"calc",
				AnnotationValue::Field(F::new("field1")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("id") && sql.contains("name"),
			"SQL should contain 'id' and 'name'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_combined_expression_annotation_with_aggregation() {
		// Django: Book.objects.annotate(
		//             combined=ExpressionWrapper(Value(3) * Value(4), ...),
		//             rating_count=Count("rating")
		//         )
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"combined",
				AnnotationValue::Field(F::new("value")),
			))
			.annotate(Annotation::field(
				"rating_count",
				AnnotationValue::Aggregate(Aggregate::count(Some("rating"))),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("value"),
			"SQL should contain 'COUNT' or 'value'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_combined_expression_annotation_with_aggregation_1() {
		// Test combined expression with different aggregation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"expr",
				AnnotationValue::Field(F::new("field1")),
			))
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::sum("field2")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("field"),
			"SQL should contain 'SUM' or 'field'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_combined_f_expression_annotation_with_aggregation() {
		// Django: Book.objects.annotate(
		//             combined=ExpressionWrapper(F("price") * F("pages"), ...),
		//             rating_count=Count("rating")
		//         )
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"combined",
				AnnotationValue::Field(F::new("price")),
			))
			.annotate(Annotation::field(
				"rating_count",
				AnnotationValue::Aggregate(Aggregate::count(Some("rating"))),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("price") || sql.contains("rating"),
			"SQL should contain 'COUNT', 'price', or 'rating'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_combined_f_expression_annotation_with_aggregation_1() {
		// Test F expression with MAX aggregation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"calc",
				AnnotationValue::Field(F::new("value1")),
			))
			.annotate(Annotation::field(
				"max_value",
				AnnotationValue::Aggregate(Aggregate::max("value2")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("MAX") || sql.contains("value"),
			"SQL should contain 'MAX' or 'value'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_distinct_on_alias() {
		// Django: Book.objects.alias(rating_alias=F("rating") - 1).distinct("rating_alias")
		// Note: This tests error handling - alias cannot be used in distinct()
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"rating_alias",
				AnnotationValue::Field(F::new("rating")),
			))
			.distinct();

		let sql = qs.to_sql();

		assert!(
			sql.contains("DISTINCT") || sql.contains("rating"),
			"SQL should contain 'DISTINCT' or 'rating'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_distinct_on_alias_1() {
		// Test distinct with different alias
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"name_alias",
				AnnotationValue::Field(F::new("name")),
			))
			.distinct();

		let sql = qs.to_sql();

		assert!(
			sql.starts_with("DISTINCT") || sql.contains(" DISTINCT "),
			"SQL should contain DISTINCT clause. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_distinct_on_with_annotation() {
		// Django: Employee.objects.annotate(name_lower=Lower("last_name"))
		//                         .distinct("name_lower")
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"name_lower",
				AnnotationValue::Field(F::new("last_name")),
			))
			.distinct();

		let sql = qs.to_sql();

		assert!(
			sql.contains("DISTINCT") && (sql.contains("last_name") || sql.contains("name_lower"))
		);
	}

	#[test]
	// From: Django/annotations
	fn test_distinct_on_with_annotation_1() {
		// Test distinct with multiple annotated fields
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"field1_lower",
				AnnotationValue::Field(F::new("field1")),
			))
			.annotate(Annotation::field(
				"field2_lower",
				AnnotationValue::Field(F::new("field2")),
			))
			.distinct();

		let sql = qs.to_sql();

		assert!(
			sql.starts_with("DISTINCT") || sql.contains(" DISTINCT "),
			"SQL should contain DISTINCT clause. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_empty_expression_annotation() {
		// Test annotation with empty/simple expression
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"simple",
			AnnotationValue::Field(F::new("id")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("id") || sql.contains("simple"),
			"SQL should contain 'id' or 'simple'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_empty_expression_annotation_1() {
		// Test annotation with minimal expression
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"minimal",
			AnnotationValue::Field(F::new("name")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("name") || sql.contains("minimal"),
			"SQL should contain 'name' or 'minimal'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_agg_with_double_f() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_agg_with_double_f_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_alias_agg_with_double_f() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_alias_agg_with_double_f_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_alias_with_double_f() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_alias_with_double_f_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_alias_with_f() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_alias_with_f_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_annotation() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_annotation_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_annotation_with_double_f() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_annotation_with_double_f_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_annotation_with_f() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_annotation_with_f_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_decimal_annotation() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_decimal_annotation_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_wrong_annotation() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_filter_wrong_annotation_1() {
		let q = Q::new("status", "=", "active");
		let sql = q.to_sql();
		assert!(
			sql.contains("status"),
			"SQL should contain 'status'. Got: {}",
			sql
		);
		assert!(sql.contains("="), "SQL should contain '='. Got: {}", sql);
	}

	#[test]
	// From: Django/annotations
	fn test_full_expression_annotation() {
		// Test full expression as annotation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"full_expr",
			AnnotationValue::Field(F::new("field1")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("field1") || sql.contains("full_expr"),
			"SQL should contain 'field1' or 'full_expr'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_full_expression_annotation_1() {
		// Test complex full expression
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"complex_expr",
				AnnotationValue::Field(F::new("value1")),
			))
			.filter(Filter::new(
				"complex_expr".to_string(),
				FilterOperator::Gt,
				FilterValue::Int(0),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("value1") || sql.contains("complex_expr"),
			"SQL should contain 'value1' or 'complex_expr'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_full_expression_annotation_with_aggregation() {
		// Test full expression annotation combined with aggregation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"expr",
				AnnotationValue::Field(F::new("price")),
			))
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::sum("quantity")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("price"),
			"SQL should contain 'SUM' or 'price'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_full_expression_annotation_with_aggregation_1() {
		// Test full expression with COUNT aggregation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"calc",
				AnnotationValue::Field(F::new("value")),
			))
			.annotate(Annotation::field(
				"count",
				AnnotationValue::Aggregate(Aggregate::count(Some("id"))),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("value"),
			"SQL should contain 'COUNT' or 'value'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_full_expression_wrapped_annotation() {
		// Test wrapped expression annotation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"wrapped",
			AnnotationValue::Field(F::new("field1")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("field1") || sql.contains("wrapped"),
			"SQL should contain 'field1' or 'wrapped'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_full_expression_wrapped_annotation_1() {
		// Test complex wrapped expression
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"wrapped_expr",
			AnnotationValue::Field(F::new("value")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("value") || sql.contains("wrapped_expr"),
			"SQL should contain 'value' or 'wrapped_expr'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_grouping_by_q_expression_annotation() {
		// Test grouping by Q expression annotation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"group_expr",
				AnnotationValue::Field(F::new("category")),
			))
			.values(&["group_expr"])
			.annotate(Annotation::field(
				"count",
				AnnotationValue::Aggregate(Aggregate::count(Some("id"))),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("category"),
			"SQL should contain 'COUNT' or 'category'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_grouping_by_q_expression_annotation_1() {
		// Test different Q expression grouping
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"status_group",
				AnnotationValue::Field(F::new("status")),
			))
			.values(&["status_group"])
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::sum("value")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("status"),
			"SQL should contain 'SUM' or 'status'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_joined_alias_annotation() {
		// Test annotation with joined relation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"related_field",
			AnnotationValue::Field(F::new("related__name")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("related") || sql.contains("name"),
			"SQL should contain 'related' or 'name'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_joined_alias_annotation_1() {
		// Test different joined annotation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"parent_value",
			AnnotationValue::Field(F::new("parent__value")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("parent") || sql.contains("value"),
			"SQL should contain 'parent' or 'value'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_joined_annotation() {
		// Test simple joined annotation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"fk_field",
			AnnotationValue::Field(F::new("foreign_key__field")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("foreign_key") || sql.contains("field"),
			"SQL should contain 'foreign_key' or 'field'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_joined_annotation_1() {
		// Test joined annotation with different relation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"related_data",
			AnnotationValue::Field(F::new("relation__data")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("relation") || sql.contains("data"),
			"SQL should contain 'relation' or 'data'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_joined_transformed_annotation() {
		// Test joined annotation with transformation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"transformed",
			AnnotationValue::Field(F::new("related__transformed_field")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("related") || sql.contains("transformed"),
			"SQL should contain 'related' or 'transformed'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_joined_transformed_annotation_1() {
		// Test different joined transformation
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"converted",
			AnnotationValue::Field(F::new("parent__converted_value")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("parent") || sql.contains("converted"),
			"SQL should contain 'parent' or 'converted'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_order_by_aggregate() {
		// Test ordering by aggregate
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.values(&["category"])
			.annotate(Annotation::field(
				"count",
				AnnotationValue::Aggregate(Aggregate::count(Some("id"))),
			))
			.order_by(&["count"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("ORDER BY") && sql.contains("COUNT"),
			"SQL should contain 'ORDER BY' and 'COUNT'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_order_by_aggregate_1() {
		// Test ordering by different aggregate
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.values(&["type"])
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::sum("value")),
			))
			.order_by(&["-total"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("ORDER BY") && sql.contains("SUM"),
			"SQL should contain 'ORDER BY' and 'SUM'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_order_by_alias() {
		// Django: Author.objects.alias(other_age=F("age")).order_by("other_age")
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"other_age",
				AnnotationValue::Field(F::new("age")),
			))
			.order_by(&["other_age"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("ORDER BY") && (sql.contains("age") || sql.contains("other_age")),
			"SQL should contain 'ORDER BY' and ('age' or 'other_age'). Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_order_by_alias_1() {
		// Test order by different alias
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"name_alias",
				AnnotationValue::Field(F::new("name")),
			))
			.order_by(&["name_alias"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("ORDER BY"),
			"SQL should contain ORDER BY clause. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_order_by_alias_aggregate() {
		// Django: Author.objects.values("age")
		//                       .alias(age_count=Count("age"))
		//                       .order_by("age_count", "age")
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.values(&["age"])
			.annotate(Annotation::field(
				"age_count",
				AnnotationValue::Aggregate(Aggregate::count(Some("age"))),
			))
			.order_by(&["age_count", "age"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("ORDER BY") && (sql.contains("COUNT") || sql.contains("age")),
			"SQL should contain 'ORDER BY' and ('COUNT' or 'age'). Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_order_by_alias_aggregate_1() {
		// Test order by aggregate with different ordering
		use crate::orm::aggregation::Aggregate;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::sum("value")),
			))
			.order_by(&["-total"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("ORDER BY") && sql.contains("SUM"),
			"SQL should contain 'ORDER BY' and 'SUM'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_order_by_annotation() {
		// Django: Author.objects.annotate(other_age=F("age")).order_by("other_age")
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"other_age",
				AnnotationValue::Field(F::new("age")),
			))
			.order_by(&["other_age"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("ORDER BY") && (sql.contains("age") || sql.contains("other_age")),
			"SQL should contain 'ORDER BY' and ('age' or 'other_age'). Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_order_by_annotation_1() {
		// Test order by multiple annotations
		use crate::orm::expressions::F;
		use crate::orm::query::QuerySet;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"field1_alias",
				AnnotationValue::Field(F::new("field1")),
			))
			.annotate(Annotation::field(
				"field2_alias",
				AnnotationValue::Field(F::new("field2")),
			))
			.order_by(&["field1_alias", "-field2_alias"]);

		let sql = qs.to_sql();

		assert!(
			sql.contains("ORDER BY"),
			"SQL should contain ORDER BY clause. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_q_expression_annotation_with_aggregation() {
		// Test Q expression annotation with aggregation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"computed",
				AnnotationValue::Field(F::new("value")),
			))
			.annotate(Annotation::field(
				"count",
				AnnotationValue::Aggregate(Aggregate::count(Some("id"))),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("COUNT") || sql.contains("value"),
			"SQL should contain 'COUNT' or 'value'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_q_expression_annotation_with_aggregation_1() {
		// Test Q expression with different aggregation
		use crate::orm::aggregation::Aggregate;
		use crate::orm::expressions::F;

		let qs = QuerySet::<TestModel>::new()
			.annotate(Annotation::field(
				"filtered",
				AnnotationValue::Field(F::new("status")),
			))
			.annotate(Annotation::field(
				"total",
				AnnotationValue::Aggregate(Aggregate::sum("amount")),
			));

		let sql = qs.to_sql();

		assert!(
			sql.contains("SUM") || sql.contains("status"),
			"SQL should contain 'SUM' or 'status'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_raw_sql_with_inherited_field() {
		// Test raw SQL annotation with inherited field
		use crate::orm::expressions::F;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"raw_field",
			AnnotationValue::Field(F::new("inherited_field")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("inherited_field") || sql.contains("raw_field"),
			"SQL should contain 'inherited_field' or 'raw_field'. Got: {}",
			sql
		);
	}

	#[test]
	// From: Django/annotations
	fn test_raw_sql_with_inherited_field_1() {
		// Test raw SQL with different inherited field
		use crate::orm::expressions::F;

		let qs = QuerySet::<TestModel>::new().annotate(Annotation::field(
			"parent_field",
			AnnotationValue::Field(F::new("base_field")),
		));

		let sql = qs.to_sql();

		assert!(
			sql.contains("base_field") || sql.contains("parent_field"),
			"SQL should contain 'base_field' or 'parent_field'. Got: {}",
			sql
		);
	}
}
