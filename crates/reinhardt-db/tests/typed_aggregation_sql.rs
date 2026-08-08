#![allow(unexpected_cfgs)]

use async_trait::async_trait;
use reinhardt_core::exception::Error;
use reinhardt_core::macros::model;
use reinhardt_db::orm::{
	DatabaseBackend, Model, OrmExecutor, QueryResult, QuerySet, QueryValue, Row, func,
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
	#[field(db_column = "display_name")]
	name: String,
	value: i64,
}

struct RecordingExecutor {
	sql: Option<String>,
	params: Vec<QueryValue>,
}

impl RecordingExecutor {
	fn postgres() -> Self {
		Self {
			sql: None,
			params: Vec::new(),
		}
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
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Row> {
		Err(reinhardt_core::exception::DatabaseError::new(
			reinhardt_core::exception::DatabaseErrorKind::Unsupported,
			"typed aggregation SQL tests fetch all rows",
		)
		.into())
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
		r#"SELECT *, "typed_annotation_records"."display_name" AS "name_copy", "typed_annotation_records"."value" AS "value_copy", ("typed_annotation_records"."value" + 1) AS "value_plus_one", CASE WHEN "typed_annotation_records"."value" > 0 THEN "typed_annotation_records"."value" ELSE 0 END AS "positive_value", COALESCE("typed_annotation_records"."value", 0) AS "value_or_zero" FROM "typed_annotation_records""#
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
		r#"SELECT "typed_annotation_records"."id", "typed_annotation_records"."value" AS "first_value", "typed_annotation_records"."value" AS "second_value", COUNT(*) AS "record_count" FROM "typed_annotation_records" GROUP BY "typed_annotation_records"."id", "typed_annotation_records"."value""#
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
		r#"SELECT *, "related"."value_i64" AS "related_value" FROM "model_records" LEFT JOIN "related_records" AS "related" ON "model_records"."related_id" = "related"."id""#
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
		r##"SELECT * FROM "typed_annotation_records" HAVING (SUM("typed_annotation_records"."value") + 1) > 10"##
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
		r#"SELECT *, COUNT(*) AS "record_count" FROM "typed_annotation_records" GROUP BY "typed_annotation_records"."id", "typed_annotation_records"."display_name", "typed_annotation_records"."value" HAVING (COUNT(*) > 1 AND SUM("typed_annotation_records"."value") >= 2 AND MIN("typed_annotation_records"."value") < 9 AND MAX("typed_annotation_records"."value") <> 0)"#
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

	let rust_field = QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("name")
				.expect("label syntax is valid"),
		)
		.expect_err("Rust field labels must be rejected");
	assert_eq!(
		rust_field.to_string(),
		"Validation error: annotation label `name` collides with model field `name`"
	);

	let physical_field = QuerySet::<TypedAnnotationRecord>::new()
		.annotate(
			func::count_all::<TypedAnnotationRecord>()
				.label("display_name")
				.expect("label syntax is valid"),
		)
		.expect_err("physical field labels must be rejected");
	assert_eq!(
		physical_field.to_string(),
		"Validation error: annotation label `display_name` collides with model field `name`"
	);

	let duplicate = QuerySet::<TypedAnnotationRecord>::new()
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
		)
		.expect_err("duplicate labels must be rejected");
	assert_eq!(
		duplicate.to_string(),
		"Validation error: annotation label `first_count` is already in use"
	);
}
