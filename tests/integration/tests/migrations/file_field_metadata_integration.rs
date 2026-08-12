//! End-to-end migration metadata coverage for storage-backed file fields.
//!
//! This deliberately exercises the generated model registration and
//! inspection surfaces instead of checking proc-macro output text. The same
//! values must survive `FieldInfo`, `FieldMetadata`, and `ModelState` while
//! PostgreSQL's physical `storage` setting remains distinct from the logical
//! file storage alias.

#![allow(unexpected_cfgs)]

use reinhardt_core::macros::model;
use reinhardt_db::migrations::{FieldType, model_registry::global_registry};
use reinhardt_db::orm::{Model, fields::FieldKwarg};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[model(
	app_label = "file_field_metadata_integration",
	table_name = "file_field_metadata_assets"
)]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct FileFieldMetadataAsset {
	#[field(primary_key = true)]
	id: i64,
	#[field(
		upload_to = "avatars/%Y/%m/%d",
		file_storage = "private_uploads",
		max_length = 255,
		storage = "external"
	)]
	avatar: reinhardt_db::orm::FileField,
}

#[rstest]
fn generated_file_field_metadata_stays_distinct_across_registration_layers() {
	// Arrange: read the actual `#[model]`-generated inspection and migration
	// registrations rather than reproducing their values by hand.
	let inspection = FileFieldMetadataAsset::field_metadata()
		.into_iter()
		.find(|field| field.name == "avatar")
		.expect("generated inspection metadata for avatar");
	let registered = global_registry()
		.get_model("file_field_metadata_integration", "FileFieldMetadataAsset")
		.expect("generated migration registration for file field");
	let field_metadata = registered
		.fields
		.get("avatar")
		.expect("registered FieldMetadata for avatar");

	// Act: convert the registered metadata into the state consumed by the
	// migration autodetector.
	let model_state = registered.to_model_state();
	let field_state = model_state
		.fields
		.get("avatar")
		.expect("ModelState field for avatar");

	// Assert: inspection carries both logical and physical channels.
	assert_eq!(
		inspection.attributes.get("file_storage"),
		Some(&FieldKwarg::String("private_uploads".to_string()))
	);
	assert_eq!(
		inspection.attributes.get("upload_to"),
		Some(&FieldKwarg::String("avatars/%Y/%m/%d".to_string()))
	);
	assert_eq!(
		inspection.attributes.get("max_length"),
		Some(&FieldKwarg::Uint(255))
	);
	assert_eq!(
		inspection.attributes.get("storage"),
		Some(&FieldKwarg::String("external".to_string()))
	);

	// Assert: registration preserves the same values as migration parameters.
	assert_eq!(
		field_metadata
			.params
			.get("model_field_type")
			.map(String::as_str),
		Some("file")
	);
	for (key, expected) in [
		("upload_to", "avatars/%Y/%m/%d"),
		("file_storage", "private_uploads"),
		("max_length", "255"),
		("storage", "external"),
	] {
		assert_eq!(
			field_metadata.params.get(key).map(String::as_str),
			Some(expected),
			"FieldMetadata must preserve `{key}`"
		);
	}

	// Assert: ModelState keeps the bounded physical type and both metadata
	// channels independently; `storage` is not conflated with `file_storage`.
	assert_eq!(field_state.field_type, FieldType::VarChar(255));
	assert_eq!(field_state.params["file_storage"], "private_uploads");
	assert_eq!(field_state.params["storage"], "external");
	assert_ne!(
		field_state.params["file_storage"],
		field_state.params["storage"]
	);
}
