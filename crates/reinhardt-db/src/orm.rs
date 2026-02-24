//! # Reinhardt ORM
//!
//! Object-Relational Mapping for Reinhardt framework.
//!
//! ## Documentation
//!
//! - [README.md](../README.md) - Feature list and API reference
//! - [USAGE_GUIDE.md](../USAGE_GUIDE.md) - Comprehensive usage guide with examples and best practices
//!
//! ## Transaction Management
//!
//! Reinhardt ORM provides a closure-based API for automatic transaction management:
//!
//! ### Basic Usage
//!
//! ```rust
//! use reinhardt_db::orm::connection::DatabaseConnection;
//! use reinhardt_db::orm::transaction::transaction;
//!
//! # async fn example() -> Result<(), anyhow::Error> {
//! let conn = DatabaseConnection::connect("sqlite::memory:").await?;
//!
//! // Automatic commit on success, rollback on error
//! let user_id = transaction(&conn, |_tx| async move {
//!     // Your database operations here
//!     // let id = insert_user("Alice").await?;
//!     Ok(42)
//! }).await?;
//!
//! assert_eq!(user_id, 42);
//! # Ok(())
//! # }
//! ```
//!
//! ### With Isolation Level
//!
//! ```rust
//! use reinhardt_db::orm::transaction::{transaction_with_isolation, IsolationLevel};
//! # use reinhardt_db::orm::connection::DatabaseConnection;
//!
//! # async fn example() -> Result<(), anyhow::Error> {
//! # let conn = DatabaseConnection::connect("sqlite::memory:").await?;
//! transaction_with_isolation(&conn, IsolationLevel::Serializable, |_tx| async move {
//!     // Critical operations requiring serializable isolation
//!     Ok(())
//! }).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Error Handling
//!
//! ```rust
//! use reinhardt_db::orm::transaction::transaction;
//! # use reinhardt_db::orm::connection::DatabaseConnection;
//!
//! # async fn example() -> Result<(), anyhow::Error> {
//! # let conn = DatabaseConnection::connect("sqlite::memory:").await?;
//! # let some_condition = true;
//! let result = transaction(&conn, |_tx| async move {
//!     // Simulate an error
//!     if some_condition {
//!         return Err(anyhow::anyhow!("Operation failed"));
//!     }
//!     Ok(42)
//! }).await;
//!
//! match result {
//!     Ok(value) => println!("Transaction committed: {}", value),
//!     Err(e) => println!("Transaction rolled back: {}", e),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! **Key Features:**
//! - ✅ **Automatic commit** on successful closure completion
//! - ✅ **Automatic rollback** on error or panic
//! - ✅ **RAII-style cleanup** with Drop implementation
//! - ✅ **Nested transactions** support via savepoints (TransactionScope API)
//! - ✅ **Isolation level** control with `transaction_with_isolation()`
//!
//! See [`transaction`](transaction/index.html) module for detailed documentation.
//!
//! ## Migration System
//!
//! The reinhardt-migrations crate provides comprehensive migration support:
//! - **Migration generation from model changes** (✅ Implemented)
//! - **Migration dependency resolution with DAG** (✅ Implemented)
//! - **Forward and backward migration execution** (✅ Implemented)
//! - **Schema introspection and diffing** (✅ Implemented)
//!
//! See `reinhardt-migrations` crate for detailed documentation.

// Core modules - always available
pub mod aggregation;
pub mod annotation;
pub mod bulk_update;
pub mod connection;
pub mod connection_ext; // reinhardt-query connection support
pub mod constraints;
pub mod expressions;
pub mod fields;
pub mod functions;
pub mod hybrid_dml;
pub mod indexes;
pub mod inspection;
pub mod into_primary_key;
pub mod model;
pub mod query_fields;
pub mod query_helpers; // Common query patterns using reinhardt-query
pub mod query_types; // Type definitions for passing reinhardt-query objects
pub mod set_operations;
pub mod sql_condition_parser;
pub mod transaction;
pub mod typed_join;
pub mod validators;
pub mod window;

// New advanced features
pub mod absolute_url_overrides;
pub mod composite_pk;
pub mod composite_synonym;
pub mod cross_db_constraints;
pub mod cte;
pub mod file_fields;
pub mod filtered_relation;
pub mod generated_field;
pub mod gis;
pub mod lambda_stmt;
pub mod lateral_join;
pub mod order_with_respect_to;
pub mod pool_types;
pub mod postgres_features;
pub mod postgres_fields;
pub mod two_phase_commit;
pub mod type_decorator;

// SQLAlchemy-style modules - default
pub mod async_query;
pub mod database_routing;
pub mod declarative;
pub mod engine;
pub mod events;
pub mod execution;
pub mod fk_accessor;
pub mod instrumentation;
pub mod loading;
pub mod many_to_many;
pub mod many_to_many_accessor;
pub mod polymorphic;
pub mod query_execution;
pub mod query_options;
pub mod reflection;
pub mod registry;
pub mod relations;
pub mod relationship;
pub mod reverse_accessor;
pub mod session;
pub mod sqlalchemy_query;
pub mod types;

// Django ORM compatibility layer
pub mod manager;

// Unified query interface facade
pub mod query;

pub use manager::{
	get_connection, init_database, init_database_with_pool_size, reinitialize_database,
};

// Re-export paste for macro usage
#[doc(hidden)]
pub use paste;

// Core exports - always available
pub use aggregation::{Aggregate, AggregateFunc, AggregateResult, AggregateValue};
pub use annotation::{Annotation, AnnotationValue, Expression, Value, When};
pub use connection::{
	DatabaseBackend, DatabaseConnection, DatabaseExecutor, QueryRow, QueryValue, Row,
	TransactionExecutor,
};
pub use constraints::{
	CheckConstraint, Constraint, ForeignKeyConstraint, OnDelete, OnUpdate, UniqueConstraint,
};
pub use expressions::{Exists, F, FieldRef, OuterRef, Q, QOperator, Subquery};
pub use functions::{
	Abs, Cast, Ceil, Concat, CurrentDate, CurrentTime, Extract, ExtractComponent, Floor, Greatest,
	Least, Length, Lower, Mod, Now, NullIf, Power, Round, SqlType, Sqrt, Substr, Trim, TrimType,
	Upper,
};
pub use indexes::{BTreeIndex, GinIndex, GistIndex, HashIndex, Index};
pub use into_primary_key::IntoPrimaryKey;
pub use model::{FieldSelector, Model, SoftDeletable, SoftDelete, Timestamped, Timestamps};
pub use query_fields::{
	Comparable, DateTimeType, Field, GroupByFields, Lookup, LookupType, LookupValue, NumericType,
	QueryFieldCompiler, StringType,
};
pub use set_operations::{CombinedQuery, SetOperation, SetOperationBuilder};
pub use transaction::{
	Atomic, IsolationLevel, Savepoint, Transaction, TransactionScope, TransactionState, atomic,
	atomic_with_isolation,
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
pub use execution::{ExecutionResult, QueryExecution, SelectExecution};
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
pub use relations::{GenericRelationConfig, GenericRelationSet};
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
pub use query_execution::{ExecutableQuery, QueryCompiler};
pub use reverse_accessor::ReverseAccessor;

// Django ORM compatibility layer
pub use manager::Manager;
// Query types are always available
pub use query::{Filter, FilterCondition, FilterOperator, FilterValue, OrmQuery, QuerySet};

// Advanced ORM features
pub use absolute_url_overrides::{HasAbsoluteUrl, clear_url_overrides, register_url_override};
pub use composite_synonym::{CompositeSynonym, FieldValue, SynonymError};
pub use lambda_stmt::{
	CACHE_STATS, CacheStatistics, LambdaRegistry, LambdaStmt, QUERY_CACHE, QueryCache,
};
pub use order_with_respect_to::{OrderError, OrderValue, OrderedModel};

// reinhardt-query re-exports for query building in client code
pub use reinhardt_query::prelude::{
	Alias, ColumnRef, Cond, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query,
	QueryStatementBuilder, QueryStatementWriter, SqliteQueryBuilder,
};
