#![warn(missing_docs)]
//! # Reinhardt Database
//!
//! Django-style database layer for Reinhardt framework.
//!
//! This crate provides a unified database abstraction that combines:
//! - **Database Backends**: Low-level database operations
//! - **Connection Pooling**: Advanced connection pool management
//! - **ORM**: Django-style ORM for database queries
//! - **Migrations**: Database schema migration system
//! - **Hybrid Types**: Common database type abstractions
//! - **Associations**: Relationship management between models
//!
//! Equivalent to Django's `django.db` package.
//!
//! ## Features
//!
//! ### Database Backends (`backends` module)
//!
//! - **Schema Editor Abstraction**: Unified `BaseDatabaseSchemaEditor` trait
//! - **Database-Specific Implementations**: PostgreSQL, MySQL, SQLite support
//! - **DDL Operations**: CREATE TABLE, ALTER TABLE, CREATE INDEX, etc.
//! - **Query Builder**: Type-safe query construction
//!
//! ### Connection Pooling (`pool` module)
//!
//! - **Advanced Pooling**: SQLAlchemy-inspired connection pool management
//! - **Dependency Injection**: Integration with Reinhardt DI system
//! - **Event Listeners**: Connection lifecycle hooks
//! - **Pool Configuration**: Fine-grained control over pool behavior
//!
//! ### ORM (`orm` module)
//!
//! - **Django-style Models**: Define database models with structs
//! - **QuerySet API**: Chainable query builder with typed latest/earliest,
//!   deterministic bulk retrieval, lazy empty querysets, and conditional partial updates
//! - **Field Types**: Rich set of field types with validation
//! - **Relationships**: ForeignKey, ManyToMany, OneToOne
//! - **Fixtures**: Django-compatible model fixture dump/load runtime with upsert,
//!   binary base64 values, SQL/JSON null provenance, foreign key, many-to-many,
//!   nullable foreign-key omission, and PostgreSQL sequence handling
//! - **Typed Relation Traversal**: Compile-time checked relation paths for SELECT filters and eager loading
//! - **Scoped N+1 Detection**: Opt-in query shape detection for focused diagnostics and tests
//!
//! ### Migrations (`migrations` module)
//!
//! - **Schema Migrations**: Track and apply database schema changes
//! - **Auto-detection**: Automatically detect model changes
//! - **Migration Files**: Generate migration files from model changes
//! - **Rollback Support**: Reverse migrations when needed
//! - **Typed Generated Columns**: Preserve `SchemaExpr` generated-column metadata in migrations
//! - **CockroachDB Migration Locking**: Serialize concurrent migrators with a
//!   sentinel-row lock instead of PostgreSQL advisory locks
//! - **MigrationStateLoader**: Django-style approach for building `ProjectState`
//!   - Replays applied migrations to reconstruct schema state
//!   - Enables accurate change detection without database introspection
//!   - Used internally by `makemigrations` command
//! - **Composite Primary Key Detection**: Autodetector emits `CreateCompositePrimaryKey`
//!   when a model adds a `primary_key` constraint covering 2+ columns
//! - **Sequence Reset Detection**: Autodetector emits `SetAutoIncrementValue`
//!   when a model adds or changes the `sequence_reset` option
//!
//! ## Available Database Backends
//!
//! The backends crate provides multiple database backend implementations:
//! - **PostgreSQL**: Full support with connection pooling
//! - **MySQL**: Full support with connection pooling
//! - **SQLite**: Full support with connection pooling
//! - **CockroachDB**: Distributed transaction support
//!
//! ## Optimization Features ✅
//!
//! - **Connection Pool Optimization**: Idle timeout, dynamic sizing, health checks
//! - **Query Caching**: LRU cache with TTL for prepared statements and results
//! - **Batch Operations**: Efficient bulk insert, update, and delete operations
//! - **N+1 Diagnostics**: Scoped warnings or test failures for repeated query shapes
//!
//! ## Enhanced Migration Tools ✅
//!
//! - **Schema Diff Detection**: Automatic detection of schema changes between DB and models
//! - **Auto-Migration Generation**: Generate migration files from detected differences
//! - **Migration Validation**: Pre-execution validation with data loss warnings
//! - **Rollback Script Generation**: Automatic rollback operations for safe migrations
//!
//! ### Native pgvector Support
//!
//! Enable the opt-in `pgvector` feature to store validated dense vectors in
//! PostgreSQL `vector(N)` columns. The extension is never installed
//! automatically: place `CreateExtension::new("vector")` before vector model
//! operations in the migration sequence.
//! When installing the extension in a custom schema, ensure that schema is in
//! PostgreSQL's `search_path` before executing vector DDL: Reinhardt renders
//! the pgvector type as the unqualified `vector(N)` identifier.
//!
//! The model macro accepts structured HNSW and IVFFlat indexes. Typed distance
//! expressions work in filters, ordering, annotations, and selected
//! expressions; every target vector remains a bound query value.
//!
//! ```rust
//! # mod migrations { pub use reinhardt_db::migrations::*; }
//! # mod orm { pub use reinhardt_db::orm::*; }
//! use reinhardt_core::macros::model;
//! use reinhardt_db::{
//!     migrations::{
//!         MigrationAutodetector, Operation, ProjectState, model_registry::global_registry,
//!         operations::postgres::CreateExtension,
//!     },
//!     orm::{Model, QuerySet, Vector},
//! };
//! use serde::{Deserialize, Serialize};
//!
//! #[model(app_label = "search", table_name = "documents")]
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct Document {
//!     #[field(primary_key = true)]
//!     id: Option<i64>,
//!     #[field(index(
//!         name = "documents_embedding_cosine_hnsw",
//!         method = "hnsw",
//!         opclass = "vector_cosine_ops",
//!         m = 16,
//!         ef_construction = 64
//!     ))]
//!     embedding: Vector<3>,
//!     #[field(index(
//!         name = "documents_summary_l2_ivfflat",
//!         method = "ivfflat",
//!         opclass = "vector_l2_ops",
//!         lists = 100
//!     ))]
//!     summary: Vector<3>,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let metadata = global_registry()
//!         .get_model("search", "Document")
//!         .ok_or("Document metadata was not registered")?;
//!     let mut target_state = ProjectState::new();
//!     target_state.add_model(metadata.to_model_state());
//!     let mut generated = MigrationAutodetector::new(ProjectState::new(), target_state)
//!         .try_generate_migrations()?;
//!     let mut migration = generated.pop().ok_or("Document migration was not generated")?;
//!     migration.operations.insert(
//!         0,
//!         CreateExtension::new("vector").into_operation()?,
//!     );
//!     assert!(matches!(
//!         migration.operations.first(),
//!         Some(Operation::CreateExtension { name, .. }) if name == "vector"
//!     ));
//!     assert!(migration.operations[1..]
//!         .iter()
//!         .any(|operation| matches!(operation, Operation::CreateTable { .. })));
//!     assert!(migration.operations[1..]
//!         .iter()
//!         .any(|operation| matches!(operation, Operation::CreateNamedIndex { .. })));
//!
//!     let target = Vector::<3>::try_from(vec![1.0, 0.0, 0.0])?;
//!     let fields = Document::new_fields();
//!     let nearest = QuerySet::<Document>::new()
//!         .filter(
//!             fields
//!                 .embedding
//!                 .clone()
//!                 .cosine_distance(target.clone())
//!                 .lt(0.5),
//!         )
//!         .order_by(
//!             fields
//!                 .embedding
//!                 .clone()
//!                 .l2_distance(target.clone())
//!                 .asc(),
//!         )
//!         .annotate_expr(
//!             "negative_inner_product",
//!             fields
//!                 .embedding
//!                 .clone()
//!                 .negative_inner_product(target.clone()),
//!         )
//!         .values(&["id"])
//!         .select_expr(
//!             "cosine_distance",
//!             fields.embedding.cosine_distance(target),
//!         )
//!         .limit(10);
//!
//!     let _ = nearest;
//!     Ok(())
//! }
//! ```
//!
//! `DatabaseMigrationExecutor` applies these operations in vector order.
//! Rolling this migration back removes the model schema and indexes but
//! deliberately leaves the database-level extension installed, because other
//! applications or schemas may share it.
//!
//! The distance methods map directly to PostgreSQL operators:
//!
//! | Method | Operator |
//! |--------|----------|
//! | `l2_distance` | `<->` |
//! | `negative_inner_product` | `<#>` |
//! | `cosine_distance` | `<=>` |
//!
//! `Vector<N>` accepts dimensions from 1 through 2000, requires exactly `N`
//! finite `f32` values, and represents only pgvector's dense `vector(N)` type.
//! `halfvec`, `bit`, `sparsevec`, binary quantization, and session tuning APIs
//! are outside this feature. An all-zero vector passes Reinhardt's finite-value
//! validation, but PostgreSQL pgvector does not index zero vectors for cosine
//! distance.
//!
//! Vector columns, values, distance expressions, and approximate indexes are
//! PostgreSQL-only. Checked construction for MySQL and SQLite returns structured
//! unsupported-backend errors. HNSW and IVFFlat indexes must be non-unique and
//! contain exactly one column or expression. Their operator class must be
//! `vector_l2_ops`, `vector_ip_ops`, or `vector_cosine_ops`; HNSW's `m` and
//! `ef_construction` and IVFFlat's `lists` must be positive when supplied.
//! Explicit index names are preserved, and duplicate physical names are
//! rejected before SQL execution.
//!
//! If a vector type, operator, or operator class is missing, PostgreSQL errors
//! that contain pgvector evidence retain their SQLSTATE and original SQLx
//! source while adding a hint to install the extension explicitly with
//! `CreateExtension::new("vector")`.
//!
//! The `pgvector` dependency has default features disabled and does not enable
//! pgvector's SQLx integration. Reinhardt implements the binary codec against
//! its workspace SQLx 0.8 dependency, avoiding a second SQLx API surface.
//!
//! ## Quick Start
//!
//! ### Using Schema Editor
//!
//! ```rust,no_run
//! # use sqlx::PgPool;
//! use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
//! use reinhardt_query::prelude::{PostgresQueryBuilder, QueryStatementBuilder};
//!
//! # async fn example() -> Result<(), sqlx::Error> {
//! # let pool = PgPool::connect("postgresql://localhost/mydb").await?;
//! let factory = SchemaEditorFactory::new_postgres(pool);
//! let editor = factory.create_for_database(DatabaseType::PostgreSQL);
//!
//! let stmt = editor.create_table_statement("users", &[
//!     ("id", "INTEGER PRIMARY KEY"),
//!     ("name", "VARCHAR(100)"),
//! ]);
//! let sql = stmt.to_string(PostgresQueryBuilder);
//! # Ok(())
//! # }
//! ```
//!
//! ### Using Connection Pool
//!
//! ```rust,no_run
//! use reinhardt_db::pool::{ConnectionPool, PoolConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let pool = ConnectionPool::new_postgres("postgres://localhost/mydb", PoolConfig::default()).await?;
//! let conn = pool.acquire().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Structured Error Handling
//!
//! Database failures retain a driver-independent category. Vendor-specific
//! codes remain available as optional diagnostic metadata:
//!
//! ```rust
//! use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
//!
//! let error = Error::from(DatabaseError::new(
//!     DatabaseErrorKind::UniqueViolation,
//!     "email already exists",
//! ).with_code("23505"));
//!
//! assert_eq!(error.database_kind(), Some(DatabaseErrorKind::UniqueViolation));
//! assert_eq!(error.database_error().and_then(DatabaseError::code), Some("23505"));
//! ```
//!
//! Transaction callbacks may propagate an application-owned error when it can
//! convert framework failures through `From<reinhardt_core::exception::Error>`:
//!
//! ```rust,no_run
//! use reinhardt_core::exception::Error;
//! use reinhardt_db::{
//!     backends::DatabaseConnection as BackendsConnection,
//!     orm::DatabaseConnectionLease,
//! };
//!
//! #[derive(Debug, thiserror::Error)]
//! enum ApplicationError {
//!     #[error("operation rejected")]
//!     Rejected,
//!     #[error(transparent)]
//!     Framework(#[from] Error),
//! }
//!
//! # async fn example() -> Result<(), ApplicationError> {
//! let owner = BackendsConnection::connect_sqlite("sqlite::memory:").await?;
//! let lease = DatabaseConnectionLease::register(owner)?;
//! let connection = lease.handle();
//! let result: Result<(), ApplicationError> = connection.atomic(async |_transaction| {
//!     Err(ApplicationError::Rejected)
//! }).await;
//!
//! result
//! # }
//! ```
//!
//! ## Architecture
//!
//! Key modules in this crate:
//!
//! - [`backends`]: Low-level database operations, schema editor, DDL generation
//! - [`backends_pool`]: Connection pool management with lifecycle hooks
//! - [`pool`]: High-level pool abstraction for `ConnectionPool`
//! - [`orm`]: Django-style model definitions, QuerySet, field types, and
//!   model-level fixture support
//! - [`migrations`]: Schema migration system with auto-detection and rollback
//! - [`hybrid`]: Cross-database compatible type system
//! - [`associations`]: Relationship management (ForeignKey, ManyToMany)
//! - `contenttypes`: Generic foreign key support
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `backends` | enabled | Database backend abstractions and schema editor |
//! | `pool` | enabled | Connection pooling support |
//! | `orm` | enabled | ORM model definitions and QuerySet API |
//! | `migrations` | enabled | Database migration system |
//! | `hybrid` | enabled | Cross-database hybrid type system |
//! | `associations` | enabled | Model relationship management |
//! | `postgres` | enabled | PostgreSQL backend |
//! | `sqlite` | disabled | SQLite backend |
//! | `mysql` | disabled | MySQL backend |
//! | `pgvector` | disabled | Native PostgreSQL dense-vector ORM and migrations |
//! | `all-databases` | disabled | Enable all database backends |
//! | `backends-pool` | disabled | Connection pool backend abstractions |
//! | `contenttypes` | disabled | Generic foreign key support |
//! | `nosql` | disabled | NoSQL/BSON type support |
//! | `di` | disabled | Dependency injection integration |
//! | `database-full` | disabled | Enable all database features |
//!
//! The `model-info` feature is intentionally target-neutral. It exists for
//! macro-generated metadata surfaces that need to compile on WASM without
//! enabling ORM, migrations, connection pools, or database drivers.

pub mod naming;

#[cfg(feature = "associations")]
pub mod associations;
#[cfg(feature = "backends")]
pub mod backends;
#[cfg(any(feature = "backends", feature = "backends-pool"))]
pub mod backends_pool;
#[cfg(feature = "contenttypes")]
pub mod contenttypes;
#[cfg(any(feature = "orm", feature = "migrations"))]
pub mod field_domain;
#[cfg(feature = "hybrid")]
pub mod hybrid;
#[cfg(any(feature = "orm", feature = "migrations"))]
pub mod m2m_naming;
#[cfg(feature = "migrations")]
pub mod migrations;
#[cfg(feature = "nosql")]
pub mod nosql;
#[cfg(feature = "orm")]
pub mod orm;
#[cfg(feature = "pool")]
pub mod pool;

#[cfg(feature = "model-info")]
pub use reinhardt_core::model_info;

/// Prelude module for convenient imports
///
/// Imports commonly used types from all modules.
#[allow(ambiguous_glob_reexports)]
pub mod prelude {
	#[cfg(feature = "backends")]
	pub use crate::backends::*;

	#[cfg(feature = "pool")]
	pub use crate::pool::*;

	#[cfg(feature = "orm")]
	pub use crate::orm::*;

	#[cfg(feature = "orm")]
	// Allow the private explicit import to keep the lease out of the public prelude glob.
	#[allow(hidden_glob_reexports)]
	// Allow the shadowing import even though the prelude does not use the lease internally.
	#[allow(unused_imports)]
	use crate::orm::DatabaseConnectionLease;

	#[cfg(feature = "migrations")]
	pub use crate::migrations::*;

	#[cfg(feature = "hybrid")]
	pub use crate::hybrid::*;

	#[cfg(feature = "associations")]
	pub use crate::associations::*;

	#[cfg(feature = "contenttypes")]
	pub use crate::contenttypes::*;

	#[cfg(feature = "nosql")]
	pub use crate::nosql::*;

	// Re-export types needed by Model derive macro
	#[cfg(feature = "migrations")]
	pub use crate::migrations::model_registry::{FieldMetadata, global_registry};
}

// Re-export top-level commonly used types
#[cfg(feature = "backends")]
pub use backends::{DatabaseBackend, DatabaseError, DatabaseErrorKind};

/// Copyable ORM connection handle used by managers and application handlers.
///
/// Standalone applications retain an [`orm::DatabaseConnectionLease`] for the
/// full lifetime of every handle. Framework bootstrap owns that lease and
/// injects this handle into request handlers.
#[cfg(feature = "orm")]
pub use orm::DatabaseConnection;
#[cfg(feature = "orm")]
pub use orm::Json;

#[cfg(feature = "pool")]
pub use pool::{ConnectionPool, PoolConfig, PoolError};
