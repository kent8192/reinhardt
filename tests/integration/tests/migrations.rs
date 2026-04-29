// Auto-generated module file for migrations integration tests
// Each test file in migrations/ subdirectory is explicitly included with #[path] attribute

#[path = "migrations/composite_pk_db_execution.rs"]
mod composite_pk_db_execution;

#[path = "migrations/state_loader_integration.rs"]
mod state_loader_integration;

#[path = "migrations/error_handling_integration.rs"]
mod error_handling_integration;

#[path = "migrations/edge_cases_integration.rs"]
mod edge_cases_integration;

#[path = "migrations/multi_database_integration.rs"]
mod multi_database_integration;

#[path = "migrations/concurrent_execution_integration.rs"]
mod concurrent_execution_integration;

// SQL Generation Validation Tests
#[path = "migrations/sql_generation_validation.rs"]
mod sql_generation_validation;

// Migration Rollback Tests
#[path = "migrations/migration_rollback_integration.rs"]
mod migration_rollback_integration;

// Schema Validation Tests
#[path = "migrations/schema_validation_integration.rs"]
mod schema_validation_integration;

// Database-Specific Optimization Tests
#[path = "migrations/db_specific_optimizations.rs"]
mod db_specific_optimizations;

// Data Migration Tests
#[path = "migrations/data_migrations_integration.rs"]
mod data_migrations_integration;

// Dependency Resolution Tests
#[path = "migrations/dependency_resolution_integration.rs"]
mod dependency_resolution_integration;

// Large Dataset Tests
#[path = "migrations/large_dataset_integration.rs"]
mod large_dataset_integration;

// Migration Squashing Tests
#[path = "migrations/migration_squashing_integration.rs"]
mod migration_squashing_integration;

// SQLite Table Recreation Tests
#[path = "migrations/sqlite_table_recreation_integration.rs"]
mod sqlite_table_recreation_integration;

// MySQL Edge Cases Tests
#[path = "migrations/mysql_edge_cases.rs"]
mod mysql_edge_cases;

// PostgreSQL ENUM Edge Cases Tests
#[path = "migrations/postgres_enum_edge_cases.rs"]
mod postgres_enum_edge_cases;

// Field Mapping Integration Tests
#[path = "migrations/field_mapping_integration.rs"]
mod field_mapping_integration;

// Makemigrations E2E Integration Tests
#[path = "migrations/makemigrations_e2e_integration.rs"]
mod makemigrations_e2e_integration;

// Merge Migration Tests
#[path = "migrations/merge_migration_integration.rs"]
mod merge_migration_integration;

// Migration Overwrite Prevention Tests
#[path = "migrations/migration_overwrite_prevention_test.rs"]
mod migration_overwrite_prevention_test;

// Macro `unique_together` propagation regression tests (reinhardt-web#4022)
#[path = "migrations/macro_unique_together_integration.rs"]
mod macro_unique_together_integration;
