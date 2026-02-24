use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub(super) fn migration() -> Migration {
	Migration {
		app_label: "tweet".to_string(),
		name: "0001_initial".to_string(),
		operations: vec![Operation::CreateTable {
			name: "tweet_tweet".to_string(),
			columns: vec![
				ColumnDefinition {
					name: "content".to_string(),
					type_definition: FieldType::VarChar(280u32),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "created_at".to_string(),
					type_definition: FieldType::TimestampTz,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Uuid,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "like_count".to_string(),
					type_definition: FieldType::Integer,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "retweet_count".to_string(),
					type_definition: FieldType::Integer,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "updated_at".to_string(),
					type_definition: FieldType::TimestampTz,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "user_id".to_string(),
					type_definition: FieldType::Uuid,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
		..Default::default()
	}
}
