use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "default".to_string(),
		name: "0001_initial".to_string(),
		operations: vec![Operation::CreateTable {
			name: "sessions".to_string(),
			columns: vec![
				ColumnDefinition {
					name: "created_at".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "expire_date".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "last_accessed".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "session_data".to_string(),
					type_definition: FieldType::VarChar(65535u32),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "session_key".to_string(),
					type_definition: FieldType::VarChar(255u32),
					not_null: true,
					unique: false,
					primary_key: true,
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
	}
}
