//! Tests for QuerySet builder API
//!
//! Tests the Django-style QuerySet builder pattern for constructing
//! SQL queries with filters, ordering, pagination, and DML operations.

use reinhardt_db::orm::model::FieldSelector;
use reinhardt_db::orm::query::{Filter, FilterCondition, OrmQuery, UpdateValue};
use reinhardt_db::orm::{FilterOperator, FilterValue, Model, QuerySet};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// -- Test Model Definition --

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestProduct {
	id: Option<i64>,
	name: String,
	price: f64,
	category: String,
	in_stock: bool,
}

#[derive(Debug, Clone)]
struct TestProductFields;

impl FieldSelector for TestProductFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for TestProduct {
	type PrimaryKey = i64;
	type Fields = TestProductFields;

	fn table_name() -> &'static str {
		"products"
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn primary_key_field() -> &'static str {
		"id"
	}

	fn new_fields() -> Self::Fields {
		TestProductFields
	}
}

// -- Basic Filter Tests --

#[rstest]
fn test_basic_eq_filter() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"name",
		FilterOperator::Eq,
		FilterValue::String("Widget".to_string()),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("WHERE"),
		"SQL should contain WHERE clause: {}",
		sql
	);
	assert!(
		sql.contains("\"name\""),
		"SQL should reference name column: {}",
		sql
	);
}

#[rstest]
fn test_multiple_filters_chained_as_and() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new()
		.filter(Filter::new(
			"category",
			FilterOperator::Eq,
			FilterValue::String("electronics".to_string()),
		))
		.filter(Filter::new(
			"price",
			FilterOperator::Lt,
			FilterValue::Float(100.0),
		));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("\"category\""),
		"SQL should reference category: {}",
		sql
	);
	assert!(
		sql.contains("\"price\""),
		"SQL should reference price: {}",
		sql
	);
	assert!(
		sql.contains("AND"),
		"Multiple filters should use AND: {}",
		sql
	);
}

// -- FilterOperator Variants --

#[rstest]
fn test_filter_operator_ne() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"category",
		FilterOperator::Ne,
		FilterValue::String("obsolete".to_string()),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains("<>"), "Ne operator should produce <>: {}", sql);
}

#[rstest]
fn test_filter_operator_gt() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"price",
		FilterOperator::Gt,
		FilterValue::Float(50.0),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains(">"), "Gt operator should produce >: {}", sql);
}

#[rstest]
fn test_filter_operator_gte() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"price",
		FilterOperator::Gte,
		FilterValue::Float(50.0),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains(">="),
		"Gte operator should produce >=: {}",
		sql
	);
}

#[rstest]
fn test_filter_operator_lt() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"price",
		FilterOperator::Lt,
		FilterValue::Integer(100),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains("<"), "Lt operator should produce <: {}", sql);
}

#[rstest]
fn test_filter_operator_lte() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"price",
		FilterOperator::Lte,
		FilterValue::Integer(100),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("<="),
		"Lte operator should produce <=: {}",
		sql
	);
}

#[rstest]
fn test_filter_operator_contains() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"name",
		FilterOperator::Contains,
		FilterValue::String("widget".to_string()),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("LIKE"),
		"Contains operator should produce LIKE: {}",
		sql
	);
}

#[rstest]
fn test_filter_operator_starts_with() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"name",
		FilterOperator::StartsWith,
		FilterValue::String("Pro".to_string()),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("LIKE"),
		"StartsWith operator should produce LIKE: {}",
		sql
	);
}

#[rstest]
fn test_filter_operator_ends_with() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"name",
		FilterOperator::EndsWith,
		FilterValue::String("Pro".to_string()),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("LIKE"),
		"EndsWith operator should produce LIKE: {}",
		sql
	);
}

#[rstest]
fn test_filter_operator_in() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"category",
		FilterOperator::In,
		FilterValue::Array(vec!["electronics".to_string(), "books".to_string()]),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains("IN"), "In operator should produce IN: {}", sql);
}

#[rstest]
fn test_filter_operator_is_null() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"category",
		FilterOperator::IsNull,
		FilterValue::Null,
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("IS NULL"),
		"IsNull operator should produce IS NULL: {}",
		sql
	);
}

#[rstest]
fn test_filter_operator_is_not_null() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"category",
		FilterOperator::IsNotNull,
		FilterValue::Null,
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("IS NOT NULL"),
		"IsNotNull operator should produce IS NOT NULL: {}",
		sql
	);
}

// -- Order By Tests --

#[rstest]
fn test_order_by_ascending() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().order_by(&["name"]);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("ORDER BY"),
		"SQL should contain ORDER BY: {}",
		sql
	);
	assert!(
		sql.contains("\"name\""),
		"SQL should reference name in ORDER BY: {}",
		sql
	);
}

#[rstest]
fn test_order_by_descending() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().order_by(&["-price"]);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("ORDER BY"),
		"SQL should contain ORDER BY: {}",
		sql
	);
	assert!(
		sql.contains("DESC"),
		"Descending order should contain DESC: {}",
		sql
	);
}

#[rstest]
fn test_order_by_multiple_fields() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().order_by(&["category", "-price"]);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("ORDER BY"),
		"SQL should contain ORDER BY: {}",
		sql
	);
	assert!(
		sql.contains("\"category\""),
		"SQL should reference category: {}",
		sql
	);
}

// -- Limit and Offset Tests --

#[rstest]
fn test_limit() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().limit(10);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains("LIMIT"), "SQL should contain LIMIT: {}", sql);
	assert!(
		sql.contains("10"),
		"SQL should contain limit value 10: {}",
		sql
	);
}

#[rstest]
fn test_offset() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().limit(10).offset(20);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains("OFFSET"), "SQL should contain OFFSET: {}", sql);
	assert!(
		sql.contains("20"),
		"SQL should contain offset value 20: {}",
		sql
	);
}

#[rstest]
fn test_paginate() {
	// Arrange
	// Page 3 with page_size 10 should be offset=20, limit=10
	let qs = QuerySet::<TestProduct>::new().paginate(3, 10);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains("LIMIT"), "Paginate should set LIMIT: {}", sql);
	assert!(
		sql.contains("OFFSET"),
		"Paginate should set OFFSET: {}",
		sql
	);
	assert!(
		sql.contains("10"),
		"Paginate page_size should be 10: {}",
		sql
	);
	assert!(
		sql.contains("20"),
		"Paginate offset for page 3 should be 20: {}",
		sql
	);
}

#[rstest]
fn test_paginate_first_page() {
	// Arrange
	// Page 1 with page_size 5 should be offset=0, limit=5
	let qs = QuerySet::<TestProduct>::new().paginate(1, 5);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains("LIMIT"), "Paginate should set LIMIT: {}", sql);
	// Page 1 offset is 0, which may or may not appear in SQL
	// The key check is that LIMIT is 5
	assert!(sql.contains("5"), "Paginate page_size should be 5: {}", sql);
}

// -- Distinct Tests --

#[rstest]
fn test_distinct() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().distinct();

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("DISTINCT"),
		"SQL should contain DISTINCT: {}",
		sql
	);
}

// -- Values and Values List Tests --

#[rstest]
fn test_values_selects_specific_fields() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().values(&["name", "price"]);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("\"name\""),
		"SQL should select name field: {}",
		sql
	);
	assert!(
		sql.contains("\"price\""),
		"SQL should select price field: {}",
		sql
	);
	// Should NOT select all columns
	assert!(
		!sql.contains("*"),
		"SQL should not use * when values are specified: {}",
		sql
	);
}

#[rstest]
fn test_values_list_selects_specific_fields() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().values_list(&["id", "name"]);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("\"id\""),
		"SQL should select id field: {}",
		sql
	);
	assert!(
		sql.contains("\"name\""),
		"SQL should select name field: {}",
		sql
	);
	assert!(
		!sql.contains("*"),
		"SQL should not use * when values_list is used: {}",
		sql
	);
}

// -- Update SQL Tests --

#[rstest]
fn test_update_sql_single_field() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"id",
		FilterOperator::Eq,
		FilterValue::Integer(1),
	));

	let mut updates = HashMap::new();
	updates.insert(
		"name".to_string(),
		UpdateValue::String("Updated Widget".to_string()),
	);

	// Act
	let (sql, params) = qs.update_sql(&updates);

	// Assert
	assert_eq!(
		sql,
		"UPDATE \"products\" SET \"name\" = $1 WHERE \"id\" = $2"
	);
	assert_eq!(params, vec!["Updated Widget", "1"]);
}

#[rstest]
fn test_update_sql_with_null() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"id",
		FilterOperator::Eq,
		FilterValue::Integer(5),
	));

	let mut updates = HashMap::new();
	updates.insert("category".to_string(), UpdateValue::Null);

	// Act
	let (sql, _params) = qs.update_sql(&updates);

	// Assert
	assert!(
		sql.contains("SET \"category\" = NULL"),
		"Update with Null should produce SET column = NULL: {}",
		sql
	);
}

// -- Delete SQL Tests --

#[rstest]
fn test_delete_sql_with_eq_filter() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"id",
		FilterOperator::Eq,
		FilterValue::Integer(42),
	));

	// Act
	let (sql, params) = qs.delete_sql();

	// Assert
	assert_eq!(sql, "DELETE FROM \"products\" WHERE \"id\" = $1");
	assert_eq!(params, vec!["42"]);
}

#[rstest]
fn test_delete_sql_with_multiple_filters() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new()
		.filter(Filter::new(
			"category",
			FilterOperator::Eq,
			FilterValue::String("obsolete".to_string()),
		))
		.filter(Filter::new(
			"in_stock",
			FilterOperator::Eq,
			FilterValue::Boolean(false),
		));

	// Act
	let (sql, params) = qs.delete_sql();

	// Assert
	assert_eq!(
		sql,
		"DELETE FROM \"products\" WHERE (\"category\" = $1 AND \"in_stock\" = $2)"
	);
	assert_eq!(params, vec!["obsolete", "false"]);
}

#[rstest]
#[should_panic(expected = "DELETE without WHERE clause is not allowed")]
fn test_delete_sql_without_filters_panics() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new();

	// Act - should panic
	let _ = qs.delete_sql();
}

// -- FilterCondition Composition Tests --

#[rstest]
fn test_filter_condition_and() {
	// Arrange
	let condition = FilterCondition::And(vec![
		FilterCondition::Single(Filter::new(
			"category",
			FilterOperator::Eq,
			FilterValue::String("electronics".to_string()),
		)),
		FilterCondition::Single(Filter::new(
			"price",
			FilterOperator::Lte,
			FilterValue::Float(500.0),
		)),
	]);

	// Act / Assert - verify the structure compiles and is constructed
	assert!(!condition.is_empty());
}

#[rstest]
fn test_filter_condition_or() {
	// Arrange
	let condition = FilterCondition::Or(vec![
		FilterCondition::Single(Filter::new(
			"category",
			FilterOperator::Eq,
			FilterValue::String("electronics".to_string()),
		)),
		FilterCondition::Single(Filter::new(
			"category",
			FilterOperator::Eq,
			FilterValue::String("books".to_string()),
		)),
	]);

	// Act / Assert
	assert!(!condition.is_empty());
}

#[rstest]
fn test_filter_condition_not() {
	// Arrange
	let condition = FilterCondition::not(FilterCondition::Single(Filter::new(
		"in_stock",
		FilterOperator::Eq,
		FilterValue::Boolean(false),
	)));

	// Act / Assert
	assert!(!condition.is_empty());
}

#[rstest]
fn test_filter_condition_nested_and_or() {
	// Arrange
	// (category = 'electronics' AND price <= 500) OR (category = 'books' AND price <= 20)
	let condition = FilterCondition::Or(vec![
		FilterCondition::And(vec![
			FilterCondition::Single(Filter::new(
				"category",
				FilterOperator::Eq,
				FilterValue::String("electronics".to_string()),
			)),
			FilterCondition::Single(Filter::new(
				"price",
				FilterOperator::Lte,
				FilterValue::Float(500.0),
			)),
		]),
		FilterCondition::And(vec![
			FilterCondition::Single(Filter::new(
				"category",
				FilterOperator::Eq,
				FilterValue::String("books".to_string()),
			)),
			FilterCondition::Single(Filter::new(
				"price",
				FilterOperator::Lte,
				FilterValue::Float(20.0),
			)),
		]),
	]);

	// Act / Assert
	assert!(!condition.is_empty());
}

#[rstest]
fn test_filter_condition_empty_and_is_empty() {
	// Arrange
	let condition = FilterCondition::And(vec![]);

	// Act / Assert
	assert!(condition.is_empty());
}

#[rstest]
fn test_filter_condition_or_filters_convenience() {
	// Arrange
	let filters = vec![
		Filter::new(
			"name",
			FilterOperator::Contains,
			FilterValue::String("widget".to_string()),
		),
		Filter::new(
			"category",
			FilterOperator::Contains,
			FilterValue::String("widget".to_string()),
		),
	];

	// Act
	let condition = FilterCondition::or_filters(filters);

	// Assert
	assert!(!condition.is_empty());
}

#[rstest]
fn test_filter_condition_and_filters_convenience() {
	// Arrange
	let filters = vec![
		Filter::new("in_stock", FilterOperator::Eq, FilterValue::Boolean(true)),
		Filter::new("price", FilterOperator::Gt, FilterValue::Float(0.0)),
	];

	// Act
	let condition = FilterCondition::and_filters(filters);

	// Assert
	assert!(!condition.is_empty());
}

// -- Combined Builder Pattern Tests --

#[rstest]
fn test_full_query_chain() {
	// Arrange & Act
	let qs = QuerySet::<TestProduct>::new()
		.filter(Filter::new(
			"in_stock",
			FilterOperator::Eq,
			FilterValue::Boolean(true),
		))
		.values(&["name", "price"])
		.order_by(&["-price"])
		.limit(10)
		.offset(0);

	let sql = qs.to_sql();

	// Assert
	assert!(sql.contains("\"name\""), "SQL should select name: {}", sql);
	assert!(
		sql.contains("\"price\""),
		"SQL should select price: {}",
		sql
	);
	assert!(sql.contains("WHERE"), "SQL should have WHERE: {}", sql);
	assert!(
		sql.contains("ORDER BY"),
		"SQL should have ORDER BY: {}",
		sql
	);
	assert!(sql.contains("LIMIT"), "SQL should have LIMIT: {}", sql);
}

#[rstest]
fn test_distinct_with_values_and_order() {
	// Arrange & Act
	let qs = QuerySet::<TestProduct>::new()
		.distinct()
		.values(&["category"])
		.order_by(&["category"]);

	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("DISTINCT"),
		"SQL should contain DISTINCT: {}",
		sql
	);
	assert!(
		sql.contains("\"category\""),
		"SQL should select category: {}",
		sql
	);
	assert!(
		sql.contains("ORDER BY"),
		"SQL should have ORDER BY: {}",
		sql
	);
}

#[rstest]
fn test_queryset_from_table() {
	// Arrange & Act
	let qs = QuerySet::<TestProduct>::new();
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("\"products\""),
		"SQL should reference products table: {}",
		sql
	);
	assert!(
		sql.contains("SELECT"),
		"SQL should be a SELECT statement: {}",
		sql
	);
}

#[rstest]
fn test_filter_with_integer_value() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"id",
		FilterOperator::Eq,
		FilterValue::Integer(42),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("\"id\""),
		"SQL should reference id column: {}",
		sql
	);
	assert!(
		sql.contains("WHERE"),
		"SQL should have WHERE clause: {}",
		sql
	);
}

#[rstest]
fn test_filter_with_boolean_value() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"in_stock",
		FilterOperator::Eq,
		FilterValue::Boolean(true),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("\"in_stock\""),
		"SQL should reference in_stock column: {}",
		sql
	);
}

#[rstest]
fn test_filter_with_null_eq_produces_is_null() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"category",
		FilterOperator::Eq,
		FilterValue::Null,
	));

	// Act
	let (sql, params) = qs
		.filter(Filter::new(
			"id",
			FilterOperator::Eq,
			FilterValue::Integer(1),
		))
		.delete_sql();

	// Assert
	assert!(
		sql.contains("IS NULL"),
		"Eq with Null value should produce IS NULL: {}",
		sql
	);
	assert_eq!(params, vec!["1"]);
}

#[rstest]
fn test_filter_with_null_ne_produces_is_not_null() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"category",
		FilterOperator::Ne,
		FilterValue::Null,
	));

	// Act
	let (sql, params) = qs
		.filter(Filter::new(
			"id",
			FilterOperator::Eq,
			FilterValue::Integer(1),
		))
		.delete_sql();

	// Assert
	assert!(
		sql.contains("IS NOT NULL"),
		"Ne with Null value should produce IS NOT NULL: {}",
		sql
	);
	assert_eq!(params, vec!["1"]);
}

// -- FilterValue From Conversion Tests --

#[rstest]
fn test_filter_value_from_string() {
	// Arrange
	let value = String::from("hello");

	// Act
	let filter_value: FilterValue = value.into();

	// Assert
	assert!(
		matches!(filter_value, FilterValue::String(s) if s == "hello"),
		"From<String> should produce FilterValue::String"
	);
}

#[rstest]
fn test_filter_value_from_str() {
	// Arrange
	let value = "world";

	// Act
	let filter_value: FilterValue = value.into();

	// Assert
	assert!(
		matches!(filter_value, FilterValue::String(s) if s == "world"),
		"From<&str> should produce FilterValue::String"
	);
}

#[rstest]
fn test_filter_value_from_i64() {
	// Arrange
	let value: i64 = 42;

	// Act
	let filter_value: FilterValue = value.into();

	// Assert
	assert!(
		matches!(filter_value, FilterValue::Integer(42)),
		"From<i64> should produce FilterValue::Integer"
	);
}

#[rstest]
fn test_filter_value_from_i32() {
	// Arrange
	let value: i32 = 99;

	// Act
	let filter_value: FilterValue = value.into();

	// Assert
	assert!(
		matches!(filter_value, FilterValue::Integer(99)),
		"From<i32> should produce FilterValue::Integer with i64 conversion"
	);
}

#[rstest]
fn test_filter_value_from_f64() {
	// Arrange
	let value: f64 = 3.14;

	// Act
	let filter_value: FilterValue = value.into();

	// Assert
	assert!(
		matches!(filter_value, FilterValue::Float(f) if (f - 3.14).abs() < f64::EPSILON),
		"From<f64> should produce FilterValue::Float"
	);
}

#[rstest]
fn test_filter_value_from_bool_true() {
	// Arrange
	let value = true;

	// Act
	let filter_value: FilterValue = value.into();

	// Assert
	assert!(
		matches!(filter_value, FilterValue::Boolean(true)),
		"From<bool> true should produce FilterValue::Boolean(true)"
	);
}

#[rstest]
fn test_filter_value_from_bool_false() {
	// Arrange
	let value = false;

	// Act
	let filter_value: FilterValue = value.into();

	// Assert
	assert!(
		matches!(filter_value, FilterValue::Boolean(false)),
		"From<bool> false should produce FilterValue::Boolean(false)"
	);
}

// -- FilterValue From conversions used in filter builder --

#[rstest]
fn test_filter_with_from_i32_value() {
	// Arrange
	let filter_value: FilterValue = 25i32.into();
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"price",
		FilterOperator::Gt,
		filter_value,
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("\"price\""),
		"Filter with i32-converted value should reference price column: {}",
		sql
	);
	assert!(
		sql.contains(">"),
		"Filter should contain > operator: {}",
		sql
	);
}

// -- OrmQuery Tests --

#[rstest]
fn test_orm_query_new() {
	// Arrange & Act
	let query = OrmQuery::new();

	// Assert - OrmQuery::new creates an empty query with no filters
	// We verify it implements Debug by formatting it
	let debug_str = format!("{:?}", query);
	assert!(
		debug_str.contains("OrmQuery"),
		"OrmQuery debug representation should contain type name: {}",
		debug_str
	);
}

#[rstest]
fn test_orm_query_filter_chaining() {
	// Arrange
	let filter1 = Filter::new(
		"name",
		FilterOperator::Eq,
		FilterValue::String("Alice".to_string()),
	);
	let filter2 = Filter::new("age", FilterOperator::Gt, FilterValue::Integer(18));

	// Act
	let query = OrmQuery::new().filter(filter1).filter(filter2);

	// Assert
	let debug_str = format!("{:?}", query);
	assert!(
		debug_str.contains("name"),
		"OrmQuery should contain first filter field: {}",
		debug_str
	);
	assert!(
		debug_str.contains("age"),
		"OrmQuery should contain second filter field: {}",
		debug_str
	);
}

#[rstest]
fn test_orm_query_default() {
	// Arrange & Act
	let query: OrmQuery = Default::default();

	// Assert - Default should produce the same as new()
	let debug_str = format!("{:?}", query);
	assert!(
		debug_str.contains("OrmQuery"),
		"Default OrmQuery should be valid: {}",
		debug_str
	);
}

#[rstest]
fn test_orm_query_single_filter() {
	// Arrange
	let filter = Filter::new(
		"status",
		FilterOperator::Eq,
		FilterValue::String("active".to_string()),
	);

	// Act
	let query = OrmQuery::new().filter(filter);

	// Assert
	let debug_str = format!("{:?}", query);
	assert!(
		debug_str.contains("status"),
		"OrmQuery should contain the filter field: {}",
		debug_str
	);
}

// -- FilterCondition::single() convenience method --

#[rstest]
fn test_filter_condition_single_convenience() {
	// Arrange
	let filter = Filter::new(
		"name",
		FilterOperator::Eq,
		FilterValue::String("test".to_string()),
	);

	// Act
	let condition = FilterCondition::single(filter);

	// Assert
	assert!(
		!condition.is_empty(),
		"FilterCondition::single() should not be empty"
	);
}

// -- Additional edge case tests --

#[rstest]
fn test_filter_condition_not_of_empty_is_empty() {
	// Arrange
	let empty_and = FilterCondition::And(vec![]);

	// Act
	let not_empty = FilterCondition::not(empty_and);

	// Assert
	assert!(
		not_empty.is_empty(),
		"NOT of an empty condition should be empty"
	);
}

#[rstest]
fn test_filter_condition_or_of_empty_is_empty() {
	// Arrange & Act
	let condition = FilterCondition::Or(vec![]);

	// Assert
	assert!(condition.is_empty(), "Empty OR condition should be empty");
}

#[rstest]
fn test_queryset_default_selects_all_columns() {
	// Arrange & Act
	let qs = QuerySet::<TestProduct>::new();
	let sql = qs.to_sql();

	// Assert
	assert_eq!(
		sql, "SELECT * FROM \"products\"",
		"Default QuerySet should generate SELECT * FROM table"
	);
}

#[rstest]
fn test_update_sql_with_boolean_value() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"id",
		FilterOperator::Eq,
		FilterValue::Integer(7),
	));

	let mut updates = HashMap::new();
	updates.insert("in_stock".to_string(), UpdateValue::Boolean(false));

	// Act
	let (sql, params) = qs.update_sql(&updates);

	// Assert
	assert!(
		sql.contains("UPDATE"),
		"Should generate UPDATE statement: {}",
		sql
	);
	assert!(
		sql.contains("\"in_stock\""),
		"Should reference in_stock column: {}",
		sql
	);
	assert_eq!(params.len(), 2, "Should have 2 params (value + where)");
}

#[rstest]
fn test_update_sql_with_integer_value() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"id",
		FilterOperator::Eq,
		FilterValue::Integer(3),
	));

	let mut updates = HashMap::new();
	updates.insert("price".to_string(), UpdateValue::Integer(999));

	// Act
	let (sql, params) = qs.update_sql(&updates);

	// Assert
	assert!(
		sql.contains("UPDATE \"products\""),
		"Should update products table: {}",
		sql
	);
	assert!(sql.contains("SET"), "Should contain SET clause: {}", sql);
	assert!(
		sql.contains("\"price\""),
		"Should reference price column: {}",
		sql
	);
	assert_eq!(params.len(), 2, "Should have 2 params");
}

#[rstest]
fn test_update_sql_with_float_value() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"id",
		FilterOperator::Eq,
		FilterValue::Integer(4),
	));

	let mut updates = HashMap::new();
	updates.insert("price".to_string(), UpdateValue::Float(29.99));

	// Act
	let (sql, params) = qs.update_sql(&updates);

	// Assert
	assert!(
		sql.contains("UPDATE \"products\""),
		"Should update products table: {}",
		sql
	);
	assert!(
		sql.contains("\"price\""),
		"Should reference price column: {}",
		sql
	);
	assert_eq!(params.len(), 2, "Should have 2 params (value + where)");
}

#[rstest]
fn test_delete_sql_with_string_filter() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"category",
		FilterOperator::Eq,
		FilterValue::String("discontinued".to_string()),
	));

	// Act
	let (sql, params) = qs.delete_sql();

	// Assert
	assert_eq!(sql, "DELETE FROM \"products\" WHERE \"category\" = $1");
	assert_eq!(params, vec!["discontinued"]);
}

#[rstest]
fn test_order_by_ascending_contains_asc() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().order_by(&["name"]);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("ASC"),
		"Ascending order should contain ASC: {}",
		sql
	);
}

#[rstest]
fn test_filter_not_in_operator() {
	// Arrange
	let qs = QuerySet::<TestProduct>::new().filter(Filter::new(
		"category",
		FilterOperator::NotIn,
		FilterValue::Array(vec!["obsolete".to_string(), "deprecated".to_string()]),
	));

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("NOT IN"),
		"NotIn operator should produce NOT IN: {}",
		sql
	);
}

#[rstest]
fn test_paginate_page_zero_saturates() {
	// Arrange
	// Page 0 should saturate to offset 0 (saturating_sub)
	let qs = QuerySet::<TestProduct>::new().paginate(0, 10);

	// Act
	let sql = qs.to_sql();

	// Assert
	assert!(
		sql.contains("LIMIT"),
		"Paginate should set LIMIT even for page 0: {}",
		sql
	);
	assert!(sql.contains("10"), "Page size should be 10: {}", sql);
}
