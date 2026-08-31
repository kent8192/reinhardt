// reinhardt-migration-source: 1
use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub(super) fn migration() -> Migration {
    Migration::new("0001_initial".to_string(), "polls".to_string())
        .add_operation(Operation::CreateTable {
            name: "choices".to_string(),
            columns: vec![
                ColumnDefinition::new("choice_text".to_string(), FieldType::VarChar(200u32))
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("id".to_string(), FieldType::BigInteger)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(true)
                    .with_auto_increment(true)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("question_id".to_string(), FieldType::BigInteger)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("votes".to_string(), FieldType::Integer)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(Some("0".to_string()))
                    .with_generated(None)
                    .with_domain_option(None),
            ],
            constraints: vec![],
            without_rowid: None,
            interleave_in_parent: None,
            partition: None,
        })
        .add_operation(Operation::CreateTable {
            name: "questions".to_string(),
            columns: vec![
                ColumnDefinition::new("id".to_string(), FieldType::BigInteger)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(true)
                    .with_auto_increment(true)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("pub_date".to_string(), FieldType::TimestampTz)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("question_text".to_string(), FieldType::VarChar(200u32))
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
            ],
            constraints: vec![],
            without_rowid: None,
            interleave_in_parent: None,
            partition: None,
        })
        .atomic(true)
        .with_initial(Some(true))
        .state_only(false)
        .database_only(false)
}
