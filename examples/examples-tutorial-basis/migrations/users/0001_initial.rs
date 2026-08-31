// reinhardt-migration-source: 1
use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub(super) fn migration() -> Migration {
    Migration::new("0001_initial".to_string(), "users".to_string())
        .add_operation(Operation::CreateTable {
            name: "users".to_string(),
            columns: vec![
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
                ColumnDefinition::new("is_active".to_string(), FieldType::Boolean)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(Some("true".to_string()))
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("is_superuser".to_string(), FieldType::Boolean)
                    .with_not_null(true)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(Some("false".to_string()))
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("last_login".to_string(), FieldType::TimestampTz)
                    .with_not_null(false)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("password_hash".to_string(), FieldType::VarChar(255u32))
                    .with_not_null(false)
                    .with_unique(false)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
                ColumnDefinition::new("username".to_string(), FieldType::VarChar(150u32))
                    .with_not_null(true)
                    .with_unique(true)
                    .with_primary_key(false)
                    .with_auto_increment(false)
                    .with_default(None)
                    .with_generated(None)
                    .with_domain_option(None),
            ],
            constraints: vec![Constraint::Unique {
                name: "users_user_username_uniq".to_string(),
                columns: vec!["username".to_string()],
            }],
            without_rowid: None,
            interleave_in_parent: None,
            partition: None,
        })
        .atomic(true)
        .with_initial(Some(true))
        .state_only(false)
        .database_only(false)
}
