//! # Reinhardt ORM
//!
//! Object-Relational Mapping for Reinhardt framework.
//!
//! [`DatabaseConnection`] is a copyable ORM capability. Standalone setup owns a
//! backend connection through [`DatabaseConnectionLease`], and the lease must
//! outlive all operations performed by its handles. Framework handlers receive
//! the handle through `#[inject]` while server bootstrap retains the lease.
//!
//! Typed relation traversal lets `QuerySet` filters and relation loading cross
//! model relations with generated `rel_<name>()` accessors. The typed path
//! records the SQL joins required by the filter, so application code does not
//! need raw join builders for common FK, reverse, or M2M lookups.
//!
//! ## Transaction Management
//!
//! ORM writes run inside closure-scoped transactions. Start an outer operation
//! with [`DatabaseConnection::atomic`], then use the supplied mutable
//! [`AtomicTransaction`] for every `*_with_conn` or `*_with_db` operation.
//! Nested [`AtomicTransaction::atomic`] callbacks use savepoints on the same
//! dedicated connection.
//!
//! ```no_run
//! use reinhardt_core::exception::Error;
//! use reinhardt_db::{backends::DatabaseConnection as BackendsConnection, orm::DatabaseConnectionLease};
//!
//! # async fn example() -> Result<(), Error> {
//! let owner = BackendsConnection::connect_sqlite("sqlite::memory:").await?;
//! let lease = DatabaseConnectionLease::register(owner)?;
//! let connection = lease.handle();
//! let result = connection.atomic(async |transaction| {
//!     transaction.atomic(async |_savepoint| {
//!         Ok::<_, Error>(())
//!     }).await?;
//!     Ok::<_, Error>(42)
//! }).await?;
//! assert_eq!(result, 42);
//! # Ok(())
//! # }
//! ```
//!
//! Callback errors roll back the relevant scope. If rollback or savepoint
//! cleanup fails, that framework error takes precedence. Callback panics are
//! rethrown after best-effort cleanup; cancellation cannot guarantee completion.
//! MySQL DDL may implicitly commit and is outside this atomicity guarantee.
//!
//! [`Transaction`], [`Savepoint`], and [`IsolationLevel`] remain SQL-builder
//! values only. They may generate SQL but cannot control a live ORM transaction.
//! [`AtomicTransaction`] is also intentionally non-`Copy` and stays bound to
//! the callback and dedicated connection created by
//! [`DatabaseConnection::atomic`] or the public write-intent variant
//! [`DatabaseConnection::atomic_write`]. Caller-owned `update_or_create`
//! execution specifically requires the latter.
//!
//! ## Typed Manager Upserts
//!
//! [`CustomManager::get_or_create`] and [`CustomManager::update_or_create`]
//! start builders whose generated [`FieldRef`] values check the model and
//! assignment value types during compilation. A lookup must cover a primary
//! key, a `unique = true` field, or an immediate, unconditional unique
//! constraint. Defaults and updates cannot replace lookup fields.
//!
//! `get_or_create().execute_with(...)` accepts any [`OrmExecutor`].
//! `update_or_create().execute_with(...)` requires an [`AtomicTransaction`]
//! created by [`DatabaseConnection::atomic_write`]; a transaction from
//! [`DatabaseConnection::atomic`] is rejected before SQL execution.

// Core modules - always available
pub mod aggregation;
/// Annotation module.
pub mod annotation;
pub mod bulk_update;
pub mod connection;
pub mod connection_ext; // reinhardt-query connection support
mod connection_registry;
/// Constraints module.
pub mod constraints;
/// Expressions module.
pub mod expressions;
pub mod field_codec;
/// Fields module.
pub mod fields;
/// Fixture loading and dumping module.
pub mod fixtures;
/// Functions module.
pub mod functions;
pub mod hybrid_dml;
/// Indexes module.
pub mod indexes;
pub mod inspection;
/// Into primary key module.
pub mod into_primary_key;
/// Typed JSON field wrapper.
pub mod json;
/// Model module.
pub mod model;
pub mod query_fields;
pub mod query_helpers; // Common query patterns using reinhardt-query
pub mod query_types; // Type definitions for passing reinhardt-query objects
/// Set operations module.
pub mod set_operations;
pub mod sql_condition_parser;
pub mod transaction;
pub mod typed_join;
/// Typed get-or-create and update-or-create support.
pub mod upsert;
/// Validators module.
pub mod validators;
/// Validated pgvector value types.
#[cfg(feature = "pgvector")]
pub mod vector;
/// Window module.
pub mod window;

// New advanced features
pub mod absolute_url_overrides;
pub mod composite_pk;
pub mod composite_synonym;
pub mod cross_db_constraints;
pub mod cte;
/// File fields module.
pub mod file_fields;
pub mod filtered_relation;
pub mod generated_field;
/// Gis module.
pub mod gis;
/// Lambda stmt module.
pub mod lambda_stmt;
/// Lateral join module.
pub mod lateral_join;
pub mod order_with_respect_to;
/// Pool types module.
pub mod pool_types;
pub mod postgres_features;
pub mod postgres_fields;
pub mod two_phase_commit;
/// Type decorator module.
pub mod type_decorator;

// SQLAlchemy-style modules - default
pub mod async_query;
pub mod database_routing;
pub mod declarative;
pub mod engine;
/// Events module.
pub mod events;
pub mod execution;
pub mod fk_accessor;
pub mod instrumentation;
pub mod loading;
pub mod many_to_many;
pub mod many_to_many_accessor;
pub mod n_plus_one;
pub mod polymorphic;
pub mod query_execution;
pub mod query_options;
pub mod reflection;
/// Registry module.
pub mod registry;
pub mod relations;
pub mod relationship;
pub mod reverse_accessor;
pub mod session;
pub mod sqlalchemy_query;
pub mod types;

// Django ORM compatibility layer
/// Manager module.
pub mod manager;

/// Custom object manager support (Issue #3980).
pub mod custom_manager;

// Unified query interface facade
pub mod query;

pub use custom_manager::{CreateWithConnOutcome, CustomManager};
#[cfg(feature = "di")]
pub use engine::register_request_database;
pub use manager::{
	ScopedDatabaseRegistration, get_connection, get_connection_lease, get_connection_registration,
	init_database, init_database_with_pool_size, install_scoped_database, reinitialize_database,
};
#[cfg(feature = "pgvector")]
pub use vector::{MAX_DENSE_VECTOR_DIMENSIONS, Vector, VectorError};

// Re-export paste for macro usage
#[doc(hidden)]
pub use paste;

// Core exports - always available
pub use aggregation::{Aggregate, AggregateFunc, AggregateResult, AggregateValue};
pub use annotation::{Annotation, AnnotationValue, Expression, Value, When};
pub use connection::{
	DatabaseBackend, DatabaseConnection, DatabaseConnectionLease, OrmExecutor, QueryResult,
	QueryRow, QueryValue, Row, TransactionExecutor,
};
pub use constraints::{
	CheckConstraint, Constraint, ForeignKeyConstraint, OnDelete, OnUpdate, UniqueConstraint,
};
pub use expressions::{Exists, F, FieldRef, OuterRef, Q, QOperator, Subquery, UniqueFieldRef};
pub use functions::{
	Abs, Cast, Ceil, Concat, CurrentDate, CurrentTime, Extract, ExtractComponent, Floor, Greatest,
	Least, Length, Lower, Mod, Now, NullIf, Power, Round, SqlType, Sqrt, Substr, Trim, TrimType,
	Upper,
};
pub use indexes::{BTreeIndex, GinIndex, GistIndex, HashIndex, Index};
pub use into_primary_key::IntoPrimaryKey;
pub use json::Json;
pub use model::{
	FieldSelector, FixtureFields, FixtureValue, Model, SoftDeletable, SoftDelete, Timestamped,
	Timestamps,
};
pub use query_fields::{
	Comparable, DateTimeType, Field, GroupByFields, Lookup, LookupType, LookupValue, NumericType,
	OrderedExpression, QueryFieldCompiler, StringType, TypedExpression, TypedPredicate,
};
#[doc(hidden)]
pub use serde;
pub use set_operations::{CombinedQuery, SetOperation, SetOperationBuilder};
pub use transaction::{
	AtomicTransaction, IsolationLevel, Savepoint, Transaction, TransactionState,
};
pub use two_phase_commit::{
	Participant, ParticipantStatus, TransactionState as TwoPhaseTransactionState, TwoPhaseCommit,
	TwoPhaseCoordinator, TwoPhaseError, TwoPhaseParticipant,
};
pub use validators::{
	EmailValidator, FieldValidators, MaxLengthValidator, MinLengthValidator, ModelValidators,
	RangeValidator, RegexValidator, RequiredValidator, URLValidator, ValidationError, Validator,
};
pub use window::{
	DenseRank, FirstValue, Frame, FrameBoundary, FrameType, Lag, LastValue, Lead, NTile, NthValue,
	Rank, RowNumber, Window, WindowFunction,
};

// Two-phase commit adapters (feature-gated)
#[cfg(feature = "postgres")]
pub use two_phase_commit::PostgresParticipantAdapter;

#[cfg(feature = "mysql")]
pub use two_phase_commit::MySqlParticipantAdapter;

// PostgreSQL-specific types
pub use postgres_fields::{
	ArrayField, BigIntegerRangeField, CITextField, DateRangeField, DateTimeRangeField, HStoreField,
	IntegerRangeField, JSONBField,
};

// PostgreSQL-specific advanced features
pub use postgres_features::{
	ArrayAgg, ArrayOverlap, FullTextSearch, JsonbAgg, JsonbBuildObject, StringAgg, TsRank,
};

// File field types
pub use file_fields::{FileField, FileFieldError, ImageField};

pub use database_routing::DatabaseRouter;
pub use events::{
	ActiveRegistryGuard, AttributeEvents, EventListener, EventRegistry, EventResult,
	InstanceEvents, MapperEvents, SessionEvents, get_active_registry, set_active_registry,
	with_event_registry,
};
pub use execution::{ExecutionResult, InsertExecution, QueryExecution, SelectExecution};
pub use field_codec::*;
// Re-export from reinhardt-hybrid
pub use crate::hybrid::{
	Comparator as HybridComparator, HybridMethod, HybridProperty, UpperCaseComparator,
};
pub use loading::{
	LoadContext, LoadOption, LoadOptionBuilder, LoadingStrategy, joinedload, lazyload, noload,
	raiseload, selectinload, subqueryload,
};
pub use polymorphic::{
	InheritanceType, PolymorphicConfig, PolymorphicIdentity, PolymorphicQuery, PolymorphicRegistry,
	PolymorphicRelation, polymorphic_registry,
};
pub use query_options::{
	CompiledCacheOption, ExecutionOptions, ForUpdateMode, IsolationLevel as QueryIsolationLevel,
	QueryOptions, QueryOptionsBuilder,
};
pub use registry::{ColumnInfo, Mapper, MapperRegistry, TableInfo, registry};
pub use relations::{
	GenericRelationConfig, GenericRelationSet, PlannedRelationJoin, RelatedFieldRef,
	RelationDescriptor, RelationJoinGraph, RelationJoinKind, RelationMultiplicity, RelationPath,
	RelationPathLike, RelationStep, RelationTarget,
};
pub use relationship::{CascadeOption, Relationship, RelationshipDirection, RelationshipType};
pub use session::{Session, SessionError};
pub use sqlalchemy_query::{Column as SqlColumn, JoinType, SelectQuery, column, select};
pub use typed_join::TypedJoin;
pub use types::{
	ArrayType, DatabaseDialect, HstoreType, InetType, JsonType, SqlTypeDefinition, SqlValue,
	TypeDecorator, TypeError, TypeRegistry, UuidType,
};

// New features - engine, migrations, many-to-many, async queries
pub use async_query::{AsyncQuery, AsyncSession};
pub use engine::{Engine, EngineConfig, create_engine, create_engine_with_config};
pub use fk_accessor::ForeignKeyAccessor;
pub use many_to_many::{AssociationTable, ManyToMany, association_table};
pub use many_to_many_accessor::ManyToManyAccessor;
pub use n_plus_one::{
	NPlusOneConfig, NPlusOneFinding, NPlusOneMode, NPlusOneReport, NPlusOneScope,
};
pub use query_execution::{ExecutableQuery, QueryCompiler};
pub use reverse_accessor::ReverseAccessor;

// Django ORM compatibility layer
pub use manager::Manager;
// Query types are always available
pub use query::{
	FieldAssignment, Filter, FilterCondition, FilterOperator, FilterValue, IntoOrderBy, OrmQuery,
	QuerySet, UpdateValue,
};

// Advanced ORM features
pub use absolute_url_overrides::{HasAbsoluteUrl, clear_url_overrides, register_url_override};
pub use composite_synonym::{CompositeSynonym, FieldValue, SynonymError};
pub use lambda_stmt::{
	CACHE_STATS, CacheStatistics, LambdaRegistry, LambdaStmt, QUERY_CACHE, QueryCache,
};
pub use order_with_respect_to::{OrderError, OrderValue, OrderedModel};

// reinhardt-query re-exports for query building in client code
pub use reinhardt_query::prelude::{
	Alias, ColumnRef, Cond, Expr, ExprTrait, IntoValue, MySqlQueryBuilder, Order,
	PostgresQueryBuilder, Query, QueryStatementBuilder, QueryStatementWriter, SqliteQueryBuilder,
};

// Re-export reinhardt-query Value as QueryBuilderValue to avoid conflict with
// annotation::Value and types::SqlValue
pub use reinhardt_query::prelude::Value as QueryBuilderValue;
