//! Integration tests for composite primary key support

use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

/// Test models module to avoid "Self" resolution issues with proc macros
#[cfg(test)]
mod test_models {
	use super::*;

	#[derive(Serialize, Deserialize)]
	#[model(app_label = "test_app", table_name = "post_tags")]
	pub(crate) struct PostTag {
		#[field(primary_key = true)]
		pub post_id: i64,

		#[field(primary_key = true)]
		pub tag_id: i64,

		#[field(max_length = 200)]
		pub description: String,
	}

	#[derive(Serialize, Deserialize)]
	#[model(app_label = "test_app", table_name = "user_roles")]
	pub(crate) struct UserRole {
		#[field(primary_key = true)]
		pub user_id: i64,

		#[field(primary_key = true)]
		pub role_id: i64,

		#[field(max_length = 100, null = true)]
		pub granted_by: Option<String>,
	}

	#[derive(Serialize, Deserialize)]
	#[model(app_label = "test_app", table_name = "users")]
	pub(crate) struct User {
		#[field(primary_key = true)]
		pub id: i64,

		#[field(index = true, max_length = 100)]
		pub email: String,

		#[field(index = true, max_length = 50)]
		pub username: String,

		#[field(max_length = 200)]
		pub full_name: String,
	}

	#[derive(Serialize, Deserialize)]
	#[model(app_label = "test_app", table_name = "simple_models")]
	pub(crate) struct SimpleModel {
		#[field(primary_key = true)]
		pub id: i64,

		#[field(max_length = 100)]
		pub name: String,
	}
}

#[test]
fn test_composite_pk_definition() {
	use reinhardt_db::orm::Model;
	use test_models::*;

	// Verify composite_primary_key() returns Some
	let composite_pk = PostTag::composite_primary_key();
	assert!(
		composite_pk.is_some(),
		"Composite primary key should be defined"
	);

	let pk = composite_pk.unwrap();
	assert_eq!(pk.fields().len(), 2, "Should have 2 primary key fields");
	assert!(
		pk.fields().contains(&"post_id".to_string()),
		"Should contain post_id field"
	);
	assert!(
		pk.fields().contains(&"tag_id".to_string()),
		"Should contain tag_id field"
	);
}

#[test]
fn test_composite_pk_values() {
	use reinhardt_db::orm::Model;
	use reinhardt_db::orm::composite_pk::PkValue;
	use test_models::*;

	// Create test instance
	let post_tag = PostTag {
		post_id: 42,
		tag_id: 123,
		description: "Test tag".to_string(),
	};

	// Get composite PK values
	let pk_values = post_tag.get_composite_pk_values();

	// Verify we have 2 values
	assert_eq!(pk_values.len(), 2, "Should have 2 PK values");

	// Verify post_id value
	assert!(pk_values.contains_key("post_id"), "Should have post_id key");
	match pk_values.get("post_id").unwrap() {
		PkValue::Int(val) => assert_eq!(*val, 42, "post_id should be 42"),
		_ => panic!("post_id should be Int type"),
	}

	// Verify tag_id value
	assert!(pk_values.contains_key("tag_id"), "Should have tag_id key");
	match pk_values.get("tag_id").unwrap() {
		PkValue::Int(val) => assert_eq!(*val, 123, "tag_id should be 123"),
		_ => panic!("tag_id should be Int type"),
	}
}

#[test]
fn test_composite_pk_with_optional_field() {
	use reinhardt_db::orm::Model;
	use test_models::*;

	// Test with Some value
	let user_role = UserRole {
		user_id: 1,
		role_id: 2,
		granted_by: Some("admin".to_string()),
	};

	let pk_values = user_role.get_composite_pk_values();
	assert_eq!(pk_values.len(), 2, "Should have 2 PK values");
	assert!(pk_values.contains_key("user_id"));
	assert!(pk_values.contains_key("role_id"));

	// Test with None value
	let user_role_none = UserRole {
		user_id: 10,
		role_id: 20,
		granted_by: None,
	};

	let pk_values_none = user_role_none.get_composite_pk_values();
	assert_eq!(pk_values_none.len(), 2, "Should have 2 PK values");
}

#[test]
fn test_composite_pk_sql_generation() {
	use reinhardt_db::orm::Model;
	use test_models::*;

	let composite_pk = PostTag::composite_primary_key().unwrap();

	// Test SQL generation
	let sql = composite_pk.to_sql();
	assert!(
		sql.contains("PRIMARY KEY"),
		"SQL should contain PRIMARY KEY"
	);
	assert!(sql.contains("post_id"), "SQL should contain post_id field");
	assert!(sql.contains("tag_id"), "SQL should contain tag_id field");
}

#[test]
fn test_composite_pk_field_metadata() {
	use reinhardt_db::orm::Model;
	use test_models::*;

	let fields = PostTag::field_metadata();

	// Find primary key fields
	let pk_fields: Vec<_> = fields.iter().filter(|f| f.primary_key).collect();

	assert_eq!(
		pk_fields.len(),
		2,
		"Should have exactly 2 primary key fields"
	);

	// Verify field names
	let pk_names: Vec<_> = pk_fields.iter().map(|f| f.name.as_str()).collect();
	assert!(pk_names.contains(&"post_id"), "Should have post_id as PK");
	assert!(pk_names.contains(&"tag_id"), "Should have tag_id as PK");
}

#[test]
fn test_model_basic_properties() {
	use reinhardt_db::orm::Model;
	use test_models::*;

	assert_eq!(PostTag::table_name(), "post_tags");
	assert_eq!(PostTag::app_label(), "test_app");

	assert_eq!(UserRole::table_name(), "user_roles");
	assert_eq!(UserRole::app_label(), "test_app");
}

#[test]
fn test_index_metadata() {
	use reinhardt_db::orm::Model;
	use test_models::*;

	let indexes = User::index_metadata();
	assert_eq!(indexes.len(), 2, "Should have 2 indexed fields");

	// Check that indexed fields are present
	let indexed_field_names: Vec<Vec<String>> =
		indexes.iter().map(|idx| idx.fields.clone()).collect();
	let flattened: Vec<String> = indexed_field_names.into_iter().flatten().collect();

	assert!(flattened.contains(&"email".to_string()));
	assert!(flattened.contains(&"username".to_string()));
}

#[test]
fn test_no_index() {
	use reinhardt_db::orm::Model;
	use test_models::*;

	let indexes = SimpleModel::index_metadata();
	assert_eq!(indexes.len(), 0, "Should have no indexed fields");
}
