# reinhardt-db

Django-style database layer for Reinhardt framework

## Overview

`reinhardt-db` provides a comprehensive database layer for Reinhardt applications, inspired by Django's ORM with powerful features for database abstraction, object-relational mapping, migrations, and connection pooling.

This crate provides a comprehensive database layer organized into multiple modules to deliver a unified database experience.

## Features

### Implemented ✓

This crate provides the following modules:

- **ORM**: Object-Relational Mapping system
  - Django-inspired Model trait
  - QuerySet API for chainable queries
  - Field types (AutoField, CharField, IntegerField, DateTimeField, etc.)
  - Timestamped and SoftDeletable traits
  - Relationship management
  - Validators and choices

- **Migrations**: Schema migration system
  - Automatic migration generation from model changes
  - Forward and backward migrations
  - Schema versioning and dependency management
  - Migration operations (CreateModel, AddField, AlterField, etc.)
  - State management and autodetection
  - **State Loader** (`MigrationStateLoader`): Django-style state reconstruction
    - Build `ProjectState` by replaying migration history
    - Avoid direct database introspection for schema detection
    - Ensure consistency between migration files and actual schema state

- **Pool**: Connection pool management
  - Database connection pooling
  - Connection lifecycle management
  - Pool configuration and sizing

- **Hybrid**: Hybrid database support
  - Multi-database routing
  - Read/write splitting
  - Database sharding support

- **Associations**: Relationship management
  - Foreign key relationships
  - Many-to-many relationships
  - One-to-one relationships
  - Lazy loading and eager loading

### Implemented ✓ (Additional Features)

- **Advanced Query Optimization**
  - Query result caching with cache hit/miss tracking
  - Query plan analysis and optimization
  - SELECT DISTINCT optimization
  - EXISTS vs IN subquery optimization
  - Cursor-based pagination (more efficient than OFFSET)
  - Bulk operations (bulk create, bulk update)
  - N+1 query prevention with select_related and prefetch_related
  - Lazy query evaluation
  - Only/Defer field optimization for reduced data transfer
  - Aggregate pushdown optimization

- **Enhanced Transaction Management**
  - Nested transactions with savepoint support
  - Isolation level control (ReadUncommitted, ReadCommitted, RepeatableRead, Serializable)
  - Named savepoints (create, release, rollback to savepoint)
  - Transaction state tracking (NotStarted, Active, Committed, RolledBack)
  - Two-phase commit (2PC) for distributed transactions
  - Atomic transaction wrapper (Django-style transaction.atomic)
  - Database-level transaction execution methods

- **Database Replication and Routing**
  - Read/write splitting via DatabaseRouter
  - Model-based database routing rules
  - Configurable default database
  - Per-model read and write database configuration
  - Multi-database support through hybrid module

## Module Architecture

The `reinhardt-db` crate is organized into three logical layers:

### Core Layers

High-level APIs for everyday database operations:

- **`orm` module**: High-level ORM API
  - Use for model CRUD operations
  - QuerySet API for building queries
  - Relationship management
  - **When to use**: Building application logic, working with models

- **`migrations` module**: Schema migration system
  - Use for database schema changes
  - Automatic migration generation
  - Migration history tracking
  - **When to use**: Managing database schema evolution

### Database Backend Layers

Low-level database connectivity and connection management:

- **`backends` module**: Low-level database drivers
  - PostgreSQL, MySQL, SQLite support
  - Query execution and schema operations
  - reinhardt-query integration for query building
  - **When to use**: Need direct database access or custom queries

- **`pool` module**: Connection pooling implementation
  - Direct connection pool management
  - Multi-database pool support
  - Event system for monitoring
  - **When to use**: Managing connection pools directly

- **`backends_pool` module**: Pool backend abstractions for DI
  - DI-compatible pool abstractions
  - Injectable pool services
  - **When to use**: Using dependency injection framework

**Key difference**: Use `pool` module for direct pool management. Use `backends_pool` module when integrating with dependency injection systems.

### Extension Layers

Advanced features for specific use cases:

- **`associations` module**: Relationship management
  - ForeignKey, OneToOne, OneToMany, ManyToMany
  - Association proxies
  - Loading strategies (lazy, eager, select-in, joined)
  - **When to use**: Complex relationships between models

- **`hybrid` module**: Hybrid properties
  - Instance-level and SQL-level properties
  - Computed properties in queries
  - **When to use**: Need computed properties usable in database queries

- **`contenttypes` module**: Generic relations
  - Django-style content type framework
  - Generic foreign keys
  - **When to use**: Polymorphic relationships (comments, tags, etc.)

- **`nosql` module**: NoSQL database support
  - MongoDB integration (implemented)
  - Unified NoSQL backend traits
  - Document, Key-Value, Column-Family, Graph paradigms
  - **When to use**: Working with NoSQL databases like MongoDB

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
reinhardt-db = "0.1.0-alpha.1"
```

### Optional Features

Enable specific features based on your needs:

```toml
[dependencies]
reinhardt-db = { version = "0.1.0-alpha.1", features = ["postgres", "orm", "migrations"] }
```

Available features:

- `database` (default): Low-level database layer
- `backends` (default): Backend implementations
- `pool` (default): Connection pooling
- `orm` (default): ORM functionality
- `migrations` (default): Migration system
- `hybrid` (default): Multi-database support
- `associations` (default): Relationship management
- `postgres`: PostgreSQL support
- `sqlite`: SQLite support
- `mysql`: MySQL support
- `all-databases`: All database backends

## Usage

### Define Models

```rust
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize)]
#[model(app_label = "myapp", table_name = "users")]
pub struct User {
    /// Primary key
    #[field(primary_key = true)]
    pub id: i64,

    /// Username (max 50 characters, unique)
    #[field(max_length = 50, unique = true)]
    pub username: String,

    /// Email address (max 254 characters)
    #[field(max_length = 254)]
    pub email: String,

    /// User's age
    pub age: i32,

    /// Account creation timestamp (auto-populated on insert)
    #[field(auto_now_add = true)]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp (auto-updated on save)
    #[field(auto_now = true)]
    pub updated_at: DateTime<Utc>,
}
```

**Field Attributes:**
- `#[field(primary_key = true)]` - Primary key
- `#[field(max_length = N)]` - Maximum length for strings
- `#[field(unique = true)]` - Unique constraint
- `#[field(auto_now_add = true)]` - Auto-populate on creation
- `#[field(auto_now = true)]` - Auto-update on save
- `#[field(null = true)]` - Allow NULL values
- `#[field(default = value)]` - Default value
- `#[field(foreign_key = "ModelType")]` - Foreign key relationship

For a complete list of field attributes, see the [Field Attributes Guide](../../docs/field_attributes.md).

**Note**: The `#[model(...)]` attribute macro automatically generates:
- `Model` trait implementation
- Type-safe field accessors (`User::field_username()`, `User::field_email()`, etc.)
- Global model registry registration
- Support for composite primary keys

### Query with QuerySet

```rust
use reinhardt::db::{QuerySet, Model};

// Get all users
let users = User::objects().all().await?;

// Filter users
let adults = User::objects()
    .filter("age__gte", 18)
    .order_by("-created_at")
    .all()
    .await?;

// Get a single user
let user = User::objects()
    .filter("username", "john")
    .first()
    .await?;
```

### Create Migrations

```rust
use reinhardt::db::{Migration, CreateModel, AddField};

// Create a new migration
let migration = Migration::new("0001_initial")
    .add_operation(CreateModel {
        name: "User",
        fields: vec![
            ("id", "AutoField"),
            ("username", "CharField(max_length=50)"),
            ("email", "EmailField"),
        ],
    });

// Apply migration
migration.apply(db).await?;
```

### Connection Pooling

```rust
use reinhardt::db::Pool;

// Create a connection pool
let pool = Pool::new("postgres://user:pass@localhost/db")
    .max_connections(10)
    .build()
    .await?;

// Get a connection
let conn = pool.get().await?;
```

## Module Organization

`` `reinhardt-db` `` is organized into the following modules:

### Core Modules
- `` `orm` `` - Object-Relational Mapping system
- `` `migrations` `` - Schema migration system
- `` `pool` `` - Connection pooling

### Backend Modules
- `` `backends` `` - Database drivers (PostgreSQL, MySQL, SQLite)
- `` `backends-pool` `` - DI-aware pool abstractions

### Extension Modules
- `` `associations` `` - Relationship management (ForeignKey, ManyToMany, etc.)
- `` `hybrid` `` - Hybrid properties and multi-database support
- `` `contenttypes` `` - Generic relations (polymorphic)
- `` `nosql` `` - NoSQL database support (MongoDB)

### Using Modules

```rust
use reinhardt::db::orm::{Model, QuerySet};
use reinhardt::db::migrations::Migration;
use reinhardt::db::pool::ConnectionPool;
```

## Supported Databases

- PostgreSQL
- MySQL
- SQLite

## Testing

### Prerequisites

Database-related tests require **Docker** for TestContainers integration:

```bash
# Verify Docker is running
docker version
docker ps
```

**CRITICAL**: This project uses Docker for TestContainers integration, NOT Podman.

- **MUST** ensure Docker Desktop is installed and running
- **MUST** ensure `DOCKER_HOST` environment variable points to Docker socket:
  - ✅ Correct: `unix:///var/run/docker.sock` or not set
  - ❌ Incorrect: `unix:///.../podman/...` (will cause container startup failures)

If both Docker and Podman are installed:
- Use `.testcontainers.properties` to force Docker usage (already configured in project root)
- Ensure `DOCKER_HOST` is not set to Podman socket

### Running Database Tests

```bash
# Run all database tests (requires Docker)
cargo test --package reinhardt-db --all-features

# Run tests for specific module
cargo test --package reinhardt-orm --all-features
cargo test --package reinhardt-migrations --all-features

# Run with PostgreSQL container (TestContainers automatically starts PostgreSQL)
cargo test --package reinhardt-orm --test orm_integration_tests
```

### TestContainers Integration

Database tests automatically use TestContainers to:
- Start PostgreSQL 17 Alpine container before tests
- Provide isolated database instance per test suite
- Clean up containers after tests complete

**Standard Fixtures** from `reinhardt-test` are available:

```rust
use reinhardt::test::fixtures::postgres_container;
use rstest::*;

#[rstest]
#[tokio::test]
async fn test_with_database(
    #[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
    let (_container, pool, _port, _database_url) = postgres_container.await;

    // Use pool for database operations
    let result = sqlx::query("SELECT 1").fetch_one(pool.as_ref()).await;
    assert!(result.is_ok());

    // Container is automatically cleaned up when dropped
}
```

For comprehensive testing standards, see:
- [Testing Standards](../../docs/TESTING_STANDARDS.md)
- [Examples Database Integration](../../examples/examples-database-integration/README.md)

### Troubleshooting

**"Cannot connect to Docker daemon" or "IncompleteMessage" errors:**

```bash
# 1. Check Docker is running
docker ps

# 2. Check DOCKER_HOST environment variable
echo $DOCKER_HOST

# 3. If DOCKER_HOST points to Podman, unset it
unset DOCKER_HOST

# 4. Verify .testcontainers.properties exists in project root
cat .testcontainers.properties
```


## associations

### Features

### Implemented ✓

#### Association Proxy (`AssociationProxy<S, A, T>`)

- **Single object attribute access**: Access attributes of related objects through foreign key and one-to-one relationships
- **Type-safe proxies**: Compile-time type checking for association chains
- **Generic implementation**: Works with any source type, associated type, and target attribute type
- **Key methods**:
  - `new()`: Create a new association proxy with custom getter functions
  - `get()`: Retrieve the target attribute through the association

#### Association Collection (`AssociationCollection<S, C, T>`)

- **Collection attribute access**: Access attributes of items in collections through one-to-many and many-to-many relationships
- **Batch operations**: Retrieve all target attributes from a collection at once
- **Collection utilities**: Count and check emptiness of collections
- **Key methods**:
  - `new()`: Create a new association collection proxy with custom getter functions
  - `get_all()`: Get all target attributes from the collection
  - `count()`: Count the number of items in the collection
  - `is_empty()`: Check if the collection is empty

#### Prelude Module

- Re-exports commonly used types for convenient importing

#### Relationship Types

- **ForeignKey** - Many-to-one relationships with cascade actions
  - Define foreign key relationships between models
  - Support for cascade operations (CASCADE, SET_NULL, SET_DEFAULT, RESTRICT, NO_ACTION)
  - Automatic reverse accessor generation

- **OneToOne** - Unique one-to-one relationships
  - Bidirectional one-to-one relationships
  - Unique constraint enforcement
  - Optional reverse relationship naming

- **OneToMany** - One-to-many relationships (reverse side of ForeignKey)
  - Collection-based access to related objects
  - Lazy loading by default
  - Custom related name support

- **ManyToMany** - Many-to-many relationships through junction tables
  - Automatic junction table management
  - Bidirectional access
  - Custom junction table configuration

- **PolymorphicAssociation** - Polymorphic one-to-many relationships
  - Generic foreign keys to multiple model types
  - Content type tracking
  - Type-safe polymorphic queries

- **PolymorphicManyToMany** - Polymorphic many-to-many relationships
  - Many-to-many with polymorphic targets
  - Generic relationship support

#### Cascade Actions

Define behavior when parent objects are deleted:

- **CASCADE** - Delete related objects when parent is deleted
- **SET_NULL** - Set foreign key to NULL when parent is deleted
- **SET_DEFAULT** - Set foreign key to default value when parent is deleted
- **RESTRICT** - Prevent deletion if related objects exist
- **NO_ACTION** - No automatic action (database constraint only)

#### Loading Strategies

Optimize how related objects are loaded:

- **LazyLoader** - Load related objects only when accessed (default)
  - Minimizes initial query overhead
  - Best for seldom-accessed relationships

- **EagerLoader** - Load related objects immediately with parent
  - Single query with JOIN
  - Best for always-accessed relationships

- **SelectInLoader** - Use SELECT IN strategy for collections
  - Efficient for loading multiple related collections
  - Avoids N+1 query problem

- **JoinedLoader** - Use SQL JOIN for single query loading
  - Fetch everything in one query
  - Best for small result sets

- **SubqueryLoader** - Use subquery for complex filtering
  - Advanced query optimization
  - Best for complex filtering requirements

#### Reverse Relationships

- **Automatic reverse accessor generation** - Related models get automatic reverse accessors
- **Custom naming** - Override default reverse accessor names with `related_name`
- **Singular forms** - Generate singular accessor names for one-to-one relationships


## contenttypes

### Features

### Implemented ✓

#### Core Content Type System

- **ContentType Model** - Represents a model type with app label and model name
  - `ContentType::new()` - Create a new content type
  - `ContentType::with_id()` - Set content type ID
  - `natural_key()` - Get (app_label, model) tuple for natural key
  - `qualified_name()` - Get fully qualified name (e.g., "blog.Post")
  - Implements `Serialize`, `Deserialize`, `PartialEq`, `Eq`, `Hash`, `Clone`

#### Content Type Registry (Runtime)

- **ContentTypeRegistry** - Runtime content type management with thread-safe caching
  - `register()` - Register a new content type with automatic ID assignment
  - `get()` - Get content type by app label and model name
  - `get_by_id()` - Get content type by ID
  - `get_or_create()` - Get existing or create new content type
  - `all()` - List all registered content types
  - `clear()` - Clear registry (mainly for testing)
  - Thread-safe with `RwLock` for concurrent access
  - Automatic ID generation for registered types

#### Global Content Type Registry

- **CONTENT_TYPE_REGISTRY** - Global singleton registry instance
  - Available via `once_cell::Lazy` for initialization
  - Shared across the application for consistent content type management

#### Generic Foreign Keys

- **GenericForeignKey** - Field for referencing any model type
  - `new()` - Create empty generic foreign key
  - `set()` - Set content type and object ID
  - `get_content_type()` - Retrieve associated content type
  - `is_set()` - Check if both content type and object ID are set
  - `clear()` - Clear content type and object ID
  - Implements `Default`, `Serialize`, `Deserialize`, `Clone`

#### Type-Safe API (Compile-Time)

- **ModelType Trait** - Compile-time type-safe content type definitions
  - `APP_LABEL` - Associated constant for app label
  - `MODEL_NAME` - Associated constant for model name
  - Type-safe methods for `ContentTypeRegistry`:
    - `get_typed<M: ModelType>()` - Type-safe get
    - `get_or_create_typed<M: ModelType>()` - Type-safe get or create
    - `register_typed<M: ModelType>()` - Type-safe register
  - Type-safe methods for `GenericForeignKey`:
    - `set_typed<M: ModelType>()` - Type-safe set with model type

#### Generic Relation Queries

- **GenericRelatable Trait** - Trait for models that can be targets of generic relations
  - `get_content_type()` - Get content type for the model
  - `get_object_id()` - Get object ID for the instance

- **GenericRelationQuery** - Helper for building generic relation queries
  - `new()` - Create query for specific content type
  - `add_object()` - Add object ID to query
  - `to_sql()` - Generate SQL query for fetching related objects

#### Database Integration

- **ContentTypePersistence** - Database-backed content type storage
  - `new()` - Create persistence backend with database URL
  - `from_pool()` - Create from existing connection pool
  - `create_table()` - Automatic table creation with indexes
  - `get()`, `get_by_id()` - Retrieve content types from database
  - `get_or_create()` - Get existing or create new content type in database
  - `save()`, `delete()` - Persist and remove content types
  - `load_all()` - Load all content types from database
  - `exists()` - Check content type existence
  - Supports PostgreSQL, MySQL, and SQLite via sqlx

- **Multi-Database Support**
  - `MultiDbContentTypeManager` - Manage content types across multiple databases
  - Per-database content type registries with isolated caching
  - Cross-database content type searches
  - Database routing for content type operations
  - `add_database()` - Register new database connections
  - `search_all_databases()` - Find content types across all databases
  - `list_databases()` - Get all registered database names

- **GenericForeignKey Constraints**
  - Database-level validation for generic foreign keys
  - `validate_content_type()` - Verify content type exists in database
  - `get_validated_content_type()` - Retrieve validated content type from database

#### ORM Integration

- **ContentTypeQuery** - ORM-style query builder for content types
  - `new()` - Create query builder from connection pool
  - `filter_app_label()`, `filter_model()`, `filter_id()` - Filter by fields
  - `order_by_app_label()`, `order_by_model()`, `order_by_id()` - Sorting
  - `order_by_*_desc()` - Descending order variants
  - `limit()`, `offset()` - Pagination support
  - `all()` - Execute query and get all results
  - `first()` - Get first result
  - `count()` - Count matching records
  - `exists()` - Check if any records match
  - Django-inspired QuerySet API with method chaining

- **ContentTypeTransaction** - Transaction-aware content type operations
  - `new()` - Create transaction context
  - `query()` - Get query builder for transaction
  - `create()` - Create content type within transaction
  - `delete()` - Delete content type within transaction
  - Full ACID transaction support for content type operations


## hybrid

### Features

### Implemented ✓

#### HybridProperty

- **Instance-level getters**: Define getters that work on struct instances
  - `HybridProperty::new()` - Create a property with instance-level behavior
  - `get()` - Get the value for an instance
- **SQL expression support**: Generate SQL expressions for database queries
  - `with_expression()` - Add SQL expression generation capability
  - `expression()` - Get the SQL expression string
- **Type-safe**: Full type safety with generics `HybridProperty<T, R>`

#### HybridMethod

- **Instance-level methods**: Define methods that accept parameters
  - `HybridMethod::new()` - Create a method with instance-level behavior
  - `call()` - Call the method for an instance with arguments
- **SQL expression methods**: Generate parameterized SQL expressions
  - `with_expression()` - Add SQL expression generation capability
  - `expression()` - Get the SQL expression string with arguments
- **Type-safe**: Full type safety with generics `HybridMethod<T, A, R>`

#### SQL Expression Builders

- **SqlExpression struct**: Serializable SQL expression container
  - `new()` - Create a SQL expression from a string
  - `concat()` - Generate CONCAT expressions
  - `lower()` - Generate LOWER expressions for case-insensitive operations
  - `upper()` - Generate UPPER expressions for case-insensitive operations
  - `coalesce()` - Generate COALESCE expressions for NULL handling
- **Expression trait**: Convert types to SQL strings
  - Implemented for `SqlExpression`, `String`, and `&str`
  - `to_sql()` - Convert to SQL string representation

#### Comparator System

- **Comparator trait**: Customize SQL comparison operations
  - `new()` - Create a comparator with an expression
  - `eq()`, `ne()` - Equality and inequality comparisons
  - `lt()`, `le()`, `gt()`, `ge()` - Ordering comparisons
- **UpperCaseComparator**: Built-in case-insensitive comparator
  - Automatically applies UPPER() to both sides of comparisons

#### Property Override Support

- **HybridPropertyOverride trait**: Define overridable property behavior
  - `get_instance()` - Get instance-level value
  - `get_expression()` - Get SQL expression (optional)
  - `set_instance()` - Set instance-level value (optional)
- **OverridableProperty wrapper**: Composition-based property override
  - `new()` - Create an overridable property with custom implementation
  - `get()`, `set()` - Instance-level getters and setters
  - `expression()` - SQL expression support
  - Enables polymorphic behavior without traditional inheritance

#### Macro Support

- **hybrid_property! macro**: Convenience macro for defining hybrid properties


## migrations

### Features

### Implemented ✓

#### Core Migration System

- **Migration Operations**: Comprehensive set of operations for schema changes
  - Model operations: `CreateModel`, `DeleteModel`, `RenameModel`
  - Field operations: `AddField`, `RemoveField`, `AlterField`, `RenameField`
  - Special operations: `RunSQL`, `RunCode` (Rust equivalent of Django's RunPython)
  - PostgreSQL-specific: `CreateExtension`, `DropExtension`, `CreateCollation`

- **State Management**: Track schema state across migrations
  - `ProjectState`: Maintains complete database schema state
  - `ModelState`: Represents individual model definitions
  - `FieldState`: Tracks field configurations
  - Support for indexes and constraints

- **Autodetection**: Automatically detect schema changes
  - `MigrationAutodetector`: Detects differences between states
  - Model creation/deletion detection
  - Field addition/removal/modification detection
  - Smart rename detection for models and fields
  - Index and constraint change detection

- **Migration Execution**
  - `MigrationExecutor`: Apply migrations to SQLite databases
  - `DatabaseMigrationExecutor`: Multi-database support (PostgreSQL, MySQL, SQLite)
  - Transaction support and rollback capability
  - Migration recorder for tracking applied migrations

- **Migration Management**
  - `MigrationLoader`: Load migrations from disk
  - `MigrationWriter`: Generate Rust migration files
  - Migration file serialization (JSON format)
  - Dependency tracking and validation

- **CLI Commands**
  - `makemigrations`: Generate migrations from model changes
    - Dry-run mode for previewing changes
    - Custom migration naming
    - App-specific migration generation
  - `migrate`: Apply migrations to database
    - Fake migrations support
    - Migration plan preview

- **Migration State Management**
  - `MigrationStateLoader`: Django-style state reconstruction from migration history
    - Build `ProjectState` by replaying applied migrations in topological order
    - Avoid direct database introspection for change detection
    - Ensure schema state consistency with migration files

- **Database Backend Support**
  - SQLite support via sqlx
  - PostgreSQL support via reinhardt-backends
  - MySQL support via reinhardt-backends
  - SQL dialect abstraction for cross-database compatibility

- **Dependency Injection Integration**
  - `MigrationService`: DI-compatible service for migrations
  - `MigrationConfig`: Configuration management
  - Integration with reinhardt-di

#### Advanced Features

- **Migration Graph**: Complete dependency resolution system (graph.rs skeleton exists)
- **Migration Squashing**: Combine multiple migrations into one for performance
- **Data Migrations**: Built-in support for complex data transformations
- **Zero-downtime Migrations**: Safe schema changes without service interruption
- **Migration Optimization**: Automatic operation reordering and combining
- **Atomic Operations**: Better transaction handling for complex migrations
- **Schema History Visualization**: Graphical representation of migration history

#### Enhanced Autodetection

- **Field Default Detection**: Automatically detect default value changes
- **Constraint Detection**: Better support for CHECK, UNIQUE, and FOREIGN KEY constraints
- **Index Optimization**: Suggest index additions based on model relationships

#### Database-Specific Features

- **PostgreSQL**: Advanced types (JSONB, Arrays, Custom types)
- **MySQL**: Storage engine management, partition support
- **SQLite**: Better handling of ALTER TABLE limitations

#### Developer Experience

- **Interactive Mode**: Guided migration creation
- **Conflict Resolution**: Automatic handling of migration conflicts
- **Migration Testing**: Built-in tools for testing migrations
- **Performance Profiling**: Measure migration execution time and identify bottlenecks


## nosql

### Features

- **Document Databases**: MongoDB (✅), CouchDB (planned)
- **Key-Value Stores**: Redis (planned), DynamoDB (planned)
- **Column-Family Stores**: Cassandra (planned)
- **Graph Databases**: Neo4j (planned)
- **Zero-Cost Abstractions**: Uses generics to minimize runtime overhead
- **Type-Safe API**: Compile-time guarantees for database operations
- **Transaction Support**: Multi-document ACID transactions (MongoDB with replica set)


## pool

### Features

### Implemented ✓

#### Core Connection Pool

- **Multi-database support**: PostgreSQL, MySQL, SQLite connection pools
  - `ConnectionPool::new_postgres()` - Create PostgreSQL connection pool
  - `ConnectionPool::new_mysql()` - Create MySQL connection pool
  - `ConnectionPool::new_sqlite()` - Create SQLite connection pool
- **Connection acquisition**: Acquire connections from pool with event emission
- **Pooled connections**: Wrapper type with automatic return-to-pool on drop
- **Pool recreation**: Recreate pools with same configuration for all database types
- **Inner pool access**: Direct access to underlying sqlx pool when needed

#### Pool Configuration

- **Flexible sizing**: Configurable min/max connection limits
  - `max_connections` - Maximum number of connections
  - `min_connections` - Minimum idle connections to maintain
  - `max_size` - Overall pool size limit
  - `min_idle` - Optional minimum idle connections
- **Timeout management**: Configurable connection and acquisition timeouts
  - `connection_timeout` - Timeout for creating new connections
  - `acquire_timeout` - Timeout for acquiring from pool
  - `idle_timeout` - Optional timeout for idle connections
- **Lifecycle settings**: Connection lifetime and idle timeout configuration
  - `max_lifetime` - Optional maximum connection lifetime
- **Health checks**: Optional test-before-acquire validation
  - `test_before_acquire` - Validate connections before use
- **Builder pattern**: `PoolOptions` for ergonomic configuration with method chaining

#### Event System

- **Connection lifecycle events**: Track connection state changes
  - `ConnectionAcquired` - Connection checked out from pool
  - `ConnectionReturned` - Connection returned to pool
  - `ConnectionCreated` - New connection established
  - `ConnectionClosed` - Connection terminated
  - `ConnectionTestFailed` - Health check failure
  - `ConnectionInvalidated` - Hard invalidation (connection unusable)
  - `ConnectionSoftInvalidated` - Soft invalidation (can complete current operation)
  - `ConnectionReset` - Connection reset
- **Event listeners**: Subscribe to pool events via `PoolEventListener` trait
- **Async event handling**: Non-blocking event emission
- **Built-in logger**: `EventLogger` for simple event logging
- **Timestamped events**: All events include UTC timestamps
- **Serializable events**: Events support serde serialization

#### Connection Management

- **Connection invalidation**:
  - Hard invalidation via `invalidate()` - connection immediately unusable
  - Soft invalidation via `soft_invalidate()` - can complete current operation
- **Connection reset**: Reset connection state via `reset()`
- **Connection ID tracking**: Unique UUID for each pooled connection
- **Automatic cleanup**: Connections automatically returned on drop with event emission

#### Pool Management

- **Multi-pool management**: `PoolManager` for managing multiple named pools
  - `add_pool()` - Register a named pool
  - `get_pool()` - Retrieve pool by name with type safety
  - `remove_pool()` - Unregister a pool
- **Type-safe pool storage**: Generic pool storage with downcasting
- **Shared configuration**: Common config across managed pools

#### Dependency Injection Support

- **Database service wrapper**: `DatabaseService` for DI frameworks
- **Database URL type**: `DatabaseUrl` wrapper for type-safe URLs
- **Pool type placeholders**: `MySqlPool`, `PostgresPool`, `SqlitePool` types
- **Manager types**: Dedicated manager types for each database backend

#### Error Handling

- **Comprehensive error types**: Detailed error variants
  - `PoolClosed` - Pool has been closed
  - `Timeout` - Operation timeout
  - `PoolExhausted` - Max connections reached
  - `InvalidConnection` - Connection validation failed
  - `Database` - sqlx database errors
  - `Config` - Configuration validation errors
  - `Connection` - Connection-specific errors
  - `PoolNotFound` - Named pool not found
- **Type-safe results**: `PoolResult<T>` type alias
- **Error propagation**: Automatic conversion from sqlx errors

## License

Licensed under the BSD 3-Clause License.
