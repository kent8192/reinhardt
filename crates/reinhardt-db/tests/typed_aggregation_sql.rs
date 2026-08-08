#![allow(unexpected_cfgs)]

use reinhardt_core::exception::Error;
use reinhardt_core::macros::model;
use reinhardt_db::orm::{Model, QuerySet, func};
use serde::{Deserialize, Serialize};

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
