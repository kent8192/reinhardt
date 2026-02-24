use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "polls",
		name: "0002_initial",
		operations: vec![
			Operation::CreateTable {
				name: "choices",
				columns: vec![
					ColumnDefinition {
						name: "choice_text",
						type_definition: FieldType::VarChar(200u32),
						not_null: false,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "id",
						type_definition: FieldType::BigInteger,
						not_null: true,
						unique: false,
						primary_key: true,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "question_id",
						type_definition: FieldType::BigInteger,
						not_null: false,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "votes",
						type_definition: FieldType::Integer,
						not_null: false,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![Constraint::ForeignKey {
					name: "fk_choices_question_id".to_string(),
					columns: vec!["question_id".to_string()],
					referenced_table: "question".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Cascade,
				}],
			},
			Operation::CreateTable {
				name: "questions",
				columns: vec![
					ColumnDefinition {
						name: "id",
						type_definition: FieldType::BigInteger,
						not_null: true,
						unique: false,
						primary_key: true,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "pub_date",
						type_definition: FieldType::DateTime,
						not_null: false,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "question_text",
						type_definition: FieldType::VarChar(200u32),
						not_null: false,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![],
			},
		],
		dependencies: vec![("polls", "0001_initial")],
		atomic: true,
		replaces: vec![],
	}
}
