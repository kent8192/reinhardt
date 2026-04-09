//! Integration tests for AdminDatabase operations
//!
//! These tests verify AdminDatabase CRUD operations and filter building
//! functions using mock database connections.

use reinhardt_admin::core::database::{
	AdminDatabase, build_composite_filter_condition, build_single_filter_expr,
	filter_value_to_sea_value,
};
use reinhardt_db::orm::annotation::Expression;
use reinhardt_db::orm::expressions::{F, OuterRef};
use reinhardt_db::orm::{
	DatabaseBackend, DatabaseConnection, Filter, FilterCondition, FilterOperator, FilterValue,
};
use reinhardt_di::Depends;
use reinhardt_query::{
	Alias, ColumnRef, Condition, PostgresQueryBuilder, Query, QueryStatementBuilder, Value,
};
use reinhardt_test::fixtures::mock_connection;
use rstest::*;
use std::collections::HashMap;
use std::sync::Arc;

// Mock User model for testing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
	id: Option<i64>,
	name: String,
}

reinhardt_test::impl_test_model!(User, i64, "users");

// ==================== AdminDatabase CRUD tests ====================

#[rstest]
#[tokio::test]
async fn test_admin_database_new(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);

	// Act & Assert
	assert_eq!(db.connection().backend(), DatabaseBackend::Postgres);
}

#[rstest]
#[tokio::test]
async fn test_bulk_delete_empty(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);

	// Act
	let result = db.bulk_delete::<User>("users", "id", vec![]).await;

	// Assert
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), 0);
}

#[rstest]
#[tokio::test]
async fn test_list_with_filters(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let filters = vec![Filter::new(
		"is_active".to_string(),
		FilterOperator::Eq,
		FilterValue::Boolean(true),
	)];

	// Act
	let result = db.list::<User>("users", filters, 0, 50).await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_get_by_id(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);

	// Act
	let result = db.get::<User>("users", "id", "1").await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_create(mock_connection: DatabaseConnection) {
	// Arrange
	// Default mock returns {"count": 0} without "id" field,
	// so create() returns Err (missing pk field). Fixes #3029
	let db = AdminDatabase::new(mock_connection);
	let mut data = HashMap::new();
	data.insert("name".to_string(), serde_json::json!("Alice"));
	data.insert("email".to_string(), serde_json::json!("alice@example.com"));

	// Act
	let result = db.create::<User>("users", None, data).await;

	// Assert
	assert!(result.is_err());
	let err_msg = result.unwrap_err().to_string();
	assert!(
		err_msg.contains("RETURNING clause did not return expected primary key field"),
		"Expected missing pk field error, got: {}",
		err_msg
	);
}

#[rstest]
#[tokio::test]
async fn test_update(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let mut data = HashMap::new();
	data.insert("name".to_string(), serde_json::json!("Alice Updated"));

	// Act
	let result = db.update::<User>("users", "id", "1", data).await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_delete(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);

	// Act
	let result = db.delete::<User>("users", "id", "1").await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_count(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let filters = vec![];

	// Act
	let result = db.count::<User>("users", filters).await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_bulk_delete_multiple_ids(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];

	// Act
	let result = db.bulk_delete::<User>("users", "id", ids).await;

	// Assert
	assert!(result.is_ok());
}

// ==================== build_composite_filter_condition tests ====================

#[rstest]
fn test_build_composite_single_condition() {
	// Arrange
	let filter = Filter::new(
		"name".to_string(),
		FilterOperator::Eq,
		FilterValue::String("Alice".to_string()),
	);
	let condition = FilterCondition::Single(filter);

	// Act
	let result = build_composite_filter_condition(&condition);

	// Assert
	assert!(result.is_ok());
	let result = result.unwrap();
	assert!(result.is_some());
	let cond = result.unwrap();
	let query = Query::select()
		.from(Alias::new("users"))
		.column(ColumnRef::Asterisk)
		.cond_where(cond)
		.to_string(PostgresQueryBuilder);
	assert!(query.contains("\"name\""));
	assert!(query.contains("'Alice'"));
}

#[rstest]
fn test_build_composite_or_condition() {
	// Arrange
	let filter1 = Filter::new(
		"name".to_string(),
		FilterOperator::Contains,
		FilterValue::String("Alice".to_string()),
	);
	let filter2 = Filter::new(
		"email".to_string(),
		FilterOperator::Contains,
		FilterValue::String("alice".to_string()),
	);
	let condition = FilterCondition::Or(vec![
		FilterCondition::Single(filter1),
		FilterCondition::Single(filter2),
	]);

	// Act
	let result = build_composite_filter_condition(&condition);

	// Assert
	assert!(result.is_ok());
	let result = result.unwrap();
	assert!(result.is_some());
	let cond = result.unwrap();
	let query = Query::select()
		.from(Alias::new("users"))
		.column(ColumnRef::Asterisk)
		.cond_where(cond)
		.to_string(PostgresQueryBuilder);
	assert!(query.contains("\"name\""));
	assert!(query.contains("\"email\""));
	assert!(query.contains("OR"));
}

#[rstest]
fn test_build_composite_and_condition() {
	// Arrange
	let filter1 = Filter::new(
		"is_active".to_string(),
		FilterOperator::Eq,
		FilterValue::Boolean(true),
	);
	let filter2 = Filter::new(
		"is_staff".to_string(),
		FilterOperator::Eq,
		FilterValue::Boolean(true),
	);
	let condition = FilterCondition::And(vec![
		FilterCondition::Single(filter1),
		FilterCondition::Single(filter2),
	]);

	// Act
	let result = build_composite_filter_condition(&condition);

	// Assert
	assert!(result.is_ok());
	let result = result.unwrap();
	assert!(result.is_some());
	let cond = result.unwrap();
	let query = Query::select()
		.from(Alias::new("users"))
		.column(ColumnRef::Asterisk)
		.cond_where(cond)
		.to_string(PostgresQueryBuilder);
	assert!(query.contains("\"is_active\""));
	assert!(query.contains("\"is_staff\""));
	assert!(query.contains("AND"));
}

#[rstest]
fn test_build_composite_nested_condition() {
	// Arrange: (name LIKE '%Alice%' OR email LIKE '%alice%') AND is_active = true
	let filter_name = Filter::new(
		"name".to_string(),
		FilterOperator::Contains,
		FilterValue::String("Alice".to_string()),
	);
	let filter_email = Filter::new(
		"email".to_string(),
		FilterOperator::Contains,
		FilterValue::String("alice".to_string()),
	);
	let filter_active = Filter::new(
		"is_active".to_string(),
		FilterOperator::Eq,
		FilterValue::Boolean(true),
	);
	let or_condition = FilterCondition::Or(vec![
		FilterCondition::Single(filter_name),
		FilterCondition::Single(filter_email),
	]);
	let and_condition =
		FilterCondition::And(vec![or_condition, FilterCondition::Single(filter_active)]);

	// Act
	let result = build_composite_filter_condition(&and_condition);

	// Assert
	assert!(result.is_ok());
	let result = result.unwrap();
	assert!(result.is_some());
	let cond = result.unwrap();
	let query = Query::select()
		.from(Alias::new("users"))
		.column(ColumnRef::Asterisk)
		.cond_where(cond)
		.to_string(PostgresQueryBuilder);
	assert!(query.contains("\"name\""));
	assert!(query.contains("\"email\""));
	assert!(query.contains("\"is_active\""));
	assert!(query.contains("OR"));
	assert!(query.contains("AND"));
}

#[rstest]
fn test_build_composite_empty_or() {
	// Arrange
	let condition = FilterCondition::Or(vec![]);

	// Act
	let result = build_composite_filter_condition(&condition);

	// Assert
	assert!(result.is_ok());
	assert!(result.unwrap().is_none());
}

#[rstest]
fn test_build_composite_empty_and() {
	// Arrange
	let condition = FilterCondition::And(vec![]);

	// Act
	let result = build_composite_filter_condition(&condition);

	// Assert
	assert!(result.is_ok());
	assert!(result.unwrap().is_none());
}

// ==================== list_with_condition / count_with_condition tests ====================

#[rstest]
#[tokio::test]
async fn test_list_with_condition_or_search(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let filter1 = Filter::new(
		"name".to_string(),
		FilterOperator::Contains,
		FilterValue::String("Alice".to_string()),
	);
	let filter2 = Filter::new(
		"email".to_string(),
		FilterOperator::Contains,
		FilterValue::String("alice".to_string()),
	);
	let search_condition = FilterCondition::Or(vec![
		FilterCondition::Single(filter1),
		FilterCondition::Single(filter2),
	]);

	// Act
	let result = db
		.list_with_condition::<User>("users", Some(&search_condition), vec![], None, 0, 50)
		.await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_list_with_condition_and_additional(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let filter1 = Filter::new(
		"name".to_string(),
		FilterOperator::Contains,
		FilterValue::String("Alice".to_string()),
	);
	let filter2 = Filter::new(
		"email".to_string(),
		FilterOperator::Contains,
		FilterValue::String("alice".to_string()),
	);
	let search_condition = FilterCondition::Or(vec![
		FilterCondition::Single(filter1),
		FilterCondition::Single(filter2),
	]);
	let additional = vec![Filter::new(
		"is_active".to_string(),
		FilterOperator::Eq,
		FilterValue::Boolean(true),
	)];

	// Act
	let result = db
		.list_with_condition::<User>("users", Some(&search_condition), additional, None, 0, 50)
		.await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_count_with_condition_or_search(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let filter1 = Filter::new(
		"name".to_string(),
		FilterOperator::Contains,
		FilterValue::String("Alice".to_string()),
	);
	let filter2 = Filter::new(
		"email".to_string(),
		FilterOperator::Contains,
		FilterValue::String("alice".to_string()),
	);
	let search_condition = FilterCondition::Or(vec![
		FilterCondition::Single(filter1),
		FilterCondition::Single(filter2),
	]);

	// Act
	let result = db
		.count_with_condition::<User>("users", Some(&search_condition), vec![])
		.await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_list_with_condition_none(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);

	// Act
	let result = db
		.list_with_condition::<User>("users", None, vec![], None, 0, 50)
		.await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_list_with_condition_empty_additional(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let filter = Filter::new(
		"name".to_string(),
		FilterOperator::Contains,
		FilterValue::String("Alice".to_string()),
	);
	let search_condition = FilterCondition::Single(filter);

	// Act
	let result = db
		.list_with_condition::<User>("users", Some(&search_condition), vec![], None, 0, 50)
		.await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_count_with_condition_none(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);

	// Act
	let result = db.count_with_condition::<User>("users", None, vec![]).await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_count_with_condition_combined(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let filter1 = Filter::new(
		"name".to_string(),
		FilterOperator::Contains,
		FilterValue::String("Alice".to_string()),
	);
	let search_condition = FilterCondition::Single(filter1);
	let additional = vec![Filter::new(
		"is_active".to_string(),
		FilterOperator::Eq,
		FilterValue::Boolean(true),
	)];

	// Act
	let result = db
		.count_with_condition::<User>("users", Some(&search_condition), additional)
		.await;

	// Assert
	assert!(result.is_ok());
}

// ==================== FieldRef/OuterRef/Expression filter tests ====================

#[rstest]
fn test_build_single_filter_expr_field_ref_eq() {
	// Arrange
	let filter = Filter::new(
		"price".to_string(),
		FilterOperator::Eq,
		FilterValue::FieldRef(F::new("discount_price")),
	);

	// Act
	let result = build_single_filter_expr(&filter);

	// Assert
	assert!(result.is_some());
	let query = Query::select()
		.from(Alias::new("products"))
		.column(ColumnRef::Asterisk)
		.cond_where(Condition::all().add(result.unwrap()))
		.to_string(PostgresQueryBuilder);
	assert!(query.contains("\"price\""));
	assert!(query.contains("\"discount_price\""));
}

#[rstest]
fn test_build_single_filter_expr_field_ref_gt() {
	// Arrange
	let filter = Filter::new(
		"price".to_string(),
		FilterOperator::Gt,
		FilterValue::FieldRef(F::new("cost")),
	);

	// Act
	let result = build_single_filter_expr(&filter);

	// Assert
	assert!(result.is_some());
}

#[rstest]
fn test_build_single_filter_expr_field_ref_all_operators() {
	// Arrange
	let operators = [
		FilterOperator::Eq,
		FilterOperator::Ne,
		FilterOperator::Gt,
		FilterOperator::Gte,
		FilterOperator::Lt,
		FilterOperator::Lte,
	];

	for op in operators {
		// Act
		let filter = Filter::new(
			"field_a".to_string(),
			op.clone(),
			FilterValue::FieldRef(F::new("field_b")),
		);
		let result = build_single_filter_expr(&filter);

		// Assert
		assert!(
			result.is_some(),
			"FieldRef with {:?} should produce Some",
			op
		);
	}
}

#[rstest]
fn test_build_single_filter_expr_outer_ref() {
	// Arrange
	let filter = Filter::new(
		"author_id".to_string(),
		FilterOperator::Eq,
		FilterValue::OuterRef(OuterRef::new("authors.id")),
	);

	// Act
	let result = build_single_filter_expr(&filter);

	// Assert
	assert!(result.is_some());
	let query = Query::select()
		.from(Alias::new("books"))
		.column(ColumnRef::Asterisk)
		.cond_where(Condition::all().add(result.unwrap()))
		.to_string(PostgresQueryBuilder);
	assert!(query.contains("author_id"));
	assert!(query.contains("authors.id"));
}

#[rstest]
fn test_build_single_filter_expr_outer_ref_all_operators() {
	// Arrange
	let operators = [
		FilterOperator::Eq,
		FilterOperator::Ne,
		FilterOperator::Gt,
		FilterOperator::Gte,
		FilterOperator::Lt,
		FilterOperator::Lte,
	];

	for op in operators {
		// Act
		let filter = Filter::new(
			"child_id".to_string(),
			op.clone(),
			FilterValue::OuterRef(OuterRef::new("parent.id")),
		);
		let result = build_single_filter_expr(&filter);

		// Assert
		assert!(
			result.is_some(),
			"OuterRef with {:?} should produce Some",
			op
		);
	}
}

#[rstest]
fn test_build_single_filter_expr_expression() {
	use reinhardt_db::orm::annotation::{AnnotationValue, Value};

	// Arrange: price > (cost * 2)
	let expr = Expression::Multiply(
		Box::new(AnnotationValue::Field(F::new("cost"))),
		Box::new(AnnotationValue::Value(Value::Int(2))),
	);
	let filter = Filter::new(
		"price".to_string(),
		FilterOperator::Gt,
		FilterValue::Expression(expr),
	);

	// Act
	let result = build_single_filter_expr(&filter);

	// Assert
	assert!(result.is_some());
}

#[rstest]
fn test_build_single_filter_expr_expression_all_operators() {
	use reinhardt_db::orm::annotation::{AnnotationValue, Value};

	// Arrange
	let operators = [
		FilterOperator::Eq,
		FilterOperator::Ne,
		FilterOperator::Gt,
		FilterOperator::Gte,
		FilterOperator::Lt,
		FilterOperator::Lte,
	];

	for op in operators {
		let expr = Expression::Add(
			Box::new(AnnotationValue::Field(F::new("base"))),
			Box::new(AnnotationValue::Value(Value::Int(10))),
		);
		let filter = Filter::new(
			"total".to_string(),
			op.clone(),
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter);

		// Assert
		assert!(
			result.is_some(),
			"Expression with {:?} should produce Some",
			op
		);
	}
}

#[rstest]
fn test_filter_value_to_sea_value_field_ref_fallback() {
	// Arrange
	let value = FilterValue::FieldRef(F::new("test_field"));

	// Act
	let sea_value = filter_value_to_sea_value(&value);

	// Assert
	match sea_value {
		Value::String(Some(s)) => assert_eq!(s.as_str(), "test_field"),
		_ => panic!("Expected String value"),
	}
}

#[rstest]
fn test_filter_value_to_sea_value_outer_ref_fallback() {
	// Arrange
	let value = FilterValue::OuterRef(OuterRef::new("outer.field"));

	// Act
	let sea_value = filter_value_to_sea_value(&value);

	// Assert
	match sea_value {
		Value::String(Some(s)) => assert_eq!(s.as_str(), "outer.field"),
		_ => panic!("Expected String value"),
	}
}

#[rstest]
fn test_filter_value_to_sea_value_expression_fallback() {
	use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

	// Arrange
	let expr = Expression::Add(
		Box::new(AnnotationValue::Field(F::new("a"))),
		Box::new(AnnotationValue::Value(OrmValue::Int(1))),
	);
	let value = FilterValue::Expression(expr);

	// Act
	let sea_value = filter_value_to_sea_value(&value);

	// Assert
	match sea_value {
		Value::String(Some(s)) => {
			assert!(s.contains("a"), "SQL should contain field name 'a'");
			assert!(s.contains("1"), "SQL should contain value '1'");
		}
		_ => panic!("Expected String value"),
	}
}

// ==================== Bug #2946: create() RETURNING clause tests ====================

#[rstest]
#[tokio::test]
async fn test_create_returns_error_when_response_has_no_id_field(
	mock_connection: DatabaseConnection,
) {
	// Arrange
	// Default mock_connection.fetch_one returns Row with {"count": Int(0)}, no "id" field
	let db = AdminDatabase::new(mock_connection);
	let mut data = HashMap::new();
	data.insert("name".to_string(), serde_json::json!("Alice"));

	// Act
	let result = db.create::<User>("users", None, data).await;

	// Assert
	// Bug #2946 was fixed: create() now returns Err when the RETURNING clause
	// does not contain the expected primary key field. Fixes #3029
	assert!(result.is_err());
	let err_msg = result.unwrap_err().to_string();
	assert!(
		err_msg.contains("RETURNING clause did not return expected primary key field"),
		"Expected missing pk field error, got: {}",
		err_msg
	);
}

#[rstest]
#[tokio::test]
async fn test_create_returns_id_when_response_has_id_field() {
	// Arrange: Create a mock that returns a row with "id" field
	use reinhardt_db::backends::{
		backend::DatabaseBackend as BackendTrait,
		connection::DatabaseConnection as BackendsConnection,
		types::{DatabaseType, QueryResult, QueryValue, Row},
	};
	use reinhardt_test::fixtures::mock::MockDatabaseBackend;

	let mut mock = MockDatabaseBackend::new();
	mock.expect_database_type()
		.return_const(DatabaseType::Postgres);
	mock.expect_placeholder()
		.returning(|idx| format!("${}", idx));
	mock.expect_supports_returning().return_const(true);
	mock.expect_supports_on_conflict().return_const(true);
	mock.expect_execute()
		.returning(|_, _| Ok(QueryResult { rows_affected: 1 }));
	mock.expect_fetch_all().returning(|_, _| Ok(Vec::new()));
	mock.expect_fetch_optional().returning(|_, _| Ok(None));

	// Return a row with "id" field
	mock.expect_fetch_one().returning(|_, _| {
		let mut row = Row::new();
		row.data.insert("id".to_string(), QueryValue::Int(42));
		Ok(row)
	});

	let backends_conn = BackendsConnection::new(Arc::new(mock));
	let conn = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db = AdminDatabase::new(conn);

	let mut data = HashMap::new();
	data.insert("name".to_string(), serde_json::json!("Alice"));

	// Act
	let result = db.create::<User>("users", None, data).await;

	// Assert
	assert!(result.is_ok());
	assert_eq!(
		result.unwrap(),
		42,
		"create() should return the ID from the response"
	);
}

#[rstest]
#[tokio::test]
async fn test_create_returns_error_when_pk_field_missing() {
	// Arrange: Create a mock that returns a row without the expected "id" field
	use reinhardt_db::backends::{
		backend::DatabaseBackend as BackendTrait,
		connection::DatabaseConnection as BackendsConnection,
		types::{DatabaseType, QueryResult, QueryValue, Row},
	};
	use reinhardt_test::fixtures::mock::MockDatabaseBackend;

	let mut mock = MockDatabaseBackend::new();
	mock.expect_database_type()
		.return_const(DatabaseType::Postgres);
	mock.expect_placeholder()
		.returning(|idx| format!("${}", idx));
	mock.expect_supports_returning().return_const(true);
	mock.expect_supports_on_conflict().return_const(true);
	mock.expect_execute()
		.returning(|_, _| Ok(QueryResult { rows_affected: 1 }));
	mock.expect_fetch_all().returning(|_, _| Ok(Vec::new()));
	mock.expect_fetch_optional().returning(|_, _| Ok(None));

	// Return a row that does NOT contain the expected "id" pk field
	mock.expect_fetch_one().returning(|_, _| {
		let mut row = Row::new();
		row.data.insert(
			"other_field".to_string(),
			QueryValue::String("some-value".to_string()),
		);
		Ok(row)
	});

	let backends_conn = BackendsConnection::new(Arc::new(mock));
	let conn = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db = AdminDatabase::new(conn);

	let mut data = HashMap::new();
	data.insert("name".to_string(), serde_json::json!("Bob"));

	// Act
	let result = db.create::<User>("users", None, data).await;

	// Assert: Missing pk field triggers an error. Fixes #2946, #3029
	assert!(result.is_err());
	let err_msg = result.unwrap_err().to_string();
	assert!(
		err_msg.contains("RETURNING clause did not return expected primary key field"),
		"Expected missing pk field error, got: {}",
		err_msg
	);
}

#[rstest]
#[tokio::test]
async fn test_create_returns_one_for_string_pk() {
	// Arrange: Create a mock that returns "id" as a string (e.g., UUID)
	use reinhardt_db::backends::{
		backend::DatabaseBackend as BackendTrait,
		connection::DatabaseConnection as BackendsConnection,
		types::{DatabaseType, QueryResult, QueryValue, Row},
	};
	use reinhardt_test::fixtures::mock::MockDatabaseBackend;

	let mut mock = MockDatabaseBackend::new();
	mock.expect_database_type()
		.return_const(DatabaseType::Postgres);
	mock.expect_placeholder()
		.returning(|idx| format!("${}", idx));
	mock.expect_supports_returning().return_const(true);
	mock.expect_supports_on_conflict().return_const(true);
	mock.expect_execute()
		.returning(|_, _| Ok(QueryResult { rows_affected: 1 }));
	mock.expect_fetch_all().returning(|_, _| Ok(Vec::new()));
	mock.expect_fetch_optional().returning(|_, _| Ok(None));

	// Return "id" as a UUID string
	mock.expect_fetch_one().returning(|_, _| {
		let mut row = Row::new();
		row.data
			.insert("id".to_string(), QueryValue::String("uuid-123".to_string()));
		Ok(row)
	});

	let backends_conn = BackendsConnection::new(Arc::new(mock));
	let conn = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db = AdminDatabase::new(conn);

	let mut data = HashMap::new();
	data.insert("name".to_string(), serde_json::json!("Bob"));

	// Act
	let result = db.create::<User>("users", None, data).await;

	// Assert: String PKs (UUIDs) return Ok(1) as affected count
	assert!(
		result.is_ok(),
		"Expected Ok(1) for string PK, got: {:?}",
		result.err()
	);
	assert_eq!(result.unwrap(), 1);
}

// ==================== update/delete affected count tests ====================

#[rstest]
#[tokio::test]
async fn test_update_returns_affected_count(mock_connection: DatabaseConnection) {
	// Arrange
	// Default mock returns rows_affected: 0
	let db = AdminDatabase::new(mock_connection);
	let mut data = HashMap::new();
	data.insert("name".to_string(), serde_json::json!("Updated Name"));

	// Act
	let result = db.update::<User>("users", "id", "999", data).await;

	// Assert
	assert!(result.is_ok());
	assert_eq!(
		result.unwrap(),
		0,
		"Update with mock should return 0 affected rows"
	);
}

#[rstest]
#[tokio::test]
async fn test_delete_returns_affected_count(mock_connection: DatabaseConnection) {
	// Arrange
	// Default mock returns rows_affected: 0
	let db = AdminDatabase::new(mock_connection);

	// Act
	let result = db.delete::<User>("users", "id", "999").await;

	// Assert
	assert!(result.is_ok());
	assert_eq!(
		result.unwrap(),
		0,
		"Delete with mock should return 0 affected rows"
	);
}

#[rstest]
#[tokio::test]
async fn test_bulk_delete_with_many_ids(mock_connection: DatabaseConnection) {
	// Arrange
	let db = AdminDatabase::new(mock_connection);
	let ids: Vec<String> = (1..=10).map(|i| i.to_string()).collect();

	// Act
	let result = db.bulk_delete::<User>("users", "id", ids).await;

	// Assert
	assert!(result.is_ok());
	assert_eq!(
		result.unwrap(),
		0,
		"Bulk delete with mock should return 0 affected rows"
	);
}
