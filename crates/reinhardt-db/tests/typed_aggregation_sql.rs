#![allow(unexpected_cfgs)]

use async_trait::async_trait;
use reinhardt_core::exception::{DatabaseErrorKind, Error};
use reinhardt_core::macros::model;
use reinhardt_db::orm::aggregation::Aggregate;
use reinhardt_db::orm::annotation::{Annotation, AnnotationValue};
use reinhardt_db::orm::{
	AggregateDateTime, AggregateValue, DatabaseBackend, Filter, FilterOperator, FilterValue,
	GroupByFields, Model, OrmExecutor, QueryResult, QuerySet, QueryValue, Row, func,
};
use serde::{Deserialize, Serialize};

#[path = "ui/typed_aggregation/support.rs"]
mod aggregate_support;

#[model(
	app_label = "typed_annotation_sql",
	table_name = "typed_annotation_records"
)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TypedAnnotationRecord {
	#[field(primary_key = true)]
	id: i64,
	#[field(db_column = "display_name", max_length = 255)]
	name: String,
	value: i64,
}

struct RecordingExecutor {
	sql: Option<String>,
	params: Vec<QueryValue>,
	fetch_one_row: Option<Row>,
}

impl RecordingExecutor {
	fn postgres() -> Self {
		Self {
			sql: None,
			params: Vec::new(),
			fetch_one_row: None,
		}
	}

	fn with_fetch_one(mut self, row: Row) -> Self {
		self.fetch_one_row = Some(row);
		self
	}
}

#[async_trait]
impl OrmExecutor for RecordingExecutor {
	fn backend(&self) -> DatabaseBackend {
		DatabaseBackend::Postgres
	}

	async fn execute(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		Err(reinhardt_core::exception::DatabaseError::new(
			reinhardt_core::exception::DatabaseErrorKind::Unsupported,
			"typed aggregation SQL tests do not execute mutations",
		)
		.into())
	}

	async fn fetch_one(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Row> {
		self.sql = Some(sql.to_owned());
		self.params = params;
		self.fetch_one_row.take().ok_or_else(|| {
			reinhardt_core::exception::DatabaseError::new(
				reinhardt_core::exception::DatabaseErrorKind::Query,
				"typed aggregation SQL test did not queue a fetch_one row",
			)
			.into()
		})
	}

	async fn fetch_all(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		self.sql = Some(sql.to_owned());
		self.params = params;
		Ok(Vec::new())
	}

	async fn fetch_optional(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		Ok(None)
	}
}

#[test]
fn aggregate_annotation_groups_root_columns_and_renders_exact_sql() {
	let query = QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid annotation label"),
		)
		.expect("aggregate annotation should be accepted")
		.to_sql()
		.expect("query should compile");

	assert_eq!(
		query,
		r#"SELECT *, COUNT(*) AS "record_count" FROM "typed_annotation_records" GROUP BY "typed_annotation_records"."id", "typed_annotation_records"."display_name", "typed_annotation_records"."value""#
	);
}

#[test]
fn scalar_and_composed_annotations_render_exact_sql() {
	let query = QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			TypedAnnotationRecord::field_name()
				.into_expression()
				.label("name_copy")
				.expect("valid physical-column annotation label"),
		)
		.expect("physical-column annotation should be accepted")
		.annotate(
			TypedAnnotationRecord::field_value()
				.into_expression()
				.label("value_copy")
				.expect("valid scalar annotation label"),
		)
		.expect("scalar annotation should be accepted")
		.annotate(
			(TypedAnnotationRecord::field_value().into_expression()
				+ func::literal::<TypedAnnotationRecord, _>(1_i64)
					.expect("integer literal should encode"))
			.label("value_plus_one")
			.expect("valid arithmetic annotation label"),
		)
		.expect("arithmetic annotation should be accepted")
		.annotate(
			func::case_when(
				TypedAnnotationRecord::field_value()
					.into_expression()
					.gt(0_i64),
				TypedAnnotationRecord::field_value().into_expression(),
			)
			.otherwise(
				func::literal::<TypedAnnotationRecord, _>(0_i64)
					.expect("integer literal should encode"),
			)
			.label("positive_value")
			.expect("valid case annotation label"),
		)
		.expect("case annotation should be accepted")
		.annotate(
			func::coalesce(
				TypedAnnotationRecord::field_value().into_expression(),
				func::literal::<TypedAnnotationRecord, _>(0_i64)
					.expect("integer literal should encode"),
			)
			.label("value_or_zero")
			.expect("valid coalesce annotation label"),
		)
		.expect("coalesce annotation should be accepted")
		.to_sql()
		.expect("query should compile");

	assert_eq!(
		query,
		r#"SELECT *, "typed_annotation_records"."display_name" AS "name_copy", "typed_annotation_records"."value" AS "value_copy", "typed_annotation_records"."value" + 1 AS "value_plus_one", CASE WHEN "typed_annotation_records"."value" > 0 THEN "typed_annotation_records"."value" ELSE 0 END AS "positive_value", COALESCE("typed_annotation_records"."value", 0) AS "value_or_zero" FROM "typed_annotation_records""#
	);
}

#[test]
fn aggregate_annotations_group_scalar_annotations_once() {
	let scalar = TypedAnnotationRecord::field_value().into_expression();
	let query = QuerySet::<TypedAnnotationRecord>::new()
		.values(&["id"])
		.annotate(scalar.clone().label("first_value").expect("valid label"))
		.expect("first scalar annotation should be accepted")
		.annotate(scalar.label("second_value").expect("valid label"))
		.expect("second scalar annotation should be accepted")
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid aggregate label"),
		)
		.expect("aggregate annotation should be accepted")
		.to_sql()
		.expect("query should compile");

	assert_eq!(
		query,
		r#"SELECT "id", "typed_annotation_records"."value" AS "first_value", "typed_annotation_records"."value" AS "second_value", COUNT(*) AS "record_count" FROM "typed_annotation_records" GROUP BY "typed_annotation_records"."id", "typed_annotation_records"."value""#
	);
}

#[test]
fn related_field_annotation_adds_a_left_join() {
	use aggregate_support::{ModelRecord, RelatedRecord};

	let query = QuerySet::<ModelRecord>::new()
		.annotate(
			ModelRecord::rel_related()
				.field(RelatedRecord::field_i64())
				.into_expression()
				.label("related_value")
				.expect("valid relation annotation label"),
		)
		.expect("related annotation should be accepted")
		.to_sql()
		.expect("query should compile");

	assert_eq!(
		query,
		r#"SELECT "model_records".*, "related"."value_i64" AS "related_value" FROM "model_records" LEFT JOIN "related_records" AS "related" ON "model_records"."related_id" = "related"."id""#
	);
}

#[test]
fn multiple_aggregate_annotations_render_together() {
	let query = QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid aggregate label"),
		)
		.expect("count annotation should be accepted")
		.annotate(
			func::sum(TypedAnnotationRecord::field_value())
				.label("value_total")
				.expect("valid aggregate label"),
		)
		.expect("sum annotation should be accepted")
		.to_sql()
		.expect("query should compile");

	assert_eq!(
		query,
		r#"SELECT *, COUNT(*) AS "record_count", SUM("typed_annotation_records"."value") AS "value_total" FROM "typed_annotation_records" GROUP BY "typed_annotation_records"."id", "typed_annotation_records"."display_name", "typed_annotation_records"."value""#
	);
}

#[test]
fn typed_having_uses_the_aggregate_expression_compiler() {
	let query = QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid aggregate label"),
		)
		.expect("aggregate annotation should be accepted")
		.having(func::avg(TypedAnnotationRecord::field_value()).gt(4.0))
		.to_sql()
		.expect("query should compile");

	assert_eq!(
		query,
		r#"SELECT *, COUNT(*) AS "record_count" FROM "typed_annotation_records" GROUP BY "typed_annotation_records"."id", "typed_annotation_records"."display_name", "typed_annotation_records"."value" HAVING AVG("typed_annotation_records"."value") > 4"#
	);
}

#[tokio::test]
async fn typed_having_binds_comparison_values_for_execution() {
	let mut executor = RecordingExecutor::postgres();

	QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid aggregate label"),
		)
		.expect("aggregate annotation should be accepted")
		.having(func::avg(TypedAnnotationRecord::field_value()).gt(4.25_f64))
		.rows_with_db(&mut executor)
		.await
		.expect("recording executor should receive the query");

	assert_eq!(
		executor.sql.as_deref(),
		Some(
			r##"SELECT *, COUNT(*) AS "record_count" FROM "typed_annotation_records" GROUP BY "typed_annotation_records"."id", "typed_annotation_records"."display_name", "typed_annotation_records"."value" HAVING AVG("typed_annotation_records"."value") > $1"##
		)
	);
	assert_eq!(executor.params, vec![QueryValue::Float(4.25)]);
}

#[test]
fn typed_having_supports_decimal_aggregate_comparisons() {
	use aggregate_support::ModelRecord;

	let query = QuerySet::<ModelRecord>::new()
		.having(func::avg(ModelRecord::field_decimal()).gt(rust_decimal::Decimal::new(425, 2)))
		.to_sql()
		.expect("decimal aggregate comparison should compile");

	assert_eq!(
		query,
		r##"SELECT * FROM "model_records" HAVING AVG("model_records"."value_decimal") > 4.25"##
	);
}

#[test]
fn typed_having_supports_combined_aggregate_arithmetic() {
	let query = QuerySet::<TypedAnnotationRecord>::new()
		.having(
			(func::sum(TypedAnnotationRecord::field_value())
				+ func::literal::<TypedAnnotationRecord, _>(1_i64)
					.expect("integer literal should encode"))
			.gt(10_i64),
		)
		.to_sql()
		.expect("combined aggregate arithmetic should compile");

	assert_eq!(
		query,
		r##"SELECT * FROM "typed_annotation_records" HAVING SUM("typed_annotation_records"."value") + 1 > 10"##
	);
}

#[test]
fn typed_having_supports_count_sum_min_max_and_multiple_conditions() {
	let query = QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid aggregate label"),
		)
		.expect("aggregate annotation should be accepted")
		.having(func::count_all::<TypedAnnotationRecord>().gt(1_i64))
		.having(func::sum(TypedAnnotationRecord::field_value()).ge(2_i64))
		.having(func::min(TypedAnnotationRecord::field_value()).lt(9_i64))
		.having(func::max(TypedAnnotationRecord::field_value()).ne(0_i64))
		.to_sql()
		.expect("query should compile");

	assert_eq!(
		query,
		r#"SELECT *, COUNT(*) AS "record_count" FROM "typed_annotation_records" GROUP BY "typed_annotation_records"."id", "typed_annotation_records"."display_name", "typed_annotation_records"."value" HAVING COUNT(*) > 1 AND SUM("typed_annotation_records"."value") >= 2 AND MIN("typed_annotation_records"."value") < 9 AND MAX("typed_annotation_records"."value") <> 0"#
	);
}

#[test]
fn typed_having_relation_aggregate_adds_a_left_join() {
	use aggregate_support::{ModelRecord, RelatedRecord};

	let query = QuerySet::<ModelRecord>::new()
		.annotate(
			func::count_all::<ModelRecord>()
				.label("record_count")
				.expect("valid aggregate label"),
		)
		.expect("aggregate annotation should be accepted")
		.having(func::count(ModelRecord::rel_related().field(RelatedRecord::field_i64())).gt(1_i64))
		.to_sql()
		.expect("query should compile");

	assert_eq!(
		query,
		r##"SELECT "model_records".*, COUNT(*) AS "record_count" FROM "model_records" LEFT JOIN "related_records" AS "related" ON "model_records"."related_id" = "related"."id" HAVING COUNT("related"."value_i64") > 1"##
	);
}

#[test]
fn annotation_rejects_invalid_and_colliding_labels() {
	let invalid = func::count_all::<TypedAnnotationRecord>()
		.label("record-count")
		.expect_err("invalid labels must fail without panic");
	assert!(matches!(
		invalid,
		Error::Validation(message)
			if message == "aggregate label must contain only ASCII letters, digits, or underscores"
	));

	let rust_field = match QuerySet::<TypedAnnotationRecord>::new().annotate(
		func::count_all::<TypedAnnotationRecord>()
			.label("name")
			.expect("label syntax is valid"),
	) {
		Ok(_) => panic!("Rust field labels must be rejected"),
		Err(error) => error,
	};
	assert_eq!(
		rust_field.to_string(),
		"Validation error: annotation label `name` collides with model field `name`"
	);

	let physical_field = match QuerySet::<TypedAnnotationRecord>::new().annotate(
		func::count_all::<TypedAnnotationRecord>()
			.label("display_name")
			.expect("label syntax is valid"),
	) {
		Ok(_) => panic!("physical field labels must be rejected"),
		Err(error) => error,
	};
	assert_eq!(
		physical_field.to_string(),
		"Validation error: annotation label `display_name` collides with model field `name`"
	);

	let duplicate = match QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("first_count")
				.expect("label syntax is valid"),
		)
		.expect("first annotation should be accepted")
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("first_count")
				.expect("label syntax is valid"),
		) {
		Ok(_) => panic!("duplicate labels must be rejected"),
		Err(error) => error,
	};
	assert_eq!(
		duplicate.to_string(),
		"Validation error: annotation label `first_count` is already in use"
	);
}

#[tokio::test]
async fn terminal_aggregate_rejects_composed_aggregate_before_fetching() {
	let composed = (func::sum(TypedAnnotationRecord::field_value())
		+ func::literal::<TypedAnnotationRecord, _>(1_i64).expect("integer literal should encode"))
	.label("value_total")
	.expect("valid aggregate label");
	let mut executor = RecordingExecutor::postgres();

	let error = QuerySet::<TypedAnnotationRecord>::new()
		.aggregate_with_db(composed, &mut executor)
		.await
		.expect_err("composed aggregate roots are unsupported for terminal decoding");

	assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Unsupported));
	assert_eq!(
		error.to_string(),
		"Database error: terminal aggregate expressions must be a single aggregate function or COUNT(*)"
	);
	assert!(executor.sql.is_none());
	assert!(executor.params.is_empty());
}

#[tokio::test]
async fn terminal_aggregate_validates_empty_input_before_opening_connection() {
	let error = QuerySet::<TypedAnnotationRecord>::new()
		.aggregate([])
		.await
		.expect_err("empty terminal aggregate input must fail validation");

	assert!(matches!(
		error,
		Error::Validation(message)
			if message == "aggregate input must contain at least one labeled expression"
	));
}

#[tokio::test]
async fn terminal_aggregate_fetches_one_row_and_decodes_multiple_labels() {
	let mut row = Row::new();
	row.insert("record_count".to_owned(), QueryValue::Int(4));
	row.insert(
		"value_total".to_owned(),
		QueryValue::String("42".to_owned()),
	);
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);

	let result = QuerySet::<TypedAnnotationRecord>::new()
		.aggregate_with_db(
			[
				func::count_all::<TypedAnnotationRecord>()
					.label("record_count")
					.expect("valid count label"),
				func::sum(TypedAnnotationRecord::field_value())
					.label("value_total")
					.expect("valid sum label"),
			],
			&mut executor,
		)
		.await
		.expect("terminal aggregate should decode the queued row");

	assert_eq!(result.get_i64("record_count").expect("count value"), 4);
	assert_eq!(result.get_i64("value_total").expect("sum value"), 42);
	assert_eq!(
		executor.sql.as_deref(),
		Some(
			r#"SELECT COUNT(*) AS "record_count", SUM("typed_annotation_records"."value") AS "value_total" FROM "typed_annotation_records""#
		)
	);
	assert!(executor.params.is_empty());
}

#[tokio::test]
async fn terminal_aggregate_normalizes_supported_min_max_scalars() {
	use aggregate_support::ModelRecord;

	let timestamp = chrono::DateTime::parse_from_rfc3339("2024-01-02T03:04:05Z")
		.expect("valid timestamp")
		.with_timezone(&chrono::Utc);
	let naive_timestamp = chrono::NaiveDate::from_ymd_opt(2024, 1, 2)
		.expect("valid date")
		.and_hms_opt(3, 4, 5)
		.expect("valid time");
	let uuid = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid UUID");
	let mut row = Row::new();
	row.insert(
		"first_name".to_owned(),
		QueryValue::String("Alice".to_owned()),
	);
	row.insert("first_uuid".to_owned(), QueryValue::Uuid(uuid));
	row.insert(
		"first_date".to_owned(),
		QueryValue::String("2024-01-02".to_owned()),
	);
	row.insert(
		"first_time".to_owned(),
		QueryValue::String("03:04:05.000000".to_owned()),
	);
	row.insert(
		"first_timestamp".to_owned(),
		QueryValue::Timestamp(timestamp),
	);
	row.insert(
		"first_timestamp_from_naive".to_owned(),
		QueryValue::NaiveTimestamp(naive_timestamp),
	);
	row.insert(
		"first_naive_timestamp".to_owned(),
		QueryValue::NaiveTimestamp(naive_timestamp),
	);
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);

	let result = QuerySet::<ModelRecord>::new()
		.aggregate_with_db(
			[
				func::min(ModelRecord::field_name())
					.label("first_name")
					.expect("valid string label"),
				func::min(ModelRecord::field_uuid())
					.label("first_uuid")
					.expect("valid UUID label"),
				func::min(ModelRecord::field_date())
					.label("first_date")
					.expect("valid date label"),
				func::min(ModelRecord::field_time())
					.label("first_time")
					.expect("valid time label"),
				func::min(ModelRecord::field_datetime())
					.label("first_timestamp")
					.expect("valid UTC timestamp label"),
				func::min(ModelRecord::field_datetime())
					.label("first_timestamp_from_naive")
					.expect("valid UTC timestamp label for naive driver value"),
				func::min(ModelRecord::field_naive_datetime())
					.label("first_naive_timestamp")
					.expect("valid naive timestamp label"),
			],
			&mut executor,
		)
		.await
		.expect("terminal aggregate should decode structured scalar values");

	assert_eq!(
		result.get("first_name").expect("string value"),
		&AggregateValue::String("Alice".to_owned())
	);
	assert_eq!(
		result.get("first_uuid").expect("UUID value"),
		&AggregateValue::Uuid(uuid)
	);
	assert_eq!(
		result.get("first_date").expect("date value"),
		&AggregateValue::Date(chrono::NaiveDate::from_ymd_opt(2024, 1, 2).expect("valid date"))
	);
	assert_eq!(
		result.get("first_time").expect("time value"),
		&AggregateValue::Time(chrono::NaiveTime::from_hms_opt(3, 4, 5).expect("valid time"))
	);
	assert_eq!(
		result.get("first_timestamp").expect("UTC timestamp value"),
		&AggregateValue::DateTime(AggregateDateTime::Utc(timestamp))
	);
	assert_eq!(
		result
			.get("first_timestamp_from_naive")
			.expect("naive driver timestamp value"),
		&AggregateValue::DateTime(AggregateDateTime::Utc(naive_timestamp.and_utc()))
	);
	assert_eq!(
		result
			.get("first_naive_timestamp")
			.expect("naive timestamp value"),
		&AggregateValue::DateTime(AggregateDateTime::Naive(naive_timestamp))
	);
}

#[tokio::test]
async fn terminal_aggregate_reports_serialization_context_for_bad_rows() {
	let mut row = Row::new();
	row.insert(
		"value_total".to_owned(),
		QueryValue::String("9223372036854775808".to_owned()),
	);
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);
	let error = QuerySet::<TypedAnnotationRecord>::new()
		.aggregate_with_db(
			func::sum(TypedAnnotationRecord::field_value())
				.label("value_total")
				.expect("valid sum label"),
			&mut executor,
		)
		.await
		.expect_err("overflow must be reported as serialization failure");

	assert!(matches!(
		error,
		Error::Serialization(message)
			if message.contains("aggregate function SUM")
				&& message.contains("label 'value_total'")
				&& message.contains("backend Postgres")
	));

	let mut missing_executor = RecordingExecutor::postgres().with_fetch_one(Row::new());
	let missing = QuerySet::<TypedAnnotationRecord>::new()
		.aggregate_with_db(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid count label"),
			&mut missing_executor,
		)
		.await
		.expect_err("missing labels must be reported");
	assert!(matches!(
		missing,
		Error::Serialization(message)
			if message.contains("aggregate function COUNT")
				&& message.contains("label 'record_count'")
				&& message.contains("backend Postgres")
	));

	let mut unexpected_row = Row::new();
	unexpected_row.insert(
		"record_count".to_owned(),
		QueryValue::String("four".to_owned()),
	);
	let mut unexpected_executor = RecordingExecutor::postgres().with_fetch_one(unexpected_row);
	let unexpected = QuerySet::<TypedAnnotationRecord>::new()
		.aggregate_with_db(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid count label"),
			&mut unexpected_executor,
		)
		.await
		.expect_err("unexpected value kinds must be reported");
	assert!(matches!(
		unexpected,
		Error::Serialization(message)
			if message.contains("aggregate function COUNT")
				&& message.contains("label 'record_count'")
				&& message.contains("backend Postgres")
	));
}

#[tokio::test]
async fn terminal_aggregate_preserves_null_and_none_short_circuits() {
	let mut row = Row::new();
	row.insert("value_average".to_owned(), QueryValue::Null);
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);
	let result = QuerySet::<TypedAnnotationRecord>::new()
		.aggregate_with_db(
			func::avg(TypedAnnotationRecord::field_value())
				.label("value_average")
				.expect("valid average label"),
			&mut executor,
		)
		.await
		.expect("non-COUNT SQL NULL should be preserved");
	assert_eq!(
		result.get("value_average").expect("average value"),
		&AggregateValue::Null
	);

	let mut none_executor = RecordingExecutor::postgres();
	let result = QuerySet::<TypedAnnotationRecord>::new()
		.none()
		.aggregate_with_db(
			[
				func::count_all::<TypedAnnotationRecord>()
					.label("record_count")
					.expect("valid count label"),
				func::sum(TypedAnnotationRecord::field_value())
					.label("value_total")
					.expect("valid sum label"),
			],
			&mut none_executor,
		)
		.await
		.expect("none aggregate should synthesize values");
	assert_eq!(result.get_i64("record_count").expect("count value"), 0);
	assert_eq!(
		result.get("value_total").expect("sum value"),
		&AggregateValue::Null
	);
	assert!(none_executor.sql.is_none());
}

#[tokio::test]
async fn terminal_aggregate_sliced_query_uses_derived_source() {
	let mut row = Row::new();
	row.insert(
		"value_total".to_owned(),
		QueryValue::String("42".to_owned()),
	);
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);
	QuerySet::<TypedAnnotationRecord>::new()
		.filter(Filter::new(
			"status",
			FilterOperator::Eq,
			FilterValue::String("paid".to_owned()),
		))
		.order_by(&["-value"])
		.offset(10)
		.limit(5)
		.aggregate_with_db(
			func::sum(TypedAnnotationRecord::field_value())
				.label("value_total")
				.expect("valid label"),
			&mut executor,
		)
		.await
		.expect("sliced aggregate should execute");
	assert_eq!(
		executor.sql.as_deref(),
		Some(
			r##"SELECT SUM("__reinhardt_aggregate_source"."__reinhardt_aggregate_operand_0") AS "value_total" FROM (SELECT "typed_annotation_records"."id", "typed_annotation_records"."value" AS "__reinhardt_aggregate_operand_0" FROM "typed_annotation_records" WHERE "status" = $1 ORDER BY "value" DESC LIMIT $2 OFFSET $3) AS "__reinhardt_aggregate_source""##
		)
	);
	assert_eq!(
		executor.params,
		vec![
			QueryValue::String("paid".to_owned()),
			QueryValue::Int(5),
			QueryValue::Int(10),
		]
	);
}

#[tokio::test]
async fn terminal_aggregate_distinct_query_uses_inner_distinct() {
	let mut row = Row::new();
	row.insert("record_count".to_owned(), QueryValue::Int(2));
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);
	QuerySet::<TypedAnnotationRecord>::new()
		.distinct()
		.aggregate_with_db(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid label"),
			&mut executor,
		)
		.await
		.expect("distinct aggregate should execute");
	assert_eq!(
		executor.sql.as_deref(),
		Some(
			r##"SELECT COUNT(*) AS "record_count" FROM (SELECT DISTINCT "typed_annotation_records"."id" FROM "typed_annotation_records") AS "__reinhardt_aggregate_source""##
		)
	);
}

#[tokio::test]
async fn terminal_aggregate_distinct_query_projects_ordering_columns() {
	let mut row = Row::new();
	row.insert("record_count".to_owned(), QueryValue::Int(2));
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);
	QuerySet::<TypedAnnotationRecord>::new()
		.distinct()
		.order_by(&["-value"])
		.aggregate_with_db(
			func::count_all::<TypedAnnotationRecord>()
				.label("record_count")
				.expect("valid label"),
			&mut executor,
		)
		.await
		.expect("distinct ordered aggregate should execute");
	assert_eq!(
		executor.sql.as_deref(),
		Some(
			r##"SELECT COUNT(*) AS "record_count" FROM (SELECT DISTINCT "typed_annotation_records"."id", "value" FROM "typed_annotation_records" ORDER BY "value" DESC) AS "__reinhardt_aggregate_source""##
		)
	);
}

#[tokio::test]
async fn terminal_aggregate_sliced_related_operand_keeps_left_join() {
	use aggregate_support::{ModelRecord, RelatedRecord};
	let mut row = Row::new();
	row.insert("related_count".to_owned(), QueryValue::Int(3));
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);
	QuerySet::<ModelRecord>::new()
		.limit(2)
		.aggregate_with_db(
			func::count(ModelRecord::rel_related().field(RelatedRecord::field_i64()))
				.label("related_count")
				.expect("valid relation label"),
			&mut executor,
		)
		.await
		.expect("related sliced aggregate should execute");
	assert_eq!(
		executor.sql.as_deref(),
		Some(
			r##"SELECT COUNT("__reinhardt_aggregate_source"."__reinhardt_aggregate_operand_0") AS "related_count" FROM (SELECT "model_records"."id", "related"."value_i64" AS "__reinhardt_aggregate_operand_0" FROM "model_records" LEFT JOIN "related_records" AS "related" ON "model_records"."related_id" = "related"."id" LIMIT $1) AS "__reinhardt_aggregate_source""##
		)
	);
}

#[tokio::test]
async fn terminal_aggregate_rejects_annotations_and_locking_shapes() {
	let aggregate = func::count_all::<TypedAnnotationRecord>()
		.label("record_count")
		.expect("valid label");
	let annotated = TypedAnnotationRecord::objects().annotate(Annotation::new(
		"legacy_count",
		AnnotationValue::Aggregate(Aggregate::count_all()),
	));
	let mut executor = RecordingExecutor::postgres();
	let annotation_error = annotated
		.aggregate_with_db(aggregate.clone(), &mut executor)
		.await
		.expect_err("legacy annotations must be rejected");
	assert_eq!(
		annotation_error.database_kind(),
		Some(DatabaseErrorKind::Unsupported)
	);
	assert_eq!(
		annotation_error
			.database_error()
			.expect("database error")
			.message(),
		"terminal aggregate cannot run on a QuerySet containing annotations"
	);

	let mut executor = RecordingExecutor::postgres();
	let locking_error = QuerySet::<TypedAnnotationRecord>::new()
		.select_for_update()
		.aggregate_with_db(aggregate, &mut executor)
		.await
		.expect_err("locking querysets must be rejected");
	assert_eq!(
		locking_error.database_kind(),
		Some(DatabaseErrorKind::Unsupported)
	);
	assert_eq!(
		locking_error
			.database_error()
			.expect("database error")
			.message(),
		"terminal aggregate cannot run on a QuerySet containing row locking"
	);
}

#[tokio::test]
async fn terminal_aggregate_rejects_grouping_and_having_shapes() {
	let aggregate = func::count_all::<TypedAnnotationRecord>()
		.label("record_count")
		.expect("valid label");
	let mut executor = RecordingExecutor::postgres();
	let grouped = QuerySet::<TypedAnnotationRecord>::new()
		.group_by(|fields| GroupByFields::new().add(&fields.value));
	let grouping_error = grouped
		.aggregate_with_db(aggregate.clone(), &mut executor)
		.await
		.expect_err("grouped querysets must be rejected");
	assert_eq!(
		grouping_error.database_kind(),
		Some(DatabaseErrorKind::Unsupported)
	);
	assert_eq!(
		grouping_error
			.database_error()
			.expect("database error")
			.message(),
		"terminal aggregate cannot run on a QuerySet containing GROUP BY"
	);

	let mut executor = RecordingExecutor::postgres();
	let having = QuerySet::<TypedAnnotationRecord>::new()
		.having(func::count_all::<TypedAnnotationRecord>().gt(0_i64));
	let having_error = having
		.aggregate_with_db(aggregate, &mut executor)
		.await
		.expect_err("having querysets must be rejected");
	assert_eq!(
		having_error.database_kind(),
		Some(DatabaseErrorKind::Unsupported)
	);
	assert_eq!(
		having_error
			.database_error()
			.expect("database error")
			.message(),
		"terminal aggregate cannot run on a QuerySet containing HAVING"
	);
}

#[tokio::test]
async fn terminal_aggregate_omits_eager_loading_but_keeps_manual_join() {
	use aggregate_support::{ModelRecord, RelatedRecord};
	let mut row = Row::new();
	row.insert("record_count".to_owned(), QueryValue::Int(1));
	let mut executor = RecordingExecutor::postgres().with_fetch_one(row);
	QuerySet::<ModelRecord>::new()
		.select_related(["related"])
		.prefetch_related(["related"])
		.inner_join_on::<RelatedRecord>("model_records.id = related_records.id")
		.limit(1)
		.aggregate_with_db(
			func::count_all::<ModelRecord>()
				.label("record_count")
				.expect("valid label"),
			&mut executor,
		)
		.await
		.expect("manual joins should remain valid");
	let sql = executor.sql.expect("recorded SQL");
	assert!(
		sql.contains("INNER JOIN \"related_records\" ON model_records.id = related_records.id")
	);
	assert!(!sql.contains("related\".*"));
}
