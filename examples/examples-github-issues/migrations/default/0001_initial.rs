use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "default",
		name: "0001_initial",
		operations: vec![Operation::CreateTable {
			name: "sessions",
			columns: vec![
				ColumnDefinition {
					name: "created_at",
					type_definition: FieldType::BigInteger,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "expire_date",
					type_definition: FieldType::BigInteger,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "last_accessed",
					type_definition: FieldType::BigInteger,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "session_data",
					type_definition: FieldType::VarChar(65535u32),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "session_key",
					type_definition: FieldType::VarChar(255u32),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
			],
			constraints: vec![],
		}],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
	}
}
