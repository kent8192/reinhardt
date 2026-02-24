//! # Reinhardt Migrations
//!
//! Database migration system for Reinhardt framework.
//!
//! ## Features
//!
//! - **Auto-detection**: Detects model changes and generates migrations
//! - **Migration Graph**: Manages dependencies between migrations
//! - **AST-Based Entry Points**: Generates Rust 2024 Edition-compliant module files
//! - **State Reconstruction**: Django-style `ProjectState` building from migration history
//! - **Zero Downtime**: Support for safe schema changes in production
//!
//! ## AST-Based Entry Point Generation
//!
//! The `makemigrations` command uses Abstract Syntax Tree (AST) parsing to generate
//! and maintain migration entry point files (`migrations/app_name.rs`). This ensures:
//!
//! 1. **Rust 2024 Edition Compliance**: Uses `app_name.rs` instead of deprecated `mod.rs`
//! 2. **Robust Module Detection**: Structurally identifies existing migration modules
//! 3. **Consistent Formatting**: Standardized output via `prettyplease`
//!
//! ### Generated Entry Point Example
//!
//! The migration system automatically generates entry point files:
//!
//! ```rust,ignore
//! // migrations/myapp.rs (auto-generated - example only)
//! pub mod _0001_initial;
//! pub mod _0002_add_field;
//!
//! pub fn all_migrations() -> Vec<fn() -> Migration> {
//!     vec![_0001_initial::migration, _0002_add_field::migration]
//! }
//! ```
//!
//! This file is automatically updated when new migrations are created.

pub mod ast_parser;
pub mod auto_migration;
pub mod autodetector;
pub mod dependency;
pub mod di_support;
pub mod executor;
pub mod fields;
pub mod graph;
pub mod introspect;
pub mod introspection;
pub mod migration;
pub mod migration_namer;
pub mod migration_numbering;
pub mod model_registry;
pub mod operation_trait;
pub mod operations;
pub mod plan;
pub mod recorder;
pub mod registry;
pub mod repository;
pub mod schema_diff;
pub mod schema_editor;
pub mod service;
pub mod source;
pub mod squash;
pub mod state_loader;
pub mod visualization;
pub mod zero_downtime;

pub use crate::contenttypes::migration::MigrationRecord;
pub use autodetector::{
	// Pattern Learning and Inference
	ChangeTracker,
	ConstraintDefinition,
	DetectedChanges,
	FieldState,
	ForeignKeyAction,
	ForeignKeyConstraintInfo,
	ForeignKeyInfo,
	IndexDefinition,
	InferenceEngine,
	InferenceRule,
	InferredIntent,
	InteractiveAutodetector,
	MigrationAutodetector,
	MigrationPrompt,
	ModelState,
	OperationRef,
	PatternMatcher,
	ProjectState,
	RuleCondition,
	SimilarityConfig,
	to_snake_case,
};
pub use dependency::{
	DependencyCondition, DependencyResolutionContext, DependencyResolver, MigrationDependency,
	OptionalDependency, SwappableDependency,
};
pub use di_support::{MigrationConfig, MigrationService as DIMigrationService};
pub use executor::{DatabaseMigrationExecutor, ExecutionResult, OperationOptimizer};
pub use fields::FieldType;
pub use graph::{MigrationGraph, MigrationKey, MigrationNode};
pub use migration::Migration;
pub use migration_namer::MigrationNamer;
pub use migration_numbering::MigrationNumbering;
pub use model_registry::{
	FieldMetadata, ManyToManyMetadata, ModelMetadata, ModelRegistry, RelationshipMetadata,
	global_registry,
};
pub use operation_trait::MigrationOperation;
pub use operations::{
	AddColumn, AlterColumn, AlterTableOptions, BulkLoadFormat, BulkLoadOptions, BulkLoadSource,
	ColumnDefinition, Constraint, CreateTable, DeferrableOption, DropColumn, IndexType,
	InterleaveSpec, MySqlAlgorithm, MySqlLock, Operation, PartitionDef, PartitionOptions,
	PartitionType, PartitionValues, SqlDialect, field_type_string_to_field_type,
};
pub use plan::{MigrationPlan, TransactionMode};

// New operations from refactored modules
pub use auto_migration::{
	AutoMigrationError, AutoMigrationGenerator, AutoMigrationResult, ValidationResult,
};
pub use operations::{
	AddField, AlterField, CreateCollation, CreateExtension, CreateModel, DeleteModel,
	DropExtension, FieldDefinition, MoveModel, RemoveField, RenameField, RenameModel, RunCode,
	RunSQL, StateOperation, special::DataMigration,
};
pub use recorder::{DatabaseMigrationRecorder, MigrationRecorder};
pub use repository::{MigrationRepository, filesystem::FilesystemRepository};
pub use schema_diff::{
	ColumnSchema, ConstraintSchema, DatabaseSchema, ForeignKeySchemaInfo, IndexSchema, SchemaDiff,
	SchemaDiffResult, TableSchema,
};
pub use schema_editor::SchemaEditor;
pub use service::MigrationService;
pub use source::{
	MigrationSource, composite::CompositeSource, filesystem::FilesystemSource,
	registry::RegistrySource,
};
pub use squash::{MigrationSquasher, SquashOptions};
pub use state_loader::MigrationStateLoader;
pub use visualization::{HistoryEntry, MigrationStats, MigrationVisualizer, OutputFormat};
pub use zero_downtime::{MigrationPhase, Strategy, ZeroDowntimeMigration};

pub use introspect::{
	GeneratedFile, GeneratedOutput, GenerationConfig, IntrospectConfig, NamingConvention,
	OutputConfig, SchemaCodeGenerator, TableFilterConfig, TypeMapper, TypeMappingError,
	escape_rust_keyword, generate_models, preview_output, sanitize_identifier, to_pascal_case,
	write_output,
};
pub use introspection::{
	ColumnInfo, DatabaseIntrospector, ForeignKeyInfo as IntrospectionForeignKeyInfo, IndexInfo,
	TableInfo, UniqueConstraintInfo,
};

// Re-export types from reinhardt-backends for convenience
pub use crate::backends::{DatabaseConnection, DatabaseType};

use thiserror::Error;

/// Trait for types that provide migrations.
///
/// This trait enables compile-time migration collection, which is necessary
/// because Rust cannot dynamically load code at runtime like Python's Django.
///
/// # Example
///
/// Application-side implementation (migration modules would be generated):
///
/// ```rust,ignore
/// use reinhardt_db::migrations::{Migration, MigrationProvider};
///
/// // In your application's migrations module
/// // These modules would be generated by `makemigrations` command:
/// // pub mod _0001_initial;
/// // pub mod _0002_add_published;
///
/// pub struct PollsMigrations;
///
/// impl MigrationProvider for PollsMigrations {
///     fn migrations() -> Vec<Migration> {
///         vec![
///             _0001_initial::migration(),
///             _0002_add_published::migration(),
///         ]
///     }
/// }
///
/// // Usage in tests:
/// // let (container, db) = postgres_with_migrations_from::<PollsMigrations>().await;
/// ```
pub trait MigrationProvider {
	/// Returns all migrations provided by this type.
	///
	/// Migrations should be returned in dependency order (base migrations first).
	fn migrations() -> Vec<Migration>;
}

#[derive(Debug, Error)]
pub enum MigrationError {
	#[error("Migration not found: {0}")]
	NotFound(String),

	#[error("Dependency error: {0}")]
	DependencyError(String),

	#[error("SQL error: {0}")]
	SqlError(#[from] sqlx::Error),

	#[error("Database error: {0}")]
	DatabaseError(#[from] crate::backends::QueryDatabaseError),

	#[error("Invalid migration: {0}")]
	InvalidMigration(String),

	#[error("Irreversible migration: {0}")]
	IrreversibleError(String),

	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),

	#[error("Format error: {0}")]
	FmtError(#[from] std::fmt::Error),

	#[error("Circular dependency detected: {cycle}")]
	CircularDependency { cycle: String },

	#[error("Node not found: {message} - {node}")]
	NodeNotFound { message: String, node: String },

	#[error("Introspection error: {0}")]
	IntrospectionError(String),

	#[error("Unsupported database: {0}")]
	UnsupportedDatabase(String),

	/// Duplicate operations detected
	///
	/// This error occurs when a new migration has identical operations
	/// to an existing migration, which usually indicates a problem with
	/// from_state construction during makemigrations.
	#[error("Duplicate operations: {0}")]
	DuplicateOperations(String),

	/// Foreign key integrity violation during table recreation
	///
	/// This error occurs when SQLite table recreation results in orphaned
	/// foreign key references, indicating data integrity issues that must
	/// be resolved before the migration can proceed.
	#[error("Foreign key violation: {0}")]
	ForeignKeyViolation(String),

	/// Path traversal attempt detected in migration path components
	///
	/// This error occurs when an app label or migration name contains
	/// path traversal sequences (e.g., `..`) that could escape the
	/// migration root directory.
	#[error("Path traversal detected: {0}")]
	PathTraversal(String),
}

pub type Result<T> = std::result::Result<T, MigrationError>;

// Prelude for migrations
pub mod prelude {
	pub use super::fields::prelude::*;
	pub use super::{ColumnDefinition, Constraint, ForeignKeyAction, Migration, Operation};
}
