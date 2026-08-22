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
  - Typed `get_or_create` and `update_or_create` builders
  - Field types (AutoField, CharField, IntegerField, DateTimeField, etc.)
  - Opt-in storage-backed `FileField` values with named aliases and lazy
    `open`, `size`, and URL access
  - Timestamped and SoftDeletable traits
  - Relationship management
  - Validators and choices
  - Django-compatible model fixture dump/load support

- **Migrations**: Schema migration system
  - Automatic migration generation from model changes
  - Forward and backward migrations
  - Schema versioning and dependency management
  - Migration operations (CreateModel, AddField, AlterField, etc.)
  - State management and autodetection
  - CockroachDB concurrent migrator serialization with a sentinel-row lock
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
  - Typed relation traversal for compile-time checked related filters and eager loading
  - Lazy query evaluation
  - Only/Defer field optimization for reduced data transfer
  - Aggregate pushdown optimization

### Streaming QuerySets

`QuerySet::iterator_with_db` and `QuerySet::iterator_with_executor` decode one
model at a time from a lifetime-bound driver stream. The stream borrows the
caller-owned executor, returns `Result<Model>` for every item, and releases its
driver resources when it completes, fails, is cancelled, or is dropped early.

```rust
use futures::StreamExt;
use reinhardt_db::orm::{Model, OrmExecutor, QuerySet};
use serde::de::DeserializeOwned;

async fn stream_models<M, E>(connection: &mut E) -> reinhardt_core::exception::Result<()>
where
    M: Model + DeserializeOwned,
    E: OrmExecutor,
{
	let mut models = QuerySet::<M>::new().iterator_with_db(connection, 128)?;
	while let Some(model) = models.next().await {
		let model = model?;
		// Process the typed model without a QuerySet-level result cache.
		let _ = model;
	}
	Ok(())
}
```

PostgreSQL, MySQL, and SQLite support driver-backed streaming. Custom executors
must implement the row-stream capability or the iterator returns an explicit
`Unsupported` database error. `chunk_size` is a driver fetch or bounded-buffer
hint rather than a promise about server internals. Unlike Django's compatibility
fallbacks, Reinhardt intentionally rejects eager `fetch_all` materialization and
repeated `LIMIT`/`OFFSET` pagination for this API.

- **Enhanced Transaction Management**
  - Nested transactions with savepoint support
  - Isolation level control (ReadUncommitted, ReadCommitted, RepeatableRead, Serializable)
  - Named savepoints (create, release, rollback to savepoint)
  - Transaction state tracking (NotStarted, Active, Committed, RolledBack)
  - Two-phase commit (2PC) for distributed transactions
  - Closure-scoped atomic transactions with nested savepoints
  - Mutable executor ownership for transaction-scoped ORM work
  - Typed callback errors with automatic conversion from framework failures
  - Typed, transaction-safe `QuerySet::select_for_update` row locking

- **Structured Database Errors**
  - `DatabaseErrorKind` provides portable connection, constraint, transaction, serialization, and query categories
  - `Error::database_kind()` supports category matching without driver-specific downcasts
  - `DatabaseError::code()` preserves an optional vendor code for diagnostics

### Structured Error Handling

Construct framework database failures with a portable category and inspect that
category at application boundaries:

```rust
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};

let error = Error::from(DatabaseError::new(
    DatabaseErrorKind::UniqueViolation,
    "email already exists",
).with_code("23505"));

assert_eq!(error.database_kind(), Some(DatabaseErrorKind::UniqueViolation));
assert_eq!(error.database_error().and_then(DatabaseError::code), Some("23505"));
```

Transaction callbacks may return an application-owned error. The error must
implement `From<reinhardt_core::exception::Error>` so begin, commit, and rollback
failures retain the same typed channel as domain failures:

```rust,no_run
use reinhardt_core::exception::Error;
use reinhardt_db::{
    backends::DatabaseConnection as BackendsConnection,
    orm::DatabaseConnectionLease,
};

#[derive(Debug, thiserror::Error)]
enum ApplicationError {
    #[error("operation rejected")]
    Rejected,
    #[error(transparent)]
    Framework(#[from] Error),
}

# async fn example() -> Result<(), ApplicationError> {
let owner = BackendsConnection::connect_sqlite("sqlite::memory:").await?;
let lease = DatabaseConnectionLease::register(owner)?;
let connection = lease.handle();
let result: Result<(), ApplicationError> = connection.atomic(async |_transaction| {
    Err(ApplicationError::Rejected)
}).await;

result
# }
```

### Transaction-safe row locking

Build row locks after configuring a `QuerySet`, then evaluate them with
`SelectForUpdate::all_with_executor` or `rows_with_executor` inside
`DatabaseConnection::atomic`. The executor remains owned by the caller, so the
lock stays on the same physical connection until commit or rollback. Ordinary
`all`, `all_with_db`, and `rows_with_db` evaluation returns a transaction error
without executing SQL.

```rust,no_run
use reinhardt_db::{
    backends::error::DatabaseError,
    orm::{QuerySet, TransactionExecutor},
};
use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

#[model(app_label = "jobs", table_name = "jobs")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Job {
    #[field(primary_key = true)]
    id: i64,
    state: String,
}

async fn claim_queued(
    transaction: &mut dyn TransactionExecutor,
) -> Result<Vec<Job>, DatabaseError> {
    QuerySet::<Job>::new()
        .filter(Job::field_state().eq("queued".to_owned()))
        .select_for_update()
        .skip_locked()
        .of_model()
        .all_with_executor(transaction)
        .await
}
```

`nowait` and `skip_locked` are mutually exclusive type states. `of_model`
targets the root model, while `of_relation` accepts only a typed relation path
rooted at that model and automatically adds the required joins. Reinhardt is
intentionally stricter than Django: unchecked table-name targets are not
accepted, and SQLite returns an unsupported-capability error instead of silently
dropping the lock.

PostgreSQL supports target lists and `no_key`; `NO KEY UPDATE` requires
PostgreSQL 9.3 or newer and `SKIP LOCKED` requires 9.5 or newer. The built-in
MySQL capability profile requires MySQL 8.0.1 or newer and does not support
`no_key`. The built-in CockroachDB v23.1 profile supports `FOR UPDATE` and
`NOWAIT`, but not `SKIP LOCKED`, PostgreSQL-distinct `no_key`, or explicit
target lists. Custom transaction executors connected to servers with different
capabilities must override `TransactionExecutor::row_lock_capabilities`.

To preserve the statement's lock scope, row locking rejects CTE-backed
querysets, derived `FROM` sources, LATERAL joins, raw aggregate projections
passed to `values`, and aggregate annotations. Use a direct non-aggregate
queryset for locking reads.

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

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt-db = "0.4.0-alpha.9"
chrono-tz = "0.10"
```

### Optional Features

Enable specific features based on your needs:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt-db = { version = "0.4.0-alpha.9", features = ["postgres", "orm", "migrations"] }
```

Available features:

- `backends` (default): Backend implementations
- `pool` (default): Connection pooling
- `postgres` (default): PostgreSQL support
- `pgvector`: Native PostgreSQL dense-vector ORM and migrations
- `orm` (default): ORM functionality
- `migrations` (default): Migration system
- `hybrid` (default): Multi-database support
- `associations` (default): Relationship management
- `sqlite`: SQLite support
- `mysql`: MySQL support
- `nosql`: NoSQL database support (MongoDB)
- `di`: DI integration for `DatabaseConnection`
- `contenttypes`: Generic relations support
- `all-databases`: All database backends

### Storage-backed `FileField` and `ImageField`

Enable `file-storage` for the typed model value and compile one storage
provider explicitly. For a local-only application, keep provider selection
narrow:

```toml
[dependencies]
reinhardt-db = { version = "0.4.0-alpha.6", default-features = false, features = ["file-storage", "sqlite"] }
reinhardt-storages = { version = "0.4.0-alpha.6", default-features = false, features = ["local"] }
```

The root facade provides equivalent one-provider features. Use
`reinhardt-web` with `file-storage-local`, `file-storage-s3`,
`file-storage-gcs`, or `file-storage-azure`; these are opt-in and are not part
of `standard` or `full`. Do not enable the storage crate's `all` feature for a
normal application.

Configure the preserved default alias and any named aliases in the composed
`[storage]` settings fragment. Every alias has an independent URL expiry
(3,600 seconds by default):

```toml
[storage]
backend = "local"
url_expiry_secs = 3600

[storage.local]
base_path = "media"

[storage.named.private_uploads]
backend = "local"
url_expiry_secs = 900

[storage.named.private_uploads.local]
base_path = "private-media"
```

`[storage]` is the `default` alias. A named alias cannot contain another named
map. Before using a model, initialize the facade with these settings and hold
the returned activation guard for the application lifetime. Initialization
checks that every `FileField` alias exists and that its backend supports atomic
exclusive creation; otherwise it fails before activation.

Declare the model field with an upload directory template and, when needed, a
named alias:

```rust
use reinhardt::model;
use reinhardt::db::orm::{FileField, Model};
use serde::{Deserialize, Serialize};

#[model(app_label = "profiles", table_name = "profiles")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Profile {
    #[field(primary_key = true)]
    id: Option<i64>,
    #[field(
        upload_to = "avatars/%Y/%m/%d",
        file_storage = "private_uploads",
        max_length = 255
    )]
    avatar: FileField,
}
```

The generated descriptor is explicit about the store and returns a typed
value. The complete upload and lazy-access flow is:

```rust,ignore
let avatar = Profile::file_avatar().store(upload).await?;
let mut profile = Profile::build().avatar(avatar).finish();
profile.save().await?;

let bytes = profile.avatar.open().await?;
let size = profile.avatar.size().await?;
let url = profile.avatar.url().await?;
```

Only the logical path is stored in the database. Hydration reads the field's
`file_storage` metadata and restores the alias on the typed value; provider
prefixes and object keys are never inferred from a row. `url()` uses the
alias's configured expiry, while `url_with_expiry` accepts an explicit
duration.

The lower-level `store` method remains an eager one-file operation. For a
model mutation, use the lifecycle methods so storage writes and one
caller-owned database closure are coordinated:

```rust,no_run
use reinhardt_core::parsers::UploadedFile;
use reinhardt_db::orm::{FileField, FileMutationError};
use std::convert::Infallible;

async fn replace_avatar(
    current: FileField,
    upload: UploadedFile,
) -> Result<(), FileMutationError<Infallible>> {
    Profile::file_avatar()
        .replace_with(current, upload, |_stored| async {
            // Return only after the caller-owned transaction has committed.
            Ok::<_, Infallible>(())
        })
        .await?;
    Ok(())
}
```

`create_with` and `replace_with` pass the newly stored `FileField` value to the
caller-owned persistence closure. `clear_with` and `delete_with` use a
no-argument persistence closure because no new value is staged. All four
methods share the same commit and cleanup contract: the closure must return
`Ok` only after the caller-owned transaction has committed. When a new file is
staged, a storage or validation failure compensates newly stored files in
reverse order. After a committed result, old-file deletion is best effort:
cleanup errors are logged and do not replace the database result or prevent
later cleanup entries.
Old committed-file cleanup is disabled by default but never suppresses
compensation for a new write. Set `cleanup = true` only when the field has
exclusive ownership of its storage objects. The descriptor also avoids deleting
an object when the old and new storage alias and logical path are identical.

`ImageField` uses the same lifecycle and stores the original bytes unchanged.
It requires a supported filename extension whose format matches the decoded
raster image, rejects corrupt, unknown, and SVG uploads, and applies inclusive
`max_width` and `max_height` limits. Request `Content-Type` is not trusted for
image validation, and no image transformation or re-encoding is performed.
Enable both `file-storage` and `image-fields` for the model-facing image API.
Multipart decoding belongs to `reinhardt-pages`; forms and admin integration
remain separate APIs.

#### Migrating the legacy descriptors

The former synchronous descriptors moved to
`orm::legacy_file_fields::LegacyFileField` and `LegacyImageField`, with
`LegacyFileFieldError`; the explicit `Legacy*` top-level names are deprecated
compatibility exports. The unprefixed `orm::FileField` is now the typed model
value. The unprefixed `orm::ImageField` is the storage-backed image value when
both `file-storage` and `image-fields` are enabled.

Changing `file_storage` for rows that already exist changes the backend alias,
not the object location. Perform an object/data migration that copies (and,
after verification, removes) the objects, or keep the old alias and redirect it
to the new backend. A settings-only alias change leaves existing logical paths
pointing at the wrong store.

### Native pgvector

Enable native dense-vector storage directly on `reinhardt-db`:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt-db = { version = "0.4.0-alpha.9", features = ["pgvector"] }
reinhardt-core = { version = "0.4.0-alpha.2", features = ["macros"] }
serde = { version = "1", features = ["derive"] }
```

Applications using the facade enable `db-pgvector` instead and import
`Vector` and `VectorError` from `reinhardt::db::pgvector`:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt = { package = "reinhardt-web", version = "0.4.0-alpha.9", features = ["db-pgvector"] }
```

Reinhardt never installs the PostgreSQL extension automatically. Add
`CreateExtension::new("vector")` explicitly before the generated vector model
operations in the migration sequence.

The following model declares both supported approximate index methods and
builds a typed query whose target vectors are bound values:

```rust
use reinhardt_core::macros::model;
use reinhardt_db::{
    migrations::{
        MigrationAutodetector, Operation, ProjectState, model_registry::global_registry,
        operations::postgres::CreateExtension,
    },
    orm::{Model, QuerySet, Vector},
};
use serde::{Deserialize, Serialize};

#[model(app_label = "search", table_name = "documents")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Document {
    #[field(primary_key = true)]
    id: Option<i64>,
    #[field(index(
        name = "documents_embedding_cosine_hnsw",
        method = "hnsw",
        opclass = "vector_cosine_ops",
        m = 16,
        ef_construction = 64
    ))]
    embedding: Vector<1536>,
    #[field(index(
        name = "documents_summary_l2_ivfflat",
        method = "ivfflat",
        opclass = "vector_l2_ops",
        lists = 100
    ))]
    summary: Vector<1536>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metadata = global_registry()
        .get_model("search", "Document")
        .ok_or("Document metadata was not registered")?;
    let mut target_state = ProjectState::new();
    target_state.add_model(metadata.to_model_state());
    let mut generated = MigrationAutodetector::new(ProjectState::new(), target_state)
        .try_generate_migrations()?;
    let mut migration = generated.pop().ok_or("Document migration was not generated")?;
    migration.operations.insert(
        0,
        CreateExtension::new("vector").into_operation()?,
    );
    assert!(matches!(
        migration.operations.first(),
        Some(Operation::CreateExtension { name, .. }) if name == "vector"
    ));
    assert!(migration.operations[1..]
        .iter()
        .any(|operation| matches!(operation, Operation::CreateTable { .. })));
    assert!(migration.operations[1..]
        .iter()
        .any(|operation| matches!(operation, Operation::CreateNamedIndex { .. })));

    let target = Vector::<1536>::try_from(vec![1.0; 1536])?;
    let fields = Document::new_fields();
    let nearest = QuerySet::<Document>::new()
        .filter(
            fields
                .embedding
                .clone()
                .cosine_distance(target.clone())
                .lt(0.5),
        )
        .order_by(
            fields
                .embedding
                .clone()
                .l2_distance(target.clone())
                .asc(),
        )
        .annotate_expr(
            "negative_inner_product",
            fields
                .embedding
                .clone()
                .negative_inner_product(target.clone()),
        )
        .values(&["id"])
        .select_expr(
            "cosine_distance",
            fields.embedding.cosine_distance(target),
        )
        .limit(10);

    let _ = nearest;
    Ok(())
}
```

`DatabaseMigrationExecutor` applies these operations in vector order. Rolling
this migration back removes the model schema and indexes but deliberately
leaves the database-level extension installed, because other applications or
schemas may share it.

The typed distance methods map directly to PostgreSQL:

| Method | PostgreSQL operator |
|--------|---------------------|
| `l2_distance` | `<->` |
| `negative_inner_product` | `<#>` |
| `cosine_distance` | `<=>` |

`Vector<N>` accepts dimensions from 1 through 2000. Construction,
deserialization, pgvector conversion, and database decoding require exactly
`N` finite `f32` elements. This feature supports only dense `vector(N)`;
`halfvec`, `bit`, `sparsevec`, binary quantization, and session tuning APIs are
not included. An all-zero vector passes Reinhardt's finite-value validation,
but PostgreSQL pgvector does not index zero vectors for cosine distance.

Vector columns, values, distance expressions, and approximate indexes are
PostgreSQL-only. Checked construction for MySQL and SQLite returns structured
unsupported-backend errors rather than emitting incompatible SQL. HNSW and
IVFFlat indexes are non-unique and accept exactly one column or expression.
Their operator class must be `vector_l2_ops`, `vector_ip_ops`, or
`vector_cosine_ops`. Optional HNSW `m` and `ef_construction` values and the
optional IVFFlat `lists` value must be positive. Explicit names are emitted
unchanged, and duplicate physical index names are rejected before SQL
execution.

If PostgreSQL reports a missing vector type, distance operator, or vector
operator class for an operation that structurally uses pgvector, Reinhardt
preserves the database error kind, SQLSTATE, and original SQLx source while
adding a hint to install the extension explicitly with
`CreateExtension::new("vector")`.

The Rust `pgvector` dependency is optional, has its default features disabled,
and does not enable its SQLx feature. Reinhardt owns the native binary codec
against the workspace SQLx 0.8 dependency, so enabling pgvector does not
introduce a second SQLx API surface. The feature is intentionally absent from
the default, `full`, and `all-databases` feature groups.

## Usage

### Define Models

`app_label` is required and identifies the application used by migrations and
the model registry. `table_name` may be omitted to derive a singular snake_case
name from the app label and struct (`HTTPRoute` in `network` becomes
`network_http_route`). The examples below keep
explicit plural table names because they represent an existing schema.

```rust
use reinhardt_db::prelude::*;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, NaiveDate, Utc};

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

    /// Calendar date when the account was opened
    pub signup_date: NaiveDate,

    /// Account creation timestamp (auto-populated on insert)
    #[field(auto_now_add = true)]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp (auto-updated on save)
    #[field(auto_now = true)]
    pub updated_at: DateTime<Utc>,
}
```

### Native Model Enum Fields

Use `ModelEnum` when a column has a finite set of domain values. Choose the
physical representation once and give every variant an explicit database
value:

```rust
use reinhardt::ModelEnum;
use reinhardt::core::serde::{Deserialize, Serialize};
use reinhardt::prelude::*;

#[derive(ModelEnum, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[model_enum(repr = "string")]
enum Status {
	#[model_enum(value = "queued")]
	Queued,
	#[model_enum(value = "in_progress")]
	Running,
}

#[derive(ModelEnum, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[model_enum(repr = "i32")]
enum Priority {
	#[model_enum(value = 10)]
	Low,
	#[model_enum(value = 20)]
	High,
}

#[model(app_label = "jobs", table_name = "jobs")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Job {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 32)]
	status: Status,
	priority: Priority,
	#[field(max_length = 32, null = true)]
	fallback_status: Option<Status>,
}
```

String enums use a character column, `i32` enums use an integer column, and
generated migrations add named check constraints for the declared values.
Nullable enum fields accept `None`; `Some(value)` uses the enum's normal codec.

Field references require enum values for filters and partial updates:

```rust,ignore
let jobs = Job::objects()
	.filter(Job::field_status().eq(Status::Queued))
	.filter(Job::field_priority().is_in([Priority::Low, Priority::High]))
	.all()
	.await?;

Job::objects()
	.filter(Job::field_id().eq(job_id))
	.update_fields([
		Job::field_status().assign(Status::Running),
		Job::field_fallback_status().assign(Some(Status::Queued)),
	])
	.await?;
```

Rust variant names, serde names, and database values are independent
contracts. Renaming `Running`, applying `#[serde(rename = "RUNNING")]`, or
changing `#[model_enum(value = "in_progress")]` affects a different boundary.
Unknown stored values fail hydration with field context, and passing a raw
string such as `.eq("queued")` to an enum field is a compile error.

**Field Attributes:**
- `#[field(primary_key = true)]` - Primary key
- `#[field(max_length = N)]` - Maximum length for strings
- `#[field(unique = true)]` - Unique constraint
- `#[field(auto_now_add = true)]` - Auto-populate on creation
- `#[field(auto_now = true)]` - Auto-update on save
- `#[field(null = true)]` - Allow NULL values
- `#[field(default = value)]` - Default value
- `#[field(db_column = "...")]` - Physical database column name
- `#[field(foreign_key = "ModelType")]` - Foreign key relationship
- `#[field(generated = SchemaExpr::..., generated_stored = true)]` - Typed generated column expression
- `#[field(generated_sql = "...", generated_stored = true)]` - Backend-specific raw SQL generated column expression

ORM executor writes alias physical `db_column` names back to their Rust field
names in `RETURNING` and MySQL reload projections. This lets creates and updates
hydrate renamed scalar and JSON fields without ambiguous in-memory key remapping.

Typed JSON fields use `Json<T>` to keep the Rust field type explicit while
storing JSON in the database. Migrations emit JSONB for PostgreSQL/CockroachDB,
JSON for MySQL, and TEXT for SQLite. Scalar wrappers such as `Json<String>` and
`Json<bool>` are still stored and hydrated as JSON values. Manager, QuerySet,
relationship accessor, and session operations preserve the typed value during
writes and hydration. For nullable fields, `None` maps to SQL `NULL`, while
`Some(Json::new(serde_json::Value::Null))` maps to a present JSON `null` value.

Vector model fields use native PostgreSQL arrays for `String`, `i32`, `i64`,
`bool`, `f32`, `f64`, and `Uuid` elements. The manager, session, and bulk-update
paths preserve those array types on PostgreSQL; MySQL and SQLite serialize the
same vectors as JSON text. Session hydration also converts date, time, and
timestamp columns to their typed chrono values on every supported backend.

```rust
use reinhardt_db::Json;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct StyleSettings {
    pub indent_width: u8,
}

#[derive(Serialize, Deserialize)]
#[model(app_label = "myapp", table_name = "projects")]
pub struct Project {
    #[field(primary_key = true)]
    pub id: i64,

    #[field]
    pub style_settings: Json<StyleSettings>,

    #[field(null = true)]
    pub metadata: Option<Json<serde_json::Value>>,
}
```

For a complete list of field attributes, see the `#[field(...)]` macro documentation in `reinhardt-db-macros`.

Generated columns should use `reinhardt_db::migrations::SchemaExpr` when the
expression can be represented with the portable DDL-safe subset:

The typed `generated` form accepts `SchemaExpr::col`, `SchemaExpr::val`,
`SchemaExpr::concat`, and `SchemaExpr::coalesce`, plus chained `binary` and
`cast` calls. Use `generated_sql` for backend-specific functions or expression
forms that cannot be reconstructed from migration files.

```rust
use reinhardt_db::migrations::SchemaExpr;

#[field(
    max_length = 201,
    generated = SchemaExpr::concat([
        SchemaExpr::col("first_name"),
        SchemaExpr::val(" "),
        SchemaExpr::col("last_name"),
    ]),
    generated_stored = true
)]
pub full_name: String,
```

**Note**: The `#[model(...)]` attribute macro automatically generates:
- `Model` trait implementation
- Type-safe field accessors (`User::field_username()`, `User::field_email()`, etc.)
- Global model registry registration
- Model fixture handler registration for `dumpdata` and `loaddata`
- Django-compatible fixture upserts with explicit null, foreign key, many-to-many,
  binary base64, and PostgreSQL sequence handling
- Custom many-to-many through table and column names round-trip as fixture arrays;
  registered through models with additional fields remain explicit-through records
- Nullable JSON fixture values preserve SQL `NULL` versus JSON `null` using the
  stable `_reinhardt_json_null_fields` sidecar emitted by `dumpdata`
- Fixture field names derived from model metadata, independent of API-facing serde renames
  and omission rules
- Single-column fixture primary keys may be supplied through either the top-level
  `pk` member or the corresponding field entry; matching values are required when both are set
- Writable fixture validation that omits database-generated columns while preserving
  required-field and Rust-type checks
- Nullable foreign-key fixture fields may be omitted; supplied values remain limited to
  scalar identifiers or `null`
- Support for composite primary keys

### Query with QuerySet

```rust
use chrono::Utc;
use futures::StreamExt;
use reinhardt_db::orm::{
    DateProjectionOrder, DateTimeTruncKind, DateTruncKind, Model,
};

// Get all users
let users = User::objects().all().await?;

// Filter users
let adults = User::objects()
    .filter(User::field_age().gte(18))
    .order_by("-created_at")
    .all()
    .await?;

// Get a single user
let user = User::objects()
    .filter(User::field_username().exact("john"))
    .first()
    .await?;

// Django-style lookup helpers on generated field accessors
let matching = User::objects()
    .filter(User::field_email().icontains("example.com"))
    .filter(User::field_id().is_in([1_i64, 2, 3]))
    .filter(User::field_deleted_at().is_null())
    .all()
    .await?;

let recent = User::objects()
    .filter(User::field_created_at().year().gte(2026))
    .all()
    .await?;

// Distinct database-side temporal projections
let signup_months = User::objects()
    .dates(
        User::field_signup_date(),
        DateTruncKind::Month,
        DateProjectionOrder::Asc,
    )
    .await?;
let mut connection = reinhardt_db::orm::manager::get_connection().await?;
let signup_days = User::objects()
    .dates_with_db(
        &mut connection,
        User::field_signup_date(),
        DateTruncKind::Day,
        DateProjectionOrder::Asc,
    )
    .await?;
let local_hours = User::objects()
    .datetimes(
        User::field_created_at(),
        DateTimeTruncKind::Hour,
        DateProjectionOrder::Desc,
        Some(chrono_tz::Asia::Tokyo),
    )
    .await?;

// Stream typed-field results through the caller-owned executor.
let mut streamed_adults = User::objects()
    .filter(User::field_age().gte(18))
    .iterator_with_db(&mut connection, 128)?;
while let Some(user) = streamed_adults.next().await {
    let _user = user?;
}

// Atomic conditional partial update
let updated = User::objects()
    .filter(User::field_id().eq(user_id))
    .filter(User::field_age().gte(18))
    .update_fields([User::field_updated_at().assign(Utc::now())])
    .await?;
```

### Typed Aggregates and Annotations

The standard typed vocabulary is the [`orm::func`](src/orm/func.rs) module.
Generated fields and relation paths carry the operand and result types through
`count`, `sum`, `avg`, `min`, and `max`; no string field names are needed.
Labels are validated identifiers, so `.label(...)` is fallible. A terminal
`aggregate` executes asynchronously and returns an [`AggregateResult`]; it is
not a row-loading operation.

```rust,no_run
use reinhardt_db::orm::{AggregateValue, QuerySet, func};

let filtered = User::objects()
    .all()
    .filter(User::field_is_active().exact(true));
let count = func::count_all::<User>().label("user_count")?;
let total_age = func::sum(User::field_age()).label("age_total")?;
let summary = filtered.aggregate([count, total_age]).await?;
assert!(matches!(summary.get("user_count")?, AggregateValue::Integer(_)));

// Annotation is a fallible, chainable builder. `all()` deserializes User and
// intentionally ignores computed annotation columns.
let annotated = filtered
    .annotate(User::field_email().into_expression().label("email_copy")?)?;
let users = annotated.all().await?;
```

For a multi-valued relation, `func::count(path)` retains duplicate joined rows.
Apply `distinct()` to the operand when the count should contain each related
value once:

```rust,no_run
let related_rows = User::objects()
    .all()
    .aggregate(func::count(User::rel_posts()).label("post_rows")?)
    .await?;
let related_values = User::objects()
    .all()
    .aggregate(
        func::count(User::rel_posts().field_id())
            .distinct()
            .label("unique_posts")?,
    )
    .await?;
```

`reinhardt-query` remains the dynamic SQL-builder boundary. Use it for raw
expressions or backend-neutral statement construction, then pass the resulting
statement to a low-level executor; it is not a replacement for the typed ORM
aggregate vocabulary. PostgreSQL-only projections such as `ArrayAgg` and
`JsonbAgg` stay behind `BackendAnnotation` and `QuerySet::annotate_backend`.
Explicit raw scalar subqueries likewise use `QuerySet::annotate_subquery` and
remain a separate, fallible boundary rather than being coerced into typed
portable aggregates.

### Typed Manager Upserts

Generated field accessors provide compile-time checked model and value types
for atomic get/create and update/create operations:

```rust,ignore
let (tag, created) = Tag::objects()
    .get_or_create()
    .lookup(Tag::field_slug(), "rust")
    .default(Tag::field_display_order(), 10_i32)
    .execute()
    .await?;
```

```rust,ignore
let (profile, created) = Profile::objects()
    .update_or_create()
    .lookup(Profile::field_user_id(), user.id)
    .set(Profile::field_last_seen(), now)
    .create_default(Profile::field_created_at(), now)
    .execute()
    .await?;
```

Lookups must cover a primary key, a `unique = true` field, or an immediate,
unconditional unique constraint. Lookup fields cannot also be defaults or
updates. `get_or_create().execute_with(...)` accepts a `DatabaseConnection` or
an `AtomicTransaction` created by `DatabaseConnection::atomic_write`;
`update_or_create().execute_with(...)` requires an
`AtomicTransaction` created by `DatabaseConnection::atomic_write`.

The returned `created` flag is true only when this invocation inserted the row.
A losing get/create race reloads the winner with `false`; a losing
update/create race locks and updates the winner before returning `false`.

See the
[typed manager upsert migration guide](../../docs/migration/0.4.0-typed-manager-upserts.md)
for map-API replacement, backend behavior, and custom-manager hook guidance.

### Plan-only QuerySet diagnostics

Use typed generated fields to build the queryset, then call `explain` with
`ExplainOptions`. The returned `ExplainOutput` records the backend, effective
format, and a separately decoded plan body; it never deserializes diagnostic
rows as models.

```rust
use reinhardt_db::orm::{ExplainFormat, ExplainOptions};

let plan = User::objects()
    .filter(User::field_email().eq("ada@example.com"))
    .order_by(User::field_created_at().desc())
    .explain(ExplainOptions::default().format(ExplainFormat::Json))
    .await?;
```

When the equivalent query must stay on a caller-owned connection, use
`explain_with_db`. Active transactions can use `explain_with_executor`.

```rust
let plan = connection.atomic(async |transaction| {
    User::objects()
        .filter(User::field_id().eq(user_id))
        .explain_with_executor(transaction, ExplainOptions::default())
        .await
        .map_err(reinhardt_core::exception::Error::from)
}).await?;
```

Backend capabilities are explicit:

| Backend | Formats | Additional plan-only options |
|---------|---------|------------------------------|
| PostgreSQL | `Text`, `Json`, `Xml`, `Yaml` | `verbose`, `costs`, `settings` |
| MySQL/MariaDB | `Text` (traditional), `Json` | none |
| SQLite | `Text` (`EXPLAIN QUERY PLAN`) | none |
| CockroachDB | `Text` | none |

Unsupported combinations return a database error classified as `Unsupported`
before the executor is called. Reinhardt intentionally exposes a stricter API
than Django: `ANALYZE`, arbitrary option strings, buffer/timing statistics, and
every other data-executing explain option are rejected by construction.
MySQL additionally rejects subqueries, CTEs, unions, and unchecked or function
expressions because its optimizer may evaluate them while producing a plan;
plain typed filters, joins, ordering, and limits remain supported. SQLite plan
row fields are diagnostic data whose exact shape may change between SQLite
releases.

### Typed retrieval helpers

Models can define their default latest/earliest ordering with generated field
metadata:

```rust
use chrono::{DateTime, Utc};
use reinhardt_db::model;
use reinhardt_db::orm::Model;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(
    app_label = "events",
    table_name = "events",
    get_latest_by = ("created_at", "id")
)]
struct Event {
    #[field(primary_key = true)]
    id: i64,
    created_at: DateTime<Utc>,
    #[field(max_length = 255, unique = true)]
    slug: String,
}

let latest = Event::objects().all().latest().await?;
let earliest = Event::objects()
    .all()
    .earliest_by(&[
		Event::ordering_created_at(),
		Event::ordering_id(),
    ])
    .await?;

let by_id = Event::objects().all().in_bulk([3_i64, 1, 3]).await?;
let by_slug = Event::objects()
    .all()
    .in_bulk_by(
        Event::unique_slug(),
        ["launch".to_string(), "archive".to_string()],
    )
    .await?;

let empty = Event::objects().all().none();
assert_eq!(empty.count().await?, 0);
```

Unlike Django's string field names, explicit ordering and unique-field bulk
lookups accept only generated typed field proofs for the same model. Bulk
retrieval returns a `BTreeMap`, so iteration is sorted by key; duplicate input
keys collapse and missing keys are omitted. Empty input and `none()` are lazy:
they do not resolve a connection or invoke an executor. The equivalent
`*_with_db` and `*_with_executor` methods keep retrieval bound to a
caller-owned connection or transaction executor. These contracts are
backend-neutral across PostgreSQL, MySQL, and SQLite.

Bulk retrieval accounts for bind parameters already used by the source
queryset when splitting lookup keys into backend-safe batches. It returns a
validation error when the source query has exhausted the backend's bind
parameter limit and no lookup key can be added safely.

When a retrieval ordering field is nullable, its relative NULL placement follows
the connected database. Filter nulls explicitly before calling `latest*` or
`earliest*` when that placement is part of the application contract.

`dates` and `datetimes` exclude nulls and perform truncation, distinct
projection, and ordering in the database. ISO weeks begin on Monday.
`datetimes` defaults to UTC and converts to the requested zone before
truncation. PostgreSQL supports IANA named zones. MySQL and SQLite return an
explicit capability error for named zones because Reinhardt cannot guarantee
MySQL time-zone tables or correct SQLite named-zone conversion. This is
intentionally stricter than Django's environment-dependent fallback behavior.
Global ORM time-zone configuration is intentionally outside this API; pass the
zone explicitly when UTC is not the desired projection.
Use `dates_with_db` / `datetimes_with_db` or the corresponding
`*_with_executor` variants to retain a caller-owned connection or transaction.

`AsyncQuery` preserves bind parameters when executing legacy `Q` filters.
Runtime field names and operators are treated as query structure and accept
only supported forms. `Q::from_sql` rejects unrecognized SQL, while
`Q::from_raw_sql` is an explicit raw-SQL boundary that must only receive
trusted SQL.

Existing callers that used `Q::from_sql` for arbitrary trusted fragments must
migrate to `Q::from_raw_sql`. Unsupported operators and unrecognized SQL now
fail closed; runtime values must be expressed with `Q::new` so they remain
bound parameters.

### Scoped N+1 Query Detection

Use `NPlusOneScope` around development diagnostics or focused tests to detect
repeated query shapes with different bind or inline literal values. The
detector is opt-in and is disabled when no scope is active. QuerySet execution
and relationship accessors are recorded by active scopes.

```rust
use reinhardt_db::orm::{NPlusOneConfig, NPlusOneScope};

let (_, report) = NPlusOneScope::warn("admin.post.list", NPlusOneConfig::default())
    .run_with_report(async {
        // Execute ORM work here.
    })
    .await;

assert!(report.findings.is_empty());
```

For tests that should fail on suspicious repeated query shapes, use
`NPlusOneScope::fail(...).run(...)` around the focused code path. Fix reported
patterns by using `select_related()` for single-object relationships and
explicit batch queries for collection relationships. Use
`NPlusOneScope::spawn(...)` for spawned tasks that should inherit the active scope.

### Typed Relation Traversal

Model derives generate typed relation path accessors such as
`Post::rel_author().into_typed().field_email()`. Use these paths in `filter()`,
single-valued `select_related()`, and direct multi-valued
`prefetch_related()` to replace string traversal like `"author__email"` with
compile-time checked relation and field names. Use `select_related()` for
forward foreign keys and one-to-one paths; use `prefetch_related()` for reverse
one-to-many and many-to-many paths.

Each `rel_*` accessor first returns a raw path, which remains usable with a
manually implemented `Model` target. Call `into_typed()` when the target uses
`#[model]` and its generated field or nested relation helpers are needed.

String relation APIs remain available in 0.4.0 for incremental migration. New
code should prefer typed paths. Invalid string relation names fail during
relation-loading builder construction when relationship metadata is available;
manual models without metadata retain legacy naming behavior. Typed related
filters are limited to SELECT queries because write builders do not emit
relation joins, and counts across multi-valued typed filters deduplicate root
primary keys.

### Create Migrations

```rust
use reinhardt_db::migrations::{Migration, CreateModel, AddField};

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
use reinhardt_db::pool::{ConnectionPool, PoolConfig};

// Create a connection pool
let config = PoolConfig {
    max_connections: 10,
    ..PoolConfig::default()
};
let pool = ConnectionPool::new_postgres("postgres://user:pass@localhost/db", config).await?;

// Acquire a connection
let conn = pool.acquire().await?;
```

## Custom Object Managers

Reinhardt supports Django-style customizable object managers via the
`CustomManager` and `HasCustomManager` traits (see `orm::custom_manager`).
Use them when you want to inject default filters, audit hooks, or access
control before queries reach the database — without touching the existing
`Model::objects()` API.

The blanket `impl<M: Model> CustomManager for Manager<M>` ensures every
existing manager already satisfies the trait, so adopting custom managers
is fully opt-in and backward compatible.

```rust,ignore
use reinhardt_db::orm::custom_manager::CustomManager;
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Result};

#[derive(Default)]
struct ActiveUserManager;

impl CustomManager for ActiveUserManager {
    type Model = User;
    fn new() -> Self { Self }

    // Default filter: only return active users by default.
    fn all(&self) -> reinhardt_db::orm::query::QuerySet<User> {
        use reinhardt_db::orm::query::{Filter, FilterOperator, FilterValue};
        reinhardt_db::orm::manager::Manager::<User>::new()
            .all()
            .filter(Filter::new(
                "is_active".to_string(),
                FilterOperator::Eq,
                FilterValue::Boolean(true),
            ))
    }

    // Veto saves with empty usernames.
    fn before_save(&self, user: &mut User) -> Result<()> {
        if user.username.is_empty() {
            return Err(DatabaseError::new(
                DatabaseErrorKind::Query,
                "username must not be empty",
            )
            .into());
        }
        Ok(())
    }
}

#[reinhardt_macros::model(table_name = "users", manager = ActiveUserManager)]
struct User {
    #[field]
    pub id: Option<i64>,
    #[field]
    pub username: String,
    #[field]
    pub is_active: bool,
}

// Use the configured manager:
let active_users = User::custom_manager().all().fetch().await?;

// The original API is unchanged:
let all_users = User::objects().all().fetch().await?;
```

### Available Hooks

| Hook | Trigger | Purpose |
|------|---------|---------|
| `before_save` | `create` / `update` | Validate / mutate model before insert |
| `before_delete` | `delete` | Block destructive operations |
| `before_bulk_update` | `bulk_update` | Validate / rewrite a batch |

Each hook returns `Result<()>`; returning `Err(_)` vetoes the operation.

See `crates/reinhardt-db/src/orm/custom_manager.rs` for the full trait
surface and the related issue at
<https://github.com/kent8192/reinhardt-web/issues/3980>.

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
use reinhardt_db::orm::{Model, QuerySet};
use reinhardt_db::migrations::Migration;
use reinhardt_db::pool::ConnectionPool;
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

# Run with PostgreSQL container (TestContainers automatically starts PostgreSQL)
cargo test --package reinhardt-db --test orm_integration_tests
```

### TestContainers Integration

Database tests automatically use TestContainers to:
- Start PostgreSQL 17 Alpine container before tests
- Provide isolated database instance per test suite
- Clean up containers after tests complete

**Standard Fixtures** from `reinhardt-test` are available:

```rust
use reinhardt_test::fixtures::postgres_container;
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
- [Testing Standards](../../instructions/TESTING_STANDARDS.md)
- [REST Tutorial Database Integration](../../examples/examples-tutorial-rest/README.md)

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
  - PostgreSQL support via sqlx
  - MySQL support via sqlx
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
