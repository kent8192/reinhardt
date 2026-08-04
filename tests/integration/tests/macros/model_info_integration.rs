//! Integration tests for the `{Model}Info` companion struct generation (Issue #4194).
//!
//! Tests the `#[model]` macro's Info struct generation including:
//! - Basic Info struct generation with correct fields
//! - Bidirectional `From` conversions (Model ↔ Info)
//! - Opt-out via `#[model(info = false)]`
//! - Field exclusion via `#[field(skip_info = true)]` and `#[field(skip_getter = true)]`
//! - Lightweight relation field inclusion for FK, OneToOne, and ManyToMany
//! - Builder with relation-aware setters
//! - Validation attribute generation from `#[field(...)]` config

use reinhardt::db::associations::{ForeignKeyField, ManyToManyField, OneToOneField};
use reinhardt::model;
use reinhardt::model_info::{ManyToManyInfo, RelationInfo};
use serde::{Deserialize, Serialize};

// --- Basic Info generation ---

#[model(app_label = "test", table_name = "persons")]
#[derive(Serialize, Deserialize)]
struct Person {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(max_length = 100)]
	name: String,

	#[field(null = true)]
	age: Option<i32>,
}

#[test]
fn test_info_struct_generated() {
	// Arrange
	let info = PersonInfo {
		id: Some(1),
		name: "Alice".to_string(),
		age: Some(30),
	};

	// Assert — Info struct exists with correct public fields
	assert_eq!(info.id, Some(1));
	assert_eq!(info.name, "Alice");
	assert_eq!(info.age, Some(30));
}

#[test]
fn test_info_from_model() {
	// Arrange
	let person = Person::build().name("Alice").age(Some(30)).finish();

	// Act
	let info: PersonInfo = person.into();

	// Assert
	assert_eq!(info.name, "Alice");
	assert_eq!(info.age, Some(30));
}

#[test]
fn test_info_into_model() {
	// Arrange
	let info = PersonInfo {
		id: Some(1),
		name: "Bob".to_string(),
		age: Some(25),
	};

	// Act
	let person: Person = info.into();

	// Assert
	assert_eq!(*person.name(), "Bob");
	assert_eq!(*person.age(), Some(25));
}

#[test]
fn test_info_roundtrip() {
	// Arrange
	let person = Person::build().name("Charlie").age(Some(40)).finish();

	// Act — Model → Info → Model
	let info: PersonInfo = person.into();
	let restored: Person = info.into();

	// Assert
	assert_eq!(*restored.name(), "Charlie");
	assert_eq!(*restored.age(), Some(40));
}

#[test]
fn test_info_debug_clone_partial_eq() {
	// Arrange
	let info = PersonInfo {
		id: Some(1),
		name: "Alice".to_string(),
		age: Some(30),
	};

	// Assert — Debug
	let debug = format!("{:?}", info);
	assert_eq!(
		debug,
		"PersonInfo { id: Some(1), name: \"Alice\", age: Some(30) }"
	);

	// Assert — Clone
	let cloned = info.clone();
	assert_eq!(cloned.name, "Alice");

	// Assert — PartialEq
	assert_eq!(info, cloned);
}

#[test]
fn test_info_serde_mirrored() {
	// Arrange
	let info = PersonInfo {
		id: Some(1),
		name: "Alice".to_string(),
		age: Some(30),
	};

	// Act — Serialize
	let json = serde_json::to_string(&info).unwrap();

	// Act — Deserialize
	let deserialized: PersonInfo = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.name, "Alice");
	assert_eq!(deserialized.age, Some(30));
}

// --- Opt-out ---
// Compile-fail verification that `info = false` suppresses Info type generation
// is in model_info_ui.rs (trybuild: tests/macros/ui/fail/info_opt_out_no_type.rs).

// --- Field exclusion ---

#[derive(Serialize, Deserialize)]
#[model(app_label = "test", table_name = "users_with_secrets")]
struct UserWithSecret {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(max_length = 100)]
	username: String,

	#[field(max_length = 255, skip_info = true)]
	password_hash: String,
}

#[test]
fn test_info_skip_field() {
	// Arrange — Info struct should NOT have password_hash field
	let info = UserWithSecretInfo {
		id: Some(1),
		username: "alice".to_string(),
		// password_hash is excluded
	};

	// Assert
	assert_eq!(info.username, "alice");
}

#[test]
fn test_info_skip_field_default_on_roundtrip() {
	// Arrange
	let info = UserWithSecretInfo {
		id: Some(1),
		username: "alice".to_string(),
	};

	// Act
	let model: UserWithSecret = info.into();

	// Assert — excluded field gets Default::default()
	assert_eq!(*model.password_hash(), "");
}

#[model(app_label = "test", table_name = "users_with_hidden_fields")]
struct UserWithHiddenField {
	#[field(primary_key = true)]
	id: Option<i64>,

	username: String,

	#[field(skip_getter = true)]
	password_hash: String,
}

#[test]
fn test_info_excludes_skip_getter_field() {
	// Arrange — the skipped field must not be part of the public Info DTO.
	let info = UserWithHiddenFieldInfo {
		id: Some(1),
		username: "alice".to_string(),
	};

	// Act
	let model: UserWithHiddenField = info.into();

	// Assert — the hidden field is initialized safely when converting from Info.
	assert_eq!(model.password_hash, "");
}

#[model(app_label = "test", table_name = "serde_redacted_users")]
#[derive(Serialize, Deserialize)]
struct SerdeRedactedUser {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(max_length = 100)]
	username: String,

	#[serde(skip_serializing)]
	#[field(max_length = 255)]
	password_hash: String,

	#[serde(skip_deserializing)]
	is_staff: bool,
}

#[test]
fn test_info_preserves_serde_field_redaction() {
	// Arrange
	let info = SerdeRedactedUserInfo {
		id: Some(1),
		username: "alice".to_string(),
		password_hash: "HASHED_SECRET".to_string(),
		is_staff: true,
	};

	// Act
	let json = serde_json::to_string(&info).unwrap();

	// Assert
	let value: serde_json::Value = serde_json::from_str(&json).unwrap();
	assert_eq!(value.get("password_hash"), None);
	assert_eq!(value["username"], "alice");
}

#[test]
fn test_info_preserves_serde_deserialization_redaction() {
	// Arrange
	let json = r#"{
		"id": 1,
		"username": "alice",
		"password_hash": "ATTACKER_HASH",
		"is_staff": true
	}"#;

	// Act
	let info: SerdeRedactedUserInfo = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(info.is_staff, false);

	// Act
	let model: SerdeRedactedUser = info.into();

	// Assert
	assert_eq!(model.is_staff(), false);
}

// --- Builder ---

#[test]
fn test_info_builder_basic() {
	// Act
	let info = PersonInfo::build()
		.id(Some(1))
		.name("Diana")
		.age(Some(28))
		.finish();

	// Assert
	assert_eq!(info.id, Some(1));
	assert_eq!(info.name, "Diana");
	assert_eq!(info.age, Some(28));
}

// --- Relationship fields ---

#[model(app_label = "test", table_name = "info_authors")]
#[derive(Serialize, Deserialize)]
struct InfoAuthor {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(max_length = 100)]
	name: String,
}

#[model(app_label = "test", table_name = "info_tags")]
#[derive(Serialize, Deserialize)]
struct InfoTag {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(max_length = 100)]
	label: String,
}

#[model(app_label = "test", table_name = "info_posts")]
#[derive(Serialize, Deserialize)]
struct InfoPost {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(max_length = 100)]
	title: String,

	#[rel(foreign_key, related_name = "info_posts")]
	author: ForeignKeyField<InfoAuthor>,

	#[rel(many_to_many, related_name = "info_posts")]
	tags: ManyToManyField<InfoPost, InfoTag>,
}

#[model(app_label = "test", table_name = "info_profiles")]
#[derive(Serialize, Deserialize)]
struct InfoProfile {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[rel(one_to_one, related_name = "info_profile")]
	user: OneToOneField<InfoAuthor>,
}

#[model(app_label = "test", table_name = "info_hidden_relations")]
#[derive(Serialize, Deserialize)]
struct InfoHiddenRelations {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(skip_info = true)]
	#[rel(foreign_key, related_name = "hidden_posts")]
	author: ForeignKeyField<InfoAuthor>,

	#[field(skip_info = true)]
	#[rel(many_to_many, related_name = "hidden_posts")]
	tags: ManyToManyField<InfoHiddenRelations, InfoTag>,
}

#[test]
fn test_info_fk_relation_field_generated() {
	// Arrange
	let post = InfoPost::build().title("Hello").author(7_i64).finish();

	// Act
	let info: InfoPostInfo = post.into();

	// Assert
	assert_eq!(info.title, "Hello");
	assert_eq!(info.author.id, 7);
	assert_eq!(info.tags.target_ids, Vec::<i64>::new());
}

#[test]
fn test_info_one_to_one_relation_field_generated() {
	// Act
	let info = InfoProfileInfo::build().id(Some(1)).user(9_i64).finish();
	let model: InfoProfile = info.clone().into();

	// Assert
	assert_eq!(info.id, Some(1));
	assert_eq!(info.user.id, 9);
	assert_eq!(*model.user_id(), 9);
}

#[test]
fn test_info_many_to_many_uses_lightweight_payload() {
	// Arrange
	let info = InfoPostInfo {
		id: Some(1),
		title: "Tagged".to_string(),
		author: RelationInfo::new(7),
		tags: ManyToManyInfo::new([2, 3]),
	};

	// Assert
	assert_eq!(info.author.id, 7);
	assert_eq!(info.tags.target_ids, vec![2, 3]);
}

#[test]
fn test_info_builder_accepts_relation_payloads() {
	// Act
	let info = InfoPostInfo::build()
		.id(Some(1))
		.title("Builder")
		.author(7_i64)
		.tags([2_i64, 3_i64])
		.finish();

	// Assert
	assert_eq!(info.id, Some(1));
	assert_eq!(info.title, "Builder");
	assert_eq!(info.author.id, 7);
	assert_eq!(info.tags.target_ids, vec![2, 3]);
}

#[test]
fn test_info_relation_serde_shape() {
	// Arrange
	let info = InfoPostInfo {
		id: Some(1),
		title: "Serde".to_string(),
		author: RelationInfo::new(7),
		tags: ManyToManyInfo::new([2, 3]),
	};

	// Act
	let json = serde_json::to_value(&info).unwrap();
	let deserialized: InfoPostInfo = serde_json::from_value(json.clone()).unwrap();

	// Assert
	assert_eq!(json["author"]["id"], 7);
	assert_eq!(json["tags"]["target_ids"], serde_json::json!([2, 3]));
	assert_eq!(deserialized.author.id, 7);
	assert_eq!(deserialized.tags.target_ids, vec![2, 3]);
}

#[test]
fn test_info_skip_relation_fields() {
	// Arrange
	let info = InfoHiddenRelationsInfo { id: Some(1) };

	// Act
	let model: InfoHiddenRelations = info.into();

	// Assert
	assert_eq!(*model.id(), Some(1));
	assert_eq!(*model.author_id(), 0);
}
