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
