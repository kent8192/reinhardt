// reinhardt-migration-source: 1
use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub(super) fn migration() -> Migration {
    Migration::new("0001_initial".to_string(), "snippets".to_string())
        .add_operation(Operation::CreateTable {
            name: "snippets".to_string(),
            columns: vec![
                ColumnDefinition::new("code".to_string(), FieldType::VarChar(10000u32))
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("created_at".to_string(), FieldType::TimestampTz)
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
                ColumnDefinition::new("language".to_string(), FieldType::VarChar(50u32))
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("title".to_string(), FieldType::VarChar(100u32))
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
