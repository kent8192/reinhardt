//! Tests for ORM model fields: expressions, aggregation, annotation,
//! transaction, window functions, and validators.

use reinhardt_db::orm::aggregation::{
	Aggregate, AggregateFunc, AggregateResult, AggregateValue, validate_identifier,
};
use reinhardt_db::orm::annotation::{Annotation, AnnotationValue, Value, When};
use reinhardt_db::orm::expressions::{Exists, F, OuterRef, Q, Subquery};
use reinhardt_db::orm::fields::{
	AutoField, BaseField, BigIntegerField, BooleanField, CharField, DateField, DateTimeField,
	DecimalField, EmailField, Field, FieldArg, FieldDeconstruction, FieldKwarg, FloatField,
	IntegerField, SlugField, TextField, URLField,
};
use reinhardt_db::orm::transaction::{IsolationLevel, Savepoint, Transaction, TransactionState};
use reinhardt_db::orm::validators::{
	EmailValidator, FieldValidators, MaxLengthValidator, MinLengthValidator, ModelValidators,
	RangeValidator, RegexValidator, RequiredValidator, URLValidator, ValidationError, Validator,
};
use reinhardt_db::orm::window::{
	DenseRank, FirstValue, Frame, FrameBoundary, FrameType, Lag, LastValue, Lead, NTile, NthValue,
	Rank, RowNumber, Window, WindowFunction,
};
use rstest::rstest;

// =============================================================================
// F expression tests
// =============================================================================

#[rstest]
fn f_expression_creation_and_to_sql() {
	// Arrange
	let field_name = "price";

	// Act
	let f = F::new(field_name);

	// Assert
	assert_eq!(f.field, "price");
	assert_eq!(f.to_sql(), "\"price\"");
}

#[rstest]
fn f_expression_display() {
	// Arrange
	let f = F::new("user_id");

	// Act
	let display = format!("{}", f);

	// Assert
	assert_eq!(display, "user_id");
}

#[rstest]
#[case("id", "\"id\"")]
#[case("user_name", "\"user_name\"")]
#[case("created_at", "\"created_at\"")]
fn f_expression_various_fields(#[case] field: &str, #[case] expected_sql: &str) {
	// Arrange

	// Act
	let f = F::new(field);

	// Assert
	assert_eq!(f.to_sql(), expected_sql);
}

// =============================================================================
// OuterRef tests
// =============================================================================

#[rstest]
fn outer_ref_creation_and_to_sql() {
	// Arrange
	let field_name = "parent_id";

	// Act
	let outer = OuterRef::new(field_name);

	// Assert
	assert_eq!(outer.field, "parent_id");
	assert_eq!(outer.to_sql(), "parent_id");
}

#[rstest]
fn outer_ref_for_subquery_usage() {
	// Arrange
	let outer = OuterRef::new("user_id");

	// Act
	let sql = outer.to_sql();

	// Assert
	assert_eq!(sql, "user_id");
}

// =============================================================================
// Q expression tests
// =============================================================================

#[rstest]
fn q_simple_condition() {
	// Arrange

	// Act
	let q = Q::new("age", ">=", "18");

	// Assert
	assert_eq!(q.to_sql(), "age >= 18");
}

#[rstest]
fn q_and_combination() {
	// Arrange
	let q1 = Q::new("status", "=", "active");
	let q2 = Q::new("verified", "=", "true");

	// Act
	let combined = q1.and(q2);

	// Assert
	let sql = combined.to_sql();
	assert!(sql.contains("status"));
	assert!(sql.contains("verified"));
	assert!(sql.contains("AND"));
}

#[rstest]
fn q_or_combination() {
	// Arrange
	let q1 = Q::new("role", "=", "admin");
	let q2 = Q::new("role", "=", "moderator");

	// Act
	let combined = q1.or(q2);

	// Assert
	let sql = combined.to_sql();
	assert!(sql.contains("role"));
	assert!(sql.contains("admin"));
	assert!(sql.contains("moderator"));
	assert!(sql.contains("OR"));
}

#[rstest]
fn q_not_negation() {
	// Arrange
	let q = Q::new("deleted", "=", "true");

	// Act
	let negated = q.not();

	// Assert
	let sql = negated.to_sql();
	assert!(sql.contains("NOT"));
}

// =============================================================================
// IsolationLevel tests
// =============================================================================

#[rstest]
#[case(IsolationLevel::ReadUncommitted, "READ UNCOMMITTED")]
#[case(IsolationLevel::ReadCommitted, "READ COMMITTED")]
#[case(IsolationLevel::RepeatableRead, "REPEATABLE READ")]
#[case(IsolationLevel::Serializable, "SERIALIZABLE")]
fn isolation_level_to_sql(#[case] level: IsolationLevel, #[case] expected: &str) {
	// Arrange (provided by case parameters)

	// Act
	let sql = level.to_sql();

	// Assert
	assert_eq!(sql, expected);
}

#[rstest]
fn isolation_level_equality() {
	// Arrange
	let level1 = IsolationLevel::Serializable;
	let level2 = IsolationLevel::Serializable;
	let level3 = IsolationLevel::ReadCommitted;

	// Act

	// Assert
	assert_eq!(level1, level2);
	assert_ne!(level1, level3);
}

// =============================================================================
// TransactionState tests
// =============================================================================

#[rstest]
fn transaction_state_initial() {
	// Arrange

	// Act
	let tx = Transaction::new();

	// Assert
	assert_eq!(tx.state().unwrap(), TransactionState::NotStarted);
	assert_eq!(tx.depth(), 0);
}

#[rstest]
fn transaction_state_begin() {
	// Arrange
	let mut tx = Transaction::new();

	// Act
	let sql = tx.begin().unwrap();

	// Assert
	assert_eq!(sql, "BEGIN TRANSACTION");
	assert_eq!(tx.state().unwrap(), TransactionState::Active);
	assert_eq!(tx.depth(), 1);
}

#[rstest]
fn transaction_state_commit() {
	// Arrange
	let mut tx = Transaction::new();
	tx.begin().unwrap();

	// Act
	let sql = tx.commit().unwrap();

	// Assert
	assert_eq!(sql, "COMMIT");
	assert_eq!(tx.state().unwrap(), TransactionState::Committed);
	assert_eq!(tx.depth(), 0);
}

#[rstest]
fn transaction_state_rollback() {
	// Arrange
	let mut tx = Transaction::new();
	tx.begin().unwrap();

	// Act
	let sql = tx.rollback().unwrap();

	// Assert
	assert_eq!(sql, "ROLLBACK");
	assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
	assert_eq!(tx.depth(), 0);
}

#[rstest]
fn transaction_nested_savepoint() {
	// Arrange
	let mut tx = Transaction::new();
	tx.begin().unwrap();

	// Act
	let nested_sql = tx.begin().unwrap();

	// Assert
	assert!(nested_sql.contains("SAVEPOINT"));
	assert_eq!(tx.depth(), 2);
}

#[rstest]
fn transaction_with_isolation_level() {
	// Arrange
	let mut tx = Transaction::new().with_isolation_level(IsolationLevel::Serializable);

	// Act
	let sql = tx.begin().unwrap();

	// Assert
	assert!(sql.contains("SERIALIZABLE"));
	assert!(sql.contains("BEGIN TRANSACTION ISOLATION LEVEL"));
}

#[rstest]
fn transaction_commit_after_committed_fails() {
	// Arrange
	let mut tx = Transaction::new();
	tx.begin().unwrap();
	tx.commit().unwrap();

	// Act
	let result = tx.commit();

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn transaction_begin_after_committed_fails() {
	// Arrange
	let mut tx = Transaction::new();
	tx.begin().unwrap();
	tx.commit().unwrap();

	// Act
	let result = tx.begin();

	// Assert
	assert!(result.is_err());
}

// =============================================================================
// Savepoint tests
// =============================================================================

#[rstest]
fn savepoint_creation() {
	// Arrange

	// Act
	let sp = Savepoint::new("my_savepoint", 1);

	// Assert
	assert_eq!(sp.name(), "my_savepoint");
	assert_eq!(sp.depth, 1);
}

#[rstest]
fn savepoint_to_sql() {
	// Arrange
	let sp = Savepoint::new("checkpoint_1", 2);

	// Act

	// Assert
	assert_eq!(sp.to_sql(), r#"SAVEPOINT "checkpoint_1""#);
	assert_eq!(sp.release_sql(), r#"RELEASE SAVEPOINT "checkpoint_1""#);
	assert_eq!(sp.rollback_sql(), r#"ROLLBACK TO SAVEPOINT "checkpoint_1""#);
}

#[rstest]
#[should_panic(expected = "Invalid savepoint name")]
fn savepoint_rejects_invalid_name() {
	// Arrange

	// Act
	Savepoint::new("invalid; DROP TABLE", 1);
}

#[rstest]
#[should_panic(expected = "Invalid savepoint name")]
fn savepoint_rejects_empty_name() {
	// Arrange

	// Act
	Savepoint::new("", 1);
}

#[rstest]
#[should_panic(expected = "Invalid savepoint name")]
fn savepoint_rejects_numeric_start() {
	// Arrange

	// Act
	Savepoint::new("123savepoint", 1);
}

// =============================================================================
// AggregateFunc tests
// =============================================================================

#[rstest]
#[case(AggregateFunc::Count, "COUNT")]
#[case(AggregateFunc::CountDistinct, "COUNT")]
#[case(AggregateFunc::Sum, "SUM")]
#[case(AggregateFunc::Avg, "AVG")]
#[case(AggregateFunc::Max, "MAX")]
#[case(AggregateFunc::Min, "MIN")]
fn aggregate_func_display(#[case] func: AggregateFunc, #[case] expected: &str) {
	// Arrange (provided by case parameters)

	// Act
	let display = format!("{}", func);

	// Assert
	assert_eq!(display, expected);
}

// =============================================================================
// Aggregate tests
// =============================================================================

#[rstest]
fn aggregate_count_with_field() {
	// Arrange

	// Act
	let agg = Aggregate::count(Some("id"));

	// Assert
	assert_eq!(agg.to_sql(), "COUNT(id)");
}

#[rstest]
fn aggregate_count_all() {
	// Arrange

	// Act
	let agg = Aggregate::count_all();

	// Assert
	assert_eq!(agg.to_sql(), "COUNT(*)");
}

#[rstest]
fn aggregate_count_distinct() {
	// Arrange

	// Act
	let agg = Aggregate::count_distinct("user_id");

	// Assert
	assert_eq!(agg.to_sql(), "COUNT(DISTINCT user_id)");
	assert!(agg.distinct);
}

#[rstest]
fn aggregate_sum() {
	// Arrange

	// Act
	let agg = Aggregate::sum("amount");

	// Assert
	assert_eq!(agg.to_sql(), "SUM(amount)");
}

#[rstest]
fn aggregate_avg() {
	// Arrange

	// Act
	let agg = Aggregate::avg("score");

	// Assert
	assert_eq!(agg.to_sql(), "AVG(score)");
}

#[rstest]
fn aggregate_max() {
	// Arrange

	// Act
	let agg = Aggregate::max("price");

	// Assert
	assert_eq!(agg.to_sql(), "MAX(price)");
}

#[rstest]
fn aggregate_min() {
	// Arrange

	// Act
	let agg = Aggregate::min("age");

	// Assert
	assert_eq!(agg.to_sql(), "MIN(age)");
}

#[rstest]
fn aggregate_with_alias() {
	// Arrange

	// Act
	let agg = Aggregate::sum("amount").with_alias("total_amount");

	// Assert
	assert_eq!(agg.to_sql(), "SUM(amount) AS total_amount");
}

#[rstest]
fn aggregate_to_sql_expr_without_alias() {
	// Arrange
	let agg = Aggregate::sum("amount").with_alias("total_amount");

	// Act
	let expr_sql = agg.to_sql_expr();

	// Assert
	assert_eq!(expr_sql, "SUM(amount)");
}

#[rstest]
#[should_panic(expected = "Invalid field name")]
fn aggregate_rejects_invalid_field() {
	// Arrange

	// Act
	Aggregate::sum("amount; DROP TABLE users");
}

#[rstest]
#[should_panic(expected = "Invalid alias")]
fn aggregate_rejects_invalid_alias() {
	// Arrange

	// Act
	Aggregate::sum("amount").with_alias("total; DROP TABLE");
}

// =============================================================================
// AggregateResult / AggregateValue tests
// =============================================================================

#[rstest]
fn aggregate_result_insert_and_get() {
	// Arrange
	let mut result = AggregateResult::new();

	// Act
	result.insert("count".to_string(), AggregateValue::Int(42));
	result.insert("avg".to_string(), AggregateValue::Float(3.14));
	result.insert("null_val".to_string(), AggregateValue::Null);

	// Assert
	assert!(matches!(result.get("count"), Some(AggregateValue::Int(42))));
	assert!(matches!(
		result.get("avg"),
		Some(AggregateValue::Float(f)) if (*f - 3.14).abs() < f64::EPSILON
	));
	assert!(matches!(result.get("null_val"), Some(AggregateValue::Null)));
	assert!(result.get("nonexistent").is_none());
}

#[rstest]
fn aggregate_result_default() {
	// Arrange

	// Act
	let result = AggregateResult::default();

	// Assert
	assert!(result.values.is_empty());
}

// =============================================================================
// validate_identifier tests
// =============================================================================

#[rstest]
#[case("user_id", true)]
#[case("name123", true)]
#[case("_internal", true)]
#[case("*", true)]
#[case("", false)]
#[case("123invalid", false)]
#[case("user-id", false)]
#[case("user; DROP TABLE", false)]
fn validate_identifier_cases(#[case] input: &str, #[case] should_be_ok: bool) {
	// Arrange (provided by case parameters)

	// Act
	let result = validate_identifier(input);

	// Assert
	assert_eq!(result.is_ok(), should_be_ok);
}

// =============================================================================
// Annotation / Expression / Value tests
// =============================================================================

#[rstest]
fn annotation_value_int() {
	// Arrange
	let val = Value::Int(42);

	// Act
	let sql = val.to_sql();

	// Assert
	assert_eq!(sql, "42");
}

#[rstest]
fn annotation_value_float() {
	// Arrange
	let val = Value::Float(3.14);

	// Act
	let sql = val.to_sql();

	// Assert
	assert_eq!(sql, "3.14");
}

#[rstest]
fn annotation_value_string() {
	// Arrange
	let val = Value::String("hello".to_string());

	// Act
	let sql = val.to_sql();

	// Assert
	assert_eq!(sql, "'hello'");
}

#[rstest]
fn annotation_value_bool() {
	// Arrange

	// Act

	// Assert
	assert_eq!(Value::Bool(true).to_sql(), "TRUE");
	assert_eq!(Value::Bool(false).to_sql(), "FALSE");
}

#[rstest]
fn annotation_value_null() {
	// Arrange
	let val = Value::Null;

	// Act
	let sql = val.to_sql();

	// Assert
	assert_eq!(sql, "NULL");
}

#[rstest]
fn annotation_creation_and_to_sql() {
	// Arrange
	let annotation = Annotation::new("total", AnnotationValue::Value(Value::Int(100)));

	// Act
	let sql = annotation.to_sql();

	// Assert
	assert_eq!(annotation.alias, "total");
	assert_eq!(sql, "100 AS \"total\"");
}

#[rstest]
fn annotation_with_field_reference() {
	// Arrange
	let f = F::new("price");
	let annotation = Annotation::new("price_ref", AnnotationValue::Field(f));

	// Act
	let sql = annotation.to_sql();

	// Assert
	assert_eq!(sql, "\"price\" AS \"price_ref\"");
}

#[rstest]
fn annotation_with_aggregate() {
	// Arrange
	let agg = Aggregate::count_all();
	let annotation = Annotation::new("item_count", AnnotationValue::Aggregate(agg));

	// Act
	let sql = annotation.to_sql();

	// Assert
	assert_eq!(sql, "COUNT(*) AS \"item_count\"");
}

#[rstest]
fn when_clause_to_sql() {
	// Arrange
	let when = When::new(
		Q::new("status", "=", "active"),
		AnnotationValue::Value(Value::Int(1)),
	);

	// Act
	let sql = when.to_sql();

	// Assert
	assert!(sql.contains("WHEN"));
	assert!(sql.contains("THEN"));
	assert!(sql.contains("status"));
	assert!(sql.contains("active"));
	assert!(sql.contains("1"));
}

// =============================================================================
// Subquery / Exists tests
// =============================================================================

#[rstest]
fn subquery_creation_and_to_sql() {
	// Arrange
	let sq = Subquery::new("SELECT id FROM users WHERE active = 1");

	// Act
	let sql = sq.to_sql();

	// Assert
	assert!(sql.starts_with("("));
	assert!(sql.ends_with(")"));
	assert!(sql.contains("SELECT id FROM users"));
}

#[rstest]
fn exists_to_sql() {
	// Arrange
	let exists = Exists::new("SELECT 1 FROM orders WHERE user_id = 123");

	// Act
	let sql = exists.to_sql();

	// Assert
	assert!(sql.starts_with("EXISTS("));
	assert!(sql.contains("SELECT 1 FROM orders"));
}

// =============================================================================
// Window function tests
// =============================================================================

#[rstest]
#[case(FrameType::Range, "RANGE")]
#[case(FrameType::Rows, "ROWS")]
#[case(FrameType::Groups, "GROUPS")]
fn frame_type_to_sql(#[case] frame_type: FrameType, #[case] expected: &str) {
	// Arrange (provided by case parameters)

	// Act

	// Assert
	assert_eq!(frame_type.to_sql(), expected);
}

#[rstest]
fn frame_boundary_to_sql() {
	// Arrange

	// Act

	// Assert
	assert_eq!(
		FrameBoundary::UnboundedPreceding.to_sql(),
		"UNBOUNDED PRECEDING"
	);
	assert_eq!(FrameBoundary::Preceding(5).to_sql(), "5 PRECEDING");
	assert_eq!(FrameBoundary::CurrentRow.to_sql(), "CURRENT ROW");
	assert_eq!(FrameBoundary::Following(3).to_sql(), "3 FOLLOWING");
	assert_eq!(
		FrameBoundary::UnboundedFollowing.to_sql(),
		"UNBOUNDED FOLLOWING"
	);
}

#[rstest]
fn frame_rows_to_sql() {
	// Arrange
	let frame = Frame::rows(
		FrameBoundary::UnboundedPreceding,
		Some(FrameBoundary::CurrentRow),
	);

	// Act
	let sql = frame.to_sql();

	// Assert
	assert_eq!(sql, "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW");
}

#[rstest]
fn frame_range_to_sql() {
	// Arrange
	let frame = Frame::range(
		FrameBoundary::Preceding(3),
		Some(FrameBoundary::Following(3)),
	);

	// Act
	let sql = frame.to_sql();

	// Assert
	assert_eq!(sql, "RANGE BETWEEN 3 PRECEDING AND 3 FOLLOWING");
}

#[rstest]
fn frame_groups_to_sql() {
	// Arrange
	let frame = Frame::groups(
		FrameBoundary::CurrentRow,
		Some(FrameBoundary::UnboundedFollowing),
	);

	// Act
	let sql = frame.to_sql();

	// Assert
	assert_eq!(sql, "GROUPS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING");
}

#[rstest]
fn frame_without_end_defaults_to_current_row() {
	// Arrange
	let frame = Frame::rows(FrameBoundary::Preceding(1), None);

	// Act
	let sql = frame.to_sql();

	// Assert
	assert_eq!(sql, "ROWS BETWEEN 1 PRECEDING AND CURRENT ROW");
}

#[rstest]
fn window_empty() {
	// Arrange
	let window = Window::new();

	// Act

	// Assert
	assert_eq!(window.to_sql(), "");
}

#[rstest]
fn window_partition_by() {
	// Arrange
	let window = Window::new().partition_by("department");

	// Act

	// Assert
	assert_eq!(window.to_sql(), "PARTITION BY department");
}

#[rstest]
fn window_order_by() {
	// Arrange
	let window = Window::new().order_by("salary DESC");

	// Act

	// Assert
	assert_eq!(window.to_sql(), "ORDER BY salary DESC");
}

#[rstest]
fn window_partition_and_order() {
	// Arrange
	let window = Window::new()
		.partition_by("department")
		.order_by("salary DESC");

	// Act

	// Assert
	assert_eq!(
		window.to_sql(),
		"PARTITION BY department ORDER BY salary DESC"
	);
}

#[rstest]
fn window_with_frame() {
	// Arrange
	let frame = Frame::rows(
		FrameBoundary::Preceding(1),
		Some(FrameBoundary::Following(1)),
	);
	let window = Window::new()
		.partition_by("department")
		.order_by("date")
		.frame(frame);

	// Act

	// Assert
	assert_eq!(
		window.to_sql(),
		"PARTITION BY department ORDER BY date ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING"
	);
}

#[rstest]
fn window_default() {
	// Arrange

	// Act
	let window = Window::default();

	// Assert
	assert_eq!(window.to_sql(), "");
}

// =============================================================================
// WindowFunction variant tests
// =============================================================================

#[rstest]
fn row_number_to_sql() {
	// Arrange
	let window = Window::new()
		.partition_by("department")
		.order_by("hire_date");
	let row_num = RowNumber::new();

	// Act
	let sql = row_num.to_sql(&window);

	// Assert
	assert_eq!(
		sql,
		"ROW_NUMBER() OVER (PARTITION BY department ORDER BY hire_date)"
	);
}

#[rstest]
fn rank_to_sql() {
	// Arrange
	let window = Window::new()
		.partition_by("department")
		.order_by("salary DESC");
	let rank = Rank::new();

	// Act
	let sql = rank.to_sql(&window);

	// Assert
	assert_eq!(
		sql,
		"RANK() OVER (PARTITION BY department ORDER BY salary DESC)"
	);
}

#[rstest]
fn dense_rank_to_sql() {
	// Arrange
	let window = Window::new().order_by("score DESC");
	let dense_rank = DenseRank::new();

	// Act
	let sql = dense_rank.to_sql(&window);

	// Assert
	assert_eq!(sql, "DENSE_RANK() OVER (ORDER BY score DESC)");
}

#[rstest]
fn ntile_to_sql() {
	// Arrange
	let window = Window::new().order_by("salary");
	let ntile = NTile::new(4);

	// Act
	let sql = ntile.to_sql(&window);

	// Assert
	assert_eq!(sql, "NTILE(4) OVER (ORDER BY salary)");
}

#[rstest]
fn lead_to_sql() {
	// Arrange
	let window = Window::new().order_by("date");
	let lead = Lead::new("value");

	// Act
	let sql = lead.to_sql(&window);

	// Assert
	assert_eq!(sql, "LEAD(value, 1) OVER (ORDER BY date)");
}

#[rstest]
fn lead_with_offset_and_default() {
	// Arrange
	let window = Window::new().order_by("date");
	let lead = Lead::new("value").offset(2).default("0");

	// Act
	let sql = lead.to_sql(&window);

	// Assert
	assert_eq!(sql, "LEAD(value, 2, 0) OVER (ORDER BY date)");
}

#[rstest]
fn lag_to_sql() {
	// Arrange
	let window = Window::new().order_by("date");
	let lag = Lag::new("value");

	// Act
	let sql = lag.to_sql(&window);

	// Assert
	assert_eq!(sql, "LAG(value, 1) OVER (ORDER BY date)");
}

#[rstest]
fn lag_with_offset_and_default() {
	// Arrange
	let window = Window::new().order_by("date");
	let lag = Lag::new("value").offset(3).default("0");

	// Act
	let sql = lag.to_sql(&window);

	// Assert
	assert_eq!(sql, "LAG(value, 3, 0) OVER (ORDER BY date)");
}

#[rstest]
fn first_value_to_sql() {
	// Arrange
	let window = Window::new()
		.partition_by("department")
		.order_by("salary DESC");
	let first_val = FirstValue::new("salary");

	// Act
	let sql = first_val.to_sql(&window);

	// Assert
	assert_eq!(
		sql,
		"FIRST_VALUE(salary) OVER (PARTITION BY department ORDER BY salary DESC)"
	);
}

#[rstest]
fn last_value_to_sql() {
	// Arrange
	let window = Window::new()
		.partition_by("department")
		.order_by("salary DESC");
	let last_val = LastValue::new("salary");

	// Act
	let sql = last_val.to_sql(&window);

	// Assert
	assert_eq!(
		sql,
		"LAST_VALUE(salary) OVER (PARTITION BY department ORDER BY salary DESC)"
	);
}

#[rstest]
fn nth_value_to_sql() {
	// Arrange
	let window = Window::new().order_by("salary DESC");
	let nth_val = NthValue::new("salary", 2);

	// Act
	let sql = nth_val.to_sql(&window);

	// Assert
	assert_eq!(sql, "NTH_VALUE(salary, 2) OVER (ORDER BY salary DESC)");
}

#[rstest]
fn row_number_default() {
	// Arrange

	// Act
	let row_num = RowNumber::default();

	// Assert (verify it creates the same as new())
	let window = Window::new().order_by("id");
	assert_eq!(row_num.to_sql(&window), "ROW_NUMBER() OVER (ORDER BY id)");
}

// =============================================================================
// ORM Validator tests
// =============================================================================

#[rstest]
fn required_validator_accepts_non_empty() {
	// Arrange
	let validator = RequiredValidator::new();

	// Act

	// Assert
	assert!(validator.validate("some text").is_ok());
}

#[rstest]
#[case("")]
#[case("   ")]
#[case("\t")]
fn required_validator_rejects_empty(#[case] input: &str) {
	// Arrange
	let validator = RequiredValidator::new();

	// Act

	// Assert
	assert!(validator.validate(input).is_err());
}

#[rstest]
fn required_validator_custom_message() {
	// Arrange
	let validator = RequiredValidator::with_message("Username is required");

	// Act

	// Assert
	assert_eq!(validator.message(), "Username is required");
}

#[rstest]
fn max_length_validator_accepts_short_string() {
	// Arrange
	let validator = MaxLengthValidator::new(10);

	// Act

	// Assert
	assert!(validator.validate("hello").is_ok());
	assert!(validator.validate("1234567890").is_ok());
}

#[rstest]
fn max_length_validator_rejects_long_string() {
	// Arrange
	let validator = MaxLengthValidator::new(10);

	// Act

	// Assert
	assert!(validator.validate("12345678901").is_err());
}

#[rstest]
fn min_length_validator_accepts_long_enough() {
	// Arrange
	let validator = MinLengthValidator::new(3);

	// Act

	// Assert
	assert!(validator.validate("hello").is_ok());
	assert!(validator.validate("abc").is_ok());
}

#[rstest]
fn min_length_validator_rejects_too_short() {
	// Arrange
	let validator = MinLengthValidator::new(3);

	// Act

	// Assert
	assert!(validator.validate("hi").is_err());
}

#[rstest]
fn email_validator_valid_emails() {
	// Arrange
	let validator = EmailValidator::new();

	// Act

	// Assert
	assert!(validator.validate("user@example.com").is_ok());
	assert!(validator.validate("user.name+tag@example.co.uk").is_ok());
	assert!(validator.validate("first.last@sub.example.com").is_ok());
}

#[rstest]
#[case("invalid")]
#[case("@example.com")]
#[case("user@")]
#[case("")]
fn email_validator_invalid_emails(#[case] input: &str) {
	// Arrange
	let validator = EmailValidator::new();

	// Act

	// Assert
	assert!(validator.validate(input).is_err());
}

#[rstest]
fn url_validator_valid_urls() {
	// Arrange
	let validator = URLValidator::new();

	// Act

	// Assert
	assert!(validator.validate("https://example.com").is_ok());
	assert!(validator.validate("http://example.com/path").is_ok());
}

#[rstest]
#[case("example.com")]
#[case("ftp://example.com")]
#[case("")]
fn url_validator_invalid_urls(#[case] input: &str) {
	// Arrange
	let validator = URLValidator::new();

	// Act

	// Assert
	assert!(validator.validate(input).is_err());
}

#[rstest]
fn regex_validator_valid_pattern() {
	// Arrange
	let validator = RegexValidator::new(r"^\d{3}-\d{4}$");

	// Act

	// Assert
	assert!(validator.validate("123-4567").is_ok());
	assert!(validator.validate("abc-defg").is_err());
}

#[rstest]
fn regex_validator_try_new_valid() {
	// Arrange

	// Act
	let result = RegexValidator::try_new(r"^\d+$");

	// Assert
	assert!(result.is_ok());
	let validator = result.unwrap();
	assert!(validator.validate("123").is_ok());
}

#[rstest]
fn regex_validator_try_new_invalid() {
	// Arrange

	// Act
	let result = RegexValidator::try_new(r"[invalid(regex");

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn regex_validator_pattern_accessor() {
	// Arrange
	let validator = RegexValidator::new(r"^\d+$");

	// Act

	// Assert
	assert_eq!(validator.pattern(), r"^\d+$");
}

#[rstest]
fn range_validator_within_range() {
	// Arrange
	let validator = RangeValidator::new(Some(0), Some(100));

	// Act

	// Assert
	assert!(validator.validate("50").is_ok());
	assert!(validator.validate("0").is_ok());
	assert!(validator.validate("100").is_ok());
}

#[rstest]
fn range_validator_outside_range() {
	// Arrange
	let validator = RangeValidator::new(Some(0), Some(100));

	// Act

	// Assert
	assert!(validator.validate("-1").is_err());
	assert!(validator.validate("101").is_err());
}

#[rstest]
fn range_validator_non_numeric_input() {
	// Arrange
	let validator = RangeValidator::new(Some(0), Some(100));

	// Act

	// Assert
	assert!(validator.validate("abc").is_err());
}

#[rstest]
fn validation_error_creation() {
	// Arrange

	// Act
	let error = ValidationError::new("email", "Enter a valid email address", "invalid_email");

	// Assert
	assert_eq!(error.field, "email");
	assert_eq!(error.message, "Enter a valid email address");
	assert_eq!(error.code, "invalid_email");
}

#[rstest]
fn validation_error_display() {
	// Arrange
	let error = ValidationError::new("email", "Invalid email", "invalid");

	// Act
	let display = format!("{}", error);

	// Assert
	assert!(display.contains("email"));
	assert!(display.contains("Invalid email"));
	assert!(display.contains("invalid"));
}

#[rstest]
fn field_validators_chain() {
	// Arrange
	let validators = FieldValidators::new()
		.with_validator(Box::new(RequiredValidator::new()))
		.with_validator(Box::new(MaxLengthValidator::new(10)));

	// Act

	// Assert
	assert!(validators.validate("hello").is_ok());
	assert!(validators.validate("").is_err());
	assert!(validators.validate("12345678901").is_err());
}

#[rstest]
fn model_validators_validate_field() {
	// Arrange
	let mut model_validators = ModelValidators::new();
	let email_validators = FieldValidators::new().with_validator(Box::new(EmailValidator::new()));
	model_validators.add_field_validator("email".to_string(), email_validators);

	// Act

	// Assert
	assert!(
		model_validators
			.validate("email", "test@example.com")
			.is_ok()
	);
	assert!(model_validators.validate("email", "invalid").is_err());
}

#[rstest]
fn model_validators_validate_all() {
	// Arrange
	let mut model_validators = ModelValidators::new();
	let username_validators =
		FieldValidators::new().with_validator(Box::new(MinLengthValidator::new(3)));
	let email_validators = FieldValidators::new().with_validator(Box::new(EmailValidator::new()));
	model_validators.add_field_validator("username".to_string(), username_validators);
	model_validators.add_field_validator("email".to_string(), email_validators);

	let mut data = std::collections::HashMap::new();
	data.insert("username".to_string(), "ab".to_string());
	data.insert("email".to_string(), "invalid".to_string());

	// Act
	let errors = model_validators.validate_all(&data);

	// Assert
	assert_eq!(errors.len(), 2);
}

#[rstest]
fn model_validators_validate_unregistered_field_passes() {
	// Arrange
	let model_validators = ModelValidators::new();

	// Act
	let result = model_validators.validate("nonexistent", "any_value");

	// Assert
	assert!(result.is_ok());
}

// =============================================================================
// BaseField tests
// =============================================================================

#[rstest]
fn base_field_new_defaults() {
	// Arrange

	// Act
	let field = BaseField::new();

	// Assert
	assert!(!field.null);
	assert!(!field.blank);
	assert!(!field.primary_key);
	assert!(!field.unique);
	assert!(field.editable);
	assert!(field.name.is_none());
	assert!(field.default.is_none());
	assert!(field.db_default.is_none());
	assert!(field.db_column.is_none());
	assert!(field.db_tablespace.is_none());
	assert!(field.choices.is_none());
}

#[rstest]
fn base_field_default_trait_matches_new() {
	// Arrange

	// Act
	let from_new = BaseField::new();
	let from_default = BaseField::default();

	// Assert
	assert_eq!(from_new.null, from_default.null);
	assert_eq!(from_new.blank, from_default.blank);
	assert_eq!(from_new.primary_key, from_default.primary_key);
	assert_eq!(from_new.unique, from_default.unique);
	assert_eq!(from_new.editable, from_default.editable);
}

#[rstest]
fn base_field_get_kwargs_empty_for_defaults() {
	// Arrange
	let field = BaseField::new();

	// Act
	let kwargs = field.get_kwargs();

	// Assert
	assert!(kwargs.is_empty());
}

#[rstest]
fn base_field_get_kwargs_with_custom_settings() {
	// Arrange
	let mut field = BaseField::new();
	field.null = true;
	field.blank = true;
	field.unique = true;
	field.primary_key = true;
	field.editable = false;

	// Act
	let kwargs = field.get_kwargs();

	// Assert
	assert_eq!(kwargs.get("null"), Some(&FieldKwarg::Bool(true)));
	assert_eq!(kwargs.get("blank"), Some(&FieldKwarg::Bool(true)));
	assert_eq!(kwargs.get("unique"), Some(&FieldKwarg::Bool(true)));
	assert_eq!(kwargs.get("primary_key"), Some(&FieldKwarg::Bool(true)));
	assert_eq!(kwargs.get("editable"), Some(&FieldKwarg::Bool(false)));
}

#[rstest]
fn base_field_get_kwargs_with_choices() {
	// Arrange
	let mut field = BaseField::new();
	let choices = vec![
		("draft".to_string(), "Draft".to_string()),
		("published".to_string(), "Published".to_string()),
	];
	field.choices = Some(choices.clone());

	// Act
	let kwargs = field.get_kwargs();

	// Assert
	assert_eq!(kwargs.get("choices"), Some(&FieldKwarg::Choices(choices)));
}

#[rstest]
fn base_field_get_kwargs_with_default_value() {
	// Arrange
	let mut field = BaseField::new();
	field.default = Some(FieldKwarg::String("hello".to_string()));

	// Act
	let kwargs = field.get_kwargs();

	// Assert
	assert_eq!(
		kwargs.get("default"),
		Some(&FieldKwarg::String("hello".to_string()))
	);
}

#[rstest]
fn base_field_get_kwargs_with_db_column() {
	// Arrange
	let mut field = BaseField::new();
	field.db_column = Some("custom_col".to_string());

	// Act
	let kwargs = field.get_kwargs();

	// Assert
	assert_eq!(
		kwargs.get("db_column"),
		Some(&FieldKwarg::String("custom_col".to_string()))
	);
}

#[rstest]
fn base_field_get_kwargs_with_db_default() {
	// Arrange
	let mut field = BaseField::new();
	field.db_default = Some(FieldKwarg::Int(0));

	// Act
	let kwargs = field.get_kwargs();

	// Assert
	assert_eq!(kwargs.get("db_default"), Some(&FieldKwarg::Int(0)));
}

#[rstest]
fn base_field_get_kwargs_with_db_tablespace() {
	// Arrange
	let mut field = BaseField::new();
	field.db_tablespace = Some("fast_storage".to_string());

	// Act
	let kwargs = field.get_kwargs();

	// Assert
	assert_eq!(
		kwargs.get("db_tablespace"),
		Some(&FieldKwarg::String("fast_storage".to_string()))
	);
}

// =============================================================================
// AutoField tests
// =============================================================================

#[rstest]
fn auto_field_new_is_primary_key() {
	// Arrange

	// Act
	let field = AutoField::new();

	// Assert
	assert!(field.base.primary_key);
	assert!(field.is_primary_key());
}

#[rstest]
fn auto_field_set_attributes_from_name() {
	// Arrange
	let mut field = AutoField::new();

	// Act
	field.set_attributes_from_name("id");

	// Assert
	assert_eq!(field.name(), Some("id"));
}

#[rstest]
fn auto_field_deconstruct() {
	// Arrange
	let mut field = AutoField::new();
	field.set_attributes_from_name("id");

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.name, Some("id".to_string()));
	assert_eq!(dec.path, "reinhardt.orm.models.AutoField");
	assert!(dec.args.is_empty());
	assert_eq!(dec.kwargs.get("primary_key"), Some(&FieldKwarg::Bool(true)));
}

#[rstest]
fn auto_field_name_none_before_set() {
	// Arrange

	// Act
	let field = AutoField::new();

	// Assert
	assert_eq!(field.name(), None);
}

#[rstest]
fn auto_field_default_trait() {
	// Arrange

	// Act
	let field = AutoField::default();

	// Assert
	assert!(field.is_primary_key());
}

// =============================================================================
// CharField tests
// =============================================================================

#[rstest]
fn char_field_new_with_max_length() {
	// Arrange

	// Act
	let field = CharField::new(255);

	// Assert
	assert_eq!(field.max_length, 255);
	assert!(!field.base.null);
	assert!(!field.base.blank);
}

#[rstest]
fn char_field_deconstruct_includes_max_length() {
	// Arrange
	let field = CharField::new(150);

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.CharField");
	assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(150)));
}

#[rstest]
fn char_field_with_null_blank() {
	// Arrange

	// Act
	let field = CharField::with_null_blank(100);

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.kwargs.get("null"), Some(&FieldKwarg::Bool(true)));
	assert_eq!(dec.kwargs.get("blank"), Some(&FieldKwarg::Bool(true)));
	assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(100)));
}

#[rstest]
fn char_field_with_choices() {
	// Arrange
	let choices = vec![
		("active".to_string(), "Active".to_string()),
		("inactive".to_string(), "Inactive".to_string()),
	];

	// Act
	let field = CharField::with_choices(20, choices.clone());
	let dec = field.deconstruct();

	// Assert
	assert_eq!(
		dec.kwargs.get("choices"),
		Some(&FieldKwarg::Choices(choices))
	);
	assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(20)));
}

// =============================================================================
// BigIntegerField tests
// =============================================================================

#[rstest]
fn big_integer_field_new_and_deconstruct() {
	// Arrange
	let mut field = BigIntegerField::new();
	field.set_attributes_from_name("population");

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.BigIntegerField");
	assert_eq!(dec.name, Some("population".to_string()));
	assert!(dec.args.is_empty());
}

#[rstest]
fn big_integer_field_default_trait() {
	// Arrange

	// Act
	let field = BigIntegerField::default();

	// Assert
	assert!(field.base.name.is_none());
	assert!(!field.base.null);
}

// =============================================================================
// BooleanField tests
// =============================================================================

#[rstest]
fn boolean_field_new_and_deconstruct() {
	// Arrange

	// Act
	let field = BooleanField::new();
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.BooleanField");
	assert!(dec.kwargs.is_empty());
}

#[rstest]
fn boolean_field_with_default_true() {
	// Arrange

	// Act
	let field = BooleanField::with_default(true);
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.kwargs.get("default"), Some(&FieldKwarg::Bool(true)));
}

#[rstest]
fn boolean_field_with_default_false() {
	// Arrange

	// Act
	let field = BooleanField::with_default(false);
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.kwargs.get("default"), Some(&FieldKwarg::Bool(false)));
}

// =============================================================================
// FloatField tests
// =============================================================================

#[rstest]
fn float_field_new_and_deconstruct() {
	// Arrange
	let mut field = FloatField::new();
	field.set_attributes_from_name("rating");

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.FloatField");
	assert_eq!(dec.name, Some("rating".to_string()));
}

// =============================================================================
// TextField tests
// =============================================================================

#[rstest]
fn text_field_new_and_deconstruct() {
	// Arrange
	let mut field = TextField::new();
	field.set_attributes_from_name("description");

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.TextField");
	assert_eq!(dec.name, Some("description".to_string()));
	assert!(dec.kwargs.is_empty());
}

// =============================================================================
// DateTimeField tests
// =============================================================================

#[rstest]
fn datetime_field_new_no_auto() {
	// Arrange

	// Act
	let field = DateTimeField::new();

	// Assert
	assert!(!field.auto_now);
	assert!(!field.auto_now_add);
}

#[rstest]
fn datetime_field_with_auto_now_add() {
	// Arrange

	// Act
	let field = DateTimeField::with_auto_now_add();
	let dec = field.deconstruct();

	// Assert
	assert!(!field.auto_now);
	assert!(field.auto_now_add);
	assert_eq!(
		dec.kwargs.get("auto_now_add"),
		Some(&FieldKwarg::Bool(true))
	);
	assert!(dec.kwargs.get("auto_now").is_none());
}

#[rstest]
fn datetime_field_with_both() {
	// Arrange

	// Act
	let field = DateTimeField::with_both();
	let dec = field.deconstruct();

	// Assert
	assert!(field.auto_now);
	assert!(field.auto_now_add);
	assert_eq!(dec.kwargs.get("auto_now"), Some(&FieldKwarg::Bool(true)));
	assert_eq!(
		dec.kwargs.get("auto_now_add"),
		Some(&FieldKwarg::Bool(true))
	);
}

#[rstest]
fn datetime_field_deconstruct_path() {
	// Arrange

	// Act
	let field = DateTimeField::new();
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.DateTimeField");
}

// =============================================================================
// DecimalField tests
// =============================================================================

#[rstest]
fn decimal_field_new_with_precision() {
	// Arrange

	// Act
	let field = DecimalField::new(10, 2);

	// Assert
	assert_eq!(field.max_digits, 10);
	assert_eq!(field.decimal_places, 2);
}

#[rstest]
fn decimal_field_deconstruct_includes_precision_kwargs() {
	// Arrange
	let field = DecimalField::new(8, 3);

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.DecimalField");
	assert_eq!(dec.kwargs.get("max_digits"), Some(&FieldKwarg::Uint(8)));
	assert_eq!(dec.kwargs.get("decimal_places"), Some(&FieldKwarg::Uint(3)));
}

// =============================================================================
// SlugField tests
// =============================================================================

#[rstest]
fn slug_field_new_defaults() {
	// Arrange

	// Act
	let field = SlugField::new();

	// Assert
	assert_eq!(field.max_length, 50);
	assert!(field.db_index);
}

#[rstest]
fn slug_field_deconstruct_omits_defaults() {
	// Arrange
	let field = SlugField::new();

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.SlugField");
	// Default max_length (50) and default db_index (true) are omitted
	assert!(dec.kwargs.get("max_length").is_none());
	assert!(dec.kwargs.get("db_index").is_none());
}

#[rstest]
fn slug_field_with_custom_options() {
	// Arrange

	// Act
	let field = SlugField::with_options(100, false);
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(100)));
	assert_eq!(dec.kwargs.get("db_index"), Some(&FieldKwarg::Bool(false)));
}

// =============================================================================
// EmailField tests
// =============================================================================

#[rstest]
fn email_field_new_default_max_length() {
	// Arrange

	// Act
	let field = EmailField::new();

	// Assert
	assert_eq!(field.max_length, 254);
}

#[rstest]
fn email_field_with_custom_max_length() {
	// Arrange

	// Act
	let field = EmailField::with_max_length(100);

	// Assert
	assert_eq!(field.max_length, 100);
}

#[rstest]
fn email_field_deconstruct() {
	// Arrange
	let field = EmailField::new();

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.EmailField");
	assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(254)));
}

// =============================================================================
// URLField tests
// =============================================================================

#[rstest]
fn url_field_new_default_max_length() {
	// Arrange

	// Act
	let field = URLField::new();

	// Assert
	assert_eq!(field.max_length, 200);
}

#[rstest]
fn url_field_deconstruct_omits_default_max_length() {
	// Arrange
	let field = URLField::new();

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.URLField");
	// Default max_length (200) is omitted from kwargs
	assert!(dec.kwargs.get("max_length").is_none());
}

#[rstest]
fn url_field_with_custom_max_length_deconstruct() {
	// Arrange
	let field = URLField::with_max_length(500);

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(500)));
}

// =============================================================================
// IntegerField tests
// =============================================================================

#[rstest]
fn integer_field_new_and_deconstruct() {
	// Arrange
	let mut field = IntegerField::new();
	field.set_attributes_from_name("age");

	// Act
	let dec = field.deconstruct();

	// Assert
	assert_eq!(dec.path, "reinhardt.orm.models.IntegerField");
	assert_eq!(dec.name, Some("age".to_string()));
}

#[rstest]
fn integer_field_with_choices() {
	// Arrange
	let choices = vec![
		("1".to_string(), "Option A".to_string()),
		("2".to_string(), "Option B".to_string()),
	];

	// Act
	let field = IntegerField::with_choices(choices.clone());
	let dec = field.deconstruct();

	// Assert
	assert_eq!(
		dec.kwargs.get("choices"),
		Some(&FieldKwarg::Choices(choices))
	);
}

// =============================================================================
// FieldDeconstruction struct tests
// =============================================================================

#[rstest]
fn field_deconstruction_struct_fields() {
	// Arrange

	// Act
	let dec = FieldDeconstruction {
		name: Some("test_field".to_string()),
		path: "reinhardt.orm.models.CharField".to_string(),
		args: vec![FieldArg::Int(42)],
		kwargs: std::collections::HashMap::new(),
	};

	// Assert
	assert_eq!(dec.name, Some("test_field".to_string()));
	assert_eq!(dec.path, "reinhardt.orm.models.CharField");
	assert_eq!(dec.args.len(), 1);
	assert!(dec.kwargs.is_empty());
}

// =============================================================================
// FieldArg variant tests
// =============================================================================

#[rstest]
fn field_arg_variants() {
	// Arrange

	// Act

	// Assert
	assert_eq!(
		FieldArg::String("hello".to_string()),
		FieldArg::String("hello".to_string())
	);
	assert_eq!(FieldArg::Int(42), FieldArg::Int(42));
	assert_eq!(FieldArg::Bool(true), FieldArg::Bool(true));
	assert_eq!(FieldArg::Float(3.14), FieldArg::Float(3.14));
}

#[rstest]
fn field_arg_inequality() {
	// Arrange

	// Act

	// Assert
	assert_ne!(FieldArg::Int(1), FieldArg::Int(2));
	assert_ne!(FieldArg::String("a".to_string()), FieldArg::Bool(true));
}

// =============================================================================
// FieldKwarg variant tests
// =============================================================================

#[rstest]
fn field_kwarg_variants() {
	// Arrange

	// Act

	// Assert
	assert_eq!(
		FieldKwarg::String("test".to_string()),
		FieldKwarg::String("test".to_string())
	);
	assert_eq!(FieldKwarg::Int(100), FieldKwarg::Int(100));
	assert_eq!(FieldKwarg::Uint(200), FieldKwarg::Uint(200));
	assert_eq!(FieldKwarg::Bool(false), FieldKwarg::Bool(false));
	assert_eq!(FieldKwarg::Float(1.5), FieldKwarg::Float(1.5));
	assert_eq!(
		FieldKwarg::Callable("my_func".to_string()),
		FieldKwarg::Callable("my_func".to_string())
	);
}

#[rstest]
fn field_kwarg_choices_variant() {
	// Arrange
	let choices = vec![("a".to_string(), "Alpha".to_string())];

	// Act

	// Assert
	assert_eq!(
		FieldKwarg::Choices(choices.clone()),
		FieldKwarg::Choices(choices)
	);
}

// =============================================================================
// Field trait default method tests (is_null, is_blank)
// =============================================================================

#[rstest]
fn field_trait_is_null_default_false() {
	// Arrange
	let field = CharField::new(100);

	// Act

	// Assert
	assert!(!field.is_null());
}

#[rstest]
fn field_trait_is_blank_default_false() {
	// Arrange
	let field = IntegerField::new();

	// Act

	// Assert
	assert!(!field.is_blank());
}

#[rstest]
fn field_trait_is_primary_key_default_false() {
	// Arrange
	let field = CharField::new(100);

	// Act

	// Assert
	assert!(!field.is_primary_key());
}

// =============================================================================
// DateField tests
// =============================================================================

#[rstest]
fn date_field_new_no_auto() {
	// Arrange

	// Act
	let field = DateField::new();

	// Assert
	assert!(!field.auto_now);
	assert!(!field.auto_now_add);
}

#[rstest]
fn date_field_with_auto_now() {
	// Arrange

	// Act
	let field = DateField::with_auto_now();
	let dec = field.deconstruct();

	// Assert
	assert!(field.auto_now);
	assert!(!field.auto_now_add);
	assert_eq!(dec.path, "reinhardt.orm.models.DateField");
	assert_eq!(dec.kwargs.get("auto_now"), Some(&FieldKwarg::Bool(true)));
}
