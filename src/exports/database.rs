//! Database, ORM, and query builder re-exports.
//!
//! [`DatabaseConnection`] is the copyable ORM handle exposed at the Reinhardt
//! crate root. Its RAII owner, `DatabaseConnectionLease`, remains available
//! explicitly under `reinhardt_db::orm` for standalone setup and framework
//! bootstrap code.

pub use reinhardt_db::orm::{
	AtomicTransaction, DatabaseBackend, DatabaseConnection, FieldAssignment, Model, OrmExecutor,
	QuerySet, SoftDeletable, SoftDelete, Timestamped, Timestamps, UpdateValue,
};

// Query expressions (Django-style F/Q objects)
pub use reinhardt_db::orm::{
	Exists, F, FieldRef, Filter, FilterOperator, FilterValue, OuterRef, Q, QOperator, Subquery,
};

// Annotations and aggregations
pub use reinhardt_db::orm::AggregateValue;

// Transaction management
pub use reinhardt_db::orm::{
	IsolationLevel, QueryValue, Savepoint, Transaction, TransactionExecutor,
};

// Database functions
pub use reinhardt_db::orm::func;
pub use reinhardt_db::orm::{
	Abs, Cast, Ceil, Concat, CurrentDate, CurrentTime, Extract, ExtractComponent, Floor, Greatest,
	Least, Length, Lower, Mod, Now, NullIf, Power, Round, SqlType, Sqrt, Substr, Trim, TrimType,
	Upper,
};

// Window functions
pub use reinhardt_db::orm::{
	DenseRank, FirstValue, Frame, FrameBoundary, FrameType, Lag, LastValue, Lead, NTile, NthValue,
	Rank, RowNumber, Window, WindowFunction,
};

// Constraints and indexes
pub use reinhardt_db::orm::{
	BTreeIndex, CheckConstraint, Constraint, ForeignKeyConstraint, GinIndex, GistIndex, HashIndex,
	Index, OnDelete, OnUpdate, UniqueConstraint,
};

// Query builder types from reinhardt-query (via reinhardt-db)
pub use reinhardt_db::orm::{IntoValue, Order, QueryBuilderValue};

// Connection pool
pub use reinhardt_db::pool::{ConnectionPool, PoolConfig, PoolError};

// Content types
pub use reinhardt_db::contenttypes::{
	CONTENT_TYPE_REGISTRY, ContentType, ContentTypeRegistry, GenericForeignKey, GenericRelatable,
	GenericRelationQuery, ModelType,
};

// Migrations
pub use reinhardt_db::migrations::{
	FieldState, Migration, MigrationAutodetector, MigrationError, MigrationPlan, MigrationRecorder,
	ModelState, ProjectState,
};
