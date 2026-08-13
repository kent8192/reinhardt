//! # Reinhardt Migrations
//!
//! Database migration system for Reinhardt framework.
//!
//! ## Features
//!
//! - **Auto-detection**: Detects model changes and generates migrations
//! - **Migration Graph**: Manages dependencies between migrations
//! - **Shared SQL Planning**: Previews the same ordered statements consumed by execution
//! - **AST-Based Entry Points**: Generates Rust 2024 Edition-compliant module files
//! - **State Reconstruction**: Django-style `ProjectState` building from migration history
//! - **Zero Downtime**: Support for safe schema changes in production
//! - **Schema Inspection**: Deterministic PostgreSQL, MySQL, and SQLite model
//!   generation with exact object selection
//!
//! ## Schema Inspection
//!
//! [`inspect_database`] reads exact table selections and optionally includes
//! backend views or PostgreSQL partitions. [`render_models_module`] produces a
//! single parseable Rust module for stdout, while
//! [`generate_models_canonical`] produces a deterministic Rust 2024 multi-file
//! set rooted at `models.rs` with child modules beneath `models/`; no `mod.rs`
//! is generated.
//! Management-command directory output validates the complete set before using
//! rollback-safe, all-or-nothing publication when the command reports failure.
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
//! ## Strict Migration Squashing
//!
//! [`MigrationCatalog`] loads the complete source tree and validates duplicate
//! identities, missing dependencies, and cycles before range selection.
//! Migration names may be exact or unique prefixes. [`MigrationCatalog::squash_range`]
//! returns a continuous, same-application ancestor range in dependency order,
//! preserves dependencies entering that range, and rejects ambiguous or
//! externally re-entering ancestry.
//! Filesystem discovery loads Rust migration implementations while ignoring
//! Rust module entry points such as `migrations.rs`.
//! Historical state reconstruction preserves original dependency chains when
//! the requested target is an original migration that a squash replaces.
//!
//! [`MigrationSquasher`] combines the selected range while retaining exact
//! replacement identities and stable metadata. Its optimizer applies only
//! proven schema reductions and treats data operations, renames, constraints,
//! indexes, bulk operations, custom operations, and unsupported future
//! operations as ordering barriers. Disabling optimization preserves every
//! operation in source order.
//!
//! [`FilesystemRepository::render`] validates that the combined migration can
//! be represented as Rust source and emits parseable Rust 2024 code.
//! [`FilesystemRepository::create_new_source`] validates names and source
//! before writing and never overwrites an existing destination. If writing or
//! synchronization fails, it attempts to remove the incomplete file through
//! the anchored application directory. A cleanup failure reports both the
//! original error and the cleanup error. Catalog loading, range selection,
//! squashing, and source creation do not require a database connection.
//!
//! ## Shared SQL Planning
//!
//! [`plan_migration_sql`] creates a read-only [`MigrationSqlPlan`] for forward
//! or backward execution. Each item is either executable [`PlannedStatement::Sql`]
//! or a non-executable [`PlannedStatement::Comment`], so Rust data operations
//! remain visible without being sent to the database. The migration executor
//! consumes this same plan and keeps table-existence checks as execution policy.
//! SQLite recreation planning may read schema metadata through the supplied
//! connection, but it does not execute DDL.
//! [`MigrationSqlPlan::render`] preserves statement order and emits
//! backend-specific SQL. Transaction wrappers are emitted only when the
//! migration plan is atomic and the selected backend supports transactional
//! DDL; MySQL DDL is never wrapped. SQLite includes the full temporary-table
//! copy, drop, and rename sequence when recreation is required. Rendering is
//! complete and uncolored, so callers can buffer the whole script before
//! publishing it.
//! Backward inspection of destructive operations must use
//! [`plan_migration_sql_with_states`] with
//! [`MigrationCatalog::state_before`] and [`MigrationCatalog::state_after`].
//! Both states are required because a post-migration state cannot retain a
//! dropped table or legacy dropped-column definition.
//!
//! [`MigrationCatalog::snapshot`] reads existing recorder state without
//! creating the recorder table. An absent backend-specific recorder relation
//! is an empty applied set; permission, query, and type errors remain errors.
//! Filtering by application retains transitive cross-application dependencies
//! in topological order and preserves applied timestamps in the immutable
//! snapshot.
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
pub mod catalog;
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
pub mod sql_plan;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite_pragma;
pub mod squash;
pub mod state_loader;
pub mod verification;
pub mod visualization;
pub mod zero_downtime;

#[cfg(feature = "contenttypes")]
pub use crate::contenttypes::migration::MigrationRecord;
pub use autodetector::{
	// Pattern Learning and Inference
	AutodetectorWarning,
	ChangeTracker,
	ConstraintDefinition,
	DetectedChanges,
	FieldState,
	ForeignKeyAction,
	ForeignKeyConstraintInfo,
	ForeignKeyInfo,
	GeneratedMigrations,
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
pub use executor::{
	DatabaseMigrationExecutor, ExecutionResult, OperationOptimizer, ReplacementMigrationSelection,
	select_replacement_migrations,
};
pub use fields::FieldType;
pub use graph::{MigrationGraph, MigrationKey, MigrationNode};
pub use migration::Migration;
pub use migration_namer::MigrationNamer;
pub use migration_numbering::MigrationNumbering;
pub use model_registry::{
	FieldMetadata, ManyToManyMetadata, ModelMetadata, ModelRegistry, RelationshipMetadata,
	global_registry,
};
// Re-export the crate-root M2M naming helpers so callers can continue to
// import them from `reinhardt_db::migrations::*` or
// `reinhardt_db::migrations::naming::*`. The actual module lives at the
// crate root because the `orm` and `migrations` features are independent.
pub use crate::m2m_naming as naming;
pub use crate::m2m_naming::{default_m2m_columns, default_through_table};
pub use operation_trait::MigrationOperation;
pub use operations::{
	AddColumn, AlterColumn, AlterTableOptions, BulkLoadFormat, BulkLoadOptions, BulkLoadSource,
	ColumnDefinition, Constraint, CreateTable, DeferrableOption, DropColumn,
	GeneratedColumnDefinition, IndexType, InterleaveSpec, MySqlAlgorithm, MySqlLock, Operation,
	PartitionDef, PartitionOptions, PartitionType, PartitionValues, SqlDialect,
	field_type_string_to_field_type,
};
pub use plan::{MigrationPlan, TransactionMode};
pub use reinhardt_query::prelude::{
	ColumnType, GeneratedStorage, SchemaBinOper, SchemaExpr, SchemaFunc,
};

// New operations from refactored modules
pub use auto_migration::{
	AutoMigrationError, AutoMigrationGenerator, AutoMigrationResult, ValidationResult,
};
pub use catalog::{AppliedMigration, MigrationCatalog, MigrationSnapshot, SquashRange};
pub use operations::{
	AddField, AlterField, CreateCollation, CreateExtension, CreateModel, DeleteModel,
	DropExtension, FieldDefinition, MoveModel, RemoveField, RenameField, RenameModel, RunCode,
	RunSQL, StateOperation, special::DataMigration,
};
pub use recorder::{DatabaseMigrationRecorder, MigrationRecorder};
pub use repository::{
	MigrationRepository,
	filesystem::{FilesystemRepository, MigrationRenderOptions},
};
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
pub use sql_plan::{
	MigrationDirection, MigrationSqlPlan, PlannedStatement, plan_migration_sql,
	plan_migration_sql_with_states,
};
pub use squash::{MigrationSquasher, SquashOptions, SquashResult};
pub use state_loader::{MigrationStateLoader, build_state_from_files};
pub use verification::{
	SchemaCheckError, SchemaContractState, SchemaFinding, SchemaVerification,
	verify_schema_contract,
};
pub use visualization::{HistoryEntry, MigrationStats, MigrationVisualizer, OutputFormat};
pub use zero_downtime::{MigrationPhase, Strategy, ZeroDowntimeMigration};

pub use introspect::{
	GeneratedFile, GeneratedOutput, GenerationConfig, IntrospectConfig, NamingConvention,
	OutputConfig, SchemaCodeGenerator, TableFilterConfig, TypeMapper, TypeMappingError,
	escape_rust_keyword, generate_models, generate_models_canonical, preview_output,
	render_models_module, sanitize_identifier, to_pascal_case, write_output,
};
pub use introspection::{
	ColumnInfo, DatabaseIntrospector, ForeignKeyInfo as IntrospectionForeignKeyInfo, IndexInfo,
	InspectDbOptions, TableInfo, UniqueConstraintInfo, inspect_database,
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

/// Errors that can occur during migration operations.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum MigrationError {
	/// The requested migration was not found.
	#[error("Migration not found: {0}")]
	NotFound(String),

	/// A migration dependency could not be resolved.
	#[error("Dependency error: {0}")]
	DependencyError(String),

	/// An SQL execution error occurred.
	#[error("SQL error: {0}")]
	SqlError(#[from] sqlx::Error),

	/// A database backend error occurred.
	#[error("Database error: {0}")]
	DatabaseError(#[from] crate::backends::QueryDatabaseError),

	/// A non-database framework error occurred during migration execution.
	#[error("Framework error: {0}")]
	FrameworkError(#[source] reinhardt_core::exception::Error),

	/// The migration definition is invalid.
	#[error("Invalid migration: {0}")]
	InvalidMigration(String),

	/// The migration cannot be reversed.
	#[error("Irreversible migration: {0}")]
	IrreversibleError(String),

	/// An I/O error occurred during migration.
	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),

	/// A formatting error occurred.
	#[error("Format error: {0}")]
	FmtError(#[from] std::fmt::Error),

	/// Circular dependency detected in migration graph.
	#[error("Circular dependency detected: {cycle}")]
	CircularDependency {
		/// Description of the dependency cycle.
		cycle: String,
	},

	/// A required migration node was not found.
	#[error("Node not found: {message} - {node}")]
	NodeNotFound {
		/// The error message.
		message: String,
		/// The node identifier.
		node: String,
	},

	/// An error occurred during database introspection.
	#[error("Introspection error: {0}")]
	IntrospectionError(String),

	/// The database type is not supported.
	#[error("Unsupported database: {0}")]
	UnsupportedDatabase(String),

	/// A migration feature is not available on the selected backend.
	#[error("{feature} is not supported by the {backend} backend")]
	UnsupportedBackendFeature {
		/// The unsupported migration feature.
		feature: &'static str,
		/// The selected backend.
		backend: &'static str,
	},

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

	/// A migration operation cannot be represented by the source renderer.
	#[error("Unsupported migration rendering: {operation}")]
	UnsupportedMigrationRendering {
		/// Description of the operation or metadata that cannot be rendered.
		operation: String,
	},
}

impl From<reinhardt_core::exception::Error> for MigrationError {
	fn from(error: reinhardt_core::exception::Error) -> Self {
		match error {
			reinhardt_core::exception::Error::Database(database_error) => {
				Self::DatabaseError(database_error)
			}
			reinhardt_core::exception::Error::DatabaseWithSource {
				database_error,
				source,
			} => Self::DatabaseError(database_error.with_boxed_source(source)),
			error => Self::FrameworkError(error),
		}
	}
}

/// Type alias for result.
pub type Result<T> = std::result::Result<T, MigrationError>;

#[cfg(test)]
mod tests {
	use std::error::Error as _;
	use std::io;

	use reinhardt_core::exception::{
		DatabaseError, DatabaseErrorKind, Error as FrameworkError, ErrorKind,
	};

	use super::MigrationError;

	#[test]
	fn framework_database_error_preserves_structured_migration_category_and_source() {
		let database_error = DatabaseError::new(
			DatabaseErrorKind::UniqueViolation,
			"duplicate migration record",
		)
		.with_code("23505");

		let migration_error = MigrationError::from(FrameworkError::from(database_error.clone()));

		match &migration_error {
			MigrationError::DatabaseError(error) => assert_eq!(error, &database_error),
			other => panic!("expected database migration error, got {other:?}"),
		}
		assert_eq!(
			migration_error
				.source()
				.and_then(|source| source.downcast_ref::<DatabaseError>())
				.map(DatabaseError::kind),
			Some(DatabaseErrorKind::UniqueViolation)
		);
	}

	#[test]
	fn non_database_framework_error_preserves_framework_category_and_source() {
		let migration_error = MigrationError::from(FrameworkError::Validation(
			"invalid migration input".to_string(),
		));

		match &migration_error {
			MigrationError::FrameworkError(error) => {
				assert_eq!(error.kind(), ErrorKind::Validation);
			}
			other => panic!("expected framework migration error, got {other:?}"),
		}
		assert_eq!(
			migration_error
				.source()
				.and_then(|source| source.downcast_ref::<FrameworkError>())
				.map(FrameworkError::kind),
			Some(ErrorKind::Validation)
		);
	}

	#[test]
	fn sourced_framework_database_error_remains_a_database_migration_error() {
		let database_error =
			DatabaseError::new(DatabaseErrorKind::Query, "type \"vector\" does not exist")
				.with_code("42704");
		let framework_error = FrameworkError::DatabaseWithSource {
			database_error,
			source: Box::new(io::Error::other("postgres driver failure")),
		};

		let migration_error = MigrationError::from(framework_error);

		let MigrationError::DatabaseError(database_error) = &migration_error else {
			panic!("expected sourced database migration error, got {migration_error:?}");
		};
		assert_eq!(database_error.kind(), DatabaseErrorKind::Query);
		assert_eq!(database_error.code(), Some("42704"));
		assert!(
			database_error
				.source()
				.and_then(|source| source.downcast_ref::<io::Error>())
				.is_some()
		);
	}

	#[cfg(feature = "pgvector")]
	mod vector_model_metadata {
		use reinhardt_core::macros::model;
		use serde::{Deserialize, Serialize};

		use crate::migrations::{FieldType, model_registry::global_registry};
		use crate::orm::{DatabaseStorageKind, Model, Vector, query_fields::Field as QueryField};

		#[model(
			app_label = "migration_vector_metadata",
			table_name = "vector_documents"
		)]
		#[derive(Clone, Debug, Serialize, Deserialize)]
		struct VectorDocument {
			#[field(primary_key = true)]
			id: i64,
			embedding: Vector<1536>,
		}

		fn assert_embedding_selector(_field: QueryField<VectorDocument, Vector<1536>>) {}

		#[test]
		fn model_vector_dimension_reaches_selector_inspection_and_migration_metadata() {
			assert_embedding_selector(VectorDocument::new_fields().embedding);

			let embedding = VectorDocument::field_metadata()
				.into_iter()
				.find(|field| field.name == "embedding")
				.expect("generated embedding inspection metadata");
			assert_eq!(
				embedding.storage_kind,
				Some(DatabaseStorageKind::Vector(1536))
			);
			assert_eq!(embedding.field_type, "reinhardt.orm.models.VectorField");

			let model = global_registry()
				.get_model("migration_vector_metadata", "VectorDocument")
				.expect("generated vector model registration");
			assert_eq!(
				model
					.fields
					.get("embedding")
					.expect("registered embedding migration field")
					.field_type,
				FieldType::Vector { dimensions: 1536 }
			);
		}
	}
}

// Prelude for migrations
/// Prelude module.
pub mod prelude {
	pub use super::fields::prelude::*;
	pub use super::{
		AlterTableOptions, ColumnDefinition, ColumnType, Constraint, DeferrableOption,
		ForeignKeyAction, GeneratedColumnDefinition, GeneratedStorage, IndexType, InterleaveSpec,
		Migration, MySqlAlgorithm, MySqlLock, Operation, PartitionDef, PartitionOptions,
		PartitionType, PartitionValues, SchemaBinOper, SchemaExpr, SchemaFunc,
	};
	pub use crate::field_domain::{FieldDomain, ModelEnumRepr, ModelEnumValue};
}
