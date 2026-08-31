// reinhardt-migration-source: 1
use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub(super) fn migration() -> Migration {
    Migration::new("0001_initial".to_string(), "default".to_string())
        .add_operation(Operation::CreateTable {
            name: "sessions".to_string(),
            columns: vec![
                ColumnDefinition::new("created_at".to_string(), FieldType::BigInteger)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("expire_date".to_string(), FieldType::BigInteger)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("last_accessed".to_string(), FieldType::BigInteger)
                    .with_not_null(false)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("session_data".to_string(), FieldType::VarChar(65535u32))
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("session_key".to_string(), FieldType::VarChar(255u32))
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(true)
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
