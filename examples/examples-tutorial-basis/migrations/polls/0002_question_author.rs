// reinhardt-migration-source: 1
use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;

pub(super) fn migration() -> Migration {
	Migration::new("0002_question_author".to_string(), "polls".to_string())
		.add_operation(Operation::AddColumn {
			table: "questions".to_string(),
			column: ColumnDefinition::new("author_id".to_string(), FieldType::BigInteger)
				.with_not_null(true)
				.with_unique(false)
				.with_primary_key(false)
				.with_auto_increment(false)
				.with_default(None)
				.with_generated(None)
				.with_domain_option(None),
			mysql_options: None,
		})
		.add_dependency("polls".to_string(), "0001_initial".to_string())
		.atomic(true)
		.with_initial(None)
		.state_only(false)
		.database_only(false)
}
