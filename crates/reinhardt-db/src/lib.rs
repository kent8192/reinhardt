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
//! - **QuerySet API**: Chainable query builder
//! - **Field Types**: Rich set of field types with validation
//! - **Relationships**: ForeignKey, ManyToMany, OneToOne
//!
//! ### Migrations (`migrations` module)
//!
//! - **Schema Migrations**: Track and apply database schema changes
//! - **Auto-detection**: Automatically detect model changes
//! - **Migration Files**: Generate migration files from model changes
//! - **Rollback Support**: Reverse migrations when needed
//! - **MigrationStateLoader**: Django-style approach for building `ProjectState`
//!   - Replays applied migrations to reconstruct schema state
//!   - Enables accurate change detection without database introspection
//!   - Used internally by `makemigrations` command
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
//! ## Feature Flags
//!
//! - `backends` (default): Database backend abstractions
//! - `pool` (default): Connection pooling support
//! - `orm` (default): ORM functionality
//! - `migrations` (default): Migration system
//! - `hybrid` (default): Hybrid type system
//! - `associations` (default): Association management
//! - `postgres` (default): PostgreSQL support
//! - `sqlite`: SQLite support
//! - `mysql`: MySQL support
//! - `all-databases`: Enable all database backends

pub mod associations;
pub mod backends;
pub mod backends_pool;
pub mod contenttypes;
pub mod hybrid;
pub mod migrations;
#[cfg(feature = "nosql")]
pub mod nosql;
pub mod orm;
pub mod pool;

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
pub use backends::{DatabaseBackend, DatabaseError};

// Re-export ORM's DatabaseConnection which wraps BackendsConnection
// This is the type used by Manager and other ORM components
#[cfg(feature = "orm")]
pub use orm::DatabaseConnection;

#[cfg(feature = "pool")]
pub use pool::{ConnectionPool, PoolConfig, PoolError};
