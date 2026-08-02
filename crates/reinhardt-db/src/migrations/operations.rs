//! Migration operations
//!
//! This module provides various migration operations inspired by Django's migration system.
//! Operations are organized into three categories:
//!
//! - **Model operations** (`models`): Create, delete, and rename models (tables)
//! - **Field operations** (`fields`): Add, remove, alter, and rename fields (columns)
//! - **Special operations** (`special`): Run raw SQL or custom code
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::migrations::operations::{
//!     models::{CreateModel, DeleteModel},
//!     fields::{AddField, RemoveField},
//!     special::RunSQL,
//!     FieldDefinition,
//! };
//! use reinhardt_db::migrations::{ProjectState, FieldType};
//!
//! let mut state = ProjectState::new();
//!
//! // Create a model
//! let create = CreateModel::new(
//!     "User",
//!     vec![
//!         FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None),
//!         FieldDefinition::new("name", FieldType::VarChar(100), false, false, Option::<&str>::None),
//!     ],
//! );
//! create.state_forwards("myapp", &mut state);
//!
//! // Add a field
//! let add = AddField::new("User", FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None));
//! add.state_forwards("myapp", &mut state);
//!
//! // Run custom SQL
//! let sql = RunSQL::new("CREATE INDEX idx_email ON myapp_user(email)");
//! ```

pub mod fields;
pub mod models;
pub mod postgres;
pub mod special;
mod to_tokens;

// Re-export commonly used types for convenience
pub use fields::{AddField, AlterField, RemoveField, RenameField};
pub use models::{CreateModel, DeleteModel, FieldDefinition, MoveModel, RenameModel};
pub use postgres::{CreateCollation, CreateExtension, DropExtension};
pub use special::{RunCode, RunSQL, StateOperation};

// Legacy types for backward compatibility
// These are maintained from the original operations.rs
#[cfg(feature = "pgvector")]
use super::IndexDefinition;
use super::{FieldState, FieldType, ModelState, ProjectState};
use pg_escape::{quote_identifier, quote_literal};
use reinhardt_query::prelude::{
	Alias, AlterTableStatement, CockroachDBQueryBuilder, ColumnDef, ColumnType as QueryColumnType,
	CreateIndexStatement, CreateTableStatement, DropIndexStatement, DropTableStatement,
	GeneratedColumn, GeneratedStorage, MySqlQueryBuilder, PostgresQueryBuilder, Query,
	QueryBuilder, SchemaExpr, SchemaFunc, SimpleExpr, SqliteQueryBuilder, Value,
};
use serde::{Deserialize, Serialize, ser::SerializeStruct};

#[cfg(feature = "pgvector")]
pub(super) fn named_index_has_target(columns: &[String], expressions: Option<&[String]>) -> bool {
	!columns.is_empty() || expressions.is_some_and(|expressions| !expressions.is_empty())
}

/// Index type for database indexes
///
/// Specifies the type of index to create. Different index types have different
/// performance characteristics and support different operators.
///
/// # Examples
///
/// ```rust
/// use reinhardt_db::migrations::operations::IndexType;
///
/// // B-Tree is the default, best for equality and range queries
/// let btree = IndexType::BTree;
///
/// // Hash is best for simple equality comparisons
/// let hash = IndexType::Hash;
///
/// // GIN is best for containment operators (arrays, JSONB)
/// let gin = IndexType::Gin;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
	/// B-tree index (default)
	///
	/// Best for: equality and range queries (=, <, >, <=, >=, BETWEEN)
	/// Supported by: All databases
	#[default]
	BTree,

	/// Hash index
	///
	/// Best for: simple equality comparisons (=)
	/// Supported by: PostgreSQL, MySQL
	Hash,

	/// GIN (Generalized Inverted Index)
	///
	/// Best for: composite values (arrays, JSONB, full-text search)
	/// Supported by: PostgreSQL
	Gin,

	/// GiST (Generalized Search Tree)
	///
	/// Best for: geometric data, full-text search, range types
	/// Supported by: PostgreSQL
	Gist,

	/// BRIN (Block Range Index)
	///
	/// Best for: very large tables with naturally ordered data
	/// Supported by: PostgreSQL
	Brin,

	/// Full-text index
	///
	/// Best for: full-text search on text columns
	/// Supported by: MySQL
	Fulltext,

	/// Spatial index
	///
	/// Best for: geometric/geographic data
	/// Supported by: MySQL
	Spatial,

	/// HNSW approximate vector index
	///
	/// Supported by: PostgreSQL with pgvector
	#[cfg(feature = "pgvector")]
	Hnsw {
		/// Maximum number of connections per layer.
		m: Option<u16>,
		/// Candidate list size used while constructing the index.
		ef_construction: Option<u16>,
	},

	/// IVFFlat approximate vector index
	///
	/// Supported by: PostgreSQL with pgvector
	#[cfg(feature = "pgvector")]
	Ivfflat {
		/// Number of inverted lists.
		lists: Option<u32>,
	},
}

impl std::fmt::Display for IndexType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			IndexType::BTree => write!(f, "btree"),
			IndexType::Hash => write!(f, "hash"),
			IndexType::Gin => write!(f, "gin"),
			IndexType::Gist => write!(f, "gist"),
			IndexType::Brin => write!(f, "brin"),
			IndexType::Fulltext => write!(f, "fulltext"),
			IndexType::Spatial => write!(f, "spatial"),
			#[cfg(feature = "pgvector")]
			IndexType::Hnsw { .. } => write!(f, "hnsw"),
			#[cfg(feature = "pgvector")]
			IndexType::Ivfflat { .. } => write!(f, "ivfflat"),
		}
	}
}

impl IndexType {
	fn is_approximate_vector(self) -> bool {
		#[cfg(feature = "pgvector")]
		{
			matches!(self, Self::Hnsw { .. } | Self::Ivfflat { .. })
		}
		#[cfg(not(feature = "pgvector"))]
		{
			let _ = self;
			false
		}
	}

	fn options_sql(self) -> Option<String> {
		match self {
			#[cfg(feature = "pgvector")]
			Self::Hnsw { m, ef_construction } => {
				let mut options = Vec::new();
				if let Some(m) = m {
					options.push(format!("m = {m}"));
				}
				if let Some(ef_construction) = ef_construction {
					options.push(format!("ef_construction = {ef_construction}"));
				}
				(!options.is_empty()).then(|| format!(" WITH ({})", options.join(", ")))
			}
			#[cfg(feature = "pgvector")]
			Self::Ivfflat { lists } => lists.map(|lists| format!(" WITH (lists = {lists})")),
			_ => None,
		}
	}
}
// ============================================================================
// MySQL-Specific ALTER TABLE Options
// ============================================================================

/// MySQL ALTER TABLE algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum MySqlAlgorithm {
	/// Instant variant.
	Instant,
	/// Inplace variant.
	Inplace,
	/// Copy variant.
	Copy,
	#[default]
	/// Default variant.
	Default,
}

impl std::fmt::Display for MySqlAlgorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			MySqlAlgorithm::Instant => write!(f, "INSTANT"),
			MySqlAlgorithm::Inplace => write!(f, "INPLACE"),
			MySqlAlgorithm::Copy => write!(f, "COPY"),
			MySqlAlgorithm::Default => write!(f, "DEFAULT"),
		}
	}
}

/// MySQL ALTER TABLE lock types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum MySqlLock {
	/// None variant.
	None,
	/// Shared variant.
	Shared,
	/// Exclusive variant.
	Exclusive,
	#[default]
	/// Default variant.
	Default,
}

impl std::fmt::Display for MySqlLock {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			MySqlLock::None => write!(f, "NONE"),
			MySqlLock::Shared => write!(f, "SHARED"),
			MySqlLock::Exclusive => write!(f, "EXCLUSIVE"),
			MySqlLock::Default => write!(f, "DEFAULT"),
		}
	}
}

/// MySQL ALTER TABLE options
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct AlterTableOptions {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	/// The algorithm.
	pub algorithm: Option<MySqlAlgorithm>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	/// The lock.
	pub lock: Option<MySqlLock>,
}

impl AlterTableOptions {
	/// Creates a new instance.
	pub fn new() -> Self {
		Self::default()
	}
	/// Sets the algorithm and returns self for chaining.
	pub fn with_algorithm(mut self, algorithm: MySqlAlgorithm) -> Self {
		self.algorithm = Some(algorithm);
		self
	}
	/// Sets the lock and returns self for chaining.
	pub fn with_lock(mut self, lock: MySqlLock) -> Self {
		self.lock = Some(lock);
		self
	}
	/// Returns the mpty.
	pub fn is_empty(&self) -> bool {
		self.algorithm.is_none() && self.lock.is_none()
	}
	/// Converts to sql suffix.
	pub fn to_sql_suffix(&self) -> String {
		let mut parts = Vec::new();
		if let Some(algo) = &self.algorithm
			&& *algo != MySqlAlgorithm::Default
		{
			parts.push(format!("ALGORITHM={}", algo));
		}
		if let Some(lock) = &self.lock
			&& *lock != MySqlLock::Default
		{
			parts.push(format!("LOCK={}", lock));
		}
		if parts.is_empty() {
			String::new()
		} else {
			format!(", {}", parts.join(", "))
		}
	}
}

// ============================================================================
// MySQL Table Partitioning
// ============================================================================

/// Partition type for MySQL table partitioning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PartitionType {
	/// Range variant.
	Range,
	/// List variant.
	List,
	/// Hash variant.
	Hash,
	/// Key variant.
	Key,
}

impl std::fmt::Display for PartitionType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PartitionType::Range => write!(f, "RANGE"),
			PartitionType::List => write!(f, "LIST"),
			PartitionType::Hash => write!(f, "HASH"),
			PartitionType::Key => write!(f, "KEY"),
		}
	}
}

/// Partition value definition
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PartitionValues {
	/// LessThan variant.
	LessThan(String),
	/// In variant.
	In(Vec<String>),
	/// ModuloCount variant.
	ModuloCount(u32),
}

/// Individual partition definition
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartitionDef {
	/// The name.
	pub name: String,
	/// The values.
	pub values: PartitionValues,
}

impl PartitionDef {
	/// Creates a new instance.
	pub fn new(name: impl Into<String>, values: PartitionValues) -> Self {
		Self {
			name: name.into(),
			values,
		}
	}
	/// Performs the less than operation.
	pub fn less_than(name: impl Into<String>, value: impl Into<String>) -> Self {
		Self::new(name, PartitionValues::LessThan(value.into()))
	}
	/// Performs the maxvalue operation.
	pub fn maxvalue(name: impl Into<String>) -> Self {
		Self::new(name, PartitionValues::LessThan("MAXVALUE".to_string()))
	}
	/// Performs the list in operation.
	pub fn list_in(name: impl Into<String>, values: Vec<String>) -> Self {
		Self::new(name, PartitionValues::In(values))
	}
}

/// CockroachDB INTERLEAVE IN PARENT specification
///
/// Used to co-locate child table rows with parent table rows,
/// improving join performance for hierarchical data.
///
/// **CockroachDB only**: This is ignored for other databases.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterleaveSpec {
	/// Parent table name
	pub parent_table: String,
	/// Columns in the parent table to interleave with
	pub parent_columns: Vec<String>,
}

/// Table partitioning options
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionOptions {
	/// The partition type.
	pub partition_type: PartitionType,
	/// The column.
	pub column: String,
	/// The partitions.
	pub partitions: Vec<PartitionDef>,
}

impl PartitionOptions {
	/// Creates a new instance.
	pub fn new(
		partition_type: PartitionType,
		column: impl Into<String>,
		partitions: Vec<PartitionDef>,
	) -> Self {
		Self {
			partition_type,
			column: column.into(),
			partitions,
		}
	}
	/// Performs the range operation.
	pub fn range(column: impl Into<String>, partitions: Vec<PartitionDef>) -> Self {
		Self::new(PartitionType::Range, column, partitions)
	}
	/// Performs the list operation.
	pub fn list(column: impl Into<String>, partitions: Vec<PartitionDef>) -> Self {
		Self::new(PartitionType::List, column, partitions)
	}
	/// Performs the hash operation.
	pub fn hash(column: impl Into<String>, num_partitions: u32) -> Self {
		Self::new(
			PartitionType::Hash,
			column,
			vec![PartitionDef::new(
				"",
				PartitionValues::ModuloCount(num_partitions),
			)],
		)
	}
	/// Performs the key operation.
	pub fn key(column: impl Into<String>, num_partitions: u32) -> Self {
		Self::new(
			PartitionType::Key,
			column,
			vec![PartitionDef::new(
				"",
				PartitionValues::ModuloCount(num_partitions),
			)],
		)
	}
	/// Converts to sql.
	pub fn to_sql(&self) -> String {
		let mut sql = format!("PARTITION BY {}({})", self.partition_type, self.column);
		match self.partition_type {
			PartitionType::Hash | PartitionType::Key => {
				if let Some(p) = self.partitions.first()
					&& let PartitionValues::ModuloCount(n) = &p.values
				{
					sql.push_str(&format!(" PARTITIONS {}", n));
				}
			}
			PartitionType::Range | PartitionType::List => {
				sql.push_str(" (");
				let defs: Vec<String> = self
					.partitions
					.iter()
					.map(|p| {
						let vals = match &p.values {
							PartitionValues::LessThan(v) => {
								if v == "MAXVALUE" {
									"VALUES LESS THAN MAXVALUE".to_string()
								} else {
									format!("VALUES LESS THAN ('{}')", v)
								}
							}
							PartitionValues::In(v) => format!(
								"VALUES IN ({})",
								v.iter()
									.map(|x| format!("'{}'", x))
									.collect::<Vec<_>>()
									.join(", ")
							),
							PartitionValues::ModuloCount(_) => String::new(),
						};
						format!("PARTITION {} {}", p.name, vals)
					})
					.collect();
				sql.push_str(&defs.join(", "));
				sql.push(')');
			}
		}
		sql
	}
}

/// Deferrable constraint option for PostgreSQL
///
/// Controls when constraint checking is performed during a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeferrableOption {
	/// DEFERRABLE INITIALLY IMMEDIATE
	Immediate,
	/// DEFERRABLE INITIALLY DEFERRED
	Deferred,
}

impl std::fmt::Display for DeferrableOption {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			DeferrableOption::Immediate => write!(f, "DEFERRABLE INITIALLY IMMEDIATE"),
			DeferrableOption::Deferred => write!(f, "DEFERRABLE INITIALLY DEFERRED"),
		}
	}
}

/// Constraint definition for tables
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type")]
pub enum Constraint {
	/// PrimaryKey constraint
	///
	/// Used for composite primary keys defined at the table level.
	/// Single-column primary keys are typically defined directly on the column.
	PrimaryKey {
		/// The constraint name.
		name: String,
		/// The columns that form the primary key.
		columns: Vec<String>,
	},
	/// ForeignKey constraint
	ForeignKey {
		/// The constraint name.
		name: String,
		/// The columns in the referencing table.
		columns: Vec<String>,
		/// The referenced table name.
		referenced_table: String,
		/// The columns in the referenced table.
		referenced_columns: Vec<String>,
		/// Action on delete of the referenced row.
		on_delete: super::ForeignKeyAction,
		/// Action on update of the referenced row.
		on_update: super::ForeignKeyAction,
		/// Optional deferrable constraint option.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		deferrable: Option<DeferrableOption>,
	},
	/// Unique constraint
	Unique {
		/// The constraint name.
		name: String,
		/// The columns that must be unique together.
		columns: Vec<String>,
	},
	/// Check constraint
	Check {
		/// The constraint name.
		name: String,
		/// The SQL check expression.
		expression: String,
	},
	/// Typed finite-value domain for a model enum.
	EnumDomain {
		/// The constraint name.
		name: String,
		/// The constrained database column.
		column: String,
		/// The structured enum domain.
		domain: crate::field_domain::FieldDomain,
	},
	/// OneToOne constraint (ForeignKey + Unique combination)
	OneToOne {
		/// The constraint name.
		name: String,
		/// The column in the referencing table.
		column: String,
		/// The referenced table name.
		referenced_table: String,
		/// The referenced column name.
		referenced_column: String,
		/// Action on delete of the referenced row.
		on_delete: super::ForeignKeyAction,
		/// Action on update of the referenced row.
		on_update: super::ForeignKeyAction,
		/// Optional deferrable constraint option.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		deferrable: Option<DeferrableOption>,
	},
	/// ManyToMany relationship metadata (intermediate table reference)
	ManyToMany {
		/// The relationship name.
		name: String,
		/// The intermediate (through) table name.
		through_table: String,
		/// The column referencing the source table.
		source_column: String,
		/// The column referencing the target table.
		target_column: String,
		/// The target table name.
		target_table: String,
	},
	/// Exclude constraint (PostgreSQL only)
	Exclude {
		/// The name.
		name: String,
		/// The elements.
		elements: Vec<(String, String)>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		/// The using.
		using: Option<String>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		/// The where clause.
		where_clause: Option<String>,
	},
}

impl Constraint {
	/// Returns the database constraint name.
	pub fn name(&self) -> &str {
		match self {
			Constraint::PrimaryKey { name, .. }
			| Constraint::ForeignKey { name, .. }
			| Constraint::Unique { name, .. }
			| Constraint::Check { name, .. }
			| Constraint::EnumDomain { name, .. }
			| Constraint::OneToOne { name, .. }
			| Constraint::ManyToMany { name, .. }
			| Constraint::Exclude { name, .. } => name,
		}
	}

	fn to_sql_for_dialect(&self, dialect: &SqlDialect) -> String {
		let quote = |identifier: &str| Operation::quote_dialect_identifier(identifier, dialect);
		let quote_columns = |columns: &[String]| {
			columns
				.iter()
				.map(|column| quote(column))
				.collect::<Vec<_>>()
				.join(", ")
		};

		match self {
			Constraint::PrimaryKey { name, columns } => format!(
				"CONSTRAINT {} PRIMARY KEY ({})",
				quote(name),
				quote_columns(columns)
			),
			Constraint::ForeignKey {
				name,
				columns,
				referenced_table,
				referenced_columns,
				on_delete,
				on_update,
				deferrable,
			} => {
				let mut sql = format!(
					"CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {} ON UPDATE {}",
					quote(name),
					quote_columns(columns),
					Operation::quote_schema_identifier(referenced_table, dialect),
					quote_columns(referenced_columns),
					on_delete.to_sql_keyword(),
					on_update.to_sql_keyword()
				);
				if let Some(option) = deferrable {
					sql.push(' ');
					sql.push_str(&option.to_string());
				}
				sql
			}
			Constraint::Unique { name, columns } => format!(
				"CONSTRAINT {} UNIQUE ({})",
				quote(name),
				quote_columns(columns)
			),
			Constraint::Check { name, expression } => {
				format!("CONSTRAINT {} CHECK ({})", quote(name), expression)
			}
			Constraint::EnumDomain {
				name,
				column,
				domain,
			} => {
				let crate::field_domain::FieldDomain::Enum { repr, values } =
					domain.clone().canonicalized();
				let literals = values
					.into_iter()
					.map(|value| match value {
						crate::field_domain::ModelEnumValue::String(value) => {
							Value::String(Some(Box::new(value))).to_sql_literal()
						}
						crate::field_domain::ModelEnumValue::I32(value) => {
							Value::Int(Some(value)).to_sql_literal()
						}
					})
					.collect::<Vec<_>>();
				let column_expression = if matches!(dialect, SqlDialect::Mysql)
					&& matches!(repr, crate::field_domain::ModelEnumRepr::String)
				{
					format!("BINARY {}", quote(column))
				} else {
					quote(column)
				};
				format!(
					"CONSTRAINT {} CHECK ({} IN ({}))",
					quote(name),
					column_expression,
					literals.join(", ")
				)
			}
			Constraint::OneToOne {
				name,
				column,
				referenced_table,
				referenced_column,
				on_delete,
				on_update,
				deferrable,
			} => {
				let mut sql = format!(
					"CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {} ON UPDATE {}",
					quote(name),
					quote(column),
					Operation::quote_schema_identifier(referenced_table, dialect),
					quote(referenced_column),
					on_delete.to_sql_keyword(),
					on_update.to_sql_keyword()
				);
				if let Some(option) = deferrable {
					sql.push(' ');
					sql.push_str(&option.to_string());
				}
				sql.push_str(&format!(
					", CONSTRAINT {} UNIQUE ({})",
					quote(&format!("{name}_unique")),
					quote(column)
				));
				sql
			}
			_ => self.to_string(),
		}
	}
}

impl std::fmt::Display for Constraint {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Constraint::PrimaryKey { name, columns } => {
				write!(
					f,
					"CONSTRAINT {} PRIMARY KEY ({})",
					name,
					columns.join(", ")
				)
			}
			Constraint::ForeignKey {
				name,
				columns,
				referenced_table,
				referenced_columns,
				on_delete,
				on_update,
				deferrable,
			} => {
				write!(
					f,
					"CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {} ON UPDATE {}",
					name,
					columns.join(", "),
					referenced_table,
					referenced_columns.join(", "),
					on_delete.to_sql_keyword(),
					on_update.to_sql_keyword()
				)?;
				if let Some(defer_opt) = deferrable {
					write!(f, " {}", defer_opt)?;
				}
				Ok(())
			}
			Constraint::Unique { name, columns } => {
				write!(f, "CONSTRAINT {} UNIQUE ({})", name, columns.join(", "))
			}
			Constraint::Check { name, expression } => {
				write!(f, "CONSTRAINT {} CHECK ({})", name, expression)
			}
			Constraint::EnumDomain {
				name: _,
				column: _,
				domain: _,
			} => {
				write!(f, "{}", self.to_sql_for_dialect(&SqlDialect::Postgres))
			}
			Constraint::OneToOne {
				name,
				column,
				referenced_table,
				referenced_column,
				on_delete,
				on_update,
				deferrable,
			} => {
				write!(
					f,
					"CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {} ON UPDATE {}",
					name,
					column,
					referenced_table,
					referenced_column,
					on_delete.to_sql_keyword(),
					on_update.to_sql_keyword()
				)?;
				if let Some(defer_opt) = deferrable {
					write!(f, " {}", defer_opt)?;
				}
				write!(f, ", CONSTRAINT {}_unique UNIQUE ({})", name, column)
			}
			Constraint::ManyToMany { through_table, .. } => {
				write!(f, "-- ManyToMany via {}", through_table)
			}
			Constraint::Exclude {
				name,
				elements,
				using,
				where_clause,
			} => {
				let elements_str: Vec<String> = elements
					.iter()
					.map(|(col, op)| format!("{} WITH {}", col, op))
					.collect();
				let using_str = using.as_deref().unwrap_or("gist");
				if let Some(where_cl) = where_clause {
					write!(
						f,
						"CONSTRAINT {} EXCLUDE USING {} ({}) WHERE ({})",
						name,
						using_str,
						elements_str.join(", "),
						where_cl
					)
				} else {
					write!(
						f,
						"CONSTRAINT {} EXCLUDE USING {} ({})",
						name,
						using_str,
						elements_str.join(", ")
					)
				}
			}
		}
	}
}

/// Source for bulk data loading
///
/// Specifies where the data for bulk loading comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum BulkLoadSource {
	/// Load from a file path
	File(String),
	/// Load from standard input (STDIN)
	Stdin,
	/// Load from a program's output (e.g., "gunzip -c file.csv.gz")
	Program(String),
}

/// Format for bulk data loading
///
/// Specifies the format of the data being loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BulkLoadFormat {
	/// Plain text format (PostgreSQL default)
	#[default]
	Text,
	/// CSV format
	Csv,
	/// Binary format (PostgreSQL-specific)
	Binary,
}

impl std::fmt::Display for BulkLoadFormat {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			BulkLoadFormat::Text => write!(f, "TEXT"),
			BulkLoadFormat::Csv => write!(f, "CSV"),
			BulkLoadFormat::Binary => write!(f, "BINARY"),
		}
	}
}

/// Options for bulk data loading
///
/// Provides fine-grained control over how data is parsed during bulk loading.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkLoadOptions {
	/// Field delimiter character (default: ',' for CSV, '\t' for TEXT)
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub delimiter: Option<char>,
	/// String to represent NULL values
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub null_string: Option<String>,
	/// Whether the file has a header row (CSV format)
	#[serde(default)]
	pub header: bool,
	/// Columns to load into (if not all columns)
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub columns: Option<Vec<String>>,
	/// Use LOCAL keyword (MySQL LOAD DATA LOCAL INFILE)
	#[serde(default)]
	pub local: bool,
	/// Quote character for CSV (default: '"')
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub quote: Option<char>,
	/// Escape character for CSV
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub escape: Option<char>,
	/// Line terminator (default: '\n')
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub line_terminator: Option<String>,
	/// Encoding of the file (MySQL-specific)
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub encoding: Option<String>,
}

impl BulkLoadOptions {
	/// Create new BulkLoadOptions with default values
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the field delimiter
	pub fn with_delimiter(mut self, delimiter: char) -> Self {
		self.delimiter = Some(delimiter);
		self
	}

	/// Set the NULL string representation
	pub fn with_null_string(mut self, null_string: impl Into<String>) -> Self {
		self.null_string = Some(null_string.into());
		self
	}

	/// Enable or disable header row
	pub fn with_header(mut self, header: bool) -> Self {
		self.header = header;
		self
	}

	/// Set specific columns to load
	pub fn with_columns(mut self, columns: Vec<String>) -> Self {
		self.columns = Some(columns);
		self
	}

	/// Enable LOCAL keyword for MySQL
	pub fn with_local(mut self, local: bool) -> Self {
		self.local = local;
		self
	}

	/// Set the quote character for CSV
	pub fn with_quote(mut self, quote: char) -> Self {
		self.quote = Some(quote);
		self
	}

	/// Set the escape character
	pub fn with_escape(mut self, escape: char) -> Self {
		self.escape = Some(escape);
		self
	}

	/// Set the line terminator
	pub fn with_line_terminator(mut self, terminator: impl Into<String>) -> Self {
		self.line_terminator = Some(terminator.into());
		self
	}

	/// Set the file encoding (MySQL-specific)
	pub fn with_encoding(mut self, encoding: impl Into<String>) -> Self {
		self.encoding = Some(encoding.into());
		self
	}
}

/// A migration operation (legacy enum for backward compatibility)
///
/// This enum is maintained for backward compatibility with existing code.
/// New code should use the specific operation types from the `models`, `fields`,
/// and `special` modules instead.
// The legacy public variants embed ColumnDefinition by value, so boxing them would break
// migration source compatibility. Structured field domains intentionally preserve that shape.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Operation {
	/// CreateTable variant.
	CreateTable {
		/// The name.
		name: String,
		/// The columns.
		columns: Vec<ColumnDefinition>,
		#[serde(default)]
		/// The constraints.
		constraints: Vec<Constraint>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		/// The without rowid.
		without_rowid: Option<bool>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		/// The interleave in parent.
		interleave_in_parent: Option<InterleaveSpec>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		/// The partition.
		partition: Option<PartitionOptions>,
	},
	/// DropTable variant.
	DropTable {
		/// The name.
		name: String,
	},
	/// AddColumn variant.
	AddColumn {
		/// The table.
		table: String,
		/// The column.
		column: ColumnDefinition,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		/// The mysql options.
		mysql_options: Option<AlterTableOptions>,
	},
	/// DropColumn variant.
	DropColumn {
		/// The table.
		table: String,
		/// The column.
		column: String,
		/// Original column definition before dropping.
		/// This is required for generating accurate rollback SQL when the
		/// column is replaced by a drop/add pair.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		old_definition: Option<ColumnDefinition>,
	},
	/// AlterColumn variant.
	AlterColumn {
		/// The table.
		table: String,
		/// The column.
		column: String,
		/// Original column definition (before alteration).
		/// This is required for generating accurate rollback SQL.
		/// If None, rollback will attempt to reconstruct from ProjectState.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		old_definition: Option<ColumnDefinition>,
		/// The new definition.
		new_definition: ColumnDefinition,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		/// The mysql options.
		mysql_options: Option<AlterTableOptions>,
	},
	/// RenameTable variant.
	RenameTable {
		/// The old name.
		old_name: String,
		/// The new name.
		new_name: String,
	},
	/// RenameColumn variant.
	RenameColumn {
		/// The table.
		table: String,
		/// The old name.
		old_name: String,
		/// The new name.
		new_name: String,
	},
	/// AddConstraint variant.
	AddConstraint {
		/// The table.
		table: String,
		/// The constraint sql.
		constraint_sql: String,
	},
	/// Adds a typed constraint with dialect-aware SQL rendering.
	AddConstraintDefinition {
		/// The table.
		table: String,
		/// The typed constraint definition.
		constraint: Constraint,
	},
	/// Generated-column dependency constraint repair applied only when migrating forward.
	AddConstraintRepair {
		/// The table.
		table: String,
		/// The constraint sql.
		constraint_sql: String,
	},
	/// Generated-column dependency constraint restore applied only when rolling back.
	RestoreConstraintOnRollback {
		/// The table.
		table: String,
		/// The constraint sql.
		constraint_sql: String,
	},
	/// DropConstraint variant.
	DropConstraint {
		/// The table.
		table: String,
		/// The constraint name.
		constraint_name: String,
	},
	/// Drops a typed constraint while retaining its definition for rollback.
	DropConstraintDefinition {
		/// The table.
		table: String,
		/// The typed constraint definition captured before it is dropped.
		constraint: Constraint,
	},
	/// CreateIndex variant.
	CreateIndex {
		/// The table.
		table: String,
		/// The columns.
		columns: Vec<String>,
		/// The unique.
		unique: bool,
		/// Index type (B-Tree, Hash, GIN, GiST, etc.)
		///
		/// If not specified, the database will use its default index type (typically B-Tree).
		#[serde(default, skip_serializing_if = "Option::is_none")]
		index_type: Option<IndexType>,
		/// Partial index condition (WHERE clause)
		///
		/// Creates a partial index that only indexes rows matching this condition.
		/// Example: "status = 'active'" creates an index only for active rows.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		where_clause: Option<String>,
		/// Create index concurrently (PostgreSQL-specific)
		///
		/// When true, creates the index without locking the table for writes.
		/// This is slower but allows concurrent operations during index creation.
		#[serde(default)]
		concurrently: bool,
		/// Expression index (PostgreSQL, SQLite, MySQL 8.0+)
		///
		/// Index on computed expressions rather than simple column references.
		/// When specified, these expressions are used instead of `columns`.
		///
		/// # Examples
		///
		/// ```rust,ignore
		/// // Index on lowercase email for case-insensitive lookups
		/// expressions: Some(vec!["LOWER(email)"]),
		/// ```
		///
		/// **Note**: When `expressions` is Some, `columns` is ignored for SQL generation.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		expressions: Option<Vec<String>>,
		/// MySQL ALTER TABLE options (ALGORITHM, LOCK)
		#[serde(default, skip_serializing_if = "Option::is_none")]
		mysql_options: Option<AlterTableOptions>,
		/// Operator class for index columns (PostgreSQL-specific)
		///
		/// Specifies a non-default operator class for the index.
		/// Commonly used with extension-provided operator classes like `gin_trgm_ops`
		/// for trigram similarity search with the pg_trgm extension.
		///
		/// # Examples
		///
		/// ```rust,ignore
		/// // GIN index with trigram operator class for fuzzy text search
		/// CreateIndex {
		///     table: "products".to_string(),
		///     columns: vec!["name".to_string()],
		///     index_type: Some(IndexType::Gin),
		///     operator_class: Some("gin_trgm_ops"),
		///     ...
		/// }
		/// ```
		#[serde(default, skip_serializing_if = "Option::is_none")]
		operator_class: Option<String>,
	},
	/// Creates an index with an explicit physical name.
	///
	/// This additive variant preserves source compatibility for legacy
	/// [`Operation::CreateIndex`] struct literals while allowing model-declared
	/// indexes to keep their configured names.
	#[cfg(feature = "pgvector")]
	CreateNamedIndex {
		/// The table.
		table: String,
		/// Explicit physical index name.
		name: String,
		/// The columns.
		columns: Vec<String>,
		/// Whether the index is unique.
		unique: bool,
		/// Typed index method and options.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		index_type: Option<IndexType>,
		/// Partial index condition.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		where_clause: Option<String>,
		/// Whether to create concurrently.
		#[serde(default)]
		concurrently: bool,
		/// Index expressions.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		expressions: Option<Vec<String>>,
		/// MySQL ALTER TABLE options.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		mysql_options: Option<AlterTableOptions>,
		/// PostgreSQL operator class.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		operator_class: Option<String>,
	},
	/// Generated-column dependency index repair applied only when migrating forward.
	CreateIndexRepair {
		/// The table.
		table: String,
		/// Explicit index name.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		name: Option<String>,
		/// The columns.
		columns: Vec<String>,
		/// The unique.
		unique: bool,
		/// Index type.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		index_type: Option<IndexType>,
		/// Partial index condition.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		where_clause: Option<String>,
		/// Create index concurrently.
		#[serde(default)]
		concurrently: bool,
		/// Expression index values.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		expressions: Option<Vec<String>>,
		/// MySQL ALTER TABLE options.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		mysql_options: Option<AlterTableOptions>,
		/// Operator class for index columns.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		operator_class: Option<String>,
	},
	/// Generated-column dependency index restore applied only when rolling back.
	RestoreIndexOnRollback {
		/// The table.
		table: String,
		/// Explicit index name.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		name: Option<String>,
		/// The columns.
		columns: Vec<String>,
		/// The unique.
		unique: bool,
		/// Index type.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		index_type: Option<IndexType>,
		/// Partial index condition.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		where_clause: Option<String>,
		/// Create index concurrently.
		#[serde(default)]
		concurrently: bool,
		/// Expression index values.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		expressions: Option<Vec<String>>,
		/// MySQL ALTER TABLE options.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		mysql_options: Option<AlterTableOptions>,
		/// Operator class for index columns.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		operator_class: Option<String>,
	},
	/// DropIndex variant.
	DropIndex {
		/// The table.
		table: String,
		/// The columns.
		columns: Vec<String>,
	},
	/// Drops an index by its explicit physical name.
	#[cfg(feature = "pgvector")]
	DropNamedIndex {
		/// The table containing the index.
		table: String,
		/// Explicit physical index name.
		name: String,
		/// Indexed columns from the removed definition.
		#[serde(default)]
		columns: Vec<String>,
		/// Whether the removed index is unique.
		#[serde(default)]
		unique: bool,
		/// Typed method and options from the removed definition.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		index_type: Option<IndexType>,
		/// Partial index condition from the removed definition.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		where_clause: Option<String>,
		/// Whether the removed index was created concurrently.
		#[serde(default)]
		concurrently: bool,
		/// Expressions from the removed definition.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		expressions: Option<Vec<String>>,
		/// MySQL options from the removed definition.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		mysql_options: Option<AlterTableOptions>,
		/// PostgreSQL operator class from the removed definition.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		operator_class: Option<String>,
	},
	/// RunSQL variant.
	RunSQL {
		/// The sql.
		sql: String,
		/// The reverse sql.
		reverse_sql: Option<String>,
	},
	/// RunRust variant.
	RunRust {
		/// The code.
		code: String,
		/// The reverse code.
		reverse_code: Option<String>,
	},
	/// AlterTableComment variant.
	AlterTableComment {
		/// The table.
		table: String,
		/// The comment.
		comment: Option<String>,
	},
	/// AlterUniqueTogether variant.
	AlterUniqueTogether {
		/// The table.
		table: String,
		/// The unique together.
		unique_together: Vec<Vec<String>>,
	},
	/// AlterModelOptions variant.
	AlterModelOptions {
		/// The table.
		table: String,
		/// The options.
		options: std::collections::HashMap<String, String>,
	},
	/// CreateInheritedTable variant.
	CreateInheritedTable {
		/// The name.
		name: String,
		/// The columns.
		columns: Vec<ColumnDefinition>,
		/// The base table.
		base_table: String,
		/// The join column.
		join_column: String,
	},
	/// AddDiscriminatorColumn variant.
	AddDiscriminatorColumn {
		/// The table.
		table: String,
		/// The column name.
		column_name: String,
		/// The default value.
		default_value: String,
	},
	/// Move a model from one app to another
	///
	/// This operation handles cross-app model moves by:
	/// 1. Optionally renaming the table (if naming convention changes between apps)
	/// 2. Updating FK references to use the new table name
	///
	/// Note: This generates a RenameTable SQL if table name changes.
	/// The state tracking (from_app -> to_app) is handled at the ProjectState level.
	MoveModel {
		/// Name of the model being moved
		model_name: String,
		/// Source app label
		from_app: String,
		/// Target app label
		to_app: String,
		/// Whether to rename the underlying table
		rename_table: bool,
		/// Old table name (if rename_table is true)
		old_table_name: Option<String>,
		/// New table name (if rename_table is true)
		new_table_name: Option<String>,
	},
	/// Create a database schema (PostgreSQL, MySQL 5.0.2+)
	///
	/// Creates a new database schema namespace. In MySQL, this is equivalent to creating a database.
	CreateSchema {
		/// Name of the schema to create
		name: String,
		/// Whether to add IF NOT EXISTS clause
		#[serde(default)]
		if_not_exists: bool,
	},
	/// Drop a database schema
	///
	/// Drops an existing database schema. Use with caution as this will drop all objects in the schema.
	DropSchema {
		/// Name of the schema to drop
		name: String,
		/// Whether to add CASCADE clause (drops all contained objects)
		#[serde(default)]
		cascade: bool,
		/// Whether to add IF EXISTS clause
		#[serde(default = "default_true")]
		if_exists: bool,
	},
	/// Create a PostgreSQL extension (PostgreSQL-specific)
	///
	/// Creates a PostgreSQL extension like PostGIS, uuid-ossp, etc.
	/// This operation is only executed on PostgreSQL databases.
	CreateExtension {
		/// Name of the extension to create
		name: String,
		/// Whether to add IF NOT EXISTS clause
		#[serde(default = "default_true")]
		if_not_exists: bool,
		/// Optional schema to install the extension in
		#[serde(default)]
		schema: Option<String>,
	},
	/// Bulk data loading operation
	///
	/// Loads large amounts of data efficiently using database-native bulk loading commands:
	/// - PostgreSQL: `COPY table FROM source WITH (FORMAT csv, ...)`
	/// - MySQL: `LOAD DATA [LOCAL] INFILE 'path' INTO TABLE table ...`
	/// - SQLite: Not supported (falls back to INSERT statements)
	///
	/// # Performance
	///
	/// Bulk loading is typically 10-100x faster than individual INSERT statements
	/// for large datasets.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{Operation, BulkLoadSource, BulkLoadFormat, BulkLoadOptions};
	///
	/// // PostgreSQL COPY FROM file
	/// let op = Operation::BulkLoad {
	///     table: "events".to_string(),
	///     source: BulkLoadSource::File("/tmp/events.csv"),
	///     format: BulkLoadFormat::Csv,
	///     options: BulkLoadOptions::new()
	///         .with_header(true)
	///         .with_delimiter(','),
	/// };
	///
	/// // MySQL LOAD DATA LOCAL INFILE
	/// let op = Operation::BulkLoad {
	///     table: "events".to_string(),
	///     source: BulkLoadSource::File("/tmp/events.csv"),
	///     format: BulkLoadFormat::Csv,
	///     options: BulkLoadOptions::new()
	///         .with_local(true)
	///         .with_delimiter(','),
	/// };
	/// ```
	BulkLoad {
		/// Target table name
		table: String,
		/// Source of the data
		source: BulkLoadSource,
		/// Format of the data
		#[serde(default)]
		format: BulkLoadFormat,
		/// Additional loading options
		#[serde(default)]
		options: BulkLoadOptions,
	},
	/// Reset the auto-increment counter for a table
	///
	/// Sets the next value produced by the table's auto-increment mechanism.
	/// Typical uses include seeding IDs after a bulk import or shifting the
	/// sequence above a range reserved for historical data.
	///
	/// # Backend Behavior
	///
	/// - **PostgreSQL / CockroachDB**: `SELECT setval(pg_get_serial_sequence('{table}', '{column}'), {value}, false)`
	///   (resolves the sequence dynamically so both default `SERIAL` conventions
	///   and user-defined sequence names work; `false` makes the NEXT generated
	///   value equal `{value}`).
	/// - **MySQL**: `ALTER TABLE {table} AUTO_INCREMENT = {value}`.
	/// - **SQLite**: `INSERT OR REPLACE INTO sqlite_sequence(name, seq) VALUES (...)`
	///   (robust against tables that have not yet inserted any rows, where a
	///   simple `UPDATE` would silently no-op).
	SetAutoIncrementValue {
		/// The table whose auto-increment counter should be set.
		table: String,
		/// The auto-increment column (used to resolve the backing sequence
		/// on PostgreSQL / CockroachDB).
		column: String,
		/// The next value the counter should produce.
		value: i64,
	},
	/// Create a composite (multi-column) PRIMARY KEY constraint on an existing table
	///
	/// Emits `ALTER TABLE {table} ADD CONSTRAINT {name} PRIMARY KEY ({cols})`
	/// on every supported backend. When `constraint_name` is `None` the name
	/// defaults to `{table}_pkey`, matching PostgreSQL's conventional
	/// auto-generated identifier.
	///
	/// `columns` must be non-empty; emitting an empty column list would
	/// produce invalid SQL and is rejected as an `InvalidMigration` error at
	/// SQL generation time.
	CreateCompositePrimaryKey {
		/// The table to add the composite primary key to.
		table: String,
		/// The ordered list of columns participating in the primary key.
		columns: Vec<String>,
		/// Optional explicit constraint name. Defaults to `{table}_pkey`
		/// when `None`.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		constraint_name: Option<String>,
	},
}

/// Default value provider for serde (returns true)
const fn default_true() -> bool {
	true
}

impl Operation {
	#[cfg(feature = "pgvector")]
	pub(crate) fn pgvector_operation_kind(
		&self,
	) -> Option<crate::backends::error::PgvectorOperationKind> {
		use crate::backends::error::PgvectorOperationKind;

		match self {
			Self::CreateTable { columns, .. } | Self::CreateInheritedTable { columns, .. }
				if columns
					.iter()
					.any(|column| matches!(column.type_definition, FieldType::Vector { .. })) =>
			{
				Some(PgvectorOperationKind::ColumnType)
			}
			Self::AddColumn { column, .. }
			| Self::AlterColumn {
				new_definition: column,
				..
			} if matches!(column.type_definition, FieldType::Vector { .. }) => {
				Some(PgvectorOperationKind::ColumnType)
			}
			Self::CreateIndex { index_type, .. }
			| Self::CreateNamedIndex { index_type, .. }
			| Self::CreateIndexRepair { index_type, .. }
				if index_type.is_some_and(IndexType::is_approximate_vector) =>
			{
				Some(PgvectorOperationKind::ApproximateIndex)
			}
			_ => None,
		}
	}

	#[cfg(not(feature = "pgvector"))]
	pub(crate) fn pgvector_operation_kind(
		&self,
	) -> Option<crate::backends::error::PgvectorOperationKind> {
		None
	}

	#[cfg(feature = "pgvector")]
	pub(crate) fn pgvector_reverse_operation_kind(
		&self,
	) -> Option<crate::backends::error::PgvectorOperationKind> {
		use crate::backends::error::PgvectorOperationKind;

		match self {
			Self::DropColumn {
				old_definition: Some(column),
				..
			}
			| Self::AlterColumn {
				old_definition: Some(column),
				..
			} if matches!(column.type_definition, FieldType::Vector { .. }) => {
				Some(PgvectorOperationKind::ColumnType)
			}
			Self::DropNamedIndex { index_type, .. }
			| Self::RestoreIndexOnRollback { index_type, .. }
				if index_type.is_some_and(IndexType::is_approximate_vector) =>
			{
				Some(PgvectorOperationKind::ApproximateIndex)
			}
			_ => None,
		}
	}

	#[cfg(not(feature = "pgvector"))]
	pub(crate) fn pgvector_reverse_operation_kind(
		&self,
	) -> Option<crate::backends::error::PgvectorOperationKind> {
		None
	}

	fn order_model_fields_by_generated_dependencies(
		model: &ModelState,
	) -> Vec<(&String, &FieldState)> {
		let mut remaining: Vec<_> = model.fields.iter().collect();
		let mut ordered = Vec::with_capacity(remaining.len());

		while !remaining.is_empty() {
			let next_index = remaining
				.iter()
				.enumerate()
				.find(|(_, (field_name, field))| {
					let Some(generated) = field.generated.as_ref() else {
						return true;
					};

					!remaining.iter().any(|(other_field_name, _)| {
						other_field_name.as_str() != field_name.as_str()
							&& Self::generated_column_references_column(
								generated,
								other_field_name.as_str(),
							)
					})
				})
				.map(|(index, _)| index);

			if let Some(index) = next_index {
				ordered.push(remaining.remove(index));
			} else {
				let cycle = remaining
					.iter()
					.map(|(name, _)| name.as_str())
					.collect::<Vec<_>>()
					.join(", ");
				panic!("generated-column dependency cycle detected among columns: {cycle}");
			}
		}

		ordered
	}

	fn generated_column_references_column(
		generated: &GeneratedColumnDefinition,
		column: &str,
	) -> bool {
		if let Some(expr) = generated.typed_expr() {
			return Self::schema_expr_references_column(&expr, column);
		}
		if let Some(raw_sql) = generated.raw_sql.as_deref() {
			return Self::expression_text_references_column(raw_sql, column);
		}
		generated
			.expr_tokens
			.as_deref()
			.is_some_and(|tokens| Self::expression_text_references_column(tokens, column))
	}

	fn schema_expr_references_column(expr: &SchemaExpr, column: &str) -> bool {
		match expr {
			SchemaExpr::Column(identifier) => identifier.to_string() == column,
			SchemaExpr::Value(_) => false,
			SchemaExpr::Binary { left, right, .. } => {
				Self::schema_expr_references_column(left, column)
					|| Self::schema_expr_references_column(right, column)
			}
			SchemaExpr::Function { args, .. } => args
				.iter()
				.any(|arg| Self::schema_expr_references_column(arg, column)),
			SchemaExpr::Cast { expr, .. } => Self::schema_expr_references_column(expr, column),
			_ => false,
		}
	}

	fn expression_text_references_column(text: &str, column: &str) -> bool {
		text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
			.any(|token| token.eq_ignore_ascii_case(column))
	}

	pub(crate) fn postgres_generated_column_dependency_violation<'a>(
		column_name: &str,
		generated: &GeneratedColumnDefinition,
		generated_column_names: impl IntoIterator<Item = &'a str>,
	) -> Option<&'a str> {
		generated_column_names.into_iter().find(|name| {
			*name != column_name && Self::generated_column_references_column(generated, name)
		})
	}

	fn validate_postgres_generated_column_dependencies(
		columns: &[ColumnDefinition],
		dialect: &SqlDialect,
	) {
		if !matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
			return;
		}

		let generated_column_names: Vec<&str> = columns
			.iter()
			.filter(|column| column.generated.is_some())
			.map(|column| column.name.as_str())
			.collect();

		for column in columns {
			let Some(generated) = column.generated.as_ref() else {
				continue;
			};

			if let Some(referenced_column) = Self::postgres_generated_column_dependency_violation(
				column.name.as_str(),
				generated,
				generated_column_names.iter().copied(),
			) {
				panic!(
					"PostgreSQL-compatible generated column `{}` cannot reference generated column `{}`",
					column.name, referenced_column
				);
			}
		}
	}

	/// Apply this operation to the project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		match self {
			Operation::CreateTable {
				name,
				columns,
				constraints,
				..
			} => {
				let mut model = ModelState::new(app_label, name.clone());
				model.table_name = name.clone();
				for column in columns {
					let mut field = FieldState::new(
						column.name.to_string(),
						column.type_definition.clone(),
						false,
					);
					field.generated = column.generated.clone();
					field.domain = column.domain.clone();
					model.add_field(field);
				}
				model.constraints = constraints
					.iter()
					.map(ProjectState::constraint_to_definition)
					.collect();
				state.add_model(model);
			}
			Operation::DropTable { name } => {
				state.remove_model_by_table_name(app_label, name);
			}
			Operation::AddColumn { table, column, .. } => {
				if let Some(model) = state.get_model_by_table_mut(app_label, table) {
					let mut field = FieldState::new(
						column.name.to_string(),
						column.type_definition.clone(),
						false,
					);
					field.generated = column.generated.clone();
					field.domain = column.domain.clone();
					model.add_field(field);
				}
			}
			Operation::DropColumn { table, column, .. } => {
				if let Some(model) = state.get_model_by_table_mut(app_label, table) {
					model.remove_field(column);
				}
			}
			Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} => {
				if let Some(model) = state.get_model_by_table_mut(app_label, table) {
					let mut field = FieldState::new(
						column.to_string(),
						new_definition.type_definition.clone(),
						false,
					);
					field.generated = new_definition.generated.clone();
					field.domain = new_definition.domain.clone();
					model.alter_field(column, field);
				}
			}
			Operation::RenameTable { old_name, new_name } => {
				state.rename_table_in_app(app_label, old_name, new_name);
			}
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => {
				if let Some(model) = state.get_model_by_table_mut(app_label, table) {
					model.rename_field(old_name, new_name.to_string());
				}
			}
			Operation::CreateInheritedTable {
				name,
				columns,
				base_table,
				join_column,
			} => {
				let mut model = ModelState::new(app_label, name.clone());
				model.table_name = name.clone();
				model.base_model = Some(base_table.to_string());
				model.inheritance_type = Some("joined_table".to_string());

				let join_field = FieldState::new(
					join_column.to_string(),
					FieldType::Custom(format!("INTEGER REFERENCES {}(id)", base_table)),
					false,
				);
				model.add_field(join_field);

				for column in columns {
					let field = FieldState::new(
						column.name.to_string(),
						column.type_definition.clone(),
						false,
					);
					model.add_field(field);
				}
				state.add_model(model);
			}
			Operation::AddDiscriminatorColumn {
				table,
				column_name,
				default_value,
			} => {
				if let Some(model) = state.get_model_by_table_mut(app_label, table) {
					model.discriminator_column = Some(column_name.to_string());
					model.inheritance_type = Some("single_table".to_string());
					let field = FieldState::new(
						column_name.to_string(),
						FieldType::Custom(format!("VARCHAR(50) DEFAULT '{}'", default_value)),
						false,
					);
					model.add_field(field);
				}
			}
			Operation::AddConstraintDefinition { table, constraint } => {
				if let Some(model) = state.find_model_by_table_mut(table) {
					if let Constraint::EnumDomain { column, domain, .. } = constraint
						&& let Some(field) = model.fields.get_mut(column)
					{
						field.domain = Some(domain.clone().canonicalized());
					}
					let definition = ProjectState::constraint_to_definition(constraint);
					if !model
						.constraints
						.iter()
						.any(|existing| existing.name == definition.name)
					{
						model.constraints.push(definition);
					}
				}
			}
			Operation::DropConstraint {
				table,
				constraint_name,
			} => {
				if let Some(model) = state.find_model_by_table_mut(table) {
					model
						.constraints
						.retain(|constraint| constraint.name != *constraint_name);
				}
			}
			Operation::DropConstraintDefinition { table, constraint } => {
				if let Some(model) = state.find_model_by_table_mut(table) {
					if let Constraint::EnumDomain { column, .. } = constraint
						&& let Some(field) = model.fields.get_mut(column)
					{
						field.domain = None;
					}
					model
						.constraints
						.retain(|definition| definition.name != constraint.name());
				}
			}
			Operation::AddConstraint { .. }
			| Operation::AddConstraintRepair { .. }
			| Operation::RestoreConstraintOnRollback { .. }
			| Operation::CreateIndex { .. }
			| Operation::CreateIndexRepair { .. }
			| Operation::RestoreIndexOnRollback { .. }
			| Operation::DropIndex { .. }
			| Operation::RunSQL { .. }
			| Operation::RunRust { .. }
			| Operation::AlterTableComment { .. }
			| Operation::AlterUniqueTogether { .. }
			| Operation::AlterModelOptions { .. }
			| Operation::SetAutoIncrementValue { .. }
			| Operation::CreateCompositePrimaryKey { .. } => {
				// Counter/constraint-level ops do not affect ProjectState
				// (they track model-level structure only).
			}
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				expressions,
				operator_class,
				..
			} => {
				if let Some(model) = state.find_model_by_table_mut(table) {
					#[cfg(not(feature = "pgvector"))]
					let _ = (index_type, expressions, operator_class);
					model.indexes.retain(|index| index.name != *name);
					model.indexes.push(IndexDefinition {
						name: name.clone(),
						fields: columns.clone(),
						unique: *unique,
						#[cfg(feature = "pgvector")]
						index_type: *index_type,
						#[cfg(feature = "pgvector")]
						operator_class: operator_class.clone(),
						#[cfg(feature = "pgvector")]
						expressions: expressions.clone(),
					});
				}
			}
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex { table, name, .. } => {
				if let Some(model) = state.find_model_by_table_mut(table) {
					model.indexes.retain(|index| index.name != *name);
				}
			}
			Operation::MoveModel {
				model_name,
				from_app,
				to_app,
				rename_table,
				old_table_name,
				new_table_name,
			} => {
				// Move the model from one app to another in the project state
				if *rename_table
					&& let (Some(old_name), Some(new_name)) = (old_table_name, new_table_name)
				{
					state.rename_table_in_app(from_app, old_name, new_name);
				}
				// First get the model, then remove it from the old location
				if let Some(model) = state.get_model(from_app, model_name).cloned() {
					state.remove_model(from_app, model_name);

					// Create a new model with updated app label
					let mut new_model = model;
					new_model.app_label = to_app.to_string();

					state.add_model(new_model);
				}
				for model in state.models.values_mut() {
					for field in model.fields.values_mut() {
						if field.params.get("fk_target_app").map(String::as_str) == Some(from_app)
							&& field
								.params
								.get("fk_target_model")
								.or_else(|| field.params.get("fk_target"))
								.map(String::as_str) == Some(model_name)
						{
							field
								.params
								.insert("fk_target_app".to_string(), to_app.to_string());
						}
					}
				}
			}
			// Schema operations don't affect ProjectState (models/fields only)
			Operation::CreateSchema { .. }
			| Operation::DropSchema { .. }
			| Operation::CreateExtension { .. } => {
				// No state changes for schema/extension operations
			}
			// BulkLoad is a data operation that doesn't affect model structure
			Operation::BulkLoad { .. } => {
				// No state changes for bulk data loading
			}
		}
	}

	/// Generate column SQL without PRIMARY KEY constraint (for composite primary keys)
	///
	/// This function is used when the table has a composite primary key defined at the table level.
	/// It generates column definitions without individual PRIMARY KEY keywords to avoid conflicts.
	fn column_to_sql_without_pk(col: &ColumnDefinition, dialect: &SqlDialect) -> String {
		let mut parts = Vec::new();

		// Column name
		parts.push(quote_identifier(&col.name));

		// Column type
		if col.auto_increment {
			match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => {
					// PostgreSQL 10+ uses GENERATED BY DEFAULT AS IDENTITY
					match &col.type_definition {
						FieldType::BigInteger => {
							parts
								.push("BIGINT GENERATED BY DEFAULT AS IDENTITY".to_string().into());
						}
						FieldType::Integer => {
							parts.push(
								"INTEGER GENERATED BY DEFAULT AS IDENTITY"
									.to_string()
									.into(),
							);
						}
						FieldType::SmallInteger => {
							parts.push(
								"SMALLINT GENERATED BY DEFAULT AS IDENTITY"
									.to_string()
									.into(),
							);
						}
						_ => {
							// Fallback for other types
							parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
						}
					}
				}
				SqlDialect::Mysql => {
					parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
					parts.push("AUTO_INCREMENT".to_string().into());
				}
				SqlDialect::Sqlite => {
					// SQLite requires the literal token `INTEGER` (not `BIGINT`/`SMALLINT`)
					// for AUTOINCREMENT columns. Widen any integer width to `INTEGER`
					// because SQLite's storage classes do not distinguish integer widths.
					match &col.type_definition {
						FieldType::BigInteger | FieldType::Integer | FieldType::SmallInteger => {
							parts.push("INTEGER".to_string().into());
						}
						_ => {
							// Non-integer auto_increment is invalid for SQLite; emit the
							// original type and let SQLite surface the error rather than
							// silently mis-emitting.
							parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
						}
					}
					// For SQLite, if part of composite PK, we don't add AUTOINCREMENT here
					// It will be handled by the table-level PRIMARY KEY constraint
				}
			}
		} else {
			parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
		}

		if let Some(generated) = &col.generated {
			parts.push(Self::generated_column_to_sql(generated, dialect).into());
		}

		// NOT NULL constraint
		if col.not_null {
			parts.push("NOT NULL".to_string().into());
		}

		// UNIQUE constraint (but NOT PRIMARY KEY)
		if col.unique {
			parts.push("UNIQUE".to_string().into());
		}

		// DEFAULT value
		if let Some(default) = &col.default {
			parts.push(format!("DEFAULT {}", default).into());
		}

		parts.join(" ")
	}

	fn generated_column_to_sql(
		generated: &GeneratedColumnDefinition,
		dialect: &SqlDialect,
	) -> String {
		if generated.storage == GeneratedStorage::Virtual
			&& matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb)
		{
			panic!("PostgreSQL-compatible generated columns require STORED storage");
		}

		let expr_sql = if let Some(expr) = &generated.expr {
			Self::schema_expr_to_sql(expr, dialect)
		} else if let Some(raw_sql) = &generated.raw_sql {
			raw_sql.clone()
		} else {
			panic!("generated columns require a typed expression or raw SQL body");
		};

		format!(
			"GENERATED ALWAYS AS ({}) {}",
			expr_sql,
			generated.storage.as_str()
		)
	}

	fn schema_expr_to_sql(expr: &SchemaExpr, dialect: &SqlDialect) -> String {
		match expr {
			SchemaExpr::Column(iden) => Self::quote_schema_identifier(&iden.to_string(), dialect),
			SchemaExpr::Value(value) => value.to_sql_literal(),
			SchemaExpr::Binary { left, op, right } => format!(
				"({} {} {})",
				Self::schema_expr_to_sql(left, dialect),
				op.as_str(),
				Self::schema_expr_to_sql(right, dialect)
			),
			SchemaExpr::Function { func, args } => match func {
				SchemaFunc::Concat if args.is_empty() => "''".to_string(),
				SchemaFunc::Concat if matches!(dialect, SqlDialect::Mysql) => format!(
					"CONCAT({})",
					args.iter()
						.map(|arg| Self::schema_expr_to_sql(arg, dialect))
						.collect::<Vec<_>>()
						.join(", ")
				),
				SchemaFunc::Concat => args
					.iter()
					.map(|arg| Self::schema_expr_to_sql(arg, dialect))
					.collect::<Vec<_>>()
					.join(" || "),
				SchemaFunc::Coalesce => format!(
					"COALESCE({})",
					args.iter()
						.map(|arg| Self::schema_expr_to_sql(arg, dialect))
						.collect::<Vec<_>>()
						.join(", ")
				),
				_ => panic!("unsupported generated-column schema function"),
			},
			SchemaExpr::Cast { expr, ty } => format!(
				"CAST({} AS {})",
				Self::schema_expr_to_sql(expr, dialect),
				Self::query_column_type_to_cast_sql(ty, dialect)
			),
			_ => panic!("unsupported generated-column schema expression"),
		}
	}

	fn quote_schema_identifier(ident: &str, dialect: &SqlDialect) -> String {
		if matches!(dialect, SqlDialect::Mysql) {
			format!("`{}`", ident.replace('`', "``"))
		} else {
			quote_identifier(ident).to_string()
		}
	}

	fn quote_dialect_identifier(ident: &str, dialect: &SqlDialect) -> String {
		match dialect {
			SqlDialect::Postgres => PostgresQueryBuilder::new().escape_identifier(ident),
			SqlDialect::Mysql => MySqlQueryBuilder.escape_identifier(ident),
			SqlDialect::Sqlite => SqliteQueryBuilder::new().escape_identifier(ident),
			SqlDialect::Cockroachdb => CockroachDBQueryBuilder::new().escape_identifier(ident),
		}
	}

	fn query_column_type_to_sql(ty: &QueryColumnType, dialect: &SqlDialect) -> String {
		match ty {
			QueryColumnType::Char(len) => format!("CHAR({})", len.unwrap_or(1)),
			QueryColumnType::String(Some(len)) if matches!(dialect, SqlDialect::Mysql) => {
				format!("CHAR({len})")
			}
			QueryColumnType::String(Some(len)) => format!("VARCHAR({len})"),
			QueryColumnType::String(None) if matches!(dialect, SqlDialect::Mysql) => {
				"CHAR".to_string()
			}
			QueryColumnType::String(None) => {
				if matches!(dialect, SqlDialect::Sqlite) {
					"TEXT".to_string()
				} else {
					"VARCHAR".to_string()
				}
			}
			QueryColumnType::Text => "TEXT".to_string(),
			QueryColumnType::Integer => {
				if matches!(dialect, SqlDialect::Mysql) {
					"INT".to_string()
				} else {
					"INTEGER".to_string()
				}
			}
			QueryColumnType::BigInteger => {
				if matches!(dialect, SqlDialect::Sqlite) {
					"INTEGER".to_string()
				} else {
					"BIGINT".to_string()
				}
			}
			QueryColumnType::SmallInteger => {
				if matches!(dialect, SqlDialect::Sqlite) {
					"INTEGER".to_string()
				} else {
					"SMALLINT".to_string()
				}
			}
			QueryColumnType::TinyInteger => {
				if matches!(dialect, SqlDialect::Mysql) {
					"TINYINT".to_string()
				} else if matches!(dialect, SqlDialect::Sqlite) {
					"INTEGER".to_string()
				} else {
					"SMALLINT".to_string()
				}
			}
			QueryColumnType::Float => {
				if matches!(dialect, SqlDialect::Mysql) {
					"FLOAT".to_string()
				} else {
					"REAL".to_string()
				}
			}
			QueryColumnType::Double => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"DOUBLE PRECISION".to_string()
				} else if matches!(dialect, SqlDialect::Sqlite) {
					"REAL".to_string()
				} else {
					"DOUBLE".to_string()
				}
			}
			QueryColumnType::Decimal(Some((precision, scale))) => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					format!("NUMERIC({precision}, {scale})")
				} else if matches!(dialect, SqlDialect::Sqlite) {
					"REAL".to_string()
				} else {
					format!("DECIMAL({precision}, {scale})")
				}
			}
			QueryColumnType::Decimal(None) => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"NUMERIC".to_string()
				} else if matches!(dialect, SqlDialect::Sqlite) {
					"REAL".to_string()
				} else {
					"DECIMAL".to_string()
				}
			}
			QueryColumnType::Boolean => {
				if matches!(dialect, SqlDialect::Mysql) {
					"TINYINT(1)".to_string()
				} else if matches!(dialect, SqlDialect::Sqlite) {
					"INTEGER".to_string()
				} else {
					"BOOLEAN".to_string()
				}
			}
			QueryColumnType::Date => {
				if matches!(dialect, SqlDialect::Sqlite) {
					"TEXT".to_string()
				} else {
					"DATE".to_string()
				}
			}
			QueryColumnType::Time => {
				if matches!(dialect, SqlDialect::Sqlite) {
					"TEXT".to_string()
				} else {
					"TIME".to_string()
				}
			}
			QueryColumnType::DateTime => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"TIMESTAMP".to_string()
				} else if matches!(dialect, SqlDialect::Sqlite) {
					"TEXT".to_string()
				} else {
					"DATETIME".to_string()
				}
			}
			QueryColumnType::Timestamp => {
				if matches!(dialect, SqlDialect::Sqlite) {
					"INTEGER".to_string()
				} else {
					"TIMESTAMP".to_string()
				}
			}
			QueryColumnType::TimestampWithTimeZone => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"TIMESTAMP WITH TIME ZONE".to_string()
				} else if matches!(dialect, SqlDialect::Sqlite) {
					"TEXT".to_string()
				} else {
					"TIMESTAMP".to_string()
				}
			}
			QueryColumnType::Binary(Some(len)) => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"BYTEA".to_string()
				} else {
					format!("BLOB({len})")
				}
			}
			QueryColumnType::Binary(None) => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"BYTEA".to_string()
				} else {
					"BLOB".to_string()
				}
			}
			QueryColumnType::VarBinary(len) => {
				if matches!(dialect, SqlDialect::Mysql) {
					format!("VARBINARY({len})")
				} else if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"BYTEA".to_string()
				} else {
					format!("BLOB({len})")
				}
			}
			QueryColumnType::Blob => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"BYTEA".to_string()
				} else {
					"BLOB".to_string()
				}
			}
			QueryColumnType::Uuid => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"UUID".to_string()
				} else if matches!(dialect, SqlDialect::Mysql) {
					"CHAR(36)".to_string()
				} else {
					"TEXT".to_string()
				}
			}
			QueryColumnType::Json => {
				if matches!(dialect, SqlDialect::Sqlite) {
					"TEXT".to_string()
				} else {
					"JSON".to_string()
				}
			}
			QueryColumnType::Jsonb => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					"JSONB".to_string()
				} else if matches!(dialect, SqlDialect::Sqlite) {
					"TEXT".to_string()
				} else {
					"JSON".to_string()
				}
			}
			QueryColumnType::Array(inner) => {
				if matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
					format!("{}[]", Self::query_column_type_to_sql(inner, dialect))
				} else if matches!(dialect, SqlDialect::Mysql) {
					"JSON".to_string()
				} else {
					"TEXT".to_string()
				}
			}
			#[cfg(feature = "pgvector")]
			QueryColumnType::Vector(dimensions) => format!("VECTOR({dimensions})"),
			QueryColumnType::Custom(custom) => custom.clone(),
			_ => panic!("unsupported generated-column cast column type"),
		}
	}

	fn query_column_type_to_cast_sql(ty: &QueryColumnType, dialect: &SqlDialect) -> String {
		if !matches!(dialect, SqlDialect::Mysql) {
			return Self::query_column_type_to_sql(ty, dialect);
		}

		match ty {
			QueryColumnType::Char(len) => format!("CHAR({})", len.unwrap_or(1)),
			QueryColumnType::String(Some(len)) => format!("CHAR({len})"),
			QueryColumnType::String(None) | QueryColumnType::Text => "CHAR".to_string(),
			QueryColumnType::TinyInteger
			| QueryColumnType::SmallInteger
			| QueryColumnType::Integer
			| QueryColumnType::BigInteger => "SIGNED".to_string(),
			QueryColumnType::Float => "FLOAT".to_string(),
			QueryColumnType::Double => "DOUBLE".to_string(),
			QueryColumnType::Decimal(Some((precision, scale))) => {
				format!("DECIMAL({precision}, {scale})")
			}
			QueryColumnType::Decimal(None) => "DECIMAL".to_string(),
			QueryColumnType::Boolean => "UNSIGNED".to_string(),
			QueryColumnType::Date => "DATE".to_string(),
			QueryColumnType::Time => "TIME".to_string(),
			QueryColumnType::DateTime
			| QueryColumnType::Timestamp
			| QueryColumnType::TimestampWithTimeZone => "DATETIME".to_string(),
			QueryColumnType::Binary(Some(len)) => format!("BINARY({len})"),
			QueryColumnType::Binary(None) | QueryColumnType::Blob => "BINARY".to_string(),
			QueryColumnType::VarBinary(len) => format!("BINARY({len})"),
			QueryColumnType::Uuid => "CHAR(36)".to_string(),
			QueryColumnType::Json | QueryColumnType::Jsonb | QueryColumnType::Array(_) => {
				"JSON".to_string()
			}
			#[cfg(feature = "pgvector")]
			QueryColumnType::Vector(dimensions) => format!("VECTOR({dimensions})"),
			QueryColumnType::Custom(custom) => custom.clone(),
			_ => panic!("unsupported generated-column cast column type"),
		}
	}

	/// Generate column SQL with all constraints
	fn column_to_sql(col: &ColumnDefinition, dialect: &SqlDialect) -> String {
		Self::column_to_sql_with_collation(col, dialect, None)
	}

	fn column_to_sql_with_collation(
		col: &ColumnDefinition,
		dialect: &SqlDialect,
		collation: Option<&str>,
	) -> String {
		let mut parts = Vec::new();

		// Column name
		parts.push(quote_identifier(&col.name));

		// Column type (with auto_increment handling for PostgreSQL)
		if col.auto_increment {
			match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => {
					// PostgreSQL 10+ uses GENERATED BY DEFAULT AS IDENTITY
					match &col.type_definition {
						FieldType::BigInteger => {
							parts
								.push("BIGINT GENERATED BY DEFAULT AS IDENTITY".to_string().into());
						}
						FieldType::Integer => {
							parts.push(
								"INTEGER GENERATED BY DEFAULT AS IDENTITY"
									.to_string()
									.into(),
							);
						}
						FieldType::SmallInteger => {
							parts.push(
								"SMALLINT GENERATED BY DEFAULT AS IDENTITY"
									.to_string()
									.into(),
							);
						}
						_ => {
							// Fallback for other types
							parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
						}
					}
				}
				SqlDialect::Mysql => {
					parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
					parts.push("AUTO_INCREMENT".to_string().into());
				}
				SqlDialect::Sqlite => {
					// SQLite requires the literal token `INTEGER` (not `BIGINT`/`SMALLINT`)
					// for AUTOINCREMENT columns. Widen any integer width to `INTEGER`
					// because SQLite's storage classes do not distinguish integer widths,
					// and `BIGINT PRIMARY KEY AUTOINCREMENT` is rejected at apply time
					// with: "AUTOINCREMENT is only allowed on an INTEGER PRIMARY KEY".
					//
					// For non-integer column types (e.g. `Uuid`), `auto_increment = true`
					// is meaningless in SQLite — the AUTOINCREMENT keyword would be
					// rejected. Emit the column type as a plain PRIMARY KEY (no
					// AUTOINCREMENT). See reinhardt-web#4378.
					let widened_to_integer = matches!(
						&col.type_definition,
						FieldType::BigInteger | FieldType::Integer | FieldType::SmallInteger
					);
					if widened_to_integer {
						parts.push("INTEGER".to_string().into());
					} else {
						parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
					}
					if let Some(collation) = collation {
						parts.push(
							format!(
								"COLLATE {}",
								Self::quote_dialect_identifier(collation, &SqlDialect::Sqlite)
							)
							.into(),
						);
					}
					// SQLite: AUTOINCREMENT requires `INTEGER PRIMARY KEY AUTOINCREMENT`.
					// Note that `INTEGER PRIMARY KEY` alone only enables rowid auto-assignment
					// (alias for the rowid); the explicit AUTOINCREMENT keyword is required to
					// guarantee monotonic, non-reused IDs (backed by sqlite_sequence).
					if col.primary_key {
						if widened_to_integer {
							parts.push("PRIMARY KEY AUTOINCREMENT".to_string().into());
						} else {
							parts.push("PRIMARY KEY".to_string().into());
						}
						// Return early to avoid duplicate PRIMARY KEY
						if col.unique {
							parts.push("UNIQUE".to_string().into());
						}
						if let Some(default) = &col.default {
							parts.push(format!("DEFAULT {}", default).into());
						}
						return parts.join(" ");
					}
				}
			}
		} else {
			parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
			if let Some(collation) = collation {
				parts.push(
					format!(
						"COLLATE {}",
						Self::quote_dialect_identifier(collation, &SqlDialect::Sqlite)
					)
					.into(),
				);
			}
		}

		if let Some(generated) = &col.generated {
			parts.push(Self::generated_column_to_sql(generated, dialect).into());
		}

		// NOT NULL constraint
		if col.not_null {
			parts.push("NOT NULL".to_string().into());
		}

		// PRIMARY KEY constraint
		if col.primary_key {
			parts.push("PRIMARY KEY".to_string().into());
		}

		// UNIQUE constraint
		if col.unique {
			parts.push("UNIQUE".to_string().into());
		}

		// DEFAULT value
		if let Some(default) = &col.default {
			parts.push(format!("DEFAULT {}", default).into());
		}

		parts.join(" ")
	}

	fn create_table_column_to_sql(
		col: &ColumnDefinition,
		dialect: &SqlDialect,
		without_primary_key: bool,
	) -> String {
		let sql = if without_primary_key {
			Self::column_to_sql_without_pk(col, dialect)
		} else {
			Self::column_to_sql(col, dialect)
		};
		let current_identifier = quote_identifier(&col.name).to_string();
		let dialect_identifier = Self::quote_schema_identifier(&col.name, dialect);
		let Some(rest) = sql.strip_prefix(&current_identifier) else {
			return sql;
		};
		format!("{dialect_identifier}{rest}")
	}

	/// Generate forward SQL
	pub fn to_sql(&self, dialect: &SqlDialect) -> String {
		match self {
			Operation::CreateTable {
				name,
				columns,
				constraints,
				without_rowid,
				interleave_in_parent,
				partition,
			} => {
				Self::validate_postgres_generated_column_dependencies(columns, dialect);

				// Detect composite primary key
				let pk_columns: Vec<&String> = columns
					.iter()
					.filter(|col| col.primary_key)
					.map(|col| &col.name)
					.collect();
				let has_composite_pk = pk_columns.len() > 1;

				let mut parts = Vec::new();
				for col in columns {
					// Use column_to_sql_without_pk for composite PKs to avoid duplicate PRIMARY KEY
					parts.push(format!(
						"  {}",
						Self::create_table_column_to_sql(col, dialect, has_composite_pk)
					));
				}

				// Add composite primary key constraint if detected
				if has_composite_pk {
					let pk_constraint_name = format!("{}_pkey", name);
					let quoted_pk_columns = pk_columns
						.iter()
						.map(|s| Self::quote_schema_identifier(s, dialect))
						.collect::<Vec<_>>()
						.join(", ");
					let pk_constraint = format!(
						"  CONSTRAINT {} PRIMARY KEY ({})",
						Self::quote_schema_identifier(&pk_constraint_name, dialect),
						quoted_pk_columns
					);
					parts.push(pk_constraint);
				}

				for constraint in constraints {
					parts.push(format!("  {}", constraint.to_sql_for_dialect(dialect)));
				}
				let mut sql = format!(
					"CREATE TABLE {} (\n{}\n)",
					Self::quote_schema_identifier(name, dialect),
					parts.join(",\n")
				);

				// SQLite: WITHOUT ROWID optimization for tables with explicit PRIMARY KEY
				if matches!(dialect, SqlDialect::Sqlite)
					&& let Some(true) = without_rowid
				{
					sql.push_str(" WITHOUT ROWID");
				}

				// MySQL: Table partitioning
				if matches!(dialect, SqlDialect::Mysql)
					&& let Some(partition_opts) = partition
				{
					sql.push(' ');
					sql.push_str(&partition_opts.to_sql());
				}

				// CockroachDB: INTERLEAVE IN PARENT for co-locating child rows with parent
				if matches!(dialect, SqlDialect::Cockroachdb)
					&& let Some(interleave) = interleave_in_parent
				{
					let quoted_columns = interleave
						.parent_columns
						.iter()
						.map(|col| quote_identifier(col))
						.collect::<Vec<_>>()
						.join(", ");
					sql.push_str(&format!(
						" INTERLEAVE IN PARENT {} ({})",
						quote_identifier(&interleave.parent_table),
						quoted_columns
					));
				}

				sql.push(';');
				sql
			}
			Operation::DropTable { name } => format!("DROP TABLE {};", quote_identifier(name)),
			Operation::AddColumn {
				table,
				column,
				mysql_options,
			} => {
				if matches!(dialect, SqlDialect::Sqlite)
					&& column
						.generated
						.as_ref()
						.is_some_and(|generated| generated.storage == GeneratedStorage::Stored)
				{
					panic!("SQLite ADD COLUMN does not support stored generated columns");
				}

				let base_sql = format!(
					"ALTER TABLE {} ADD COLUMN {}",
					quote_identifier(table),
					Self::column_to_sql(column, dialect)
				);

				// MySQL: Add ALGORITHM/LOCK options
				if matches!(dialect, SqlDialect::Mysql)
					&& let Some(opts) = mysql_options
				{
					let suffix = opts.to_sql_suffix();
					if !suffix.is_empty() {
						return format!("{}{};", base_sql, suffix);
					}
				}

				format!("{};", base_sql)
			}
			Operation::DropColumn { table, column, .. } => {
				format!(
					"ALTER TABLE {} DROP COLUMN {};",
					quote_identifier(table),
					quote_identifier(column)
				)
			}
			Operation::AlterColumn {
				table,
				column,
				old_definition,
				new_definition,
				mysql_options,
				..
			} => {
				let sql_type = new_definition.type_definition.to_sql_for_dialect(dialect);
				match dialect {
					SqlDialect::Postgres | SqlDialect::Cockroachdb => {
						let mut statements = Vec::new();
						if old_definition
							.as_ref()
							.is_some_and(|old_definition| old_definition.default.is_some())
						{
							statements.push(format!(
								"ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT;",
								quote_identifier(table),
								quote_identifier(column)
							));
						}
						statements.push(format!(
							"ALTER TABLE {} ALTER COLUMN {} TYPE {};",
							quote_identifier(table),
							quote_identifier(column),
							sql_type
						));
						if let Some(default) = &new_definition.default {
							statements.push(format!(
								"ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {};",
								quote_identifier(table),
								quote_identifier(column),
								default
							));
						}
						statements.join(" ")
					}
					SqlDialect::Mysql => {
						let base_sql = format!(
							"ALTER TABLE {} MODIFY COLUMN {}",
							quote_identifier(table),
							Self::column_to_sql(new_definition, dialect)
						);

						// MySQL: Add ALGORITHM/LOCK options
						if let Some(opts) = mysql_options {
							let suffix = opts.to_sql_suffix();
							if !suffix.is_empty() {
								return format!("{}{};", base_sql, suffix);
							}
						}

						format!("{};", base_sql)
					}
					SqlDialect::Sqlite => {
						format!(
							"-- SQLite does not support ALTER COLUMN, table recreation required for {}",
							quote_identifier(table)
						)
					}
				}
			}
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => {
				format!(
					"ALTER TABLE {} RENAME COLUMN {} TO {};",
					quote_identifier(table),
					quote_identifier(old_name),
					quote_identifier(new_name)
				)
			}
			Operation::RenameTable { old_name, new_name } => {
				format!(
					"ALTER TABLE {} RENAME TO {};",
					quote_identifier(old_name),
					quote_identifier(new_name)
				)
			}
			Operation::AddConstraint {
				table,
				constraint_sql,
			}
			| Operation::AddConstraintRepair {
				table,
				constraint_sql,
			} => {
				format!(
					"ALTER TABLE {} ADD {};",
					Self::quote_schema_identifier(table, dialect),
					constraint_sql
				)
			}
			Operation::AddConstraintDefinition { table, constraint } => format!(
				"ALTER TABLE {} ADD {};",
				Self::quote_schema_identifier(table, dialect),
				constraint.to_sql_for_dialect(dialect)
			),
			Operation::RestoreConstraintOnRollback { .. } => {
				"-- rollback-only generated-column constraint restore".to_string()
			}
			Operation::DropConstraint {
				table,
				constraint_name,
			} => {
				format!(
					"ALTER TABLE {} DROP CONSTRAINT {};",
					quote_identifier(table),
					quote_identifier(constraint_name)
				)
			}
			Operation::DropConstraintDefinition { table, constraint } => format!(
				"ALTER TABLE {} DROP CONSTRAINT {};",
				Self::quote_schema_identifier(table, dialect),
				Self::quote_dialect_identifier(constraint.name(), dialect)
			),
			Operation::CreateIndex {
				table,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			}
			| Operation::CreateIndexRepair {
				table,
				name: _,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				let unique_str = if *unique { "UNIQUE " } else { "" };

				// PostgreSQL: CONCURRENTLY keyword (must come before UNIQUE)
				let concurrent_str = if *concurrently && matches!(dialect, SqlDialect::Postgres) {
					"CONCURRENTLY "
				} else {
					""
				};

				// MySQL: FULLTEXT/SPATIAL prefix (replaces UNIQUE for these types)
				let (mysql_prefix, effective_unique) = match (index_type, dialect) {
					(Some(IndexType::Fulltext), SqlDialect::Mysql) => ("FULLTEXT ", ""),
					(Some(IndexType::Spatial), SqlDialect::Mysql) => ("SPATIAL ", ""),
					_ => ("", unique_str),
				};

				// Determine what to index: expressions or columns
				let (index_content, name_suffix) =
					if let Some(exprs) = expressions.as_ref().filter(|e| !e.is_empty()) {
						// For expression indexes, use expressions and generate a hash-based suffix
						// Expressions are assumed to be properly formatted, no additional quoting needed
						let content = if let Some(operator_class) = operator_class
							&& matches!(dialect, SqlDialect::Postgres)
						{
							exprs
								.iter()
								.map(|expression| format!("{expression} {operator_class}"))
								.collect::<Vec<_>>()
								.join(", ")
						} else {
							exprs.join(", ")
						};
						let suffix = "expr";
						(content, suffix.to_string())
					} else {
						// Use columns with optional operator class
						let content = if let Some(op_class) = operator_class {
							// Apply operator class to each column (PostgreSQL-specific)
							if matches!(dialect, SqlDialect::Postgres) {
								columns
									.iter()
									.map(|c| format!("{} {}", quote_identifier(c), op_class))
									.collect::<Vec<_>>()
									.join(", ")
							} else {
								// Quote column names for safety (reserved words, special chars)
								columns
									.iter()
									.map(|c| quote_identifier(c).to_string())
									.collect::<Vec<_>>()
									.join(", ")
							}
						} else {
							// Quote column names for safety (reserved words, special chars)
							columns
								.iter()
								.map(|c| quote_identifier(c).to_string())
								.collect::<Vec<_>>()
								.join(", ")
						};
						(content, columns.join("_"))
					};

				let idx_name = match self {
					Operation::CreateIndexRepair {
						name: Some(name), ..
					} => name.clone(),
					_ => format!("idx_{}_{}", table, name_suffix),
				};

				// Index type clause (USING type) - PostgreSQL, CockroachDB
				let using_clause = match (index_type, dialect) {
					(Some(IndexType::BTree), _) => String::new(), // Default, no need to specify
					(Some(idx_type), SqlDialect::Postgres | SqlDialect::Cockroachdb) => {
						format!(" USING {}", idx_type)
					}
					// MySQL FULLTEXT/SPATIAL handled via prefix, not USING
					(Some(IndexType::Fulltext | IndexType::Spatial), SqlDialect::Mysql) => {
						String::new()
					}
					_ => String::new(),
				};

				// Build base SQL with correct syntax per dialect
				// PostgreSQL: CREATE [UNIQUE] INDEX [CONCURRENTLY] name [USING type] ON table (cols)
				// MySQL: CREATE [FULLTEXT|SPATIAL|UNIQUE] INDEX name ON table (cols)
				// SQLite: CREATE [UNIQUE] INDEX name ON table (cols)
				let mut sql = match dialect {
					SqlDialect::Postgres | SqlDialect::Cockroachdb => {
						// CONCURRENTLY goes between INDEX and index_name
						format!(
							"CREATE {}INDEX {}{}",
							effective_unique,
							concurrent_str,
							quote_identifier(&idx_name)
						)
					}
					SqlDialect::Mysql => {
						// MySQL doesn't support CONCURRENTLY or USING (except for FULLTEXT/SPATIAL prefix)
						format!(
							"CREATE {}{}INDEX {}",
							mysql_prefix,
							effective_unique,
							quote_identifier(&idx_name)
						)
					}
					SqlDialect::Sqlite => {
						// SQLite doesn't support CONCURRENTLY or USING
						format!(
							"CREATE {}INDEX {}",
							effective_unique,
							quote_identifier(&idx_name)
						)
					}
				};
				// PostgreSQL: ON table USING method (columns)
				// MySQL/SQLite: ON table (columns)
				// Quote table name for safety (reserved words, special chars)
				sql.push_str(&format!(
					" ON {}{} ({})",
					quote_identifier(table),
					using_clause,
					index_content
				));

				if let Some(options) = index_type.and_then(|index_type| index_type.options_sql()) {
					sql.push_str(&options);
				}

				// Add WHERE clause for partial indexes (PostgreSQL, SQLite, CockroachDB - not MySQL)
				if let Some(where_cond) = where_clause
					&& !matches!(dialect, SqlDialect::Mysql)
				{
					sql.push_str(&format!(" WHERE {}", where_cond));
				}

				// MySQL: Add ALGORITHM/LOCK options
				if matches!(dialect, SqlDialect::Mysql)
					&& let Some(opts) = mysql_options
				{
					let suffix = opts.to_sql_suffix();
					if !suffix.is_empty() {
						sql.push_str(&suffix);
					}
				}

				sql.push(';');
				sql
			}
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => Operation::CreateIndexRepair {
				table: table.clone(),
				name: Some(name.clone()),
				columns: columns.clone(),
				unique: *unique,
				index_type: *index_type,
				where_clause: where_clause.clone(),
				concurrently: *concurrently,
				expressions: expressions.clone(),
				mysql_options: *mysql_options,
				operator_class: operator_class.clone(),
			}
			.to_sql(dialect),
			Operation::RestoreIndexOnRollback { .. } => {
				"-- rollback-only generated-column index restore".to_string()
			}
			Operation::DropIndex { table, columns } => {
				let idx_name = format!("idx_{}_{}", table, columns.join("_"));
				match dialect {
					SqlDialect::Mysql => {
						format!(
							"DROP INDEX {} ON {};",
							quote_identifier(&idx_name),
							quote_identifier(table)
						)
					}
					SqlDialect::Postgres | SqlDialect::Sqlite | SqlDialect::Cockroachdb => {
						format!("DROP INDEX {};", quote_identifier(&idx_name))
					}
				}
			}
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex { table, name, .. } => match dialect {
				SqlDialect::Mysql => format!(
					"DROP INDEX {} ON {};",
					quote_identifier(name),
					quote_identifier(table)
				),
				SqlDialect::Postgres | SqlDialect::Sqlite | SqlDialect::Cockroachdb => {
					format!("DROP INDEX {};", quote_identifier(name))
				}
			},
			Operation::RunSQL { sql, .. } => sql.to_string(),
			Operation::RunRust { code, .. } => {
				// For SQL generation, RunRust is a no-op comment
				format!("-- RunRust: {}", code.lines().next().unwrap_or(""))
			}
			Operation::AlterTableComment { table, comment } => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => {
					if let Some(comment_text) = comment {
						format!(
							"COMMENT ON TABLE {} IS '{}';",
							quote_identifier(table),
							comment_text
						)
					} else {
						format!("COMMENT ON TABLE {} IS NULL;", quote_identifier(table))
					}
				}
				SqlDialect::Mysql => {
					if let Some(comment_text) = comment {
						format!(
							"ALTER TABLE {} COMMENT='{}';",
							quote_identifier(table),
							comment_text
						)
					} else {
						format!("ALTER TABLE {} COMMENT='';", quote_identifier(table))
					}
				}
				SqlDialect::Sqlite => String::new(),
			},
			Operation::AlterUniqueTogether {
				table,
				unique_together,
			} => {
				let mut sql = Vec::new();
				for (idx, fields) in unique_together.iter().enumerate() {
					let constraint_name = format!("{}_{}_uniq", table, idx);
					let fields_str = fields
						.iter()
						.map(|f| quote_identifier(f))
						.collect::<Vec<_>>()
						.join(", ");
					sql.push(format!(
						"ALTER TABLE {} ADD CONSTRAINT {} UNIQUE ({});",
						quote_identifier(table),
						quote_identifier(&constraint_name),
						fields_str
					));
				}
				sql.join("\n")
			}
			Operation::AlterModelOptions { .. } => String::new(),
			Operation::CreateInheritedTable {
				name,
				columns,
				base_table,
				join_column,
			} => {
				let mut parts = Vec::new();
				parts.push(format!(
					"  {} INTEGER REFERENCES {}(id)",
					quote_identifier(join_column),
					quote_identifier(base_table)
				));
				for col in columns {
					parts.push(format!("  {}", Self::column_to_sql(col, dialect)));
				}
				format!(
					"CREATE TABLE {} (\n{}\n);",
					quote_identifier(name),
					parts.join(",\n")
				)
			}
			Operation::AddDiscriminatorColumn {
				table,
				column_name,
				default_value,
			} => {
				format!(
					"ALTER TABLE {} ADD COLUMN {} VARCHAR(50) DEFAULT '{}';",
					quote_identifier(table),
					quote_identifier(column_name),
					default_value
				)
			}
			Operation::MoveModel {
				rename_table,
				old_table_name,
				new_table_name,
				..
			} => {
				// MoveModel generates a RenameTable SQL if table name changes
				// Otherwise it's a state-only operation (no SQL needed)
				if *rename_table {
					if let (Some(old_name), Some(new_name)) = (old_table_name, new_table_name) {
						match dialect {
							SqlDialect::Postgres | SqlDialect::Sqlite | SqlDialect::Cockroachdb => {
								format!(
									"ALTER TABLE {} RENAME TO {};",
									quote_identifier(old_name),
									quote_identifier(new_name)
								)
							}
							SqlDialect::Mysql => {
								format!(
									"RENAME TABLE {} TO {};",
									quote_identifier(old_name),
									quote_identifier(new_name)
								)
							}
						}
					} else {
						"-- MoveModel: No table rename specified".to_string()
					}
				} else {
					// State-only operation, no SQL needed
					"-- MoveModel: State-only operation (no table rename)".to_string()
				}
			}
			Operation::CreateSchema {
				name,
				if_not_exists,
			} => {
				let if_not_exists_clause = if *if_not_exists { " IF NOT EXISTS" } else { "" };
				format!(
					"CREATE SCHEMA{} {};",
					if_not_exists_clause,
					quote_identifier(name)
				)
			}
			Operation::DropSchema {
				name,
				cascade,
				if_exists,
			} => {
				let if_exists_clause = if *if_exists { " IF EXISTS" } else { "" };
				let cascade_clause = if *cascade { " CASCADE" } else { "" };
				format!(
					"DROP SCHEMA{} {}{};",
					if_exists_clause,
					quote_identifier(name),
					cascade_clause
				)
			}
			Operation::CreateExtension {
				name,
				if_not_exists,
				schema,
			} => {
				// PostgreSQL-specific
				let if_not_exists_clause = if *if_not_exists { " IF NOT EXISTS" } else { "" };
				let schema_clause = if let Some(s) = schema {
					format!(" SCHEMA {}", quote_identifier(s))
				} else {
					String::new()
				};
				format!(
					"CREATE EXTENSION{} {}{};",
					if_not_exists_clause,
					quote_identifier(name),
					schema_clause
				)
			}
			Operation::BulkLoad {
				table,
				source,
				format,
				options,
			} => Self::bulk_load_to_sql(table, source, format, options, dialect),
			Operation::SetAutoIncrementValue {
				table,
				column,
				value,
			} => Self::set_auto_increment_to_sql(table, column, *value, dialect),
			Operation::CreateCompositePrimaryKey {
				table,
				columns,
				constraint_name,
			} => Self::create_composite_pk_to_sql(table, columns, constraint_name.as_deref()),
		}
	}

	/// Generates forward SQL after validating backend-specific field support.
	pub fn try_to_sql(&self, dialect: &SqlDialect) -> super::Result<String> {
		self.validate_for_dialect(dialect)?;
		Ok(self.to_sql(dialect))
	}

	fn validate_approximate_vector_index(
		index_type: Option<IndexType>,
		unique: bool,
		columns: &[String],
		expressions: Option<&[String]>,
		operator_class: Option<&str>,
		dialect: &SqlDialect,
	) -> super::Result<()> {
		let Some(_index_type) = index_type.filter(|index_type| index_type.is_approximate_vector())
		else {
			return Ok(());
		};

		let backend = match dialect {
			SqlDialect::Postgres => None,
			SqlDialect::Mysql => Some("mysql"),
			SqlDialect::Sqlite => Some("sqlite"),
			SqlDialect::Cockroachdb => Some("cockroachdb"),
		};
		if let Some(backend) = backend {
			return Err(super::MigrationError::UnsupportedBackendFeature {
				feature: "approximate vector indexes",
				backend,
			});
		}

		if unique {
			return Err(super::MigrationError::InvalidMigration(
				"approximate vector indexes cannot be unique".to_string(),
			));
		}

		let expression_count = expressions.map_or(0, <[String]>::len);
		let has_exactly_one_target = (columns.len() == 1 && expression_count == 0)
			|| (columns.is_empty() && expression_count == 1);
		if !has_exactly_one_target {
			return Err(super::MigrationError::InvalidMigration(
				"approximate vector indexes require exactly one column or expression".to_string(),
			));
		}

		if !operator_class.is_some_and(|operator_class| {
			matches!(
				operator_class,
				"vector_l2_ops" | "vector_ip_ops" | "vector_cosine_ops"
			)
		}) {
			return Err(super::MigrationError::InvalidMigration(
				"approximate vector indexes require a supported vector operator class".to_string(),
			));
		}

		#[cfg(feature = "pgvector")]
		let invalid_option = match _index_type {
			IndexType::Hnsw { m: Some(m), .. } if !(2..=100).contains(&m) => {
				Some("m must be in the range 2..=100")
			}
			IndexType::Hnsw {
				ef_construction: Some(ef_construction),
				..
			} if !(4..=1000).contains(&ef_construction) => {
				Some("ef_construction must be in the range 4..=1000")
			}
			IndexType::Hnsw { m, ef_construction }
				if ef_construction.unwrap_or(64) < 2 * m.unwrap_or(16) =>
			{
				Some("ef_construction must be at least twice m")
			}
			IndexType::Ivfflat { lists: Some(lists) } if !(1..=32768).contains(&lists) => {
				Some("lists must be in the range 1..=32768")
			}
			_ => None,
		};
		#[cfg(feature = "pgvector")]
		if let Some(option) = invalid_option {
			return Err(super::MigrationError::InvalidMigration(format!(
				"{option} for approximate vector indexes"
			)));
		}

		Ok(())
	}

	/// Validates operation invariants that require the migration project state.
	///
	/// Expression index result types cannot be inferred from [`ProjectState`],
	/// so PostgreSQL remains responsible for validating expression targets.
	pub fn validate_for_state(&self, state: &ProjectState) -> super::Result<()> {
		self.validate_for_state_with_unknown_table_policy(state, false)
	}

	/// Validates known migration state while deferring targets whose table history is absent.
	pub(crate) fn validate_for_partial_state(&self, state: &ProjectState) -> super::Result<()> {
		self.validate_for_state_with_unknown_table_policy(state, true)
	}

	fn validate_for_state_with_unknown_table_policy(
		&self,
		state: &ProjectState,
		allow_unknown_table: bool,
	) -> super::Result<()> {
		#[cfg(feature = "pgvector")]
		if let Self::CreateIndex {
			table,
			columns,
			index_type,
			expressions,
			..
		}
		| Self::CreateNamedIndex {
			table,
			columns,
			index_type,
			expressions,
			..
		} = self && index_type.is_some_and(IndexType::is_approximate_vector)
			&& expressions
				.as_ref()
				.is_none_or(|expressions| expressions.is_empty())
			&& columns.len() == 1
		{
			let column = &columns[0];
			let Some(model) = state.find_model_by_table(table) else {
				if allow_unknown_table {
					return Ok(());
				}
				return Err(super::MigrationError::InvalidMigration(format!(
					"approximate vector index targets unknown table `{table}`"
				)));
			};
			let field = model.fields.get(column).ok_or_else(|| {
				super::MigrationError::InvalidMigration(format!(
					"approximate vector index on table `{table}` targets unknown column `{column}`"
				))
			})?;
			if !matches!(field.field_type, FieldType::Vector { .. }) {
				return Err(super::MigrationError::InvalidMigration(format!(
					"approximate vector index on table `{table}` targets non-vector column `{column}`"
				)));
			}
		}

		#[cfg(not(feature = "pgvector"))]
		let _ = (state, allow_unknown_table);

		Ok(())
	}

	/// Validates every field definition rendered by this operation.
	pub fn validate_for_dialect(&self, dialect: &SqlDialect) -> super::Result<()> {
		#[cfg(feature = "pgvector")]
		if let Self::CreateNamedIndex { name, .. } | Self::DropNamedIndex { name, .. } = self {
			if name.is_empty() {
				return Err(super::MigrationError::InvalidMigration(
					"physical index name must not be empty".to_string(),
				));
			}
			if name.contains('\0') {
				return Err(super::MigrationError::InvalidMigration(
					"physical index name must not contain NUL".to_string(),
				));
			}
		}

		match self {
			Self::CreateTable { columns, .. } => {
				for column in columns {
					column.type_definition.try_to_sql_for_dialect(dialect)?;
				}
			}
			Self::AddColumn { column, .. } => {
				column.type_definition.try_to_sql_for_dialect(dialect)?;
			}
			Self::AlterColumn { new_definition, .. } => {
				new_definition
					.type_definition
					.try_to_sql_for_dialect(dialect)?;
			}
			Self::CreateInheritedTable { columns, .. } => {
				for column in columns {
					column.type_definition.try_to_sql_for_dialect(dialect)?;
				}
			}
			Self::CreateExtension { .. } => {
				let backend = match dialect {
					SqlDialect::Postgres => None,
					SqlDialect::Mysql => Some("mysql"),
					SqlDialect::Sqlite => Some("sqlite"),
					SqlDialect::Cockroachdb => Some("cockroachdb"),
				};
				if let Some(backend) = backend {
					return Err(super::MigrationError::UnsupportedBackendFeature {
						feature: "PostgreSQL extensions",
						backend,
					});
				}
			}
			Self::CreateIndex {
				columns,
				unique,
				index_type,
				expressions,
				operator_class,
				..
			}
			| Self::CreateIndexRepair {
				columns,
				unique,
				index_type,
				expressions,
				operator_class,
				..
			}
			| Self::RestoreIndexOnRollback {
				columns,
				unique,
				index_type,
				expressions,
				operator_class,
				..
			} => Self::validate_approximate_vector_index(
				*index_type,
				*unique,
				columns,
				expressions.as_deref(),
				operator_class.as_deref(),
				dialect,
			)?,
			#[cfg(feature = "pgvector")]
			Self::CreateNamedIndex {
				columns,
				unique,
				index_type,
				expressions,
				operator_class,
				..
			}
			| Self::DropNamedIndex {
				columns,
				unique,
				index_type,
				expressions,
				operator_class,
				..
			} => Self::validate_approximate_vector_index(
				*index_type,
				*unique,
				columns,
				expressions.as_deref(),
				operator_class.as_deref(),
				dialect,
			)?,
			_ => {}
		}
		Ok(())
	}

	/// Generate `SetAutoIncrementValue` SQL for each dialect
	///
	/// PostgreSQL / CockroachDB resolve the backing sequence via
	/// `pg_get_serial_sequence(...)` so that both the default
	/// `{table}_{column}_seq` naming and user-customized sequences work without
	/// the caller having to know the sequence name.
	fn set_auto_increment_to_sql(
		table: &str,
		column: &str,
		value: i64,
		dialect: &SqlDialect,
	) -> String {
		match dialect {
			SqlDialect::Postgres | SqlDialect::Cockroachdb => {
				// pg_get_serial_sequence takes a regclass literal for the table
				// and a text literal for the column. `setval(..., value, false)`
				// makes the NEXT generated value equal `value`, matching the
				// intent of "set the auto-increment to <value>".
				format!(
					"SELECT setval(pg_get_serial_sequence({}, {}), {}, false);",
					quote_literal(table),
					quote_literal(column),
					value
				)
			}
			SqlDialect::Mysql => {
				format!(
					"ALTER TABLE {} AUTO_INCREMENT = {};",
					quote_identifier(table),
					value
				)
			}
			SqlDialect::Sqlite => {
				// INSERT OR REPLACE so the statement works whether or not a
				// sqlite_sequence row already exists for the table. UPDATE
				// would silently no-op on fresh tables that have never had
				// a row inserted.
				format!(
					"INSERT OR REPLACE INTO sqlite_sequence(name, seq) VALUES ({}, {});",
					quote_literal(table),
					value
				)
			}
		}
	}

	/// Generate `CreateCompositePrimaryKey` SQL
	///
	/// Produces `ALTER TABLE ... ADD CONSTRAINT ... PRIMARY KEY (...)` for
	/// every supported backend. Emits guaranteed-fail SQL if the column list
	/// is empty so the migration aborts at execution time instead of silently
	/// succeeding.
	///
	/// Workaround for the shared infallible `String` return type used by every
	/// `to_sql` arm. Converting the entire pipeline to `Result` would cascade
	/// through dozens of call sites, so this arm instead emits a deliberately
	/// invalid SQL statement (a bare identifier) that every supported backend's
	/// parser rejects before execution. This replaces the earlier `SELECT 1/0`
	/// fallback, which silently returned `NULL` on SQLite and lax-mode MySQL
	/// (reinhardt-web#4325). The identifier text encodes the diagnostic so it
	/// surfaces in the parser error message.
	///
	/// The long-term fix — migrating the `to_sql` family to `Result` and
	/// returning a structured `MigrationError::EmptyCompositePrimaryKey` — is
	/// shown in the ideal implementation below.
	///
	/// Remove this workaround once the `to_sql` family is migrated to a
	/// fallible signature.
	///
	/// Ideal implementation (without workaround):
	///   fn create_composite_pk_to_sql(
	///       table: &str,
	///       columns: &[String],
	///       constraint_name: Option<&str>,
	///   ) -> Result<String, MigrationError> {
	///       if columns.is_empty() {
	///           return Err(MigrationError::EmptyCompositePrimaryKey {
	///               table: table.to_owned(),
	///           });
	///       }
	///       // ... build the ALTER TABLE statement ...
	///   }
	fn create_composite_pk_to_sql(
		table: &str,
		columns: &[String],
		constraint_name: Option<&str>,
	) -> String {
		if columns.is_empty() {
			// Deliberately invalid SQL: a bare identifier is not a valid
			// statement in PostgreSQL, MySQL, or SQLite grammar, so every
			// backend's parser rejects it before execution. This avoids the
			// lax-mode MySQL / SQLite silent-pass that the previous
			// `SELECT 1/0` fallback was prone to (reinhardt-web#4325). The
			// identifier text preserves the diagnostic in the parser error.
			return format!(
				"SYNTAX_ERROR_create_composite_pk_on_{}_requires_at_least_one_column;",
				table.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
			);
		}

		let default_name;
		let name: &str = match constraint_name {
			Some(n) => n,
			None => {
				default_name = format!("{}_pkey", table);
				&default_name
			}
		};

		let quoted_columns = columns
			.iter()
			.map(|c| quote_identifier(c).to_string())
			.collect::<Vec<_>>()
			.join(", ");

		format!(
			"ALTER TABLE {} ADD CONSTRAINT {} PRIMARY KEY ({});",
			quote_identifier(table),
			quote_identifier(name),
			quoted_columns
		)
	}

	/// Generate bulk load SQL for different dialects
	fn bulk_load_to_sql(
		table: &str,
		source: &BulkLoadSource,
		format: &BulkLoadFormat,
		options: &BulkLoadOptions,
		dialect: &SqlDialect,
	) -> String {
		match dialect {
			SqlDialect::Postgres | SqlDialect::Cockroachdb => {
				Self::postgres_copy_from_sql(table, source, format, options)
			}
			SqlDialect::Mysql => Self::mysql_load_data_sql(table, source, format, options),
			SqlDialect::Sqlite => {
				// SQLite does not support bulk loading natively
				format!(
					"-- SQLite does not support bulk loading. Use INSERT statements instead for table {}",
					quote_identifier(table)
				)
			}
		}
	}

	/// Generate PostgreSQL COPY FROM SQL
	fn postgres_copy_from_sql(
		table: &str,
		source: &BulkLoadSource,
		format: &BulkLoadFormat,
		options: &BulkLoadOptions,
	) -> String {
		let source_clause = match source {
			BulkLoadSource::File(path) => format!("'{}'", path),
			BulkLoadSource::Stdin => "STDIN".to_string(),
			BulkLoadSource::Program(cmd) => format!("PROGRAM '{}'", cmd),
		};

		let columns_clause = if let Some(cols) = &options.columns {
			let quoted_cols = cols
				.iter()
				.map(|c| quote_identifier(c))
				.collect::<Vec<_>>()
				.join(", ");
			format!(" ({})", quoted_cols)
		} else {
			String::new()
		};

		let mut with_options = Vec::new();

		// Format
		with_options.push(format!("FORMAT {}", format));

		// Delimiter
		if let Some(delim) = options.delimiter {
			with_options.push(format!("DELIMITER '{}'", delim));
		}

		// NULL string
		if let Some(null_str) = &options.null_string {
			with_options.push(format!("NULL '{}'", null_str));
		}

		// Header
		if options.header {
			with_options.push("HEADER true".to_string());
		}

		// Quote character
		if let Some(quote) = options.quote {
			with_options.push(format!("QUOTE '{}'", quote));
		}

		// Escape character
		if let Some(escape) = options.escape {
			with_options.push(format!("ESCAPE '{}'", escape));
		}

		format!(
			"COPY {}{} FROM {} WITH ({});",
			quote_identifier(table),
			columns_clause,
			source_clause,
			with_options.join(", ")
		)
	}

	/// Generate MySQL LOAD DATA SQL
	fn mysql_load_data_sql(
		table: &str,
		source: &BulkLoadSource,
		format: &BulkLoadFormat,
		options: &BulkLoadOptions,
	) -> String {
		let local_clause = if options.local { " LOCAL" } else { "" };

		let file_path = match source {
			BulkLoadSource::File(path) => path.clone(),
			BulkLoadSource::Stdin => {
				return format!(
					"-- MySQL does not support LOAD DATA from STDIN directly for table {}",
					quote_identifier(table)
				);
			}
			BulkLoadSource::Program(_) => {
				return format!(
					"-- MySQL does not support LOAD DATA from PROGRAM directly for table {}",
					quote_identifier(table)
				);
			}
		};

		let columns_clause = if let Some(cols) = &options.columns {
			let quoted_cols = cols
				.iter()
				.map(|c| quote_identifier(c))
				.collect::<Vec<_>>()
				.join(", ");
			format!(" ({})", quoted_cols)
		} else {
			String::new()
		};

		// Field terminator (delimiter)
		let delimiter = options.delimiter.unwrap_or(match format {
			BulkLoadFormat::Csv => ',',
			BulkLoadFormat::Text | BulkLoadFormat::Binary => '\t',
		});

		let mut field_options = Vec::new();
		field_options.push(format!("TERMINATED BY '{}'", delimiter));

		// Quote character for CSV
		if *format == BulkLoadFormat::Csv {
			let quote = options.quote.unwrap_or('"');
			field_options.push(format!("ENCLOSED BY '{}'", quote));
		}

		// Escape character
		if let Some(escape) = options.escape {
			field_options.push(format!("ESCAPED BY '{}'", escape));
		}

		// Line terminator
		let line_terminator = options
			.line_terminator
			.clone()
			.unwrap_or_else(|| "\\n".to_string());

		// Encoding
		let encoding_clause = if let Some(enc) = &options.encoding {
			format!(" CHARACTER SET {}", enc)
		} else {
			String::new()
		};

		// Header handling (skip first line)
		let ignore_clause = if options.header {
			" IGNORE 1 LINES"
		} else {
			""
		};

		format!(
			"LOAD DATA{} INFILE '{}'{} INTO TABLE {} FIELDS {} LINES TERMINATED BY '{}'{}{};",
			local_clause,
			file_path,
			encoding_clause,
			quote_identifier(table),
			field_options.join(" "),
			line_terminator,
			ignore_clause,
			columns_clause
		)
	}

	/// Generate reverse SQL (for rollback)
	///
	/// # Arguments
	///
	/// * `dialect` - SQL dialect for generating database-specific SQL
	/// * `project_state` - Project state for accessing model definitions (needed for DropTable, etc.)
	///
	/// # Returns
	///
	/// * `Ok(Some(stmts))` - Reverse DDL as one or more SQL statements; each element is a
	///   single statement intended to be dispatched separately through
	///   `SchemaEditor::execute()` (which is backed by sqlx Extended Query and accepts only
	///   one statement per payload). Operations that revert to a single payload (e.g.
	///   `DropTable`, `AddColumn`) return a one-element `Vec`; operations that need
	///   multiple payloads to round-trip cleanly (e.g. `AlterColumn` on PostgreSQL and
	///   CockroachDB, which split type reversion and NOT NULL restoration into two
	///   statements) return a multi-element `Vec`.
	/// * `Ok(None)` - Operation is not reversible (see Design Limitation below)
	/// * `Err(_)` - Error generating reverse SQL
	///
	/// # Design Limitation
	///
	/// Destructive operations (`DropTable`, `DropColumn`, `DropConstraint`, `AlterColumn`)
	/// require a pre-operation `ProjectState` snapshot to generate reverse SQL. When the
	/// `project_state` parameter does not contain the necessary model/column/constraint
	/// definition, this method returns `Ok(None)` instead of failing.
	///
	/// This is an intentional design decision: the migration system cannot reconstruct
	/// lost schema information. Callers must provide the state from before the operation
	/// was applied to enable proper rollback. This matches Django's migration behavior
	/// where `state_forwards` must be called before operations are reversed.
	pub fn to_reverse_sql(
		&self,
		dialect: &SqlDialect,
		project_state: &ProjectState,
	) -> super::Result<Option<Vec<String>>> {
		match self {
			Operation::CreateTable { name, .. } => Ok(Some(vec![format!(
				"DROP TABLE {};",
				quote_identifier(name)
			)])),
			Operation::AddColumn { table, column, .. } => Ok(Some(vec![format!(
				"ALTER TABLE {} DROP COLUMN {};",
				quote_identifier(table),
				quote_identifier(&column.name)
			)])),
			Operation::RunSQL { reverse_sql, .. } => {
				Ok(reverse_sql.as_ref().map(|s| vec![s.to_string()]))
			}
			Operation::RunRust { reverse_code, .. } => Ok(reverse_code.as_ref().map(|code| {
				vec![format!(
					"-- RunRust (reverse): {}",
					code.lines().next().unwrap_or("")
				)]
			})),
			// Phase 1: Simple reverse operations
			Operation::RenameTable { old_name, new_name } => Ok(Some(vec![format!(
				"ALTER TABLE {} RENAME TO {};",
				quote_identifier(new_name),
				quote_identifier(old_name)
			)])),
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => Ok(Some(vec![format!(
				"ALTER TABLE {} RENAME COLUMN {} TO {};",
				quote_identifier(table),
				quote_identifier(new_name),
				quote_identifier(old_name)
			)])),
			Operation::CreateIndex {
				table,
				columns,
				expressions,
				..
			} => {
				// Mirror the forward SQL naming convention so rollback targets
				// expression indexes as well as column indexes.
				let name_suffix = if expressions
					.as_ref()
					.is_some_and(|expressions| !expressions.is_empty())
				{
					"expr".to_string()
				} else {
					columns.join("_")
				};
				let index_name = format!("idx_{}_{}", table, name_suffix);
				// MySQL requires `DROP INDEX <name> ON <table>`; PostgreSQL/SQLite/CockroachDB
				// only need the index name. Mirror the dialect dispatch used by the forward
				// `Operation::DropIndex` SQL generator above.
				let sql = match dialect {
					SqlDialect::Mysql => format!(
						"DROP INDEX {} ON {};",
						quote_identifier(&index_name),
						quote_identifier(table)
					),
					SqlDialect::Postgres | SqlDialect::Sqlite | SqlDialect::Cockroachdb => {
						format!("DROP INDEX {};", quote_identifier(&index_name))
					}
				};
				Ok(Some(vec![sql]))
			}
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex { table, name, .. } => {
				let sql = match dialect {
					SqlDialect::Mysql => format!(
						"DROP INDEX {} ON {};",
						quote_identifier(name),
						quote_identifier(table)
					),
					SqlDialect::Postgres | SqlDialect::Sqlite | SqlDialect::Cockroachdb => {
						format!("DROP INDEX {};", quote_identifier(name))
					}
				};
				Ok(Some(vec![sql]))
			}
			Operation::CreateIndexRepair { .. } => Ok(None),
			Operation::RestoreIndexOnRollback {
				table,
				name,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				let sql = Operation::CreateIndexRepair {
					table: table.clone(),
					name: name.clone(),
					columns: columns.clone(),
					unique: *unique,
					index_type: *index_type,
					where_clause: where_clause.clone(),
					concurrently: *concurrently,
					expressions: expressions.clone(),
					mysql_options: *mysql_options,
					operator_class: operator_class.clone(),
				}
				.try_to_sql(dialect)?;
				Ok(Some(vec![sql]))
			}
			Operation::AddConstraint {
				table,
				constraint_sql,
			} => {
				// Extract constraint name from SQL
				// Expects format: "CONSTRAINT <name> ..." or "ADD CONSTRAINT <name> ..."
				let constraint_name =
					Self::extract_constraint_name(constraint_sql).ok_or_else(|| {
						super::MigrationError::InvalidMigration(format!(
							"Cannot extract constraint name from: {}",
							constraint_sql
						))
					})?;
				Ok(Some(vec![format!(
					"ALTER TABLE {} DROP CONSTRAINT {};",
					quote_identifier(table),
					quote_identifier(&constraint_name)
				)]))
			}
			Operation::AddConstraintDefinition { table, constraint } => Ok(Some(vec![format!(
				"ALTER TABLE {} DROP CONSTRAINT {};",
				Self::quote_schema_identifier(table, dialect),
				Self::quote_dialect_identifier(constraint.name(), dialect)
			)])),
			Operation::AddConstraintRepair { .. } => Ok(None),
			Operation::RestoreConstraintOnRollback {
				table,
				constraint_sql,
			} => Ok(Some(vec![
				Operation::AddConstraint {
					table: table.clone(),
					constraint_sql: constraint_sql.clone(),
				}
				.to_sql(dialect),
			])),
			// Phase 2: Complex reverse operations using ProjectState
			Operation::DropColumn {
				table,
				column,
				old_definition,
			} => {
				// Prefer the explicit definition carried by replacement-style
				// migrations; fall back to ProjectState for legacy migrations.
				let resolved_old_def = old_definition.clone().or_else(|| {
					project_state
						.find_model_by_table(table)
						.and_then(|model| model.get_field(column))
						.map(|field| ColumnDefinition::from_field_state(column.clone(), field))
				});

				let Some(old_def) = resolved_old_def else {
					// Cannot reconstruct without state
					return Ok(None);
				};

				let col_sql = Self::column_to_sql(&old_def, dialect);
				Ok(Some(vec![format!(
					"ALTER TABLE {} ADD COLUMN {};",
					quote_identifier(table),
					col_sql
				)]))
			}
			Operation::AlterColumn {
				table,
				column,
				old_definition,
				new_definition: _,
				..
			} => {
				// Resolve the original column definition to revert to.
				// Prioritize the explicit `old_definition` over ProjectState lookup.
				let resolved_old_def = old_definition.clone().or_else(|| {
					project_state
						.find_model_by_table(table)
						.and_then(|model| model.get_field(column))
						.map(|field| ColumnDefinition::from_field_state(column.clone(), field))
				});

				let Some(old_def) = resolved_old_def else {
					// Cannot reconstruct without state
					return Ok(None);
				};

				let type_sql = old_def.type_definition.to_sql_for_dialect(dialect);
				let null_clause = if old_def.not_null { " NOT NULL" } else { "" };

				// Dispatch reverse SQL per dialect.
				// SQLite is handled via the SQLite-recreation path before reaching
				// here (see executor::rollback_migration and
				// `Operation::reverse_requires_sqlite_recreation`). The placeholder
				// comment below is a defensive fallback: emitting executable
				// ALTER COLUMN syntax on SQLite would always error (#4582).
				let stmts = match dialect {
					SqlDialect::Postgres | SqlDialect::Cockroachdb => {
						// Emit type reversion and nullability restoration as two
						// independent single-statement payloads. The executor
						// (see `MigrationExecutor::rollback_migration`) iterates
						// the returned `Vec<String>` and dispatches each through
						// `SchemaEditor::execute()` — backed by sqlx Extended
						// Query, which accepts only one statement per payload.
						//
						// This shape is required by CockroachDB, which rejects
						// the comma-combined `ALTER TABLE ... ALTER COLUMN c
						// TYPE T, ALTER COLUMN c {SET|DROP} NOT NULL` form that
						// PostgreSQL accepts. Unifying both dialects on the
						// multi-statement path keeps the contract uniform and
						// restores NOT NULL rollback fidelity on Cockroach.
						// Refs #4630, #4640.
						let nullability_clause = if old_def.not_null {
							"SET NOT NULL"
						} else {
							"DROP NOT NULL"
						};
						vec![
							format!(
								"ALTER TABLE {table} ALTER COLUMN {column} TYPE {type_sql};",
								table = quote_identifier(table),
								column = quote_identifier(column),
								type_sql = type_sql,
							),
							format!(
								"ALTER TABLE {table} ALTER COLUMN {column} {nullability_clause};",
								table = quote_identifier(table),
								column = quote_identifier(column),
								nullability_clause = nullability_clause,
							),
						]
					}
					SqlDialect::Mysql => vec![format!(
						"ALTER TABLE {} MODIFY COLUMN {} {}{};",
						quote_identifier(table),
						quote_identifier(column),
						type_sql,
						null_clause
					)],
					SqlDialect::Sqlite => vec![format!(
						"-- SQLite does not support ALTER COLUMN, table recreation required for {}",
						quote_identifier(table)
					)],
				};
				Ok(Some(stmts))
			}
			Operation::DropIndex { table, columns } => {
				// Enhancement opportunity: Full index reconstruction would preserve
				// index_type, where_clause, operator_class, and other advanced properties.
				// The current implementation generates a basic CREATE INDEX statement.
				let columns_joined = columns.join("_");
				let index_name = format!("idx_{}_{}", table, columns_joined);
				let columns_list = columns
					.iter()
					.map(|c| quote_identifier(c).to_string())
					.collect::<Vec<_>>()
					.join(", ");
				Ok(Some(vec![format!(
					"CREATE INDEX {} ON {} ({});",
					quote_identifier(&index_name),
					quote_identifier(table),
					columns_list
				)]))
			}
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				if !named_index_has_target(columns, expressions.as_deref()) {
					return Ok(None);
				}
				Ok(Some(vec![
					Operation::CreateNamedIndex {
						table: table.clone(),
						name: name.clone(),
						columns: columns.clone(),
						unique: *unique,
						index_type: *index_type,
						where_clause: where_clause.clone(),
						concurrently: *concurrently,
						expressions: expressions.clone(),
						mysql_options: *mysql_options,
						operator_class: operator_class.clone(),
					}
					.try_to_sql(dialect)?,
				]))
			}
			Operation::DropConstraint {
				table,
				constraint_name,
			} => {
				// Retrieve constraint definition from ProjectState
				if let Some(model) = project_state.find_model_by_table(table)
					&& let Some(constraint_def) = model
						.constraints
						.iter()
						.find(|c| c.name == *constraint_name)
				{
					let constraint = constraint_def.to_constraint();
					return Ok(Some(vec![match constraint {
						Constraint::EnumDomain { .. } => Operation::AddConstraintDefinition {
							table: table.clone(),
							constraint,
						}
						.to_sql(dialect),
						_ => format!(
							"ALTER TABLE {} ADD {};",
							quote_identifier(table),
							constraint
						),
					}]));
				}
				// Cannot reconstruct without state
				Ok(None)
			}
			Operation::DropConstraintDefinition { table, constraint } => Ok(Some(vec![
				Operation::AddConstraintDefinition {
					table: table.clone(),
					constraint: constraint.clone(),
				}
				.to_sql(dialect),
			])),
			Operation::DropTable { name } => {
				// Retrieve table definition from ProjectState and reconstruct CREATE TABLE
				if let Some(model) = project_state.find_model_by_table(name) {
					let mut parts = Vec::new();

					// Convert fields to column definitions
					for (field_name, field) in
						Self::order_model_fields_by_generated_dependencies(model)
					{
						let col_def = ColumnDefinition::from_field_state(field_name.clone(), field);
						parts.push(format!("  {}", Self::column_to_sql(&col_def, dialect)));
					}

					// Add constraints
					for constraint_def in &model.constraints {
						let constraint = constraint_def.to_constraint();
						parts.push(format!("  {}", constraint.to_sql_for_dialect(dialect)));
					}

					return Ok(Some(vec![format!(
						"CREATE TABLE {} (\n{}\n);",
						quote_identifier(name),
						parts.join(",\n")
					)]));
				}
				// Cannot reconstruct without state
				Ok(None)
			}
			Operation::BulkLoad { table, .. } => {
				// Reverse of bulk load is to truncate the table (remove loaded data)
				// Note: This removes ALL data, not just the data loaded by this operation
				Ok(Some(vec![format!(
					"TRUNCATE TABLE {};",
					quote_identifier(table)
				)]))
			}
			_ => Ok(None),
		}
	}

	/// Apply operation to project state (backward/reverse)
	///
	/// This method updates the ProjectState to reflect the reverse of this operation.
	/// Used during migration rollback to track state changes.
	///
	/// # Arguments
	///
	/// * `app_label` - Application label for the model being modified
	/// * `state` - Mutable reference to the ProjectState to update
	///
	/// # Limitations
	///
	/// Some operations cannot fully reverse state without additional snapshot information:
	/// - `DropTable`: Cannot recreate model structure (columns, constraints) without snapshot
	/// - `DropColumn`: Cannot recreate column definition without snapshot
	/// - `AlterColumn`: Cannot restore original column definition without snapshot
	///
	/// For these operations, use `to_reverse_sql` with ProjectState before the operation
	/// is applied to generate proper reverse SQL.
	pub fn state_backwards(&self, app_label: &str, state: &mut ProjectState) {
		match self {
			Operation::CreateTable { name, .. } => {
				// Reverse: Remove the model from state
				state
					.models
					.remove(&(app_label.to_string(), name.to_string()));
			}
			Operation::DropTable { name: _ } => {
				// Cannot reconstruct ModelState without snapshot.
				// For proper rollback, use to_reverse_sql with pre-operation ProjectState.
			}
			Operation::RenameTable { old_name, new_name } => {
				state.rename_table_in_app(app_label, new_name, old_name);
			}
			Operation::AddColumn { table, column, .. } => {
				// Reverse: Remove the column from the model
				if let Some(model) = state.find_model_by_table_mut(table) {
					model.remove_field(&column.name);
				}
			}
			Operation::DropColumn {
				table: _,
				column: _,
				..
			} => {
				// Cannot reconstruct column definition without snapshot.
				// For proper rollback, use to_reverse_sql with pre-operation ProjectState.
			}
			Operation::AlterColumn {
				table: _,
				column: _,
				..
			} => {
				// Cannot restore original column definition without snapshot.
				// For proper rollback, use to_reverse_sql with pre-operation ProjectState.
			}
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => {
				// Reverse: Rename field back from new_name to old_name
				if let Some(model) = state.find_model_by_table_mut(table) {
					model.rename_field(new_name, old_name.to_string());
				}
			}
			Operation::AddConstraint { table, .. } => {
				// Reverse: Would need to remove the constraint
				// This requires parsing constraint_sql to get the name
				if let Some(model) = state.find_model_by_table_mut(table) {
					// Cannot reliably remove without constraint name extraction
					// Constraints vector remains unchanged
					let _ = model;
				}
			}
			Operation::AddConstraintDefinition { table, constraint } => {
				if let Some(model) = state.find_model_by_table_mut(table) {
					if let Constraint::EnumDomain { column, .. } = constraint
						&& let Some(field) = model.fields.get_mut(column)
					{
						field.domain = None;
					}
					model
						.constraints
						.retain(|definition| definition.name != constraint.name());
				}
			}
			Operation::DropConstraint {
				table: _,
				constraint_name: _,
			} => {}
			Operation::DropConstraintDefinition { table, constraint } => {
				if let Some(model) = state.find_model_by_table_mut(table) {
					let definition = ProjectState::constraint_to_definition(constraint);
					if !model
						.constraints
						.iter()
						.any(|item| item.name == constraint.name())
					{
						if let Constraint::EnumDomain { column, domain, .. } = constraint
							&& let Some(field) = model.fields.get_mut(column)
						{
							field.domain = Some(domain.clone().canonicalized());
						}
						model.constraints.push(definition);
					}
				}
			}
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex { table, name, .. } => {
				if let Some(model) = state.find_model_by_table_mut(table) {
					model.indexes.retain(|index| index.name != *name);
				}
			}
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				expressions,
				operator_class,
				..
			} => {
				if !named_index_has_target(columns, expressions.as_deref()) {
					return;
				}
				if let Some(model) = state.find_model_by_table_mut(table) {
					#[cfg(not(feature = "pgvector"))]
					let _ = (index_type, operator_class);
					model.indexes.retain(|index| index.name != *name);
					model.indexes.push(IndexDefinition {
						name: name.clone(),
						fields: columns.clone(),
						unique: *unique,
						#[cfg(feature = "pgvector")]
						index_type: *index_type,
						#[cfg(feature = "pgvector")]
						operator_class: operator_class.clone(),
						#[cfg(feature = "pgvector")]
						expressions: expressions.clone(),
					});
				}
			}
			_ => {
				// Other operations don't affect schema state
			}
		}
	}

	/// Extract constraint name from constraint SQL
	///
	/// Supports patterns:
	/// - "CONSTRAINT name CHECK ..."
	/// - "ADD CONSTRAINT name ..."
	fn extract_constraint_name(constraint_sql: &str) -> Option<String> {
		let sql = constraint_sql.trim();

		// Pattern 1: "CONSTRAINT name ..."
		if sql.starts_with("CONSTRAINT ") || sql.contains(" CONSTRAINT ") {
			let parts: Vec<&str> = sql.split_whitespace().collect();
			if let Some(pos) = parts.iter().position(|&s| s == "CONSTRAINT")
				&& pos + 1 < parts.len()
			{
				return Some(parts[pos + 1].to_string());
			}
		}

		None
	}
}

/// Column definition for legacy operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColumnDefinition {
	/// The name.
	pub name: String,
	/// The type definition.
	pub type_definition: FieldType,
	#[serde(default)]
	/// The not null.
	pub not_null: bool,
	#[serde(default)]
	/// The unique.
	pub unique: bool,
	#[serde(default)]
	/// The primary key.
	pub primary_key: bool,
	#[serde(default)]
	/// The auto increment.
	pub auto_increment: bool,
	#[serde(default)]
	/// The default.
	pub default: Option<String>,
	#[serde(default)]
	/// Generated-column metadata.
	pub generated: Option<GeneratedColumnDefinition>,
	/// Structured domain constraints for this column.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub domain: Option<crate::field_domain::FieldDomain>,
}

impl ColumnDefinition {
	/// Create a new column definition
	pub fn new(name: impl Into<String>, type_def: FieldType) -> Self {
		Self {
			name: name.into(),
			type_definition: type_def,
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			generated: None,
			domain: None,
		}
	}

	/// Sets structured column-domain metadata and returns self for chaining.
	pub fn with_domain(mut self, domain: crate::field_domain::FieldDomain) -> Self {
		self.domain = Some(domain.canonicalized());
		self
	}

	/// Create a ColumnDefinition from FieldState with attribute parsing
	///
	/// Reads boolean attributes (primary_key, unique, auto_increment) and the
	/// default expression from `FieldState.params`, and derives the NOT NULL
	/// constraint from `FieldState.nullable` (the single source of truth
	/// populated by `FieldMetadata::is_nullable()` in
	/// `ModelMetadata::to_model_state()`).
	///
	/// # Arguments
	///
	/// * `name` - Column name
	/// * `field_state` - FieldState containing field metadata and params
	///
	/// # Notes
	///
	/// - If `primary_key` is true, `not_null` is forced to true regardless of
	///   `FieldState.nullable` — primary keys cannot accept NULL.
	/// - Default values are false/None for unspecified attributes
	pub fn from_field_state(name: impl Into<String>, field_state: &FieldState) -> Self {
		let name_str = name.into();
		let params = &field_state.params;

		// Parse attributes from params HashMap
		let primary_key = params
			.get("primary_key")
			.and_then(|v| v.parse::<bool>().ok())
			.unwrap_or(false);

		// Derive `not_null` from FieldState's nullability field (single source
		// of truth set in `ModelMetadata::to_model_state()` from
		// `FieldMetadata::is_nullable()`). Primary keys are always NOT NULL
		// regardless of the nullability flag.
		//
		// Fixes #4573: the previous implementation derived `not_null` from a
		// `params["not_null"]` key, which the `#[model]` proc-macro only
		// emits conditionally (when its `is_not_null` calculation returns
		// `true`). The same macro also always emits `params["null"]`, which
		// `is_nullable()` reads into `field_state.nullable`. Keeping the two
		// keys as parallel sources of truth for nullability was the root
		// cause of the drift: any code path that bypassed or mis-routed the
		// `not_null` emission (e.g., offline state reconstruction, future
		// macro refactors, or hand-built `FieldState` values in tests) would
		// produce NULLABLE columns for non-Optional fields. Consolidating on
		// `field_state.nullable` removes that class of regression.
		let not_null = !field_state.nullable || primary_key;

		let unique = params
			.get("unique")
			.and_then(|v| v.parse::<bool>().ok())
			.unwrap_or(false);

		let auto_increment = params
			.get("auto_increment")
			.and_then(|v| v.parse::<bool>().ok())
			.unwrap_or(false);

		let default = params.get("default").cloned();
		let generated = field_state.generated.clone();

		// Resolve ForeignKey column type from the referenced model's primary
		// key in the global `ModelRegistry`. This addresses the macro-level
		// limitation that the target model's PK type is not knowable at
		// macro-expansion time (see issue #4430). The macro emits a
		// placeholder `FieldType::Uuid` for `ForeignKeyField<T>` `_id`
		// columns and tags the field with the `fk_target` parameter; here
		// we look up the referenced model and adopt its PK column type.
		//
		// If the lookup fails (e.g., the target model has not been
		// registered yet), we fall back to the placeholder field type so
		// existing behavior is preserved and the caller can surface a
		// downstream error rather than crash here.
		let type_definition = resolve_foreign_key_column_type(field_state)
			.unwrap_or_else(|| field_state.field_type.clone());

		Self {
			name: name_str,
			type_definition,
			not_null,
			unique,
			primary_key,
			auto_increment,
			default,
			generated,
			domain: field_state.domain.clone(),
		}
	}
}

pub(crate) fn truncate_identifier_with_hash(logical_name: &str) -> String {
	const MAX_IDENTIFIER_LENGTH: usize = 63;
	const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
	const FNV_PRIME: u64 = 0x00000100000001b3;
	if logical_name.len() <= MAX_IDENTIFIER_LENGTH {
		return logical_name.to_string();
	}

	let hash = logical_name
		.as_bytes()
		.iter()
		.fold(FNV_OFFSET_BASIS, |hash, byte| {
			(hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
		});
	let hash = format!("{hash:016x}");
	let prefix_len = MAX_IDENTIFIER_LENGTH - hash.len() - 1;
	let boundary = logical_name
		.char_indices()
		.map(|(index, _)| index)
		.take_while(|index| *index <= prefix_len)
		.last()
		.unwrap_or(0);
	format!("{}_{}", &logical_name[..boundary], hash)
}

/// Generated-column metadata for migration operations.
#[derive(Debug, Clone)]
pub struct GeneratedColumnDefinition {
	/// Runtime typed generated expression.
	pub expr: Option<Box<SchemaExpr>>,
	/// Canonical Rust tokens used when writing migration files.
	pub expr_tokens: Option<String>,
	/// Explicit backend-specific raw SQL body.
	pub raw_sql: Option<String>,
	/// Generated-column storage mode.
	pub storage: GeneratedStorage,
}

impl PartialEq for GeneratedColumnDefinition {
	fn eq(&self, other: &Self) -> bool {
		self.eq_with_dialect(other, None)
	}
}

impl GeneratedColumnDefinition {
	pub(crate) fn eq_for_dialect(&self, other: &Self, dialect: &SqlDialect) -> bool {
		self.eq_with_dialect(other, Some(dialect))
	}

	fn eq_with_dialect(&self, other: &Self, dialect: Option<&SqlDialect>) -> bool {
		if self.storage != other.storage {
			return false;
		}

		match (self.typed_expr(), other.typed_expr()) {
			(Some(left), Some(right)) => left == right,
			(Some(left), None) => other
				.raw_sql
				.as_deref()
				.is_some_and(|raw_sql| Self::typed_expr_matches_raw_sql(&left, raw_sql, dialect)),
			(None, Some(right)) => self
				.raw_sql
				.as_deref()
				.is_some_and(|raw_sql| Self::typed_expr_matches_raw_sql(&right, raw_sql, dialect)),
			(None, None) => match (self.raw_sql.as_deref(), other.raw_sql.as_deref()) {
				(Some(left), Some(right)) => Self::raw_sql_matches(left, right),
				_ => self.expr_tokens == other.expr_tokens && self.expr == other.expr,
			},
		}
	}

	/// Create a typed generated-column definition.
	pub fn typed(
		expr: SchemaExpr,
		expr_tokens: impl Into<String>,
		storage: GeneratedStorage,
	) -> Self {
		Self {
			expr: Some(Box::new(expr)),
			expr_tokens: Some(expr_tokens.into()),
			raw_sql: None,
			storage,
		}
	}

	/// Create generated-column metadata from reconstructable expression tokens.
	///
	/// This is used by metadata-driven schema builders when the derive macro
	/// records the expression tokens but cannot construct a runtime `SchemaExpr`.
	pub fn tokens(expr_tokens: impl Into<String>, storage: GeneratedStorage) -> Self {
		Self {
			expr: None,
			expr_tokens: Some(expr_tokens.into()),
			raw_sql: None,
			storage,
		}
	}

	pub(crate) fn typed_expr(&self) -> Option<SchemaExpr> {
		self.expr.as_deref().cloned().or_else(|| {
			self.expr_tokens
				.as_deref()
				.and_then(super::ast_parser::parse_schema_expr_tokens)
		})
	}

	fn raw_sql_matches(left: &str, right: &str) -> bool {
		normalize_generated_expression_sql(left) == normalize_generated_expression_sql(right)
	}

	fn typed_expr_matches_raw_sql(
		expr: &SchemaExpr,
		raw_sql: &str,
		dialect: Option<&SqlDialect>,
	) -> bool {
		let normalized_raw_sql = normalize_generated_expression_sql(raw_sql);
		if let Some(dialect) = dialect {
			return normalize_generated_expression_sql(&Operation::schema_expr_to_sql(
				expr, dialect,
			)) == normalized_raw_sql;
		}

		[
			SqlDialect::Postgres,
			SqlDialect::Sqlite,
			SqlDialect::Mysql,
			SqlDialect::Cockroachdb,
		]
		.iter()
		.any(|dialect| {
			normalize_generated_expression_sql(&Operation::schema_expr_to_sql(expr, dialect))
				== normalized_raw_sql
		})
	}

	fn rehydrate_expr_from_tokens(&mut self) {
		if self.expr.is_none() {
			self.expr = self.typed_expr().map(Box::new);
		}
	}

	/// Create an explicit raw-SQL generated-column definition.
	pub fn raw_sql(sql: impl Into<String>, storage: GeneratedStorage) -> Self {
		Self {
			expr: None,
			expr_tokens: None,
			raw_sql: Some(sql.into()),
			storage,
		}
	}

	/// Convert to the query-layer generated-column metadata.
	pub fn to_query_generated(&self) -> Option<GeneratedColumn> {
		if let Some(expr) = self.typed_expr() {
			Some(GeneratedColumn::typed(expr, self.storage))
		} else {
			self.raw_sql
				.as_ref()
				.map(|sql| GeneratedColumn::raw_sql(sql.clone(), self.storage))
		}
	}
}

fn normalize_generated_expression_sql(sql: &str) -> String {
	let sql = strip_balanced_outer_parens(sql.trim());
	let mut normalized = String::new();
	let mut chars = sql.chars().peekable();
	let mut in_string = false;

	while let Some(ch) = chars.next() {
		if in_string {
			normalized.push(ch);
			if ch == '\'' {
				if matches!(chars.peek().copied(), Some('\'')) {
					if let Some(escaped_quote) = chars.next() {
						normalized.push(escaped_quote);
					}
				} else {
					in_string = false;
				}
			}
			continue;
		}

		match ch {
			'\'' => {
				in_string = true;
				normalized.push(ch);
			}
			'"' | '`' => {}
			character if character.is_ascii_whitespace() => {}
			character => normalized.push(character.to_ascii_lowercase()),
		}
	}

	normalized
}

fn strip_balanced_outer_parens(mut sql: &str) -> &str {
	loop {
		let trimmed = sql.trim();
		if trimmed.len() < 2 || !trimmed.starts_with('(') || !trimmed.ends_with(')') {
			return trimmed;
		}
		if !outer_parens_wrap_expression(trimmed) {
			return trimmed;
		}
		sql = &trimmed[1..trimmed.len() - 1];
	}
}

fn outer_parens_wrap_expression(sql: &str) -> bool {
	let mut depth = 0usize;
	let mut in_string = false;
	let mut chars = sql.char_indices().peekable();

	while let Some((index, ch)) = chars.next() {
		if in_string {
			if ch == '\'' {
				if chars
					.peek()
					.is_some_and(|(_, escaped_quote)| *escaped_quote == '\'')
				{
					chars.next();
				} else {
					in_string = false;
				}
			}
			continue;
		}

		match ch {
			'\'' => in_string = true,
			'(' => depth += 1,
			')' => {
				depth = depth.saturating_sub(1);
				if depth == 0 && index < sql.len() - 1 {
					return false;
				}
			}
			_ => {}
		}
	}

	depth == 0
}

impl Serialize for GeneratedColumnDefinition {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let mut state = serializer.serialize_struct("GeneratedColumnDefinition", 3)?;
		state.serialize_field("expr_tokens", &self.expr_tokens)?;
		state.serialize_field("raw_sql", &self.raw_sql)?;
		let storage = match self.storage {
			GeneratedStorage::Stored => "stored",
			GeneratedStorage::Virtual => "virtual",
			_ => "stored",
		};
		state.serialize_field("storage", storage)?;
		state.end()
	}
}

impl<'de> Deserialize<'de> for GeneratedColumnDefinition {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		#[derive(Deserialize)]
		struct Helper {
			#[serde(default)]
			expr_tokens: Option<String>,
			#[serde(default)]
			raw_sql: Option<String>,
			storage: String,
		}

		let helper = Helper::deserialize(deserializer)?;
		let storage = match helper.storage.as_str() {
			"stored" | "Stored" | "STORED" => GeneratedStorage::Stored,
			"virtual" | "Virtual" | "VIRTUAL" => GeneratedStorage::Virtual,
			_ => GeneratedStorage::Stored,
		};

		let mut definition = Self {
			expr: None,
			expr_tokens: helper.expr_tokens,
			raw_sql: helper.raw_sql,
			storage,
		};
		definition.rehydrate_expr_from_tokens();
		Ok(definition)
	}
}

/// Resolve the column type of a `ForeignKeyField<T>` `_id` column by
/// looking up the target model's primary key in the global
/// `ModelRegistry`. Returns `None` if `field_state` is not tagged as a
/// foreign key column or if the target model / its PK cannot be
/// resolved.
///
/// This indirection exists because the `#[model]` macro cannot resolve
/// the target model's PK type at macro-expansion time (the registry is
/// populated at process startup via `#[ctor::ctor]`). See issue #4430.
///
/// # Lookup Strategy
///
/// The resolver coordinates two lookup paths against the
/// `ModelRegistry`:
///
/// 1. **Qualified `(fk_target_app, fk_target)` lookup.** The
///    `#[model]` macro emits `fk_target_app` for every
///    `ForeignKeyField<T>` field by reading the target type's *own*
///    `<T as Model>::app_label()` at registration time. That value is
///    authoritative — it respects `#[app_label = "..."]` overrides
///    and matches whatever key the target was registered under,
///    regardless of how the user spelled the type (bare ident,
///    `use`-imported ident, absolute path, or crate-relative path).
///    The qualified lookup is therefore trusted as the primary
///    resolution path.
/// 2. **By-name lookup.** Used as a defensive fallback for cases
///    where `fk_target_app` is absent (e.g. manually-constructed
///    `FieldState` outside the macro path) or the qualified lookup
///    misses (e.g. the target model isn't registered yet during
///    partial registry population at startup). The by-name lookup
///    returns `Some` only when *exactly one* model is registered
///    under the name; on ambiguity it returns `None`.
///
/// When both paths return `None` and the name is ambiguous across two
/// or more apps (`ModelRegistry::count_models_by_name > 1`), the
/// resolver emits a `tracing::warn!` so operators see a targeted
/// diagnostic. A genuinely missing name returns `None` silently —
/// that case is normal during partial registry population at startup.
///
/// See issue #4436 and PR #4440 review threads on `model_derive.rs`
/// line 2863 and `operations.rs` line 2836.
fn resolve_foreign_key_column_type(field_state: &FieldState) -> Option<FieldType> {
	resolve_foreign_key_column_type_with(field_state, super::model_registry::global_registry())
}

/// Registry-injected variant of [`resolve_foreign_key_column_type`].
///
/// Exists so unit tests can exercise the qualified-hit / by-name
/// fallback / ambiguous-miss branches against a local
/// [`super::model_registry::ModelRegistry`] without touching global
/// state. Production code paths go through
/// [`resolve_foreign_key_column_type`].
fn resolve_foreign_key_column_type_with(
	field_state: &FieldState,
	registry: &super::model_registry::ModelRegistry,
) -> Option<FieldType> {
	let target_model = field_state.params.get("fk_target")?;
	// `fk_target_app` is sourced from the target type's own
	// `Model::app_label()` (see `model_derive.rs`), so the qualified
	// lookup is authoritative. The by-name fallback is defensive: it
	// covers manually-constructed `FieldState`s and partial-registry
	// init races where the target isn't registered yet.
	let target = match field_state.params.get("fk_target_app") {
		Some(app) => registry
			.find_model_qualified(app, target_model)
			.or_else(|| registry.find_model_by_name(target_model)),
		None => registry.find_model_by_name(target_model),
	};
	let target = match target {
		Some(t) => t,
		None => {
			// `find_model_by_name` returns `None` for both "missing"
			// and "ambiguous". Warn only on ambiguity so operators see
			// a targeted message; silent on genuinely missing targets
			// (normal during partial registry population at startup).
			if registry.count_models_by_name(target_model) > 1 {
				tracing::warn!(
					model_name = %target_model,
					fk_target_app = ?field_state.params.get("fk_target_app"),
					"FK target name is ambiguous across apps and the qualified \
					 lookup did not resolve a unique target. Refusing to resolve \
					 to avoid silent wrong-target resolution. Ensure the FK \
					 target type is registered and that its `Model::app_label()` \
					 matches one of the registered apps.",
				);
			}
			return None;
		}
	};
	// Find the primary key field of the target model.
	let pk_field = target
		.fields
		.values()
		.find(|f| f.params.get("primary_key").map(String::as_str) == Some("true"))?;
	Some(pk_field.field_type.clone())
}

/// Convert a field type string (e.g., "reinhardt.orm.models.CharField") to FieldType.
///
/// This function parses the field type path generated by the `#[model(...)]` macro
/// and converts it to the corresponding `FieldType` enum variant.
///
/// # Arguments
///
/// * `field_type` - The field type path string (e.g., "reinhardt.orm.models.CharField")
/// * `attributes` - Field attributes containing parameters like max_length, max_digits, etc.
///
/// # Returns
///
/// * `Ok(FieldType)` - The converted FieldType
/// * `Err(String)` - Error message if the field type is unsupported
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::operations::field_type_string_to_field_type;
/// use std::collections::HashMap;
///
/// let mut attrs = HashMap::new();
/// attrs.insert("max_length".to_string(), "100".to_string());
///
/// let field_type = field_type_string_to_field_type("reinhardt.orm.models.CharField", &attrs);
/// assert!(field_type.is_ok());
/// ```
pub fn field_type_string_to_field_type(
	field_type: &str,
	attributes: &std::collections::HashMap<String, String>,
) -> Result<FieldType, String> {
	// Extract the type name from the full path
	let type_name = field_type.split('.').next_back().unwrap_or(field_type);

	match type_name {
		// Integer types
		"IntegerField"
		| "PositiveIntegerField"
		| "SmallIntegerField"
		| "PositiveSmallIntegerField" => Ok(FieldType::Integer),
		"BigIntegerField" | "PositiveBigIntegerField" => Ok(FieldType::BigInteger),
		"AutoField" => Ok(FieldType::Integer),
		"BigAutoField" => Ok(FieldType::BigInteger),
		"SmallAutoField" => Ok(FieldType::SmallInteger),

		// String types
		"CharField" => {
			let max_length = attributes
				.get("max_length")
				.and_then(|v| v.parse::<u32>().ok())
				.ok_or_else(|| "CharField requires max_length attribute".to_string())?;
			Ok(FieldType::VarChar(max_length))
		}
		"TextField" => Ok(FieldType::Text),
		"SlugField" => {
			let max_length = attributes
				.get("max_length")
				.and_then(|v| v.parse::<u32>().ok())
				.unwrap_or(50);
			Ok(FieldType::VarChar(max_length))
		}
		"EmailField" => {
			let max_length = attributes
				.get("max_length")
				.and_then(|v| v.parse::<u32>().ok())
				.unwrap_or(254);
			Ok(FieldType::VarChar(max_length))
		}
		"URLField" => {
			let max_length = attributes
				.get("max_length")
				.and_then(|v| v.parse::<u32>().ok())
				.unwrap_or(200);
			Ok(FieldType::VarChar(max_length))
		}

		// Boolean type
		"BooleanField" => Ok(FieldType::Boolean),
		"NullBooleanField" => Ok(FieldType::Boolean),

		// Date/time types
		"DateField" => Ok(FieldType::Date),
		"TimeField" => Ok(FieldType::Time),
		"DateTimeField" => Ok(FieldType::DateTime),
		"DurationField" => Ok(FieldType::BigInteger), // Stored as microseconds

		// Numeric types
		"FloatField" => Ok(FieldType::Float),
		"DecimalField" => {
			let precision = attributes
				.get("max_digits")
				.and_then(|v| v.parse::<u32>().ok())
				.unwrap_or(10);
			let scale = attributes
				.get("decimal_places")
				.and_then(|v| v.parse::<u32>().ok())
				.unwrap_or(2);
			Ok(FieldType::Decimal { precision, scale })
		}

		// Binary types
		"BinaryField" => Ok(FieldType::Binary),

		// UUID type
		"UUIDField" => Ok(FieldType::Uuid),

		// JSON types
		"JSONField" => Ok(FieldType::Json),
		"JsonField" => Ok(FieldType::Jsonb),

		// File fields (stored as path strings)
		"FileField" | "ImageField" => {
			let max_length = attributes
				.get("max_length")
				.and_then(|v| v.parse::<u32>().ok())
				.unwrap_or(100);
			Ok(FieldType::VarChar(max_length))
		}

		// IP Address fields
		"GenericIPAddressField" | "IPAddressField" => {
			// PostgreSQL uses INET, others use VARCHAR
			Ok(FieldType::VarChar(39)) // Max length for IPv6
		}

		// Relationship fields (stored as foreign key reference)
		"ForeignKey" => {
			// ForeignKey is typically stored as integer ID
			Ok(FieldType::BigInteger)
		}
		"OneToOneField" => Ok(FieldType::BigInteger),

		// Unknown type
		other => Err(format!("Unsupported field type: {}", other)),
	}
}

/// SQL dialect for generating database-specific SQL
#[derive(Debug, Clone, Copy)]
pub enum SqlDialect {
	/// Sqlite variant.
	Sqlite,
	/// Postgres variant.
	Postgres,
	/// Mysql variant.
	Mysql,
	/// Cockroachdb variant.
	Cockroachdb,
}

// ============================================================================
// SQLite Table Recreation Support
// ============================================================================

/// Represents a SQLite table recreation operation
///
/// SQLite has limited ALTER TABLE support - operations like DROP COLUMN,
/// ALTER COLUMN TYPE, and constraint modifications require recreating the table.
///
/// This struct generates the 4-step SQL pattern:
/// 1. CREATE TABLE temp_table (with new schema)
/// 2. INSERT INTO temp_table SELECT columns FROM old_table
/// 3. DROP TABLE old_table
/// 4. ALTER TABLE temp_table RENAME TO old_table
///
/// This type is integrated into `DatabaseMigrationExecutor` which automatically
/// detects SQLite operations requiring recreation and applies the 4-step process
/// within the migration's transaction context.
#[derive(Debug, Clone)]
pub struct SqliteTableRecreation {
	/// Original table name
	pub table_name: String,
	/// New column definitions (after modification)
	pub new_columns: Vec<ColumnDefinition>,
	/// Columns to copy from old table (in order matching new_columns)
	pub columns_to_copy: Vec<String>,
	/// Constraints for the new table (parsed from introspection)
	pub constraints: Vec<Constraint>,
	/// Raw constraint SQL strings (for AddConstraint operations)
	pub raw_constraint_sqls: Vec<String>,
	/// Existing raw table constraints preserved from SQLite CREATE TABLE SQL
	pub raw_constraints: Vec<SqliteRecreatedConstraint>,
	/// Explicit column collations preserved from SQLite CREATE TABLE SQL
	pub column_collations: Vec<(String, String)>,
	/// Indexes to recreate after the table rename
	pub indexes: Vec<SqliteRecreatedIndex>,
	/// Trigger definitions to recreate after the table rename
	pub triggers: Vec<String>,
	/// WITHOUT ROWID option
	pub without_rowid: bool,
	/// STRICT table option
	pub strict: bool,
}

/// SQLite index metadata preserved across table recreation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteRecreatedIndex {
	/// Index name
	pub name: String,
	/// Indexed columns
	pub columns: Vec<String>,
	/// Whether the index is unique
	pub unique: bool,
	/// Original CREATE INDEX SQL from sqlite_master.
	pub sql: Option<String>,
}

/// SQLite table constraint metadata preserved verbatim across recreation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteRecreatedConstraint {
	/// Declared constraint name, when the original clause was named.
	pub name: Option<String>,
	/// SQLite autoindex name assigned to an unnamed table constraint.
	pub physical_name: Option<String>,
	/// Columns referenced by the constraint in declaration order.
	pub columns: Vec<String>,
	/// Original table-level constraint clause.
	pub sql: String,
}

impl SqliteRecreatedConstraint {
	fn references_column(&self, column_name: &str) -> bool {
		self.columns
			.iter()
			.any(|column| column.eq_ignore_ascii_case(column_name))
	}
}

impl SqliteRecreatedIndex {
	fn normalized_sql(&self) -> Option<String> {
		let mut sql = self.sql.as_ref()?.trim().trim_end_matches(';').to_string();
		sql.push(';');
		Some(sql)
	}

	fn references_column(&self, column_name: &str) -> bool {
		self.columns.iter().any(|column| column == column_name)
			|| self
				.sql
				.as_deref()
				.is_some_and(|sql| sqlite_sql_references_identifier(sql, column_name))
	}
}

fn sqlite_sql_references_identifier(sql: &str, identifier: &str) -> bool {
	let unquoted = identifier.to_ascii_lowercase();
	let double_quoted = format!("\"{}\"", identifier).to_ascii_lowercase();
	let bracket_quoted = format!("[{}]", identifier).to_ascii_lowercase();
	let backtick_quoted = format!("`{}`", identifier).to_ascii_lowercase();
	let sql = sql.to_ascii_lowercase();
	sql.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
		.any(|token| token == unquoted)
		|| sql.contains(&double_quoted)
		|| sql.contains(&bracket_quoted)
		|| sql.contains(&backtick_quoted)
}

impl SqliteTableRecreation {
	fn columns_to_copy(columns: &[ColumnDefinition]) -> Vec<String> {
		columns
			.iter()
			.filter(|column| column.generated.is_none())
			.map(|column| column.name.to_string())
			.collect()
	}

	/// Create a new table recreation for adding a column.
	pub fn for_add_column(
		table_name: impl Into<String>,
		current_columns: Vec<ColumnDefinition>,
		column_to_add: ColumnDefinition,
		current_constraints: Vec<Constraint>,
	) -> Self {
		let table_name = table_name.into();
		let columns_to_copy = Self::columns_to_copy(&current_columns);
		let mut new_columns = current_columns;
		new_columns.push(column_to_add);

		Self {
			table_name,
			new_columns,
			columns_to_copy,
			constraints: current_constraints,
			raw_constraint_sqls: Vec::new(),
			raw_constraints: Vec::new(),
			column_collations: Vec::new(),
			indexes: Vec::new(),
			triggers: Vec::new(),
			without_rowid: false,
			strict: false,
		}
	}

	/// Create a new table recreation for dropping a column
	pub fn for_drop_column(
		table_name: impl Into<String>,
		current_columns: Vec<ColumnDefinition>,
		column_to_drop: &str,
		current_constraints: Vec<Constraint>,
	) -> Self {
		let table_name = table_name.into();
		let new_columns: Vec<_> = current_columns
			.into_iter()
			.filter(|c| c.name != column_to_drop)
			.collect();
		let columns_to_copy = Self::columns_to_copy(&new_columns);

		// Filter out constraints that reference the dropped column
		let constraints: Vec<_> = current_constraints
			.into_iter()
			.filter(|c| !Self::constraint_references_column(c, column_to_drop))
			.collect();

		Self {
			table_name,
			new_columns,
			columns_to_copy,
			constraints,
			raw_constraint_sqls: Vec::new(),
			raw_constraints: Vec::new(),
			column_collations: Vec::new(),
			indexes: Vec::new(),
			triggers: Vec::new(),
			without_rowid: false,
			strict: false,
		}
	}

	/// Create a new table recreation for altering a column type
	pub fn for_alter_column(
		table_name: impl Into<String>,
		current_columns: Vec<ColumnDefinition>,
		column_name: &str,
		new_definition: ColumnDefinition,
		current_constraints: Vec<Constraint>,
	) -> Self {
		let table_name = table_name.into();
		let new_columns: Vec<_> = current_columns
			.into_iter()
			.map(|c| {
				if c.name == column_name {
					new_definition.clone()
				} else {
					c
				}
			})
			.collect();
		let columns_to_copy = Self::columns_to_copy(&new_columns);

		Self {
			table_name,
			new_columns,
			columns_to_copy,
			constraints: current_constraints,
			raw_constraint_sqls: Vec::new(),
			raw_constraints: Vec::new(),
			column_collations: Vec::new(),
			indexes: Vec::new(),
			triggers: Vec::new(),
			without_rowid: false,
			strict: false,
		}
	}

	/// Create a new table recreation for adding a constraint
	///
	/// Since SQLite doesn't support `ALTER TABLE ADD CONSTRAINT`, we need to
	/// recreate the table with the new constraint included.
	pub fn for_add_constraint(
		table_name: impl Into<String>,
		current_columns: Vec<ColumnDefinition>,
		current_constraints: Vec<Constraint>,
		constraint_sql: String,
	) -> Self {
		let table_name = table_name.into();
		let columns_to_copy = Self::columns_to_copy(&current_columns);

		Self {
			table_name,
			new_columns: current_columns,
			columns_to_copy,
			constraints: current_constraints,
			raw_constraint_sqls: vec![constraint_sql],
			raw_constraints: Vec::new(),
			column_collations: Vec::new(),
			indexes: Vec::new(),
			triggers: Vec::new(),
			without_rowid: false,
			strict: false,
		}
	}

	/// Creates a recreation plan that adds a typed constraint.
	pub fn for_add_constraint_definition(
		table_name: impl Into<String>,
		current_columns: Vec<ColumnDefinition>,
		mut current_constraints: Vec<Constraint>,
		constraint: Constraint,
	) -> Self {
		let table_name = table_name.into();
		let columns_to_copy = Self::columns_to_copy(&current_columns);
		current_constraints.push(constraint);

		Self {
			table_name,
			new_columns: current_columns,
			columns_to_copy,
			constraints: current_constraints,
			raw_constraint_sqls: Vec::new(),
			raw_constraints: Vec::new(),
			column_collations: Vec::new(),
			indexes: Vec::new(),
			triggers: Vec::new(),
			without_rowid: false,
			strict: false,
		}
	}

	/// Create a new table recreation for dropping a constraint
	///
	/// Since SQLite doesn't support `ALTER TABLE DROP CONSTRAINT`, we need to
	/// recreate the table without the specified constraint.
	pub fn for_drop_constraint(
		table_name: impl Into<String>,
		current_columns: Vec<ColumnDefinition>,
		current_constraints: Vec<Constraint>,
		constraint_name: &str,
	) -> Self {
		let table_name = table_name.into();
		let columns_to_copy = Self::columns_to_copy(&current_columns);

		// Filter out the constraint by name
		let constraints: Vec<_> = current_constraints
			.into_iter()
			.filter(|c| !Self::constraint_has_name(c, constraint_name))
			.collect();

		Self {
			table_name,
			new_columns: current_columns,
			columns_to_copy,
			constraints,
			raw_constraint_sqls: Vec::new(),
			raw_constraints: Vec::new(),
			column_collations: Vec::new(),
			indexes: Vec::new(),
			triggers: Vec::new(),
			without_rowid: false,
			strict: false,
		}
	}

	/// Adds indexes that should be recreated after the replacement table is renamed.
	pub fn with_indexes(mut self, indexes: Vec<SqliteRecreatedIndex>) -> Self {
		self.indexes = indexes;
		self
	}

	/// Adds existing raw table constraints that must be preserved verbatim.
	pub fn with_raw_constraints(mut self, raw_constraints: Vec<SqliteRecreatedConstraint>) -> Self {
		self.raw_constraints = raw_constraints;
		self
	}

	/// Adds explicit column collations that must survive table recreation.
	pub fn with_column_collations(mut self, column_collations: Vec<(String, String)>) -> Self {
		self.column_collations = column_collations;
		self
	}

	/// Adds trigger definitions that should be recreated after the replacement table is renamed.
	pub fn with_triggers(mut self, triggers: Vec<String>) -> Self {
		self.triggers = triggers;
		self
	}

	/// Preserves whether the recreated table uses SQLite's WITHOUT ROWID storage.
	pub fn with_without_rowid(mut self, without_rowid: bool) -> Self {
		self.without_rowid = without_rowid;
		self
	}

	/// Preserves whether the recreated table uses SQLite's STRICT type enforcement.
	pub fn with_strict(mut self, strict: bool) -> Self {
		self.strict = strict;
		self
	}

	/// Removes indexes that reference a column no longer present in the replacement table.
	pub fn without_indexes_referencing(mut self, column_name: &str) -> Self {
		self.indexes
			.retain(|index| !index.references_column(column_name));
		self
	}

	/// Removes preserved raw constraints that reference a dropped column.
	pub fn without_raw_constraints_referencing(mut self, column_name: &str) -> Self {
		self.raw_constraints
			.retain(|constraint| !constraint.references_column(column_name));
		self
	}

	/// Removes a preserved raw constraint by its declared name.
	pub fn without_raw_constraint_named(mut self, constraint_name: &str) -> Self {
		self.raw_constraints.retain(|constraint| {
			let logical_name_differs = constraint
				.name
				.as_deref()
				.is_none_or(|name| !name.eq_ignore_ascii_case(constraint_name));
			let physical_name_differs = constraint
				.physical_name
				.as_deref()
				.is_none_or(|name| !name.eq_ignore_ascii_case(constraint_name));
			logical_name_differs && physical_name_differs
		});
		self
	}

	/// Generate the SQL statements for table recreation and dependent object restoration.
	pub fn to_sql_statements(&self) -> Vec<String> {
		self.try_to_sql_statements().unwrap_or_else(|error| {
			panic!(
				"SQLite recreation requires checked SQL rendering; use try_to_sql_statements: {error}"
			)
		})
	}

	/// Generates table-recreation SQL after validating every replacement column.
	pub fn try_to_sql_statements(&self) -> super::Result<Vec<String>> {
		for column in &self.new_columns {
			column
				.type_definition
				.try_to_sql_for_dialect(&SqlDialect::Sqlite)?;
		}
		Ok(self.render_sql_statements())
	}

	fn render_sql_statements(&self) -> Vec<String> {
		let temp_table = format!("{}_new", self.table_name);
		let quote =
			|identifier: &str| Operation::quote_dialect_identifier(identifier, &SqlDialect::Sqlite);
		let quoted_temp_table = quote(&temp_table);
		let quoted_table = quote(&self.table_name);

		// Step 1: CREATE TABLE with new schema
		let column_defs: Vec<String> = self
			.new_columns
			.iter()
			.map(|column| {
				let collation = self
					.column_collations
					.iter()
					.find(|(name, _)| name.eq_ignore_ascii_case(&column.name))
					.map(|(_, collation)| collation.as_str());
				Operation::column_to_sql_with_collation(column, &SqlDialect::Sqlite, collation)
			})
			.collect();

		let constraint_defs: Vec<String> = self
			.constraints
			.iter()
			.map(|constraint| constraint.to_sql_for_dialect(&SqlDialect::Sqlite))
			.collect();

		let mut create_parts = column_defs;
		create_parts.extend(constraint_defs);
		create_parts.extend(
			self.raw_constraints
				.iter()
				.map(|constraint| constraint.sql.clone()),
		);
		// Include raw constraint SQLs from the current AddConstraint operation.
		create_parts.extend(self.raw_constraint_sqls.clone());

		let mut create_sql = format!(
			"CREATE TABLE {} (\n  {}\n)",
			quoted_temp_table,
			create_parts.join(",\n  ")
		);
		match (self.without_rowid, self.strict) {
			(true, true) => create_sql.push_str(" WITHOUT ROWID, STRICT"),
			(true, false) => create_sql.push_str(" WITHOUT ROWID"),
			(false, true) => create_sql.push_str(" STRICT"),
			(false, false) => {}
		}
		create_sql.push(';');

		// Step 2: Copy data
		let columns_list = self
			.columns_to_copy
			.iter()
			.map(|column| Operation::quote_dialect_identifier(column, &SqlDialect::Sqlite))
			.collect::<Vec<_>>()
			.join(", ");
		let insert_sql = format!(
			"INSERT INTO {} ({}) SELECT {} FROM {};",
			quoted_temp_table, columns_list, columns_list, quoted_table
		);

		// Step 3: Drop old table
		let drop_sql = format!("DROP TABLE {quoted_table};");

		// Step 4: Rename new table
		let rename_sql = format!(
			"ALTER TABLE {} RENAME TO {};",
			quoted_temp_table, quoted_table
		);

		let mut statements = vec![create_sql, insert_sql, drop_sql, rename_sql];
		for index in &self.indexes {
			if let Some(sql) = index.normalized_sql() {
				statements.push(sql);
				continue;
			}
			let unique = if index.unique { "UNIQUE " } else { "" };
			let columns = index
				.columns
				.iter()
				.map(|column| quote(column))
				.collect::<Vec<_>>()
				.join(", ");
			statements.push(format!(
				"CREATE {unique}INDEX {} ON {} ({});",
				quote(&index.name),
				quoted_table,
				columns
			));
		}
		for trigger in &self.triggers {
			let trigger = trigger.trim().trim_end_matches(';');
			if !trigger.is_empty() {
				statements.push(format!("{trigger};"));
			}
		}
		statements
	}

	/// Check if a constraint references a specific column
	fn constraint_references_column(constraint: &Constraint, column_name: &str) -> bool {
		match constraint {
			Constraint::PrimaryKey { columns, .. } => columns.iter().any(|c| c == column_name),
			Constraint::ForeignKey { columns, .. } => columns.iter().any(|c| c == column_name),
			Constraint::Unique { columns, .. } => columns.iter().any(|c| c == column_name),
			Constraint::Check { expression, .. } => expression.contains(column_name),
			Constraint::EnumDomain { column, .. } => column == column_name,
			Constraint::OneToOne { column, .. } => column == column_name,
			Constraint::ManyToMany { source_column, .. } => source_column == column_name,
			Constraint::Exclude { elements, .. } => {
				elements.iter().any(|(col, _)| col == column_name)
			}
		}
	}

	/// Check if a constraint has the specified name
	fn constraint_has_name(constraint: &Constraint, constraint_name: &str) -> bool {
		match constraint {
			Constraint::PrimaryKey { name, .. } => name == constraint_name,
			Constraint::ForeignKey { name, .. } => name == constraint_name,
			Constraint::Unique { name, .. } => name == constraint_name,
			Constraint::Check { name, .. } => name == constraint_name,
			Constraint::EnumDomain { name, .. } => name == constraint_name,
			Constraint::OneToOne { name, .. } => name == constraint_name,
			Constraint::ManyToMany { name, .. } => name == constraint_name,
			Constraint::Exclude { name, .. } => name == constraint_name,
		}
	}
}

impl Operation {
	/// Check if this operation requires SQLite table recreation
	pub fn requires_sqlite_recreation(&self) -> bool {
		matches!(
			self,
			Operation::DropColumn { .. }
				| Operation::AlterColumn { .. }
				| Operation::AddConstraint { .. }
				| Operation::AddConstraintDefinition { .. }
				| Operation::AddConstraintRepair { .. }
				| Operation::DropConstraint { .. }
				| Operation::DropConstraintDefinition { .. }
		) || matches!(
			self,
			Operation::AddColumn { column, .. }
				if column
					.generated
					.as_ref()
					.is_some_and(|generated| generated.storage == GeneratedStorage::Stored)
		)
	}

	/// Check if the reverse of this operation requires SQLite table recreation
	///
	/// When rolling back a migration on SQLite, some reverse operations also require
	/// table recreation. This method identifies those cases.
	///
	/// | Forward Operation | Reverse Operation | Requires Recreation |
	/// |-------------------|-------------------|---------------------|
	/// | AddColumn         | DropColumn        | Yes                 |
	/// | AlterColumn       | AlterColumn       | Yes                 |
	/// | AddConstraint     | DropConstraint    | Yes                 |
	/// | DropConstraint    | AddConstraint     | Yes                 |
	pub fn reverse_requires_sqlite_recreation(&self) -> bool {
		matches!(
			self,
			// AddColumn → Reverse DropColumn (requires recreation)
			Operation::AddColumn { .. }
				// AlterColumn → Reverse AlterColumn (requires recreation)
					| Operation::AlterColumn { .. }
					// AddConstraint → Reverse DropConstraint (requires recreation)
					| Operation::AddConstraint { .. }
					| Operation::AddConstraintDefinition { .. }
					// DropConstraint → Reverse AddConstraint (requires recreation)
					| Operation::DropConstraint { .. }
					| Operation::DropConstraintDefinition { .. }
					// RestoreConstraintOnRollback → Reverse AddConstraint (requires recreation)
					| Operation::RestoreConstraintOnRollback { .. }
		) || matches!(
			self,
			Operation::DropColumn {
				old_definition: Some(old_definition),
				..
			} if old_definition
				.generated
				.as_ref()
				.is_some_and(|generated| generated.storage == GeneratedStorage::Stored)
		)
	}

	/// Generate the reverse operation (for rollback on SQLite)
	///
	/// This method returns the conceptual reverse `Operation`, which can be used
	/// with `handle_sqlite_recreation()` for databases that don't support direct
	/// ALTER TABLE operations.
	///
	/// # Arguments
	///
	/// * `project_state` - Project state for accessing model definitions
	///
	/// # Returns
	///
	/// * `Ok(Some(op))` - Reverse operation generated successfully
	/// * `Ok(None)` - Operation is not reversible or state information is missing
	/// * `Err(_)` - Error generating reverse operation
	pub fn to_reverse_operation(
		&self,
		project_state: &ProjectState,
	) -> super::Result<Option<Operation>> {
		match self {
			Operation::CreateTable { name, .. } => {
				Ok(Some(Operation::DropTable { name: name.clone() }))
			}
			Operation::DropTable { name } => {
				// Reconstruct CreateTable from ProjectState
				if let Some(model) = project_state.find_model_by_table(name) {
					let columns: Vec<ColumnDefinition> =
						Self::order_model_fields_by_generated_dependencies(model)
							.into_iter()
							.map(|(field_name, field)| {
								ColumnDefinition::from_field_state(field_name.clone(), field)
							})
							.collect();
					let constraints: Vec<Constraint> = model
						.constraints
						.iter()
						.map(|c| c.to_constraint())
						.collect();
					return Ok(Some(Operation::CreateTable {
						name: name.clone(),
						columns,
						constraints,
						without_rowid: None,
						interleave_in_parent: None,
						partition: None,
					}));
				}
				Ok(None)
			}
			Operation::AddColumn { table, column, .. } => Ok(Some(Operation::DropColumn {
				table: table.clone(),
				column: column.name.clone(),
				old_definition: None,
			})),
			Operation::DropColumn {
				table,
				column,
				old_definition,
			} => {
				// Prefer the explicit definition carried by replacement-style
				// migrations; fall back to ProjectState for legacy migrations.
				let resolved_old_def = old_definition.clone().or_else(|| {
					project_state
						.find_model_by_table(table)
						.and_then(|model| model.get_field(column))
						.map(|field| ColumnDefinition::from_field_state(column.clone(), field))
				});

				if let Some(col_def) = resolved_old_def {
					return Ok(Some(Operation::AddColumn {
						table: table.clone(),
						column: col_def,
						mysql_options: None,
					}));
				}
				Ok(None)
			}
			Operation::AlterColumn {
				table,
				column,
				old_definition,
				new_definition: _,
				..
			} => {
				// Reconstruct AlterColumn with the original definition. Prefer the
				// explicit `old_definition` carried by the forward operation; fall
				// back to ProjectState lookup only if `old_definition` is absent.
				let resolved_old_def = old_definition.clone().or_else(|| {
					project_state
						.find_model_by_table(table)
						.and_then(|model| model.get_field(column))
						.map(|field| ColumnDefinition::from_field_state(column.clone(), field))
				});

				if let Some(col_def) = resolved_old_def {
					return Ok(Some(Operation::AlterColumn {
						table: table.clone(),
						column: column.clone(),
						old_definition: None,
						new_definition: col_def,
						mysql_options: None,
					}));
				}
				Ok(None)
			}
			Operation::AddConstraint {
				table,
				constraint_sql,
			} => {
				// Extract constraint name to create DropConstraint
				if let Some(constraint_name) = Self::extract_constraint_name(constraint_sql) {
					return Ok(Some(Operation::DropConstraint {
						table: table.clone(),
						constraint_name,
					}));
				}
				Err(super::MigrationError::InvalidMigration(format!(
					"Cannot extract constraint name from: {}",
					constraint_sql
				)))
			}
			Operation::AddConstraintDefinition { table, constraint } => {
				Ok(Some(Operation::DropConstraintDefinition {
					table: table.clone(),
					constraint: constraint.clone(),
				}))
			}
			Operation::AddConstraintRepair { .. } => Ok(None),
			Operation::RestoreConstraintOnRollback {
				table,
				constraint_sql,
			} => Ok(Some(Operation::AddConstraint {
				table: table.clone(),
				constraint_sql: constraint_sql.clone(),
			})),
			Operation::DropConstraint {
				table,
				constraint_name,
			} => {
				// Reconstruct AddConstraint from ProjectState
				if let Some(model) = project_state.find_model_by_table(table)
					&& let Some(constraint_def) = model
						.constraints
						.iter()
						.find(|c| c.name == *constraint_name)
				{
					let constraint = constraint_def.to_constraint();
					return Ok(Some(match constraint {
						Constraint::EnumDomain { .. } => Operation::AddConstraintDefinition {
							table: table.clone(),
							constraint,
						},
						_ => Operation::AddConstraint {
							table: table.clone(),
							constraint_sql: constraint.to_string(),
						},
					}));
				}
				Ok(None)
			}
			Operation::DropConstraintDefinition { table, constraint } => {
				Ok(Some(Operation::AddConstraintDefinition {
					table: table.clone(),
					constraint: constraint.clone(),
				}))
			}
			Operation::RenameTable { old_name, new_name } => Ok(Some(Operation::RenameTable {
				old_name: new_name.clone(),
				new_name: old_name.clone(),
			})),
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => Ok(Some(Operation::RenameColumn {
				table: table.clone(),
				old_name: new_name.clone(),
				new_name: old_name.clone(),
			})),
			Operation::CreateIndex { table, columns, .. } => Ok(Some(Operation::DropIndex {
				table: table.clone(),
				columns: columns.clone(),
			})),
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => Ok(Some(Operation::DropNamedIndex {
				table: table.clone(),
				name: name.clone(),
				columns: columns.clone(),
				unique: *unique,
				index_type: *index_type,
				where_clause: where_clause.clone(),
				concurrently: *concurrently,
				expressions: expressions.clone(),
				mysql_options: *mysql_options,
				operator_class: operator_class.clone(),
			})),
			Operation::CreateIndexRepair { .. } | Operation::RestoreIndexOnRollback { .. } => {
				Ok(None)
			}
			Operation::DropIndex { table, columns } => {
				// Basic index recreation (without advanced properties)
				// Note: Cannot determine if the original index was unique from DropIndex alone
				Ok(Some(Operation::CreateIndex {
					table: table.clone(),
					columns: columns.clone(),
					unique: false,
					index_type: None,
					where_clause: None,
					concurrently: false,
					expressions: None,
					mysql_options: None,
					operator_class: None,
				}))
			}
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				if !named_index_has_target(columns, expressions.as_deref()) {
					return Ok(None);
				}
				Ok(Some(Operation::CreateNamedIndex {
					table: table.clone(),
					name: name.clone(),
					columns: columns.clone(),
					unique: *unique,
					index_type: *index_type,
					where_clause: where_clause.clone(),
					concurrently: *concurrently,
					expressions: expressions.clone(),
					mysql_options: *mysql_options,
					operator_class: operator_class.clone(),
				}))
			}
			// Operations that are not reversible as Operations
			Operation::RunSQL { .. } | Operation::RunRust { .. } | Operation::BulkLoad { .. } => {
				Ok(None)
			}
			// Other operations - not reversible via to_reverse_operation
			_ => Ok(None),
		}
	}
}

// Re-export for convenience (legacy)
pub use Operation::{AddColumn, AlterColumn, CreateTable, DropColumn};

/// Operation statement types (reinhardt-query or sanitized raw SQL)
pub enum OperationStatement {
	/// TableCreate variant.
	TableCreate(CreateTableStatement),
	/// TableDrop variant.
	TableDrop(DropTableStatement),
	/// TableAlter variant.
	TableAlter(AlterTableStatement),
	/// TableRename variant.
	TableRename(AlterTableStatement),
	/// IndexCreate variant.
	IndexCreate(CreateIndexStatement),
	/// IndexDrop variant.
	IndexDrop(DropIndexStatement),
	/// Sanitized raw SQL (identifiers escaped with pg_escape::quote_identifier)
	RawSql(String),
	/// An operation whose SQL must be rendered after the database dialect is known.
	DialectOperation(Box<Operation>),
}

impl OperationStatement {
	/// Execute the operation statement
	pub async fn execute<'c, E>(&self, executor: E) -> Result<(), sqlx::Error>
	where
		E: sqlx::Executor<'c, Database = sqlx::Postgres>,
	{
		use crate::backends::types::DatabaseType;
		let sql = self
			.try_to_sql_string(DatabaseType::Postgres)
			.map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
		sqlx::query(&sql).execute(executor).await?;
		Ok(())
	}

	/// Converts to SQL while preserving checked backend feature errors.
	pub fn try_to_sql_string(
		&self,
		db_type: crate::backends::types::DatabaseType,
	) -> super::Result<String> {
		use crate::backends::sql_build_helpers;

		let map_query_error = |error: reinhardt_query::QueryBuildError| match error {
			reinhardt_query::QueryBuildError::UnsupportedBackendFeature { feature, .. } => {
				super::MigrationError::UnsupportedBackendFeature {
					feature,
					backend: match db_type {
						crate::backends::types::DatabaseType::Postgres => "postgres",
						crate::backends::types::DatabaseType::Mysql => "mysql",
						crate::backends::types::DatabaseType::Sqlite => "sqlite",
					},
				}
			}
			_ => super::MigrationError::InvalidMigration(error.to_string()),
		};

		match self {
			Self::TableCreate(stmt) => match db_type {
				crate::backends::types::DatabaseType::Postgres => PostgresQueryBuilder
					.build_create_table_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
				crate::backends::types::DatabaseType::Mysql => MySqlQueryBuilder
					.build_create_table_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
				crate::backends::types::DatabaseType::Sqlite => SqliteQueryBuilder
					.build_create_table_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
			},
			Self::TableDrop(stmt) => Ok(sql_build_helpers::build_drop_table_sql(db_type, stmt)),
			Self::TableAlter(stmt) | Self::TableRename(stmt) => match db_type {
				crate::backends::types::DatabaseType::Postgres => PostgresQueryBuilder
					.build_alter_table_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
				crate::backends::types::DatabaseType::Mysql => MySqlQueryBuilder
					.build_alter_table_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
				crate::backends::types::DatabaseType::Sqlite => SqliteQueryBuilder
					.build_alter_table_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
			},
			Self::IndexCreate(stmt) => match db_type {
				crate::backends::types::DatabaseType::Postgres => PostgresQueryBuilder
					.build_create_index_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
				crate::backends::types::DatabaseType::Mysql => MySqlQueryBuilder
					.build_create_index_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
				crate::backends::types::DatabaseType::Sqlite => SqliteQueryBuilder
					.build_create_index_checked(stmt)
					.map(|(sql, _)| sql)
					.map_err(map_query_error),
			},
			Self::IndexDrop(stmt) => Ok(sql_build_helpers::build_drop_index_sql(db_type, stmt)),
			Self::RawSql(sql) => Ok(sql.clone()),
			Self::DialectOperation(operation) => {
				let dialect = match db_type {
					crate::backends::types::DatabaseType::Postgres => SqlDialect::Postgres,
					crate::backends::types::DatabaseType::Mysql => SqlDialect::Mysql,
					crate::backends::types::DatabaseType::Sqlite => SqlDialect::Sqlite,
				};
				operation.try_to_sql(&dialect)
			}
		}
	}

	/// Convert to SQL string for logging/debugging
	///
	/// # Arguments
	///
	/// * `db_type` - Database type to generate SQL for (PostgreSQL, MySQL, SQLite)
	pub fn to_sql_string(&self, db_type: crate::backends::types::DatabaseType) -> String {
		self.try_to_sql_string(db_type).unwrap_or_else(|error| {
			panic!(
				"operation statement requires checked SQL rendering; use try_to_sql_string: {error}"
			)
		})
	}
}

impl Operation {
	/// Converts to a query statement after validating the selected dialect.
	pub fn try_to_statement(&self, dialect: &SqlDialect) -> super::Result<OperationStatement> {
		self.validate_for_dialect(dialect)?;
		Ok(self.to_statement())
	}

	/// Convert Operation to reinhardt-query statement or sanitized raw SQL
	pub fn to_statement(&self) -> OperationStatement {
		match self {
			Operation::CreateTable {
				name,
				columns,
				constraints,
				..
			} => {
				if constraints
					.iter()
					.any(|constraint| matches!(constraint, Constraint::EnumDomain { .. }))
				{
					OperationStatement::DialectOperation(Box::new(self.clone()))
				} else {
					OperationStatement::TableCreate(self.build_create_table(
						name,
						columns,
						constraints,
					))
				}
			}
			Operation::DropTable { name } => {
				OperationStatement::TableDrop(self.build_drop_table(name))
			}
			Operation::AddColumn { table, column, .. } => {
				OperationStatement::TableAlter(self.build_add_column(table, column))
			}
			Operation::DropColumn { table, column, .. } => {
				OperationStatement::TableAlter(self.build_drop_column(table, column))
			}
			Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} => OperationStatement::TableAlter(self.build_alter_column(
				table,
				column,
				new_definition,
			)),
			Operation::RenameTable { old_name, new_name } => {
				OperationStatement::TableRename(self.build_rename_table(old_name, new_name))
			}
			// reinhardt-query does not support RENAME COLUMN, use sanitized raw SQL
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => OperationStatement::RawSql(format!(
				"ALTER TABLE {} RENAME COLUMN {} TO {}",
				quote_identifier(table),
				quote_identifier(old_name),
				quote_identifier(new_name)
			)),
			Operation::DropConstraintDefinition { .. } => {
				OperationStatement::DialectOperation(Box::new(self.clone()))
			}
			Operation::AddConstraint {
				table,
				constraint_sql,
			}
			| Operation::AddConstraintRepair {
				table,
				constraint_sql,
			} => {
				// NOTE: constraint_sql validation is the caller's responsibility
				OperationStatement::RawSql(format!(
					"ALTER TABLE {} ADD {}",
					quote_identifier(table),
					constraint_sql
				))
			}
			Operation::AddConstraintDefinition { .. } => {
				OperationStatement::DialectOperation(Box::new(self.clone()))
			}
			Operation::RestoreConstraintOnRollback { .. } => OperationStatement::RawSql(
				"-- rollback-only generated-column constraint restore".to_string(),
			),
			Operation::DropConstraint {
				table,
				constraint_name,
				..
			} => OperationStatement::RawSql(format!(
				"ALTER TABLE {} DROP CONSTRAINT {}",
				quote_identifier(table),
				quote_identifier(constraint_name)
			)),
			Operation::CreateIndex {
				table,
				columns,
				unique,
				index_type,
				..
			}
			| Operation::CreateIndexRepair {
				table,
				columns,
				unique,
				index_type,
				..
			} => {
				if index_type.is_some_and(IndexType::is_approximate_vector) {
					return OperationStatement::DialectOperation(Box::new(self.clone()));
				}
				let idx_name = format!("idx_{}_{}", table, columns.join("_"));
				OperationStatement::IndexCreate(
					self.build_create_index(&idx_name, table, columns, *unique),
				)
			}
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				..
			} => {
				if index_type.is_some_and(IndexType::is_approximate_vector) {
					return OperationStatement::DialectOperation(Box::new(self.clone()));
				}
				OperationStatement::IndexCreate(
					self.build_create_index(name, table, columns, *unique),
				)
			}
			Operation::RestoreIndexOnRollback { .. } => OperationStatement::RawSql(
				"-- rollback-only generated-column index restore".to_string(),
			),
			Operation::DropIndex { table, columns } => {
				let idx_name = format!("idx_{}_{}", table, columns.join("_"));
				OperationStatement::IndexDrop(self.build_drop_index(&idx_name))
			}
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex { name, .. } => {
				OperationStatement::IndexDrop(self.build_drop_index(name))
			}
			Operation::RunSQL { sql, .. } => OperationStatement::RawSql(sql.to_string()),
			Operation::RunRust { code, .. } => {
				// RunRust operations don't produce SQL
				OperationStatement::RawSql(format!(
					"-- RunRust: {}",
					code.lines().next().unwrap_or("")
				))
			}
			Operation::AlterTableComment { table, comment } => {
				// PostgreSQL-specific COMMENT ON TABLE
				OperationStatement::RawSql(if let Some(comment_text) = comment {
					format!(
						"COMMENT ON TABLE {} IS '{}'",
						quote_identifier(table),
						comment_text.replace('\'', "''") // Escape single quotes
					)
				} else {
					format!("COMMENT ON TABLE {} IS NULL", quote_identifier(table))
				})
			}
			Operation::AlterUniqueTogether {
				table,
				unique_together,
			} => {
				let mut sqls = Vec::new();
				for (idx, fields) in unique_together.iter().enumerate() {
					let constraint_name = format!("{}_{}_uniq", table, idx);
					let fields_str: Vec<String> = fields
						.iter()
						.map(|f| quote_identifier(f).to_string())
						.collect();
					sqls.push(format!(
						"ALTER TABLE {} ADD CONSTRAINT {} UNIQUE ({})",
						quote_identifier(table),
						quote_identifier(&constraint_name),
						fields_str.join(", ")
					));
				}
				OperationStatement::RawSql(sqls.join(";\n"))
			}
			Operation::AlterModelOptions { .. } => OperationStatement::RawSql(String::new()),
			Operation::CreateInheritedTable {
				name,
				columns,
				base_table,
				join_column,
			} => {
				let mut stmt = Query::create_table();
				stmt.table(Alias::new(name.as_str())).if_not_exists();

				// Add join column (foreign key to base table)
				let join_col = ColumnDef::new(Alias::new(join_column.as_str()));
				let join_col = join_col.integer();
				stmt.col(join_col);

				// Add other columns
				for col in columns {
					let mut column = ColumnDef::new(Alias::new(col.name.as_str()));
					column = self.apply_column_type(column, &col.type_definition);
					stmt.col(column);
				}

				// Add foreign key
				let mut fk = reinhardt_query::prelude::ForeignKey::create();
				fk.from_tbl(Alias::new(name.as_str()))
					.from_col(Alias::new(join_column.as_str()))
					.to_tbl(Alias::new(base_table.as_str()))
					.to_col(Alias::new("id"));
				stmt.foreign_key_from_builder(&mut fk);

				OperationStatement::TableCreate(stmt.to_owned())
			}
			Operation::AddDiscriminatorColumn {
				table,
				column_name,
				default_value,
			} => {
				let mut stmt = Query::alter_table();
				stmt.table(Alias::new(table.as_str()));

				let mut col = ColumnDef::new(Alias::new(column_name.as_str()));
				col = col
					.string_len(50)
					.default(SimpleExpr::from(default_value.to_string()));
				stmt.add_column(col);

				OperationStatement::TableAlter(stmt.to_owned())
			}
			Operation::MoveModel {
				rename_table,
				old_table_name,
				new_table_name,
				..
			} => {
				// MoveModel generates a table rename if table name changes
				if *rename_table {
					if let (Some(old_name), Some(new_name)) = (old_table_name, new_table_name) {
						OperationStatement::TableRename(self.build_rename_table(old_name, new_name))
					} else {
						// No table rename needed
						OperationStatement::RawSql("-- MoveModel: State-only operation".to_string())
					}
				} else {
					// State-only operation, no SQL
					OperationStatement::RawSql("-- MoveModel: State-only operation".to_string())
				}
			}
			Operation::CreateSchema {
				name,
				if_not_exists,
			} => {
				// Use schema.rs helper (reinhardt-query doesn't support CREATE SCHEMA)
				let sql = if *if_not_exists {
					format!("CREATE SCHEMA IF NOT EXISTS {}", quote_identifier(name))
				} else {
					format!("CREATE SCHEMA {}", quote_identifier(name))
				};
				OperationStatement::RawSql(sql)
			}
			Operation::DropSchema {
				name,
				cascade,
				if_exists,
			} => {
				// Use schema.rs helper (reinhardt-query doesn't support DROP SCHEMA)
				let if_exists_clause = if *if_exists { " IF EXISTS" } else { "" };
				let cascade_clause = if *cascade { " CASCADE" } else { "" };
				let sql = format!(
					"DROP SCHEMA{} {}{}",
					if_exists_clause,
					quote_identifier(name),
					cascade_clause
				);
				OperationStatement::RawSql(sql)
			}
			Operation::CreateExtension {
				name,
				if_not_exists,
				schema,
			} => {
				// PostgreSQL-specific: Use extensions.rs helper
				let if_not_exists_clause = if *if_not_exists { " IF NOT EXISTS" } else { "" };
				let schema_clause = if let Some(s) = schema {
					format!(" SCHEMA {}", quote_identifier(s))
				} else {
					String::new()
				};
				let sql = format!(
					"CREATE EXTENSION{} {}{}",
					if_not_exists_clause,
					quote_identifier(name),
					schema_clause
				);
				OperationStatement::RawSql(sql)
			}
			Operation::BulkLoad {
				table,
				source,
				format,
				options,
			} => {
				// BulkLoad uses dialect-specific raw SQL
				// Default to PostgreSQL COPY FROM syntax for to_statement()
				OperationStatement::RawSql(Self::postgres_copy_from_sql(
					table, source, format, options,
				))
			}
			Operation::SetAutoIncrementValue { table, .. } => {
				// `to_statement` has no dialect context, but `SetAutoIncrementValue`
				// renders fundamentally different SQL per backend (PostgreSQL
				// `setval`, MySQL `ALTER TABLE AUTO_INCREMENT`, SQLite
				// `sqlite_sequence` upsert). Silently emitting PostgreSQL-only
				// SQL here would break MySQL/SQLite migrations.
				//
				// Emit guaranteed-fail SQL that aborts execution with a visible
				// diagnostic pointing callers at the dialect-aware `to_sql`
				// path. Converting the signature to
				// `Result<OperationStatement, MigrationError>` would cascade
				// through dozens of call sites.
				OperationStatement::RawSql(format!(
					"SELECT 1/0 AS \"SetAutoIncrementValue on {} requires dialect-aware rendering; call Operation::to_sql(&dialect) instead of to_statement()\";",
					table.replace('"', "\"\"")
				))
			}
			Operation::CreateCompositePrimaryKey {
				table,
				columns,
				constraint_name,
			} => OperationStatement::RawSql(Self::create_composite_pk_to_sql(
				table,
				columns,
				constraint_name.as_deref(),
			)),
		}
	}

	/// Build CREATE TABLE statement
	fn build_create_table(
		&self,
		name: &str,
		columns: &[ColumnDefinition],
		constraints: &[Constraint],
	) -> CreateTableStatement {
		let mut stmt = Query::create_table();
		stmt.table(Alias::new(name)).if_not_exists();

		for col in columns {
			let mut column = ColumnDef::new(Alias::new(col.name.as_str()));
			column = self.apply_column_type(column, &col.type_definition);

			if col.not_null {
				column = column.not_null(true);
			}
			if col.unique {
				column = column.unique(true);
			}
			if col.primary_key {
				column = column.primary_key(true);
			}
			if col.auto_increment {
				column = column.auto_increment(true);
			}
			if let Some(default) = &col.default {
				column = column.default(SimpleExpr::from(self.convert_default_value(default)));
			}
			if let Some(generated) = &col.generated {
				column = self.apply_generated_column(column, generated);
			}

			stmt.col(column);
		}

		// Add table-level constraints
		for constraint in constraints {
			match constraint {
				Constraint::PrimaryKey { columns, .. } => {
					let col_idens: Vec<Alias> =
						columns.iter().map(|c| Alias::new(c.as_str())).collect();
					stmt.primary_key(col_idens);
				}
				Constraint::ForeignKey {
					name,
					columns,
					referenced_table,
					referenced_columns,
					on_delete,
					on_update,
					..
				} => {
					let mut fk = reinhardt_query::prelude::ForeignKey::create();
					fk.name(Alias::new(name.as_str()))
						.from_tbl(Alias::new(name.as_str()))
						.to_tbl(Alias::new(referenced_table.as_str()));

					for col in columns {
						fk.from_col(Alias::new(col.as_str()));
					}
					for col in referenced_columns {
						fk.to_col(Alias::new(col.as_str()));
					}

					fk.on_delete((*on_delete).into());
					fk.on_update((*on_update).into());

					stmt.foreign_key_from_builder(&mut fk);
				}
				Constraint::Unique { columns, .. } => {
					let col_idens: Vec<Alias> =
						columns.iter().map(|c| Alias::new(c.as_str())).collect();
					stmt.unique(col_idens);
				}
				Constraint::Check { name, expression } => {
					// Note: reinhardt-query doesn't have direct CHECK constraint support
					// This would need to be handled with raw SQL if needed
					let _ = (name, expression); // Suppress unused warnings
				}
				Constraint::EnumDomain { .. } => {
					// Typed CHECK constraints are emitted through the operation SQL path.
				}
				Constraint::OneToOne {
					name,
					column,
					referenced_table,
					referenced_column,
					on_delete,
					on_update,
					..
				} => {
					// OneToOne is ForeignKey + Unique
					let mut fk = reinhardt_query::prelude::ForeignKey::create();
					fk.name(Alias::new(name.as_str()))
						.from_tbl(Alias::new(name.as_str()))
						.to_tbl(Alias::new(referenced_table.as_str()))
						.from_col(Alias::new(column.as_str()))
						.to_col(Alias::new(referenced_column.as_str()))
						.on_delete((*on_delete).into())
						.on_update((*on_update).into());

					stmt.foreign_key_from_builder(&mut fk);

					// Add UNIQUE constraint separately if needed
					// Note: This should ideally be handled via UNIQUE column definition
				}
				Constraint::ManyToMany { .. } => {
					// ManyToMany is metadata only, no actual constraint in this table
					// The intermediate table handles the relationship
				}
				Constraint::Exclude { .. } => {
					// Exclude constraints are PostgreSQL-specific and not directly supported by reinhardt-query
					// They need to be handled with raw SQL if needed
				}
			}
		}

		stmt.to_owned()
	}

	/// Build DROP TABLE statement
	fn build_drop_table(&self, name: &str) -> DropTableStatement {
		Query::drop_table()
			.table(Alias::new(name))
			.if_exists()
			.cascade()
			.to_owned()
	}

	/// Build ALTER TABLE ADD COLUMN statement
	fn build_add_column(&self, table: &str, column: &ColumnDefinition) -> AlterTableStatement {
		let mut stmt = Query::alter_table();
		stmt.table(Alias::new(table));

		let mut col_def = ColumnDef::new(Alias::new(column.name.as_str()));
		col_def = self.apply_column_type(col_def, &column.type_definition);

		if column.not_null {
			col_def = col_def.not_null(true);
		}
		if let Some(default) = &column.default {
			col_def = col_def.default(SimpleExpr::from(self.convert_default_value(default)));
		}
		if let Some(generated) = &column.generated {
			col_def = self.apply_generated_column(col_def, generated);
		}

		stmt.add_column(col_def);
		stmt.to_owned()
	}

	/// Build ALTER TABLE DROP COLUMN statement
	fn build_drop_column(&self, table: &str, column: &str) -> AlterTableStatement {
		Query::alter_table()
			.table(Alias::new(table))
			.drop_column(Alias::new(column))
			.to_owned()
	}

	/// Build ALTER TABLE ALTER COLUMN statement
	fn build_alter_column(
		&self,
		table: &str,
		column: &str,
		new_definition: &ColumnDefinition,
	) -> AlterTableStatement {
		let mut stmt = Query::alter_table();
		stmt.table(Alias::new(table));

		let mut col_def = ColumnDef::new(Alias::new(column));
		col_def = self.apply_column_type(col_def, &new_definition.type_definition);

		if new_definition.not_null {
			col_def = col_def.not_null(true);
		}
		if let Some(generated) = &new_definition.generated {
			col_def = self.apply_generated_column(col_def, generated);
		}

		stmt.modify_column(col_def);
		stmt.to_owned()
	}

	/// Build ALTER TABLE RENAME statement
	fn build_rename_table(&self, old_name: &str, new_name: &str) -> AlterTableStatement {
		Query::alter_table()
			.table(Alias::new(old_name))
			.rename_table(Alias::new(new_name))
			.to_owned()
	}

	/// Build CREATE INDEX statement
	fn build_create_index(
		&self,
		name: &str,
		table: &str,
		columns: &[String],
		unique: bool,
	) -> CreateIndexStatement {
		let mut stmt = Query::create_index();
		stmt.name(Alias::new(name)).table(Alias::new(table));

		for col in columns {
			stmt.col(Alias::new(col));
		}

		if unique {
			stmt.unique();
		}

		stmt.to_owned()
	}

	/// Build DROP INDEX statement
	fn build_drop_index(&self, name: &str) -> DropIndexStatement {
		Query::drop_index().name(Alias::new(name)).to_owned()
	}

	/// Apply column type to ColumnDef using `reinhardt_query`'s fluent API
	fn apply_column_type(&self, col_def: ColumnDef, field_type: &FieldType) -> ColumnDef {
		use FieldType;
		match field_type {
			FieldType::Integer => col_def.integer(),
			FieldType::BigInteger => col_def.big_integer(),
			FieldType::SmallInteger => col_def.small_integer(),
			FieldType::TinyInt => col_def.tiny_integer(),
			FieldType::VarChar(max_length) => col_def.string_len(*max_length),
			FieldType::Char(max_length) => col_def.char_len(*max_length),
			FieldType::Text | FieldType::TinyText | FieldType::MediumText | FieldType::LongText => {
				col_def.text()
			}
			// Use custom "BOOLEAN" type name instead of col_def.boolean() to ensure
			// consistent type naming across all databases. This is important for SQLite
			// where col_def.boolean() would generate "INTEGER", but we need "BOOLEAN"
			// so that sqlx's type_info().name() returns "BOOLEAN" and our convert_row
			// can properly detect boolean columns and convert integer 0/1 to bool values.
			FieldType::Boolean => col_def.custom(Alias::new("BOOLEAN")),
			FieldType::DateTime => col_def.timestamp(),
			FieldType::TimestampTz => col_def.timestamp_with_time_zone(),
			FieldType::Date => col_def.date(),
			FieldType::Time => col_def.time(),
			FieldType::Decimal { precision, scale } => col_def.decimal(*precision, *scale),
			FieldType::Float => col_def.float(),
			FieldType::Double | FieldType::Real => col_def.double(),
			FieldType::Json => col_def.json(),
			FieldType::Jsonb => col_def.jsonb(),
			FieldType::Uuid => col_def.uuid(),
			FieldType::Binary | FieldType::Bytea => col_def.binary(0),
			FieldType::Blob | FieldType::TinyBlob | FieldType::MediumBlob | FieldType::LongBlob => {
				col_def.binary(0)
			}
			FieldType::MediumInt => col_def.integer(),
			FieldType::Year => col_def.small_integer(),
			FieldType::Enum { values } => {
				col_def.custom(Alias::new(format!("ENUM({})", values.join(","))))
			}
			FieldType::Set { values } => {
				col_def.custom(Alias::new(format!("SET({})", values.join(","))))
			}
			FieldType::ForeignKey { .. } => {
				// ForeignKey is a relationship, the actual column is typically an integer
				col_def.integer()
			}
			FieldType::OneToOne { .. } => {
				// OneToOne is a relationship, not a column type
				// The actual column will be a foreign key (typically BigInteger)
				col_def.big_integer()
			}
			FieldType::ManyToMany { .. } => {
				// ManyToMany is a relationship, not a column type
				// No column is created in the model table (uses intermediate table)
				col_def.big_integer()
			}
			// PostgreSQL-specific types
			FieldType::Array(inner) => {
				// PostgreSQL array type: use custom with array notation
				let inner_sql = inner.to_sql_string();
				col_def.custom(Alias::new(format!("{}[]", inner_sql)))
			}
			FieldType::HStore => col_def.custom(Alias::new("HSTORE")),
			FieldType::CIText => col_def.custom(Alias::new("CITEXT")),
			FieldType::Int4Range => col_def.custom(Alias::new("INT4RANGE")),
			FieldType::Int8Range => col_def.custom(Alias::new("INT8RANGE")),
			FieldType::NumRange => col_def.custom(Alias::new("NUMRANGE")),
			FieldType::DateRange => col_def.custom(Alias::new("DATERANGE")),
			FieldType::TsRange => col_def.custom(Alias::new("TSRANGE")),
			FieldType::TsTzRange => col_def.custom(Alias::new("TSTZRANGE")),
			FieldType::TsVector => col_def.custom(Alias::new("TSVECTOR")),
			FieldType::TsQuery => col_def.custom(Alias::new("TSQUERY")),
			#[cfg(feature = "pgvector")]
			FieldType::Vector { dimensions } => {
				if !(1..=2_000).contains(dimensions) {
					panic!(
						"vector fields require checked statement construction; use try_to_statement"
					);
				}
				let dimensions = u32::try_from(*dimensions).unwrap_or_else(|_| {
					panic!(
						"vector fields require checked statement construction; use try_to_statement"
					)
				});
				col_def.vector(dimensions)
			}
			FieldType::Custom(custom_type) => col_def.custom(Alias::new(custom_type)),
		}
	}

	/// Apply generated-column metadata to a query column definition.
	fn apply_generated_column(
		&self,
		col_def: ColumnDef,
		generated: &GeneratedColumnDefinition,
	) -> ColumnDef {
		if let Some(query_generated) = generated.to_query_generated() {
			match (query_generated.expr, query_generated.raw_sql) {
				(Some(expr), None) => col_def.generated(expr, query_generated.storage),
				(None, Some(raw_sql)) => col_def.generated_sql(raw_sql, query_generated.storage),
				_ => col_def,
			}
		} else {
			col_def
		}
	}

	/// Convert default value string to `reinhardt_query::prelude::Value`
	fn convert_default_value(&self, default: &str) -> Value {
		let trimmed = default.trim();

		// NULL
		if trimmed.eq_ignore_ascii_case("null") {
			return Value::String(None);
		}

		// Boolean
		if trimmed.eq_ignore_ascii_case("true") {
			return Value::Bool(Some(true));
		}
		if trimmed.eq_ignore_ascii_case("false") {
			return Value::Bool(Some(false));
		}

		// Integer
		if let Ok(i) = trimmed.parse::<i64>() {
			return Value::BigInt(Some(i));
		}

		// Float
		if let Ok(f) = trimmed.parse::<f64>() {
			return Value::Double(Some(f));
		}

		// String (quoted)
		if (trimmed.starts_with('"') && trimmed.ends_with('"'))
			|| (trimmed.starts_with('\'') && trimmed.ends_with('\''))
		{
			let unquoted = &trimmed[1..trimmed.len() - 1];
			return Value::String(Some(Box::new(unquoted.to_string())));
		}

		// JSON array/object
		if ((trimmed.starts_with('[') && trimmed.ends_with(']'))
			|| (trimmed.starts_with('{') && trimmed.ends_with('}')))
			&& let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed)
		{
			return json_to_sea_value(&json);
		}

		// SQL constants that should remain unquoted
		const SQL_CONSTANTS: &[&str] = &[
			"CURRENT_TIMESTAMP",
			"CURRENT_DATE",
			"CURRENT_TIME",
			"CURRENT_USER",
			"SESSION_USER",
			"LOCALTIME",
			"LOCALTIMESTAMP",
		];

		// SQL function calls (e.g., NOW(), CURRENT_TIMESTAMP()) - keep unquoted
		if trimmed.ends_with("()") || trimmed.contains('(') {
			return Value::String(Some(Box::new(trimmed.to_string())));
		}

		// SQL constants - keep unquoted
		if SQL_CONSTANTS
			.iter()
			.any(|c| trimmed.eq_ignore_ascii_case(c))
		{
			return Value::String(Some(Box::new(trimmed.to_string())));
		}

		// Default: plain string - auto-quote as SQL string literal
		Value::String(Some(Box::new(format!("'{}'", trimmed.replace('\'', "''")))))
	}
}

/// Helper function to convert `serde_json::Value` to `reinhardt_query::prelude::Value`
fn json_to_sea_value(json: &serde_json::Value) -> Value {
	match json {
		serde_json::Value::Null => Value::String(None),
		serde_json::Value::Bool(b) => Value::Bool(Some(*b)),
		serde_json::Value::Number(n) => {
			if let Some(i) = n.as_i64() {
				Value::BigInt(Some(i))
			} else if let Some(f) = n.as_f64() {
				Value::Double(Some(f))
			} else {
				Value::String(Some(Box::new(n.to_string())))
			}
		}
		serde_json::Value::String(s) => Value::String(Some(Box::new(s.clone()))),
		serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
			// Store as JSON string
			Value::String(Some(Box::new(json.to_string())))
		}
	}
}

// MigrationOperation trait implementation for legacy Operation enum
use super::operation_trait::MigrationOperation;

impl MigrationOperation for Operation {
	fn migration_name_fragment(&self) -> Option<String> {
		match self {
			Operation::CreateTable { name, .. } => Some(name.to_lowercase()),
			Operation::DropTable { name } => Some(format!("delete_{}", name.to_lowercase())),
			Operation::AddColumn { table, column, .. } => Some(format!(
				"{}_{}",
				table.to_lowercase(),
				column.name.to_lowercase()
			)),
			Operation::DropColumn { table, column, .. } => Some(format!(
				"remove_{}_{}",
				table.to_lowercase(),
				column.to_lowercase()
			)),
			Operation::AlterColumn { table, column, .. } => Some(format!(
				"alter_{}_{}",
				table.to_lowercase(),
				column.to_lowercase()
			)),
			Operation::RenameTable { old_name, new_name } => Some(format!(
				"rename_{}_to_{}",
				old_name.to_lowercase(),
				new_name.to_lowercase()
			)),
			Operation::RenameColumn {
				table, new_name, ..
			} => Some(format!(
				"rename_{}_{}",
				table.to_lowercase(),
				new_name.to_lowercase()
			)),
			Operation::DropConstraintDefinition { constraint, .. } => Some(format!(
				"drop_constraint_{}",
				constraint.name().to_lowercase()
			)),
			Operation::AddConstraint { table, .. }
			| Operation::AddConstraintDefinition { table, .. }
			| Operation::AddConstraintRepair { table, .. }
			| Operation::RestoreConstraintOnRollback { table, .. } => {
				Some(format!("add_constraint_{}", table.to_lowercase()))
			}
			Operation::DropConstraint {
				table: _,
				constraint_name,
				..
			} => Some(format!(
				"drop_constraint_{}",
				constraint_name.to_lowercase()
			)),
			Operation::CreateIndex { table, unique, .. }
			| Operation::CreateIndexRepair { table, unique, .. }
			| Operation::RestoreIndexOnRollback { table, unique, .. } => {
				if *unique {
					Some(format!("create_unique_index_{}", table.to_lowercase()))
				} else {
					Some(format!("create_index_{}", table.to_lowercase()))
				}
			}
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex { table, unique, .. } => {
				if *unique {
					Some(format!("create_unique_index_{}", table.to_lowercase()))
				} else {
					Some(format!("create_index_{}", table.to_lowercase()))
				}
			}
			Operation::DropIndex { table, .. } => {
				Some(format!("drop_index_{}", table.to_lowercase()))
			}
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex { name, .. } => Some(format!("drop_index_{}", name.to_lowercase())),
			Operation::RunSQL { .. } => None,  // Triggers auto-naming
			Operation::RunRust { .. } => None, // Triggers auto-naming
			Operation::AlterTableComment { table, .. } => {
				Some(format!("alter_comment_{}", table.to_lowercase()))
			}
			Operation::AlterUniqueTogether { table, .. } => {
				Some(format!("alter_unique_{}", table.to_lowercase()))
			}
			Operation::AlterModelOptions { table, .. } => {
				Some(format!("alter_options_{}", table.to_lowercase()))
			}
			Operation::CreateInheritedTable { name, .. } => {
				Some(format!("create_inherited_{}", name.to_lowercase()))
			}
			Operation::AddDiscriminatorColumn { table, .. } => {
				Some(format!("add_discriminator_{}", table.to_lowercase()))
			}
			Operation::MoveModel {
				model_name,
				from_app,
				to_app,
				..
			} => Some(format!(
				"move_{}_{}_{}_{}",
				from_app.to_lowercase(),
				model_name.to_lowercase(),
				to_app.to_lowercase(),
				model_name.to_lowercase()
			)),
			Operation::CreateSchema { name, .. } => {
				Some(format!("create_schema_{}", name.to_lowercase()))
			}
			Operation::DropSchema { name, .. } => {
				Some(format!("drop_schema_{}", name.to_lowercase()))
			}
			Operation::CreateExtension { name, .. } => {
				Some(format!("create_extension_{}", name.to_lowercase()))
			}
			Operation::BulkLoad { table, .. } => {
				Some(format!("bulk_load_{}", table.to_lowercase()))
			}
			Operation::SetAutoIncrementValue { table, column, .. } => Some(format!(
				"set_auto_increment_{}_{}",
				table.to_lowercase(),
				column.to_lowercase()
			)),
			Operation::CreateCompositePrimaryKey { table, .. } => {
				Some(format!("composite_pk_{}", table.to_lowercase()))
			}
		}
	}

	fn describe(&self) -> String {
		match self {
			Operation::CreateTable { name, .. } => format!("Create table {}", name),
			Operation::DropTable { name } => format!("Drop table {}", name),
			Operation::AddColumn { table, column, .. } => {
				format!("Add column {} to {}", column.name, table)
			}
			Operation::DropColumn { table, column, .. } => {
				format!("Drop column {} from {}", column, table)
			}
			Operation::AlterColumn { table, column, .. } => {
				format!("Alter column {} on {}", column, table)
			}
			Operation::RenameTable { old_name, new_name } => {
				format!("Rename table {} to {}", old_name, new_name)
			}
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => format!("Rename column {} to {} on {}", old_name, new_name, table),
			Operation::AddConstraint { table, .. }
			| Operation::AddConstraintDefinition { table, .. }
			| Operation::AddConstraintRepair { table, .. }
			| Operation::RestoreConstraintOnRollback { table, .. } => {
				format!("Add constraint on {}", table)
			}
			Operation::DropConstraint {
				table,
				constraint_name,
				..
			} => format!("Drop constraint {} from {}", constraint_name, table),
			Operation::DropConstraintDefinition { table, constraint } => {
				format!("Drop constraint {} from {}", constraint.name(), table)
			}
			Operation::CreateIndex { table, unique, .. }
			| Operation::CreateIndexRepair { table, unique, .. }
			| Operation::RestoreIndexOnRollback { table, unique, .. } => {
				if *unique {
					format!("Create unique index on {}", table)
				} else {
					format!("Create index on {}", table)
				}
			}
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex { table, unique, .. } => {
				if *unique {
					format!("Create unique index on {}", table)
				} else {
					format!("Create index on {}", table)
				}
			}
			Operation::DropIndex { table, .. } => format!("Drop index on {}", table),
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex { table, name, .. } => {
				format!("Drop index {} on {}", name, table)
			}
			Operation::RunSQL { sql, .. } => {
				let preview = if sql.len() > 50 {
					format!("{}...", &sql[..50])
				} else {
					(*sql).to_string()
				};
				format!("RunSQL: {}", preview)
			}
			Operation::RunRust { code, .. } => {
				let preview = if code.len() > 50 {
					format!("{}...", &code[..50])
				} else {
					(*code).to_string()
				};
				format!("RunRust: {}", preview)
			}
			Operation::AlterTableComment { table, comment } => match comment {
				Some(c) => format!("Set comment on {} to '{}'", table, c),
				None => format!("Remove comment from {}", table),
			},
			Operation::AlterUniqueTogether { table, .. } => {
				format!("Alter unique_together on {}", table)
			}
			Operation::AlterModelOptions { table, .. } => {
				format!("Alter model options on {}", table)
			}
			Operation::CreateInheritedTable {
				name, base_table, ..
			} => {
				format!("Create inherited table {} from {}", name, base_table)
			}
			Operation::AddDiscriminatorColumn {
				table, column_name, ..
			} => format!("Add discriminator column {} to {}", column_name, table),
			Operation::MoveModel {
				model_name,
				from_app,
				to_app,
				..
			} => format!("Move model {} from {} to {}", model_name, from_app, to_app),
			Operation::CreateSchema { name, .. } => format!("Create schema {}", name),
			Operation::DropSchema { name, .. } => format!("Drop schema {}", name),
			Operation::CreateExtension { name, .. } => format!("Create extension {}", name),
			Operation::BulkLoad { table, source, .. } => {
				let source_desc = match source {
					BulkLoadSource::File(path) => format!("file '{}'", path),
					BulkLoadSource::Stdin => "STDIN".to_string(),
					BulkLoadSource::Program(cmd) => format!("program '{}'", cmd),
				};
				format!("Bulk load data into {} from {}", table, source_desc)
			}
			Operation::SetAutoIncrementValue {
				table,
				column,
				value,
			} => format!("Set auto-increment of {}.{} to {}", table, column, value),
			Operation::CreateCompositePrimaryKey { table, columns, .. } => format!(
				"Create composite primary key on {} ({})",
				table,
				columns.join(", ")
			),
		}
	}

	/// Normalize operation for semantic comparison
	///
	/// Returns a normalized version where order-independent elements are sorted.
	/// This enables detection of semantically equivalent operations regardless of element ordering.
	fn normalize(&self) -> Self
	where
		Self: Sized + Clone,
	{
		match self {
			// CreateTable: Sort columns and constraints
			Operation::CreateTable {
				name,
				columns,
				constraints,
				without_rowid,
				interleave_in_parent,
				partition,
			} => {
				let mut sorted_columns = columns.clone();
				sorted_columns.sort_by(|a, b| a.name.cmp(&b.name));

				let mut sorted_constraints = constraints.clone();
				sorted_constraints.sort();

				Operation::CreateTable {
					name: name.clone(),
					columns: sorted_columns,
					constraints: sorted_constraints,
					without_rowid: *without_rowid,
					interleave_in_parent: interleave_in_parent.clone(),
					partition: partition.clone(),
				}
			}
			// CreateIndex: Sort columns
			Operation::CreateIndex {
				table,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				let mut sorted_columns = columns.clone();
				sorted_columns.sort();

				Operation::CreateIndex {
					table: table.clone(),
					columns: sorted_columns,
					unique: *unique,
					index_type: *index_type,
					where_clause: where_clause.clone(),
					concurrently: *concurrently,
					expressions: expressions.clone(),
					mysql_options: *mysql_options,
					operator_class: operator_class.clone(),
				}
			}
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				let mut sorted_columns = columns.clone();
				sorted_columns.sort();

				Operation::CreateNamedIndex {
					table: table.clone(),
					name: name.clone(),
					columns: sorted_columns,
					unique: *unique,
					index_type: *index_type,
					where_clause: where_clause.clone(),
					concurrently: *concurrently,
					expressions: expressions.clone(),
					mysql_options: *mysql_options,
					operator_class: operator_class.clone(),
				}
			}
			// DropIndex: Sort columns
			Operation::DropIndex { table, columns } => {
				let mut sorted_columns = columns.clone();
				sorted_columns.sort();

				Operation::DropIndex {
					table: table.clone(),
					columns: sorted_columns,
				}
			}
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				let mut sorted_columns = columns.clone();
				sorted_columns.sort();
				Operation::DropNamedIndex {
					table: table.clone(),
					name: name.clone(),
					columns: sorted_columns,
					unique: *unique,
					index_type: *index_type,
					where_clause: where_clause.clone(),
					concurrently: *concurrently,
					expressions: expressions.clone(),
					mysql_options: *mysql_options,
					operator_class: operator_class.clone(),
				}
			}
			// AlterUniqueTogether: Sort field lists and sort within each list
			Operation::AlterUniqueTogether {
				table,
				unique_together,
			} => {
				let mut sorted_unique_together: Vec<Vec<String>> = unique_together
					.iter()
					.map(|field_list| {
						let mut sorted = field_list.clone();
						sorted.sort();
						sorted
					})
					.collect();
				sorted_unique_together.sort();

				Operation::AlterUniqueTogether {
					table: table.clone(),
					unique_together: sorted_unique_together,
				}
			}
			// AlterModelOptions: HashMap cannot be sorted, but we can normalize by converting to sorted Vec
			// However, since HashMap doesn't guarantee order and the operation uses HashMap,
			// we'll just clone it as-is. For true semantic equality, this would need to be changed
			// to a BTreeMap at the type level.
			Operation::AlterModelOptions { table, options } => Operation::AlterModelOptions {
				table: table.clone(),
				options: options.clone(),
			},
			// All other operations: Return clone (order doesn't matter or not applicable)
			_ => self.clone(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::autodetector::{ForeignKeyAction, ForeignKeyInfo};
	use FieldType;
	use reinhardt_query::prelude::SchemaBinOper;
	use rstest::rstest;

	#[test]
	fn sqlite_table_recreation_quotes_fallback_index_identifiers() {
		let recreation = SqliteTableRecreation::for_drop_column(
			"order\"items",
			vec![
				ColumnDefinition::new("id", FieldType::Integer),
				ColumnDefinition::new("select\"value", FieldType::Text),
				ColumnDefinition::new("obsolete", FieldType::Text),
			],
			"obsolete",
			vec![],
		)
		.with_indexes(vec![SqliteRecreatedIndex {
			name: "idx\"quoted".to_string(),
			columns: vec!["select\"value".to_string()],
			unique: false,
			sql: None,
		}]);

		let statements = recreation.to_sql_statements();

		assert_eq!(
			statements[0],
			"CREATE TABLE \"order\"\"items_new\" (\n  id INTEGER,\n  \"select\"\"value\" TEXT\n);"
		);
		assert_eq!(
			statements[1],
			"INSERT INTO \"order\"\"items_new\" (\"id\", \"select\"\"value\") SELECT \"id\", \"select\"\"value\" FROM \"order\"\"items\";"
		);
		assert_eq!(statements[2], "DROP TABLE \"order\"\"items\";");
		assert_eq!(
			statements[3],
			"ALTER TABLE \"order\"\"items_new\" RENAME TO \"order\"\"items\";"
		);
		assert_eq!(
			statements[4],
			"CREATE INDEX \"idx\"\"quoted\" ON \"order\"\"items\" (\"select\"\"value\");"
		);
	}

	#[test]
	fn test_create_table_to_statement() {
		let op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Integer,
					not_null: false,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
					generated: None,
					domain: None,
				},
				ColumnDefinition {
					name: "name".to_string(),
					type_definition: FieldType::VarChar(100),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("CREATE TABLE"),
			"SQL should contain CREATE TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("id") && sql.contains("name"),
			"SQL should contain both 'id' and 'name' columns, got: {}",
			sql
		);
	}

	#[test]
	fn test_drop_table_to_statement() {
		let op = Operation::DropTable {
			name: "users".to_string(),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("DROP TABLE"),
			"SQL should contain DROP TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("CASCADE"),
			"SQL should include CASCADE option, got: {}",
			sql
		);
	}

	#[test]
	fn test_add_column_to_statement() {
		let op = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition {
				name: "email".to_string(),
				type_definition: FieldType::VarChar(255),
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: Some("''".to_string()),
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should contain ALTER TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("ADD COLUMN"),
			"SQL should contain ADD COLUMN clause, got: {}",
			sql
		);
		assert!(
			sql.contains("email"),
			"SQL should reference 'email' column, got: {}",
			sql
		);
	}

	#[test]
	fn test_drop_column_to_statement() {
		let op = Operation::DropColumn {
			table: "users".to_string(),
			column: "email".to_string(),
			old_definition: None,
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should contain ALTER TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("DROP COLUMN"),
			"SQL should contain DROP COLUMN clause, got: {}",
			sql
		);
		assert!(
			sql.contains("email"),
			"SQL should reference 'email' column, got: {}",
			sql
		);
	}

	#[test]
	fn test_alter_column_to_statement() {
		let op = Operation::AlterColumn {
			table: "users".to_string(),
			column: "age".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition {
				name: "age".to_string(),
				type_definition: FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should contain ALTER TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("age"),
			"SQL should reference 'age' column, got: {}",
			sql
		);
	}

	#[test]
	fn test_rename_table_to_statement() {
		let op = Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "accounts".to_string(),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("users"),
			"SQL should reference old table name 'users', got: {}",
			sql
		);
		assert!(
			sql.contains("accounts"),
			"SQL should reference new table name 'accounts', got: {}",
			sql
		);
	}

	#[test]
	fn test_rename_column_to_statement() {
		let op = Operation::RenameColumn {
			table: "users".to_string(),
			old_name: "name".to_string(),
			new_name: "full_name".to_string(),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should contain ALTER TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("RENAME COLUMN"),
			"SQL should contain RENAME COLUMN clause, got: {}",
			sql
		);
		assert!(
			sql.contains("name"),
			"SQL should reference old column name 'name', got: {}",
			sql
		);
		assert!(
			sql.contains("full_name"),
			"SQL should reference new column name 'full_name', got: {}",
			sql
		);
	}

	#[test]
	fn test_add_constraint_to_statement() {
		let op = Operation::AddConstraint {
			table: "users".to_string(),
			constraint_sql: "CONSTRAINT age_check CHECK (age >= 0)".to_string(),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should contain ALTER TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("ADD"),
			"SQL should contain ADD keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("age_check"),
			"SQL should contain constraint name 'age_check', got: {}",
			sql
		);
	}

	#[test]
	fn enum_domain_constraint_escapes_string_literals() {
		let constraint = Constraint::EnumDomain {
			name: "jobs_status_model_enum_check".to_string(),
			column: "job_status".to_string(),
			domain: crate::field_domain::FieldDomain::Enum {
				repr: crate::field_domain::ModelEnumRepr::String,
				values: vec![
					crate::field_domain::ModelEnumValue::String("queued".to_string()),
					crate::field_domain::ModelEnumValue::String("owner's".to_string()),
				],
			},
		};

		assert_eq!(
			constraint.to_string(),
			"CONSTRAINT \"jobs_status_model_enum_check\" CHECK (\"job_status\" IN ('owner''s', 'queued'))"
		);
	}

	#[test]
	fn enum_domain_constraint_renders_integer_literals() {
		let constraint = Constraint::EnumDomain {
			name: "jobs_status_model_enum_check".to_string(),
			column: "status_code".to_string(),
			domain: crate::field_domain::FieldDomain::Enum {
				repr: crate::field_domain::ModelEnumRepr::I32,
				values: vec![
					crate::field_domain::ModelEnumValue::I32(10),
					crate::field_domain::ModelEnumValue::I32(2),
				],
			},
		};

		assert_eq!(
			constraint.to_string(),
			"CONSTRAINT \"jobs_status_model_enum_check\" CHECK (\"status_code\" IN (2, 10))"
		);
	}

	#[test]
	fn enum_domain_constraint_names_are_deterministic_and_bounded() {
		let logical = format!(
			"{}_{}_model_enum_check",
			"very_long_model_enum_table_name".repeat(2),
			"very_long_resolved_database_column".repeat(2)
		);
		let first = truncate_identifier_with_hash(&logical);
		let second = truncate_identifier_with_hash(&logical);

		assert_eq!(first, second);
		assert_eq!(first.len(), 63);
		assert_ne!(
			first,
			truncate_identifier_with_hash(&format!("{logical}_different"))
		);
	}

	#[test]
	fn enum_domain_constraint_name_has_a_stable_known_hash_suffix() {
		let logical = "model_enum_jobs_with_a_name_that_exceeds_postgres_identifier_limits_job_status_model_enum_check";

		assert_eq!(
			truncate_identifier_with_hash(logical),
			"model_enum_jobs_with_a_name_that_exceeds_postg_cb8793507fca29f1"
		);
	}

	#[test]
	fn enum_domain_constraint_uses_mysql_identifier_quoting_in_create_table() {
		let operation = Operation::CreateTable {
			name: "jobs".to_string(),
			columns: vec![],
			constraints: vec![Constraint::EnumDomain {
				name: "jobs_status_model_enum_check".to_string(),
				column: "job_status".to_string(),
				domain: crate::field_domain::FieldDomain::Enum {
					repr: crate::field_domain::ModelEnumRepr::String,
					values: vec![crate::field_domain::ModelEnumValue::String(
						"queued".to_string(),
					)],
				},
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};

		let sql = operation.to_sql(&SqlDialect::Mysql);

		assert!(
			sql.contains("CHECK (BINARY `job_status` IN ('queued'))"),
			"{sql}"
		);
	}

	#[test]
	fn create_table_state_restores_enum_domain_constraints() {
		let operation = Operation::CreateTable {
			name: "jobs".to_string(),
			columns: vec![ColumnDefinition::new("job_status", FieldType::VarChar(32))],
			constraints: vec![Constraint::EnumDomain {
				name: "jobs_status_model_enum_check".to_string(),
				column: "job_status".to_string(),
				domain: crate::field_domain::FieldDomain::Enum {
					repr: crate::field_domain::ModelEnumRepr::String,
					values: vec![crate::field_domain::ModelEnumValue::String(
						"queued".to_string(),
					)],
				},
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let mut state = ProjectState::new();

		operation.state_forwards("tasks", &mut state);

		let model = state.find_model_by_table("jobs").expect("jobs model");
		assert_eq!(model.constraints.len(), 1);
		assert_eq!(
			model.constraints[0].to_constraint().name(),
			"jobs_status_model_enum_check"
		);
	}

	#[test]
	fn drop_table_reverse_sql_uses_mysql_enum_identifier_quoting() {
		let operation = Operation::DropTable {
			name: "jobs".to_string(),
		};
		let mut state = ProjectState::new();
		let create_table = Operation::CreateTable {
			name: "jobs".to_string(),
			columns: vec![ColumnDefinition::new("job_status", FieldType::VarChar(32))],
			constraints: vec![Constraint::EnumDomain {
				name: "jobs_status_model_enum_check".to_string(),
				column: "job_status".to_string(),
				domain: crate::field_domain::FieldDomain::Enum {
					repr: crate::field_domain::ModelEnumRepr::String,
					values: vec![crate::field_domain::ModelEnumValue::String(
						"queued".to_string(),
					)],
				},
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		create_table.state_forwards("tasks", &mut state);

		let sql = operation
			.to_reverse_sql(&SqlDialect::Mysql, &state)
			.expect("reverse SQL should be generated")
			.expect("drop table should be reversible")
			.join("\n");

		assert!(
			sql.contains("CHECK (BINARY `job_status` IN ('queued'))"),
			"{sql}"
		);
	}

	#[test]
	fn enum_domain_constraint_uses_mysql_identifier_quoting_when_added() {
		let constraint = Constraint::EnumDomain {
			name: "jobs_status_model_enum_check".to_string(),
			column: "job_status".to_string(),
			domain: crate::field_domain::FieldDomain::Enum {
				repr: crate::field_domain::ModelEnumRepr::String,
				values: vec![crate::field_domain::ModelEnumValue::String(
					"queued".to_string(),
				)],
			},
		};
		let operation = Operation::AddConstraintDefinition {
			table: "jobs".to_string(),
			constraint,
		};

		let sql = operation.to_sql(&SqlDialect::Mysql);

		assert!(
			sql.contains("CHECK (BINARY `job_status` IN ('queued'))"),
			"{sql}"
		);
	}

	#[test]
	fn enum_domain_constraint_statement_preserves_mysql_dialect() {
		let operation = Operation::AddConstraintDefinition {
			table: "jobs".to_string(),
			constraint: Constraint::EnumDomain {
				name: "jobs_status_model_enum_check".to_string(),
				column: "job_status".to_string(),
				domain: crate::field_domain::FieldDomain::Enum {
					repr: crate::field_domain::ModelEnumRepr::String,
					values: vec![crate::field_domain::ModelEnumValue::String(
						"queued".to_string(),
					)],
				},
			},
		};

		let sql = operation
			.to_statement()
			.to_sql_string(crate::backends::types::DatabaseType::Mysql);

		assert!(
			sql.contains("ALTER TABLE `jobs` ADD CONSTRAINT `jobs_status_model_enum_check`"),
			"{sql}"
		);
		assert!(
			sql.contains("CHECK (BINARY `job_status` IN ('queued'))"),
			"{sql}"
		);
	}

	#[test]
	fn create_table_statement_preserves_enum_domain_check_for_mysql() {
		let operation = Operation::CreateTable {
			name: "jobs".to_string(),
			columns: vec![ColumnDefinition::new("job_status", FieldType::VarChar(32))],
			constraints: vec![Constraint::EnumDomain {
				name: "jobs_status_model_enum_check".to_string(),
				column: "job_status".to_string(),
				domain: crate::field_domain::FieldDomain::Enum {
					repr: crate::field_domain::ModelEnumRepr::String,
					values: vec![crate::field_domain::ModelEnumValue::String(
						"queued".to_string(),
					)],
				},
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};

		let sql = operation
			.to_statement()
			.to_sql_string(crate::backends::types::DatabaseType::Mysql);

		assert!(sql.starts_with("CREATE TABLE `jobs`"), "{sql}");
		assert!(sql.contains("`job_status` VARCHAR(32)"), "{sql}");
		assert!(
			sql.contains("CHECK (BINARY `job_status` IN ('queued'))"),
			"{sql}"
		);
	}

	#[test]
	fn enum_bearing_create_table_quotes_mixed_constraints_for_mysql() {
		let operation = Operation::CreateTable {
			name: "order-items".to_string(),
			columns: vec![
				ColumnDefinition::new("select", FieldType::VarChar(32)),
				ColumnDefinition::new("owner-id", FieldType::Integer),
				ColumnDefinition::new("profile-id", FieldType::Integer),
			],
			constraints: vec![
				Constraint::EnumDomain {
					name: "order-status-check".to_string(),
					column: "select".to_string(),
					domain: crate::field_domain::FieldDomain::Enum {
						repr: crate::field_domain::ModelEnumRepr::String,
						values: vec![crate::field_domain::ModelEnumValue::String(
							"queued".to_string(),
						)],
					},
				},
				Constraint::ForeignKey {
					name: "fk-order-owner".to_string(),
					columns: vec!["owner-id".to_string()],
					referenced_table: "user-table".to_string(),
					referenced_columns: vec!["primary-id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::NoAction,
					deferrable: None,
				},
				Constraint::Unique {
					name: "uq-order-select".to_string(),
					columns: vec!["select".to_string()],
				},
				Constraint::OneToOne {
					name: "oto-order-profile".to_string(),
					column: "profile-id".to_string(),
					referenced_table: "profile-table".to_string(),
					referenced_column: "primary-id".to_string(),
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::NoAction,
					deferrable: None,
				},
			],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};

		let sql = operation.to_sql(&SqlDialect::Mysql);

		assert!(
			sql.contains(
				"CONSTRAINT `fk-order-owner` FOREIGN KEY (`owner-id`) REFERENCES `user-table`(`primary-id`)"
			),
			"{sql}"
		);
		assert!(
			sql.contains("CONSTRAINT `uq-order-select` UNIQUE (`select`)"),
			"{sql}"
		);
		assert!(
			sql.contains("CONSTRAINT `oto-order-profile` FOREIGN KEY (`profile-id`) REFERENCES `profile-table`(`primary-id`)"),
			"{sql}"
		);
		assert!(
			sql.contains("CONSTRAINT `oto-order-profile_unique` UNIQUE (`profile-id`)"),
			"{sql}"
		);
	}

	#[test]
	fn drop_constraint_deserializes_legacy_shape_without_typed_snapshot() {
		let operation: Operation = serde_json::from_str(
			r#"{
				"type": "DropConstraint",
				"table": "jobs",
				"constraint_name": "jobs_status_check"
			}"#,
		)
		.expect("legacy DropConstraint JSON should remain readable");

		assert_eq!(
			operation,
			Operation::DropConstraint {
				table: "jobs".to_string(),
				constraint_name: "jobs_status_check".to_string(),
			}
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn drop_named_index_deserializes_legacy_shape_without_false_reverse_definition() {
		let operation: Operation = serde_json::from_str(
			r#"{
				"type": "DropNamedIndex",
				"table": "documents",
				"name": "documents_embedding_ann"
			}"#,
		)
		.expect("legacy DropNamedIndex JSON should remain readable");

		assert!(matches!(
			&operation,
			Operation::DropNamedIndex {
				table,
				name,
				columns,
				index_type: None,
				expressions: None,
				..
			} if table == "documents"
				&& name == "documents_embedding_ann"
				&& columns.is_empty()
		));
		assert_eq!(
			operation
				.to_reverse_sql(&SqlDialect::Postgres, &ProjectState::new())
				.expect("legacy reverse SQL decision"),
			None
		);
		assert_eq!(
			operation
				.to_reverse_operation(&ProjectState::new())
				.expect("legacy reverse operation decision"),
			None
		);
	}

	#[test]
	fn legacy_drop_constraint_rust_source_shape_still_typechecks() {
		let operation = Operation::DropConstraint {
			table: "users".to_string(),
			constraint_name: "users_status_check".to_string(),
		};

		assert!(matches!(operation, Operation::DropConstraint { .. }));
	}

	#[test]
	fn add_constraint_definition_state_transitions_are_symmetric() {
		let domain = crate::field_domain::FieldDomain::Enum {
			repr: crate::field_domain::ModelEnumRepr::String,
			values: vec![crate::field_domain::ModelEnumValue::String(
				"queued".to_string(),
			)],
		};
		let constraint = Constraint::EnumDomain {
			name: "jobs_status_model_enum_check".to_string(),
			column: "job_status".to_string(),
			domain: domain.clone(),
		};
		let operation = Operation::AddConstraintDefinition {
			table: "jobs".to_string(),
			constraint: constraint.clone(),
		};
		let mut state = ProjectState::new();
		let mut model = ModelState::new("tasks", "jobs");
		model.table_name = "jobs".to_string();
		state.add_model(model);

		operation.state_forwards("tasks", &mut state);

		let model = state.get_model("tasks", "jobs").expect("jobs model");
		assert_eq!(model.constraints.len(), 1);
		assert_eq!(model.constraints[0].name, "jobs_status_model_enum_check");
		assert_eq!(model.constraints[0].constraint_type, "enum_domain");
		assert_eq!(model.constraints[0].to_constraint(), constraint);

		operation.state_backwards("tasks", &mut state);

		assert!(
			state
				.get_model("tasks", "jobs")
				.expect("jobs model")
				.constraints
				.is_empty()
		);
	}

	#[test]
	fn add_constraint_definition_reverse_sql_uses_mysql_quoting() {
		let operation = Operation::AddConstraintDefinition {
			table: "jobs".to_string(),
			constraint: Constraint::EnumDomain {
				name: "jobs_status_model_enum_check".to_string(),
				column: "job_status".to_string(),
				domain: crate::field_domain::FieldDomain::Enum {
					repr: crate::field_domain::ModelEnumRepr::String,
					values: vec![crate::field_domain::ModelEnumValue::String(
						"queued".to_string(),
					)],
				},
			},
		};

		let sql = operation
			.to_reverse_sql(&SqlDialect::Mysql, &ProjectState::new())
			.expect("reverse SQL")
			.expect("reversible operation");

		assert_eq!(
			sql,
			vec!["ALTER TABLE `jobs` DROP CONSTRAINT `jobs_status_model_enum_check`;".to_string()]
		);
	}

	#[test]
	fn hashed_enum_domain_constraint_uses_mysql_identifier_quoting_when_added() {
		let table = "model_enum_jobs_with_a_name_that_exceeds_postgres_identifier_limits";
		let column = "job_status_with_a_name_that_exceeds_postgres_identifier_limits";
		let name = truncate_identifier_with_hash(&format!("{table}_{column}_model_enum_check"));
		let constraint = Constraint::EnumDomain {
			name,
			column: column.to_string(),
			domain: crate::field_domain::FieldDomain::Enum {
				repr: crate::field_domain::ModelEnumRepr::String,
				values: vec![crate::field_domain::ModelEnumValue::String(
					"queued".to_string(),
				)],
			},
		};
		let operation = Operation::AddConstraintDefinition {
			table: table.to_string(),
			constraint,
		};

		let sql = operation.to_sql(&SqlDialect::Mysql);

		assert!(
			sql.contains(&format!("CHECK (BINARY `{column}` IN ('queued'))")),
			"{sql}"
		);
	}

	#[test]
	fn create_table_state_forwards_preserves_column_domain() {
		let domain = crate::field_domain::FieldDomain::Enum {
			repr: crate::field_domain::ModelEnumRepr::I32,
			values: vec![crate::field_domain::ModelEnumValue::I32(1)],
		};
		let operation = Operation::CreateTable {
			name: "jobs".to_string(),
			columns: vec![
				ColumnDefinition::new("status", FieldType::Integer).with_domain(domain.clone()),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let mut state = ProjectState::new();

		operation.state_forwards("tasks", &mut state);

		assert_eq!(
			state.get_model("tasks", "jobs").unwrap().fields["status"].domain,
			Some(domain)
		);
	}

	#[test]
	fn add_column_state_forwards_preserves_column_domain() {
		let domain = crate::field_domain::FieldDomain::Enum {
			repr: crate::field_domain::ModelEnumRepr::I32,
			values: vec![crate::field_domain::ModelEnumValue::I32(1)],
		};
		let mut state = ProjectState::new();
		let mut model = ModelState::new("tasks", "jobs");
		model.table_name = "jobs".to_string();
		state.add_model(model);
		let operation = Operation::AddColumn {
			table: "jobs".to_string(),
			column: ColumnDefinition::new("status", FieldType::Integer).with_domain(domain.clone()),
			mysql_options: None,
		};

		operation.state_forwards("tasks", &mut state);

		assert_eq!(
			state.get_model("tasks", "jobs").unwrap().fields["status"].domain,
			Some(domain)
		);
	}

	#[test]
	fn alter_column_state_forwards_preserves_column_domain() {
		let domain = crate::field_domain::FieldDomain::Enum {
			repr: crate::field_domain::ModelEnumRepr::I32,
			values: vec![crate::field_domain::ModelEnumValue::I32(1)],
		};
		let mut model = ModelState::new("tasks", "jobs");
		model.table_name = "jobs".to_string();
		model.add_field(FieldState::new("status", FieldType::Integer, false));
		let mut state = ProjectState::new();
		state.add_model(model);
		let operation = Operation::AlterColumn {
			table: "jobs".to_string(),
			column: "status".to_string(),
			new_definition: ColumnDefinition::new("status", FieldType::Integer)
				.with_domain(domain.clone()),
			old_definition: None,
			mysql_options: None,
		};

		operation.state_forwards("tasks", &mut state);

		assert_eq!(
			state.get_model("tasks", "jobs").unwrap().fields["status"].domain,
			Some(domain)
		);
	}

	#[test]
	fn test_drop_constraint_to_statement() {
		let op = Operation::DropConstraint {
			table: "users".to_string(),
			constraint_name: "age_check".to_string(),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should contain ALTER TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("DROP CONSTRAINT"),
			"SQL should contain DROP CONSTRAINT clause, got: {}",
			sql
		);
		assert!(
			sql.contains("age_check"),
			"SQL should reference constraint 'age_check', got: {}",
			sql
		);
	}

	#[test]
	fn test_create_index_to_statement() {
		let op = Operation::CreateIndex {
			table: "users".to_string(),
			columns: vec!["email".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("CREATE INDEX"),
			"SQL should contain CREATE INDEX keywords, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("email"),
			"SQL should reference 'email' column, got: {}",
			sql
		);
	}

	#[test]
	fn test_create_unique_index_to_statement() {
		let op = Operation::CreateIndex {
			table: "users".to_string(),
			columns: vec!["email".to_string()],
			unique: true,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("CREATE UNIQUE INDEX"),
			"SQL should contain CREATE UNIQUE INDEX keywords, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("email"),
			"SQL should reference 'email' column, got: {}",
			sql
		);
	}

	#[test]
	fn test_drop_index_to_statement() {
		let op = Operation::DropIndex {
			table: "users".to_string(),
			columns: vec!["email".to_string()],
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("DROP INDEX"),
			"SQL should contain DROP INDEX keywords, got: {}",
			sql
		);
		assert!(
			sql.contains("idx_users_email"),
			"SQL should contain generated index name 'idx_users_email', got: {}",
			sql
		);
	}

	#[test]
	fn test_run_sql_to_statement() {
		let op = Operation::RunSQL {
			sql: "CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"".to_string(),
			reverse_sql: Some("DROP EXTENSION \"uuid-ossp\"".to_string()),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("CREATE EXTENSION"),
			"SQL should contain CREATE EXTENSION keywords, got: {}",
			sql
		);
		assert!(
			sql.contains("uuid-ossp"),
			"SQL should reference 'uuid-ossp' extension, got: {}",
			sql
		);
	}

	#[test]
	fn test_alter_table_comment_to_statement() {
		let op = Operation::AlterTableComment {
			table: "users".to_string(),
			comment: Some("User accounts table".to_string()),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("COMMENT ON TABLE"),
			"SQL should contain COMMENT ON TABLE keywords, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("User accounts table"),
			"SQL should include comment text 'User accounts table', got: {}",
			sql
		);
	}

	#[test]
	fn test_alter_table_comment_null_to_statement() {
		let op = Operation::AlterTableComment {
			table: "users".to_string(),
			comment: None,
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("COMMENT ON TABLE"),
			"SQL should contain COMMENT ON TABLE keywords, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("NULL"),
			"SQL should include NULL for null comment, got: {}",
			sql
		);
	}

	#[test]
	fn test_alter_unique_together_to_statement() {
		let op = Operation::AlterUniqueTogether {
			table: "users".to_string(),
			unique_together: vec![vec!["email".to_string(), "username".to_string()]],
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should contain ALTER TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("ADD CONSTRAINT"),
			"SQL should contain ADD CONSTRAINT clause, got: {}",
			sql
		);
		assert!(
			sql.contains("UNIQUE"),
			"SQL should contain UNIQUE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("email") && sql.contains("username"),
			"SQL should reference both 'email' and 'username' columns, got: {}",
			sql
		);
	}

	#[test]
	fn test_alter_unique_together_empty() {
		let op = Operation::AlterUniqueTogether {
			table: "users".to_string(),
			unique_together: vec![],
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert_eq!(
			sql, "",
			"SQL should be empty for empty unique_together constraint"
		);
	}

	#[test]
	fn test_alter_model_options_to_statement() {
		let mut options = std::collections::HashMap::new();
		options.insert("db_table".to_string(), "custom_users".to_string());

		let op = Operation::AlterModelOptions {
			table: "users".to_string(),
			options,
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert_eq!(sql, "", "SQL should be empty for model options operation");
	}

	#[test]
	fn test_create_inherited_table_to_statement() {
		let op = Operation::CreateInheritedTable {
			name: "admin_users".to_string(),
			columns: vec![ColumnDefinition {
				name: "admin_level".to_string(),
				type_definition: FieldType::Integer,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: Some("1".to_string()),
				generated: None,
				domain: None,
			}],
			base_table: "users".to_string(),
			join_column: "user_id".to_string(),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("CREATE TABLE"),
			"SQL should contain CREATE TABLE keywords, got: {}",
			sql
		);
		assert!(
			sql.contains("admin_users"),
			"SQL should reference 'admin_users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("user_id"),
			"SQL should include join column 'user_id', got: {}",
			sql
		);
	}

	#[test]
	fn test_add_discriminator_column_to_statement() {
		let op = Operation::AddDiscriminatorColumn {
			table: "users".to_string(),
			column_name: "user_type".to_string(),
			default_value: "regular".to_string(),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should contain ALTER TABLE keyword, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"SQL should reference 'users' table, got: {}",
			sql
		);
		assert!(
			sql.contains("ADD COLUMN"),
			"SQL should contain ADD COLUMN clause, got: {}",
			sql
		);
		assert!(
			sql.contains("user_type"),
			"SQL should reference 'user_type' column, got: {}",
			sql
		);
	}

	#[test]
	fn test_state_forwards_create_table() {
		let mut state = ProjectState::new();
		let op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Integer,
					not_null: false,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
					generated: None,
					domain: None,
				},
				ColumnDefinition {
					name: "name".to_string(),
					type_definition: FieldType::VarChar(100),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};

		op.state_forwards("myapp", &mut state);
		let model = state.get_model("myapp", "users");
		assert!(model.is_some(), "Model 'users' should exist in state");
		let model = model.unwrap();
		assert_eq!(model.table_name, "users");
		assert_eq!(
			model.fields.len(),
			2,
			"Model should have exactly 2 fields, got: {}",
			model.fields.len()
		);
		assert!(
			model.fields.contains_key("id"),
			"Model should contain 'id' field"
		);
		assert!(
			model.fields.contains_key("name"),
			"Model should contain 'name' field"
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn checked_vector_create_and_alter_sql_preserve_dimensions() {
		let vector_column = ColumnDefinition::new("embedding", FieldType::Vector { dimensions: 3 });
		let create = Operation::CreateTable {
			name: "documents".to_owned(),
			columns: vec![vector_column.clone()],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let alter = Operation::AlterColumn {
			table: "documents".to_owned(),
			column: "embedding".to_owned(),
			old_definition: None,
			new_definition: vector_column,
			mysql_options: None,
		};

		assert!(
			create
				.try_to_sql(&SqlDialect::Postgres)
				.unwrap()
				.contains("VECTOR(3)")
		);
		assert!(
			alter
				.try_to_sql(&SqlDialect::Postgres)
				.unwrap()
				.contains("TYPE VECTOR(3)")
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn checked_vector_ddl_rejects_unsupported_backends() {
		let operation = Operation::AddColumn {
			table: "documents".to_owned(),
			column: ColumnDefinition::new("embedding", FieldType::Vector { dimensions: 3 }),
			mysql_options: None,
		};

		for (dialect, backend) in [
			(SqlDialect::Mysql, "mysql"),
			(SqlDialect::Sqlite, "sqlite"),
			(SqlDialect::Cockroachdb, "cockroachdb"),
		] {
			assert!(matches!(
				operation.try_to_sql(&dialect),
				Err(crate::migrations::MigrationError::UnsupportedBackendFeature {
					feature: "vector field",
					backend: actual_backend,
				}) if actual_backend == backend
			));
		}
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn legacy_vector_operation_sql_fails_fast_on_unsupported_backends() {
		let operation = Operation::AddColumn {
			table: "documents".to_owned(),
			column: ColumnDefinition::new("embedding", FieldType::Vector { dimensions: 3 }),
			mysql_options: None,
		};

		for dialect in [SqlDialect::Mysql, SqlDialect::Sqlite] {
			let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				operation.to_sql(&dialect)
			}));

			let payload = result.expect_err("legacy rendering must not return vector SQL");
			let message = payload
				.downcast_ref::<String>()
				.map(String::as_str)
				.or_else(|| payload.downcast_ref::<&str>().copied())
				.unwrap();
			assert!(message.contains("try_to_sql"));
		}
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn checked_inherited_table_vector_ddl_rejects_unsupported_backends() {
		let operation = Operation::CreateInheritedTable {
			name: "child_documents".to_owned(),
			columns: vec![ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 3 },
			)],
			base_table: "documents".to_owned(),
			join_column: "document_id".to_owned(),
		};

		for (dialect, backend) in [
			(SqlDialect::Mysql, "mysql"),
			(SqlDialect::Sqlite, "sqlite"),
			(SqlDialect::Cockroachdb, "cockroachdb"),
		] {
			assert!(matches!(
				operation.try_to_sql(&dialect),
				Err(crate::migrations::MigrationError::UnsupportedBackendFeature {
					feature: "vector field",
					backend: actual_backend,
				}) if actual_backend == backend
			));
		}
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn checked_vector_statement_rendering_rejects_unsupported_backends() {
		let operation = Operation::CreateTable {
			name: "documents".to_owned(),
			columns: vec![ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 3 },
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let statement = operation.try_to_statement(&SqlDialect::Postgres).unwrap();

		for (database_type, backend) in [
			(crate::backends::types::DatabaseType::Mysql, "mysql"),
			(crate::backends::types::DatabaseType::Sqlite, "sqlite"),
		] {
			assert!(matches!(
				statement.try_to_sql_string(database_type),
				Err(crate::migrations::MigrationError::UnsupportedBackendFeature {
					feature: "pgvector column types",
					backend: actual_backend,
				}) if actual_backend == backend
			));
		}
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn legacy_vector_statement_rendering_fails_fast_on_unsupported_backends() {
		let operation = Operation::CreateTable {
			name: "documents".to_owned(),
			columns: vec![ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 3 },
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let statement = operation.to_statement();

		for database_type in [
			crate::backends::types::DatabaseType::Mysql,
			crate::backends::types::DatabaseType::Sqlite,
		] {
			let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				statement.to_sql_string(database_type)
			}));

			let payload =
				result.expect_err("legacy statement rendering must not return vector SQL");
			let message = payload
				.downcast_ref::<String>()
				.map(String::as_str)
				.or_else(|| payload.downcast_ref::<&str>().copied())
				.unwrap();
			assert!(message.contains("try_to_sql_string"));
		}
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn checked_vector_statement_construction_rejects_invalid_dimensions() {
		for dimensions in [0, 2_001, usize::MAX] {
			let operation = Operation::CreateTable {
				name: "documents".to_owned(),
				columns: vec![ColumnDefinition::new(
					"embedding",
					FieldType::Vector { dimensions },
				)],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			};

			assert!(matches!(
				operation.try_to_statement(&SqlDialect::Postgres),
				Err(crate::migrations::MigrationError::InvalidMigration(message))
					if message == format!(
						"vector dimensions must be between 1 and 2000, got {dimensions}"
					)
			));
		}
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn legacy_vector_statement_construction_rejects_invalid_dimensions_before_conversion() {
		for dimensions in [0, 2_001, usize::MAX] {
			let operation = Operation::CreateTable {
				name: "documents".to_owned(),
				columns: vec![ColumnDefinition::new(
					"embedding",
					FieldType::Vector { dimensions },
				)],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			};
			let result =
				std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation.to_statement()));

			let payload = match result {
				Err(payload) => payload,
				Ok(_) => panic!("legacy statement construction must reject invalid dimensions"),
			};
			let message = payload
				.downcast_ref::<String>()
				.map(String::as_str)
				.or_else(|| payload.downcast_ref::<&str>().copied())
				.unwrap();
			assert!(message.contains("try_to_statement"));
		}
	}

	#[test]
	fn test_state_forwards_drop_table() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new("id".to_string(), FieldType::Integer, false));
		state.add_model(model);

		let op = Operation::DropTable {
			name: "myapp_users".to_string(),
		};

		op.state_forwards("myapp", &mut state);
		assert!(
			state.get_model("myapp", "users").is_none(),
			"Model 'users' should be removed from state after drop"
		);
	}

	#[test]
	fn state_forwards_drops_a_table_after_it_was_renamed() {
		// Arrange
		let mut state = ProjectState::new();
		let mut model = ModelState::new("auth", "User");
		model.table_name = "users".to_string();
		state.add_model(model);

		// Act
		Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "auth_user".to_string(),
		}
		.state_forwards("auth", &mut state);
		Operation::DropTable {
			name: "auth_user".to_string(),
		}
		.state_forwards("auth", &mut state);

		// Assert
		assert!(state.get_model("auth", "User").is_none());
	}

	#[test]
	fn test_state_forwards_add_column() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new("id".to_string(), FieldType::Integer, false));
		state.add_model(model);

		let op = Operation::AddColumn {
			table: "myapp_users".to_string(),
			column: ColumnDefinition {
				name: "email".to_string(),
				type_definition: FieldType::VarChar(255),
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};

		op.state_forwards("myapp", &mut state);
		let model = state.get_model("myapp", "users").unwrap();
		assert_eq!(
			model.fields.len(),
			2,
			"Model should have 2 fields after adding 'email', got: {}",
			model.fields.len()
		);
		assert!(
			model.fields.contains_key("email"),
			"Model should contain newly added 'email' field"
		);
	}

	#[test]
	fn test_state_forwards_drop_column() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new("id".to_string(), FieldType::Integer, false));
		model.add_field(FieldState::new(
			"email".to_string(),
			FieldType::VarChar(255),
			false,
		));
		state.add_model(model);

		let op = Operation::DropColumn {
			table: "myapp_users".to_string(),
			column: "email".to_string(),
			old_definition: None,
		};

		op.state_forwards("myapp", &mut state);
		let model = state.get_model("myapp", "users").unwrap();
		assert_eq!(
			model.fields.len(),
			1,
			"Model should have 1 field after dropping 'email', got: {}",
			model.fields.len()
		);
		assert!(
			!model.fields.contains_key("email"),
			"Model should not contain dropped 'email' field"
		);
	}

	#[test]
	fn test_state_forwards_rename_table() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "User");
		model.table_name = "users".to_string();
		model.add_field(FieldState::new("id".to_string(), FieldType::Integer, false));
		state.add_model(model);

		let op = Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "accounts".to_string(),
		};

		op.state_forwards("myapp", &mut state);
		assert!(
			state.get_model("myapp", "User").is_some(),
			"Model identity should remain unchanged after a table rename"
		);
		assert_eq!(
			state.get_model("myapp", "User").unwrap().table_name,
			"accounts"
		);
	}

	#[test]
	fn state_forwards_rename_table_scopes_the_lookup_to_the_app() {
		let mut state = ProjectState::new();
		let mut accounts_model = ModelState::new("accounts", "User");
		accounts_model.table_name = "users".to_string();
		state.add_model(accounts_model);
		let mut audit_model = ModelState::new("audit", "User");
		audit_model.table_name = "users".to_string();
		state.add_model(audit_model);

		Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "accounts_user".to_string(),
		}
		.state_forwards("accounts", &mut state);

		assert_eq!(
			state.get_model("accounts", "User").unwrap().table_name,
			"accounts_user"
		);
		assert_eq!(
			state.get_model("audit", "User").unwrap().table_name,
			"users"
		);
	}

	#[test]
	fn state_forwards_uses_the_renamed_table_for_column_operations() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "User");
		model.table_name = "users".to_string();
		state.add_model(model);
		Operation::RenameTable {
			old_name: "users".into(),
			new_name: "user".into(),
		}
		.state_forwards("myapp", &mut state);
		Operation::AddColumn {
			table: "user".into(),
			column: ColumnDefinition::new("email", FieldType::VarChar(255)),
			mysql_options: None,
		}
		.state_forwards("myapp", &mut state);
		assert!(state.get_model("myapp", "User").unwrap().has_field("email"));
	}

	#[test]
	fn test_state_forwards_rename_table_updates_table_and_foreign_key_metadata() {
		let mut state = ProjectState::new();
		let mut user = ModelState::new("myapp", "User");
		user.table_name = "users".to_string();
		state.add_model(user);

		let mut post = ModelState::new("myapp", "Post");
		post.table_name = "posts".to_string();
		post.add_field(FieldState::with_foreign_key(
			"user_id",
			FieldType::Integer,
			false,
			ForeignKeyInfo {
				referenced_table: "users".to_string(),
				referenced_column: "id".to_string(),
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::Cascade,
			},
		));
		post.add_foreign_key_constraint_from_field("user_id");
		state.add_model(post);

		Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "user".to_string(),
		}
		.state_forwards("myapp", &mut state);

		assert_eq!(state.get_model("myapp", "User").unwrap().table_name, "user");
		let post = state.get_model("myapp", "Post").unwrap();
		assert_eq!(
			post.fields["user_id"]
				.foreign_key
				.as_ref()
				.unwrap()
				.referenced_table,
			"user"
		);
		assert_eq!(
			post.constraints[0]
				.foreign_key_info
				.as_ref()
				.unwrap()
				.referenced_table,
			"user"
		);
	}

	#[test]
	fn state_forwards_move_model_renames_foreign_key_references() {
		let mut state = ProjectState::new();
		let mut user = ModelState::new("accounts", "User");
		user.table_name = "users".to_string();
		state.add_model(user);
		let mut post = ModelState::new("blog", "Post");
		post.table_name = "posts".to_string();
		post.add_field(FieldState::with_foreign_key(
			"user_id",
			FieldType::Integer,
			false,
			ForeignKeyInfo {
				referenced_table: "users".to_string(),
				referenced_column: "id".to_string(),
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::Cascade,
			},
		));
		post.fields
			.get_mut("user_id")
			.unwrap()
			.params
			.insert("fk_target_app".to_string(), "accounts".to_string());
		post.fields
			.get_mut("user_id")
			.unwrap()
			.params
			.insert("fk_target_model".to_string(), "User".to_string());
		post.add_foreign_key_constraint_from_field("user_id");
		state.add_model(post);

		Operation::MoveModel {
			model_name: "User".to_string(),
			from_app: "accounts".to_string(),
			to_app: "auth".to_string(),
			rename_table: true,
			old_table_name: Some("users".to_string()),
			new_table_name: Some("auth_user".to_string()),
		}
		.state_forwards("auth", &mut state);

		assert_eq!(
			state.get_model("auth", "User").unwrap().table_name,
			"auth_user"
		);
		let post = state.get_model("blog", "Post").unwrap();
		assert_eq!(
			post.fields["user_id"]
				.foreign_key
				.as_ref()
				.unwrap()
				.referenced_table,
			"auth_user"
		);
		assert_eq!(
			post.constraints[0]
				.foreign_key_info
				.as_ref()
				.unwrap()
				.referenced_table,
			"auth_user"
		);
		assert_eq!(
			post.fields["user_id"].params.get("fk_target_app"),
			Some(&"auth".to_string())
		);
		assert_eq!(
			post.fields["user_id"].params.get("fk_target_model"),
			Some(&"User".to_string())
		);
	}

	#[test]
	fn state_forwards_move_model_updates_foreign_key_field_target_app() {
		let mut state = ProjectState::new();
		state.add_model(ModelState::new("accounts", "User"));

		let mut post = ModelState::new("blog", "Post");
		let mut field = FieldState::new("user_id", FieldType::Uuid, false);
		field
			.params
			.insert("fk_target_app".to_string(), "accounts".to_string());
		field
			.params
			.insert("fk_target".to_string(), "User".to_string());
		post.add_field(field);
		state.add_model(post);

		Operation::MoveModel {
			model_name: "User".to_string(),
			from_app: "accounts".to_string(),
			to_app: "auth".to_string(),
			rename_table: false,
			old_table_name: None,
			new_table_name: None,
		}
		.state_forwards("auth", &mut state);

		assert_eq!(
			state.get_model("blog", "Post").unwrap().fields["user_id"]
				.params
				.get("fk_target_app"),
			Some(&"auth".to_string())
		);
	}

	#[test]
	fn test_state_forwards_rename_column() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new(
			"name".to_string(),
			FieldType::VarChar(255),
			false,
		));
		state.add_model(model);

		let op = Operation::RenameColumn {
			table: "myapp_users".to_string(),
			old_name: "name".to_string(),
			new_name: "full_name".to_string(),
		};

		op.state_forwards("myapp", &mut state);
		let model = state.get_model("myapp", "users").unwrap();
		assert!(
			!model.fields.contains_key("name"),
			"Old field name 'name' should not exist after rename"
		);
		assert!(
			model.fields.contains_key("full_name"),
			"New field name 'full_name' should exist after rename"
		);
	}

	#[test]
	fn test_to_reverse_sql_create_table() {
		let op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};

		let state = ProjectState::default();
		let reverse = op.to_reverse_sql(&SqlDialect::Postgres, &state);
		assert!(
			reverse.is_ok() && reverse.as_ref().ok().unwrap().is_some(),
			"CreateTable should have reverse SQL operation"
		);
		let sql = reverse.unwrap().unwrap().join("\n");
		assert!(
			sql.contains("DROP TABLE"),
			"Reverse SQL should contain DROP TABLE, got: {}",
			sql
		);
		assert!(
			sql.contains("users"),
			"Reverse SQL should reference 'users' table, got: {}",
			sql
		);
	}

	#[test]
	fn test_to_reverse_sql_drop_table() {
		let op = Operation::DropTable {
			name: "users".to_string(),
		};

		let state = ProjectState::default();
		let reverse = op.to_reverse_sql(&SqlDialect::Postgres, &state);
		assert!(
			reverse.is_ok() && reverse.as_ref().ok().unwrap().is_none(),
			"DropTable should not have reverse SQL (cannot recreate table structure)"
		);
	}

	#[test]
	fn test_to_reverse_sql_add_column() {
		let op = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition {
				name: "email".to_string(),
				type_definition: FieldType::VarChar(255),
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};

		let state = ProjectState::default();
		let reverse = op.to_reverse_sql(&SqlDialect::Postgres, &state);
		assert!(
			reverse.is_ok() && reverse.as_ref().ok().unwrap().is_some(),
			"AddColumn should have reverse SQL operation"
		);
		let sql = reverse.unwrap().unwrap().join("\n");
		assert!(
			sql.contains("DROP COLUMN"),
			"Reverse SQL should contain DROP COLUMN, got: {}",
			sql
		);
		assert!(
			sql.contains("email"),
			"Reverse SQL should reference 'email' column, got: {}",
			sql
		);
	}

	/// Build an [`Operation::AlterColumn`] that carries an `old_definition`,
	/// so reverse-SQL generation has all the inputs it needs without a
	/// populated `ProjectState`. Used by the dialect-dispatch regression
	/// tests for reinhardt-web#4582.
	fn alter_column_with_old_def() -> Operation {
		Operation::AlterColumn {
			table: "products".to_string(),
			column: "name".to_string(),
			old_definition: Some(ColumnDefinition {
				name: "name".to_string(),
				type_definition: FieldType::VarChar(50),
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			}),
			new_definition: ColumnDefinition {
				name: "name".to_string(),
				type_definition: FieldType::Text,
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
			mysql_options: None,
		}
	}

	/// Reverse SQL for Postgres must use `ALTER COLUMN ... TYPE` syntax
	/// (regression coverage for reinhardt-web#4582).
	#[test]
	fn test_to_reverse_sql_alter_column_postgres() {
		// Arrange
		let op = alter_column_with_old_def();
		let state = ProjectState::default();

		// Act: reverse SQL is now returned as a multi-statement `Vec<String>`
		// (see `Operation::to_reverse_sql` doc); flatten to a single string for
		// the substring-based assertions below.
		let stmts = op
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.expect("reverse SQL should succeed")
			.expect("reverse SQL should be present");
		let sql = stmts.join("\n");

		// Assert
		assert!(
			sql.contains("ALTER COLUMN") && sql.contains("TYPE"),
			"Postgres reverse SQL should use ALTER COLUMN ... TYPE syntax, got: {}",
			sql
		);
		assert!(
			sql.contains("VARCHAR(50)"),
			"Postgres reverse SQL should restore VARCHAR(50), got: {}",
			sql
		);
		// Multi-statement contract (#4640): type reversion and nullability
		// restoration are emitted as two independent statements so
		// `SchemaEditor::execute()` (sqlx Extended Query) can dispatch each
		// payload without rejection.
		assert_eq!(
			stmts.len(),
			2,
			"Postgres AlterColumn reverse SQL must emit two statements \
			 (type + nullability), got: {:?}",
			stmts
		);
		assert!(
			stmts[1].contains("DROP NOT NULL"),
			"Postgres second statement must restore DROP NOT NULL (was_nullable), got: {}",
			stmts[1]
		);
	}

	/// Reverse SQL for MySQL must use `MODIFY COLUMN` syntax — the previous
	/// implementation emitted Postgres `ALTER COLUMN ... TYPE` which MySQL
	/// rejects with error 1064 (reinhardt-web#4582).
	#[test]
	fn test_to_reverse_sql_alter_column_mysql() {
		// Arrange
		let op = alter_column_with_old_def();
		let state = ProjectState::default();

		// Act: MySQL AlterColumn reverse SQL remains a single `MODIFY COLUMN`
		// statement (one-element `Vec`).
		let stmts = op
			.to_reverse_sql(&SqlDialect::Mysql, &state)
			.expect("reverse SQL should succeed")
			.expect("reverse SQL should be present");
		assert_eq!(
			stmts.len(),
			1,
			"MySQL AlterColumn reverse SQL should remain a single statement, got: {:?}",
			stmts
		);
		let sql = stmts.join("\n");

		// Assert
		assert!(
			sql.contains("MODIFY COLUMN"),
			"MySQL reverse SQL should use MODIFY COLUMN syntax, got: {}",
			sql
		);
		assert!(
			!sql.contains("ALTER COLUMN"),
			"MySQL reverse SQL must not emit Postgres ALTER COLUMN syntax, got: {}",
			sql
		);
		assert!(
			!sql.contains(" TYPE "),
			"MySQL reverse SQL must not contain Postgres ' TYPE ' token, got: {}",
			sql
		);
		assert!(
			sql.contains("VARCHAR(50)"),
			"MySQL reverse SQL should restore VARCHAR(50), got: {}",
			sql
		);
	}

	#[rstest]
	#[case::postgres(SqlDialect::Postgres)]
	#[case::cockroachdb(SqlDialect::Cockroachdb)]
	fn test_to_sql_alter_column_sets_default_for_postgres_family(#[case] dialect: SqlDialect) {
		// Arrange
		let op = Operation::AlterColumn {
			table: "users".to_string(),
			column: "is_active".to_string(),
			old_definition: Some(ColumnDefinition {
				name: "is_active".to_string(),
				type_definition: FieldType::Boolean,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			}),
			new_definition: ColumnDefinition {
				name: "is_active".to_string(),
				type_definition: FieldType::Boolean,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: Some("true".to_string()),
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};

		// Act
		let sql = op.to_sql(&dialect);

		// Assert
		assert!(
			sql.contains("ALTER COLUMN is_active SET DEFAULT true"),
			"AlterColumn must apply new database defaults, got: {}",
			sql
		);
	}

	#[rstest]
	#[case::postgres(SqlDialect::Postgres)]
	#[case::cockroachdb(SqlDialect::Cockroachdb)]
	fn test_to_sql_alter_column_drops_default_for_postgres_family(#[case] dialect: SqlDialect) {
		// Arrange
		let op = Operation::AlterColumn {
			table: "users".to_string(),
			column: "is_active".to_string(),
			old_definition: Some(ColumnDefinition {
				name: "is_active".to_string(),
				type_definition: FieldType::Boolean,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: Some("true".to_string()),
				generated: None,
				domain: None,
			}),
			new_definition: ColumnDefinition {
				name: "is_active".to_string(),
				type_definition: FieldType::Boolean,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};

		// Act
		let sql = op.to_sql(&dialect);

		// Assert
		assert!(
			sql.contains("ALTER COLUMN is_active DROP DEFAULT"),
			"AlterColumn must remove dropped database defaults, got: {}",
			sql
		);
	}

	#[test]
	fn test_to_sql_alter_column_mysql_preserves_full_column_definition() {
		// Arrange
		let op = Operation::AlterColumn {
			table: "users".to_string(),
			column: "is_active".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition {
				name: "is_active".to_string(),
				type_definition: FieldType::Boolean,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: Some("true".to_string()),
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};

		// Act
		let sql = op.to_sql(&SqlDialect::Mysql);

		// Assert
		assert!(
			sql.contains("MODIFY COLUMN is_active TINYINT(1) NOT NULL DEFAULT true"),
			"MySQL AlterColumn must include type, nullability, and default, got: {}",
			sql
		);
	}

	/// Reverse SQL for CockroachDB must emit the column-type reversion **and**
	/// the nullability restoration as two independent single-statement payloads
	/// — the same shape PostgreSQL uses post-#4640.
	///
	/// History: PR #4633 split CockroachDB off from PostgreSQL because
	/// CockroachDB rejects the comma-combined `ALTER COLUMN ... TYPE T,
	/// ALTER COLUMN ... {SET|DROP} NOT NULL` form that PostgreSQL accepts.
	/// The interim stop-gap (#4633) emitted only the column-type reversion
	/// and dropped NOT NULL rollback fidelity. This Issue (#4640) restored
	/// full fidelity by changing `Operation::to_reverse_sql` to return
	/// `Vec<String>`, so each statement is dispatched separately by
	/// `SchemaEditor::execute()` (sqlx Extended Query, single-statement).
	///
	/// This test pins the post-#4640 multi-statement contract: a future
	/// regression that re-folds CockroachDB into a single payload (and thus
	/// drops nullability) — or that emits the Postgres comma-combined form —
	/// will fail here at `cargo test` time instead of at downstream
	/// CockroachDB deployments.
	///
	/// Identifier quoting note: `quote_identifier` here is
	/// `pg_escape::quote_identifier`, which only quotes identifiers that fall
	/// outside PostgreSQL's unquoted-identifier grammar (anything outside
	/// `[a-z_][a-z0-9_]*`, or a reserved keyword). `products` and `name`
	/// are plain lowercase ASCII matching that grammar, so the expected
	/// output is unquoted. Identifier-quoting-by-default is tracked
	/// separately in #4674.
	#[test]
	fn test_to_reverse_sql_alter_column_cockroachdb() {
		// Arrange
		let op = alter_column_with_old_def();
		let state = ProjectState::default();

		// Act
		let stmts = op
			.to_reverse_sql(&SqlDialect::Cockroachdb, &state)
			.expect("reverse SQL should succeed")
			.expect("reverse SQL should be present");

		// Assert: exact two-element Vec, type reversion first, nullability
		// restoration second. `not_null` on the `old_definition` (see
		// `alter_column_with_old_def`) is `false`, so the second statement
		// is `DROP NOT NULL`. Asserting `==` (not `contains`) is
		// intentional — a permissive `contains` check would allow a future
		// regression to re-fold these into a single comma-combined payload
		// (which CockroachDB rejects).
		assert_eq!(
			stmts,
			vec![
				"ALTER TABLE products ALTER COLUMN name TYPE VARCHAR(50);".to_string(),
				"ALTER TABLE products ALTER COLUMN name DROP NOT NULL;".to_string(),
			],
			"CockroachDB reverse SQL must emit exactly [type_stmt, nullability_stmt], \
			 got: {:?}",
			stmts
		);

		// Defensive: every emitted statement must be a single SQL statement.
		// `SchemaEditor::execute()` is a single-statement dispatcher (sqlx
		// Extended Query); a regression that re-introduces the Postgres
		// comma-combined form inside a single Vec element would re-break
		// CockroachDB rollback. Trim trailing `;` so the check looks only at
		// internal separators.
		for stmt in &stmts {
			let trimmed = stmt.trim().trim_end_matches(';').trim();
			assert!(
				!trimmed.contains(';'),
				"each emitted statement must be a single SQL statement, got: {}",
				stmt
			);
			assert!(
				!stmt.contains(", ALTER COLUMN"),
				"emitted statements must not use the Postgres comma-combined form \
				 (CockroachDB rejects it), got: {}",
				stmt
			);
		}
	}

	/// SQLite has no general `ALTER COLUMN`; the rollback path is handled
	/// via table recreation in the executor. `to_reverse_sql` therefore
	/// returns an inert comment so that any accidental fall-through does
	/// not produce executable SQL that SQLite would reject
	/// (reinhardt-web#4582).
	#[test]
	fn test_to_reverse_sql_alter_column_sqlite() {
		// Arrange
		let op = alter_column_with_old_def();
		let state = ProjectState::default();

		// Act: SQLite emits a single inert comment (executor recreates the
		// table via the SQLite-recreation path), so the returned `Vec` is
		// one element.
		let stmts = op
			.to_reverse_sql(&SqlDialect::Sqlite, &state)
			.expect("reverse SQL should succeed")
			.expect("reverse SQL should be present");
		assert_eq!(
			stmts.len(),
			1,
			"SQLite AlterColumn reverse SQL should remain a single comment, got: {:?}",
			stmts
		);
		let sql = &stmts[0];

		// Assert
		assert!(
			sql.trim_start().starts_with("--"),
			"SQLite reverse SQL should be a SQL comment (recreation handled by executor), got: {}",
			sql
		);
		// Strip leading `--` (and any whitespace) before checking; the comment
		// body itself is allowed to mention ALTER COLUMN as English prose.
		let body = sql.trim_start_matches("--").trim_start();
		assert!(
			!body.to_uppercase().contains("ALTER TABLE"),
			"SQLite reverse SQL body must not emit executable ALTER TABLE statement, got: {}",
			sql
		);
	}

	/// `to_reverse_operation` for `AlterColumn` must prefer the explicit
	/// `old_definition` carried by the forward operation, instead of
	/// always falling back to an (often empty) `ProjectState` lookup
	/// (reinhardt-web#4582).
	#[test]
	fn test_to_reverse_operation_alter_column_uses_old_definition() {
		// Arrange
		let op = alter_column_with_old_def();
		let state = ProjectState::default();

		// Act
		let reverse = op
			.to_reverse_operation(&state)
			.expect("reverse operation should succeed")
			.expect("reverse operation should be present (old_definition is supplied)");

		// Assert
		match reverse {
			Operation::AlterColumn { new_definition, .. } => {
				assert!(
					matches!(new_definition.type_definition, FieldType::VarChar(50)),
					"reverse AlterColumn should restore VARCHAR(50), got: {:?}",
					new_definition.type_definition
				);
			}
			other => panic!("reverse operation should be AlterColumn, got: {:?}", other),
		}
	}

	/// `DropColumn` operations emitted for generated-column replacements carry
	/// the dropped definition so rollback can recreate the original generated
	/// column without relying on an external ProjectState snapshot.
	#[test]
	fn test_to_reverse_operation_drop_column_uses_old_definition() {
		// Arrange
		let generated = GeneratedColumnDefinition::typed(
			SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			GeneratedStorage::Stored,
		);
		let op = Operation::DropColumn {
			table: "users".to_string(),
			column: "full_name".to_string(),
			old_definition: Some(ColumnDefinition {
				name: "full_name".to_string(),
				type_definition: FieldType::VarChar(201),
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: Some(generated.clone()),
				domain: None,
			}),
		};
		let state = ProjectState::default();

		// Act
		let reverse = op
			.to_reverse_operation(&state)
			.expect("reverse operation should succeed")
			.expect("reverse operation should be present (old_definition is supplied)");

		// Assert
		match reverse {
			Operation::AddColumn { column, .. } => {
				assert_eq!(column.name, "full_name");
				assert_eq!(column.generated, Some(generated));
			}
			other => panic!("reverse operation should be AddColumn, got: {:?}", other),
		}
	}

	#[test]
	fn test_to_reverse_operation_drop_table_orders_generated_dependencies() {
		// Arrange
		let mut source = FieldState::new("source", FieldType::Integer, false);
		source.generated = None;
		let mut z = FieldState::new("z", FieldType::Integer, false);
		z.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("source"),
			"SchemaExpr::col(\"source\")",
			GeneratedStorage::Stored,
		));
		let mut a = FieldState::new("a", FieldType::Integer, false);
		a.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("z"),
			"SchemaExpr::col(\"z\")",
			GeneratedStorage::Stored,
		));
		let mut model = ModelState::new("metrics", "Metric");
		model.add_field(a);
		model.add_field(source);
		model.add_field(z);
		let mut state = ProjectState::new();
		state.add_model(model);
		let op = Operation::DropTable {
			name: "metrics_metric".to_string(),
		};

		// Act
		let reverse = op
			.to_reverse_operation(&state)
			.expect("reverse operation should succeed")
			.expect("reverse operation should be present");

		// Assert
		let Operation::CreateTable { columns, .. } = reverse else {
			panic!("reverse operation should be CreateTable, got: {reverse:?}");
		};
		let column_names: Vec<_> = columns.iter().map(|column| column.name.as_str()).collect();
		assert_eq!(column_names, vec!["source", "z", "a"]);
	}

	#[test]
	#[should_panic(expected = "generated-column dependency cycle detected among columns")]
	fn test_to_reverse_operation_drop_table_rejects_generated_dependency_cycles() {
		let mut a = FieldState::new("a", FieldType::Integer, false);
		a.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("b"),
			"SchemaExpr::col(\"b\")",
			GeneratedStorage::Stored,
		));
		let mut b = FieldState::new("b", FieldType::Integer, false);
		b.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("a"),
			"SchemaExpr::col(\"a\")",
			GeneratedStorage::Stored,
		));
		let mut model = ModelState::new("metrics", "Metric");
		model.add_field(a);
		model.add_field(b);
		let mut state = ProjectState::new();
		state.add_model(model);
		let op = Operation::DropTable {
			name: "metrics_metric".to_string(),
		};

		let _ = op.to_reverse_operation(&state);
	}

	#[test]
	fn generated_dependency_repair_operations_are_rollback_neutral() {
		let state = ProjectState::default();
		let create_repair = Operation::CreateIndexRepair {
			table: "users".to_string(),
			name: None,
			columns: vec!["full_name".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let restore_index = Operation::RestoreIndexOnRollback {
			table: "users".to_string(),
			name: None,
			columns: vec!["full_name".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let add_repair = Operation::AddConstraintRepair {
			table: "users".to_string(),
			constraint_sql: "CONSTRAINT uq_users_full_name UNIQUE (full_name)".to_string(),
		};
		let restore_constraint = Operation::RestoreConstraintOnRollback {
			table: "users".to_string(),
			constraint_sql: "CONSTRAINT uq_users_full_name UNIQUE (full_name)".to_string(),
		};

		assert_eq!(
			create_repair
				.to_reverse_sql(&SqlDialect::Postgres, &state)
				.expect("repair reverse should not fail"),
			None
		);
		assert_eq!(
			add_repair
				.to_reverse_sql(&SqlDialect::Postgres, &state)
				.expect("repair reverse should not fail"),
			None
		);
		assert_eq!(
			restore_index
				.to_reverse_sql(&SqlDialect::Postgres, &state)
				.expect("restore reverse should render"),
			Some(vec![
				"CREATE INDEX idx_users_full_name ON users (full_name);".to_string()
			])
		);
		assert_eq!(
			restore_constraint
				.to_reverse_sql(&SqlDialect::Postgres, &state)
				.expect("restore reverse should render"),
			Some(vec![
				"ALTER TABLE users ADD CONSTRAINT uq_users_full_name UNIQUE (full_name);"
					.to_string()
			])
		);
		assert!(restore_constraint.reverse_requires_sqlite_recreation());
	}

	#[test]
	fn stored_generated_drop_column_reverse_uses_sqlite_recreation() {
		let generated = GeneratedColumnDefinition::typed(
			SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			GeneratedStorage::Stored,
		);
		let op = Operation::DropColumn {
			table: "users".to_string(),
			column: "full_name".to_string(),
			old_definition: Some(ColumnDefinition {
				name: "full_name".to_string(),
				type_definition: FieldType::VarChar(201),
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: Some(generated),
				domain: None,
			}),
		};

		assert!(op.reverse_requires_sqlite_recreation());
	}

	#[test]
	fn test_to_reverse_sql_run_sql_with_reverse() {
		let op = Operation::RunSQL {
			sql: "CREATE INDEX idx_name ON users(name)".to_string(),
			reverse_sql: Some("DROP INDEX idx_name".to_string()),
		};

		let state = ProjectState::default();
		let reverse = op.to_reverse_sql(&SqlDialect::Postgres, &state);
		assert!(
			reverse.is_ok() && reverse.as_ref().ok().unwrap().is_some(),
			"RunSQL with reverse_sql should have reverse SQL"
		);
		let sql = reverse.unwrap().unwrap().join("\n");
		assert!(
			sql.contains("DROP INDEX"),
			"Reverse SQL should contain provided reverse_sql, got: {}",
			sql
		);
	}

	#[test]
	fn test_to_reverse_sql_run_sql_without_reverse() {
		let op = Operation::RunSQL {
			sql: "CREATE INDEX idx_name ON users(name)".to_string(),
			reverse_sql: None,
		};

		let state = ProjectState::default();
		let reverse = op.to_reverse_sql(&SqlDialect::Postgres, &state);
		assert!(
			reverse.is_ok() && reverse.as_ref().ok().unwrap().is_none(),
			"RunSQL without reverse_sql should not have reverse SQL"
		);
	}

	#[test]
	fn test_column_definition_new() {
		let col = ColumnDefinition::new("id", FieldType::Integer);
		assert_eq!(col.name, "id", "Column name should be 'id'");
		assert_eq!(
			col.type_definition,
			FieldType::Integer,
			"Column type should be Integer"
		);
		assert!(!col.not_null, "not_null should default to false");
		assert!(!col.unique, "unique should default to false");
		assert!(!col.primary_key, "primary_key should default to false");
		assert!(
			!col.auto_increment,
			"auto_increment should default to false"
		);
		assert!(col.default.is_none(), "default should be None");
	}

	// -----------------------------------------------------------------------
	// Regression tests for issue #4573:
	// `ColumnDefinition::from_field_state` previously read a non-existent
	// `params["not_null"]` key and silently emitted NULLABLE columns for
	// every non-PK field, violating the non-Optional Rust type ↔ NOT NULL
	// schema contract. These tests pin the corrected behavior:
	//   not_null = !field_state.nullable || primary_key
	// -----------------------------------------------------------------------

	#[rstest]
	fn from_field_state_non_optional_bool_with_true_default() {
		// Arrange
		// Models a Rust field declared as `pub is_active: bool` with
		// `#[field(default = true)]`. The proc-macro sets
		// `field_state.nullable = false` via `FieldMetadata::with_nullable`,
		// and emits the default value as `params["default"] = "true"`.
		let mut field_state = FieldState::new("is_active", FieldType::Boolean, false);
		field_state
			.params
			.insert("default".to_string(), "true".to_string());

		// Act
		let col = ColumnDefinition::from_field_state("is_active", &field_state);

		// Assert
		assert_eq!(col.name, "is_active", "Column name should round-trip");
		assert_eq!(
			col.type_definition,
			FieldType::Boolean,
			"Boolean field type should round-trip"
		);
		assert!(
			col.not_null,
			"Non-Optional bool must emit NOT NULL (regression #4573)"
		);
		assert_eq!(
			col.default,
			Some("true".to_string()),
			"`#[field(default = true)]` must propagate as Some(\"true\")"
		);
		assert!(!col.primary_key, "Non-PK field must not be primary_key");
	}

	#[rstest]
	fn from_field_state_non_optional_bool_with_false_default() {
		// Arrange
		// Models a Rust field declared as `pub is_superuser: bool` with
		// `#[field(default = false)]` — the symptom previously hand-patched
		// in PR #4513.
		let mut field_state = FieldState::new("is_superuser", FieldType::Boolean, false);
		field_state
			.params
			.insert("default".to_string(), "false".to_string());

		// Act
		let col = ColumnDefinition::from_field_state("is_superuser", &field_state);

		// Assert
		assert!(
			col.not_null,
			"Non-Optional bool with default=false must emit NOT NULL"
		);
		assert_eq!(
			col.default,
			Some("false".to_string()),
			"default=false must propagate as Some(\"false\")"
		);
	}

	#[rstest]
	fn from_field_state_optional_bool_with_default() {
		// Arrange
		// Models a Rust field declared as `pub maybe_flag: Option<bool>` with
		// `#[field(default = true)]`. Existing behavior must be preserved:
		// nullable column with default still set.
		let mut field_state = FieldState::new("maybe_flag", FieldType::Boolean, true);
		field_state
			.params
			.insert("default".to_string(), "true".to_string());

		// Act
		let col = ColumnDefinition::from_field_state("maybe_flag", &field_state);

		// Assert
		assert!(
			!col.not_null,
			"Optional bool must remain NULLABLE — no regression on Option<T>"
		);
		assert_eq!(
			col.default,
			Some("true".to_string()),
			"Default propagation must work for Optional fields too"
		);
	}

	#[rstest]
	fn from_field_state_non_optional_non_bool() {
		// Arrange
		// Models a Rust field declared as `pub username: String` with no
		// default. This confirms the fix is type-agnostic (not bool-only) and
		// prevents required string fields from being emitted as NULLABLE.
		let field_state = FieldState::new("username", FieldType::VarChar(150), false);

		// Act
		let col = ColumnDefinition::from_field_state("username", &field_state);

		// Assert
		assert!(
			col.not_null,
			"Non-Optional String must emit NOT NULL (regression #4573 — bug \
			 affected all field types, not just bool)"
		);
		assert!(
			col.default.is_none(),
			"No default annotation → default = None"
		);
	}

	#[rstest]
	fn from_field_state_primary_key_is_always_not_null() {
		// Arrange
		// A primary key column must be NOT NULL even when the nullability
		// flag would otherwise allow NULL. This pins the `|| primary_key`
		// short-circuit in the corrected expression.
		let mut field_state = FieldState::new("id", FieldType::Uuid, true);
		field_state
			.params
			.insert("primary_key".to_string(), "true".to_string());

		// Act
		let col = ColumnDefinition::from_field_state("id", &field_state);

		// Assert
		assert!(
			col.primary_key,
			"primary_key param must propagate to ColumnDefinition"
		);
		assert!(
			col.not_null,
			"Primary key must be NOT NULL regardless of nullable flag"
		);
	}

	#[rstest]
	fn from_field_state_optional_field_remains_nullable() {
		// Arrange
		// Models a Rust field declared as `pub last_login: Option<DateTime<Utc>>`
		// with no default. Existing nullable-field behavior must be preserved.
		let field_state = FieldState::new("last_login", FieldType::TimestampTz, true);

		// Act
		let col = ColumnDefinition::from_field_state("last_login", &field_state);

		// Assert
		assert!(
			!col.not_null,
			"Optional field with no default must remain NULLABLE"
		);
		assert!(col.default.is_none(), "No default → default = None");
		assert!(!col.primary_key, "Non-PK field must not be primary_key");
	}

	#[test]
	fn test_convert_default_value_null() {
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let value = op.convert_default_value("null");
		assert!(
			matches!(value, Value::String(None)),
			"NULL value should be converted to Value::String(None)"
		);
	}

	#[test]
	fn test_convert_default_value_bool() {
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let value = op.convert_default_value("true");
		assert!(
			matches!(value, Value::Bool(Some(true))),
			"'true' should be converted to Value::Bool(Some(true))"
		);

		let value = op.convert_default_value("false");
		assert!(
			matches!(value, Value::Bool(Some(false))),
			"'false' should be converted to Value::Bool(Some(false))"
		);
	}

	#[test]
	fn test_convert_default_value_integer() {
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let value = op.convert_default_value("42");
		assert!(
			matches!(value, Value::BigInt(Some(42))),
			"Integer '42' should be converted to Value::BigInt(Some(42))"
		);
	}

	#[test]
	fn test_convert_default_value_float() {
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let value = op.convert_default_value("3.15");
		assert!(
			matches!(value, Value::Double(_)),
			"Float '3.15' should be converted to Value::Double"
		);
	}

	#[test]
	fn test_convert_default_value_string() {
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let value = op.convert_default_value("'hello'");
		match value {
			Value::String(Some(s)) => assert_eq!(
				*s, "hello",
				"Quoted string should be unquoted and stored as 'hello'"
			),
			_ => {
				panic!("Expected Value::String(Some(\"hello\")), got different variant")
			}
		}
	}

	#[rstest]
	#[case("pending", "'pending'")]
	#[case("active", "'active'")]
	#[case("hello world", "'hello world'")]
	#[case("it's", "'it''s'")]
	fn test_convert_default_value_plain_string(#[case] input: &str, #[case] expected: &str) {
		// Arrange
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};

		// Act
		let value = op.convert_default_value(input);

		// Assert
		match value {
			Value::String(Some(s)) => assert_eq!(
				*s, expected,
				"Plain string '{input}' should be auto-quoted as SQL string literal"
			),
			_ => {
				panic!("Expected Value::String(Some(\"{expected}\")), got {value:?}")
			}
		}
	}

	#[rstest]
	#[case("CURRENT_TIMESTAMP")]
	#[case("current_timestamp")]
	#[case("CURRENT_DATE")]
	#[case("CURRENT_TIME")]
	#[case("CURRENT_USER")]
	#[case("SESSION_USER")]
	#[case("LOCALTIME")]
	#[case("LOCALTIMESTAMP")]
	fn test_convert_default_value_sql_constant(#[case] input: &str) {
		// Arrange
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};

		// Act
		let value = op.convert_default_value(input);

		// Assert
		match value {
			Value::String(Some(s)) => {
				assert_eq!(*s, input, "SQL constant '{input}' should remain unquoted")
			}
			_ => {
				panic!("Expected Value::String(Some(\"{input}\")), got {value:?}")
			}
		}
	}

	#[rstest]
	#[case("NOW()")]
	#[case("uuid_generate_v4()")]
	#[case("gen_random_uuid()")]
	fn test_convert_default_value_sql_function(#[case] input: &str) {
		// Arrange
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};

		// Act
		let value = op.convert_default_value(input);

		// Assert
		match value {
			Value::String(Some(s)) => {
				assert_eq!(*s, input, "SQL function '{input}' should remain unquoted")
			}
			_ => {
				panic!("Expected Value::String(Some(\"{input}\")), got {value:?}")
			}
		}
	}

	#[test]
	fn test_apply_column_type_integer() {
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let col = ColumnDef::new(Alias::new("id"));
		let _col = op.apply_column_type(col, &FieldType::Integer);
		// This test verifies that INTEGER type application doesn't panic
		// Internal state cannot be easily asserted with reinhardt_query's ColumnDef API
	}

	#[test]
	fn test_apply_column_type_varchar_with_length() {
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let col = ColumnDef::new(Alias::new("name"));
		let _col = op.apply_column_type(col, &FieldType::VarChar(100));
		// This test verifies that VARCHAR(100) type application doesn't panic
		// Internal state cannot be easily asserted with reinhardt_query's ColumnDef API
	}

	#[test]
	fn test_apply_column_type_custom() {
		let op = Operation::CreateTable {
			name: "test".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let col = ColumnDef::new(Alias::new("data"));
		let _col = op.apply_column_type(col, &FieldType::Custom("CUSTOM_TYPE".to_string()));
		// This test verifies that custom type application doesn't panic
		// Internal state cannot be easily asserted with reinhardt_query's ColumnDef API
	}

	#[cfg(feature = "pgvector")]
	fn vector_index_operation(index_type: IndexType) -> Operation {
		Operation::CreateIndex {
			table: "source".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(index_type),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		}
	}

	#[cfg(feature = "pgvector")]
	fn vector_index_project_state(field_type: FieldType) -> ProjectState {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("search", "Source");
		model.table_name = "source".to_string();
		model.add_field(FieldState::new("embedding", field_type, false));
		state.add_model(model);
		state
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn explicit_vector_index_rejects_scalar_column_in_project_state() {
		// Arrange
		let operation = vector_index_operation(IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		});
		let state = vector_index_project_state(FieldType::Text);

		// Act
		let result = operation.validate_for_state(&state);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == "approximate vector index on table `source` targets non-vector column `embedding`"
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn explicit_named_vector_index_rejects_scalar_column_in_project_state() {
		// Arrange
		let operation = Operation::CreateNamedIndex {
			table: "source".to_string(),
			name: "source_embedding_ann".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(IndexType::Ivfflat { lists: Some(100) }),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_l2_ops".to_string()),
		};
		let state = vector_index_project_state(FieldType::Integer);

		// Act
		let result = operation.validate_for_state(&state);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == "approximate vector index on table `source` targets non-vector column `embedding`"
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn explicit_vector_index_accepts_vector_column_in_project_state() {
		// Arrange
		let operation = vector_index_operation(IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		});
		let state = vector_index_project_state(FieldType::Vector { dimensions: 1536 });

		// Act
		let result = operation.validate_for_state(&state);

		// Assert
		result.unwrap();
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn explicit_vector_index_rejects_unknown_column_in_project_state() {
		// Arrange
		let operation = vector_index_operation(IndexType::Ivfflat { lists: Some(100) });
		let mut state = vector_index_project_state(FieldType::Vector { dimensions: 1536 });
		state
			.find_model_by_table_mut("source")
			.unwrap()
			.fields
			.remove("embedding");

		// Act
		let result = operation.validate_for_state(&state);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == "approximate vector index on table `source` targets unknown column `embedding`"
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn explicit_vector_expression_index_defers_result_type_validation_to_postgres() {
		// Arrange
		let operation = Operation::CreateIndex {
			table: "source".to_string(),
			columns: Vec::new(),
			unique: false,
			index_type: Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			where_clause: None,
			concurrently: false,
			expressions: Some(vec!["normalize(embedding)".to_string()]),
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		};
		let state = vector_index_project_state(FieldType::Text);

		// Act
		let result = operation.validate_for_state(&state);

		// Assert
		result.unwrap();
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case(IndexType::BTree)]
	#[case(IndexType::Hnsw {
		m: Some(16),
		ef_construction: Some(64),
	})]
	#[case(IndexType::Ivfflat { lists: Some(100) })]
	fn vector_index_type_serde_roundtrip_preserves_legacy_and_data_bearing_variants(
		#[case] index_type: IndexType,
	) {
		// Act
		let serialized = serde_json::to_string(&index_type).unwrap();
		let reparsed: IndexType = serde_json::from_str(&serialized).unwrap();

		// Assert
		assert_eq!(reparsed, index_type);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_hnsw_migration_renders_forward_and_backward_sql() {
		// Arrange
		let operation = vector_index_operation(IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		});
		let state = ProjectState::default();

		// Act
		let forward_sql = operation.try_to_sql(&SqlDialect::Postgres).unwrap();
		let statement_sql = operation
			.try_to_statement(&SqlDialect::Postgres)
			.unwrap()
			.try_to_sql_string(crate::backends::types::DatabaseType::Postgres)
			.unwrap();
		let backward_sql = operation
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap();

		// Assert
		assert_eq!(
			forward_sql,
			"CREATE INDEX idx_source_embedding ON source USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);"
		);
		assert_eq!(statement_sql, forward_sql);
		assert_eq!(backward_sql, vec!["DROP INDEX idx_source_embedding;"]);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn pgvector_error_hint_derives_context_from_migration_operations() {
		let vector_column = Operation::CreateTable {
			name: "source".to_string(),
			columns: vec![ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 3 },
			)],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let vector_index = vector_index_operation(IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		});
		let restore_vector_column = Operation::AlterColumn {
			table: "source".to_string(),
			column: "embedding".to_string(),
			old_definition: Some(ColumnDefinition::new(
				"embedding",
				FieldType::Vector { dimensions: 3 },
			)),
			new_definition: ColumnDefinition::new("embedding", FieldType::Text),
			mysql_options: None,
		};
		let restore_vector_index =
			named_vector_index_drop(Some(IndexType::Ivfflat { lists: Some(100) }));
		let rollback_restore_vector_index = Operation::RestoreIndexOnRollback {
			table: "source".to_string(),
			name: Some("source_embedding_hnsw".to_string()),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		};

		assert_eq!(
			vector_column.pgvector_operation_kind(),
			Some(crate::backends::error::PgvectorOperationKind::ColumnType)
		);
		assert_eq!(
			vector_index.pgvector_operation_kind(),
			Some(crate::backends::error::PgvectorOperationKind::ApproximateIndex)
		);
		assert_eq!(vector_index.pgvector_reverse_operation_kind(), None);
		assert_eq!(restore_vector_column.pgvector_operation_kind(), None);
		assert_eq!(
			restore_vector_column.pgvector_reverse_operation_kind(),
			Some(crate::backends::error::PgvectorOperationKind::ColumnType)
		);
		assert_eq!(restore_vector_index.pgvector_operation_kind(), None);
		assert_eq!(
			restore_vector_index.pgvector_reverse_operation_kind(),
			Some(crate::backends::error::PgvectorOperationKind::ApproximateIndex)
		);
		assert_eq!(
			rollback_restore_vector_index.pgvector_operation_kind(),
			None
		);
		assert_eq!(
			rollback_restore_vector_index.pgvector_reverse_operation_kind(),
			Some(crate::backends::error::PgvectorOperationKind::ApproximateIndex)
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_ivfflat_migration_renders_exact_sql() {
		// Arrange
		let mut operation = vector_index_operation(IndexType::Ivfflat { lists: Some(100) });
		if let Operation::CreateIndex { operator_class, .. } = &mut operation {
			*operator_class = Some("vector_l2_ops".to_string());
		}

		// Act
		let sql = operation.try_to_sql(&SqlDialect::Postgres).unwrap();

		// Assert
		assert_eq!(
			sql,
			"CREATE INDEX idx_source_embedding ON source USING ivfflat (embedding vector_l2_ops) WITH (lists = 100);"
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_restore_preserves_hnsw_metadata() {
		// Arrange
		let operation = Operation::RestoreIndexOnRollback {
			table: "source".to_string(),
			name: Some("source_embedding_cosine_hnsw".to_string()),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		};
		let state = ProjectState::default();

		// Act
		let sql = operation
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap();

		// Assert
		assert_eq!(
			sql,
			vec![
				"CREATE INDEX source_embedding_cosine_hnsw ON source USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);"
			]
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case(SqlDialect::Mysql, "mysql")]
	#[case(SqlDialect::Sqlite, "sqlite")]
	fn vector_index_migration_rejects_non_postgres_backends(
		#[case] dialect: SqlDialect,
		#[case] backend: &'static str,
	) {
		// Arrange
		let operation = vector_index_operation(IndexType::Hnsw {
			m: None,
			ef_construction: None,
		});

		// Act
		let result = operation.try_to_sql(&dialect);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::UnsupportedBackendFeature {
				feature: "approximate vector indexes",
				backend: actual_backend,
			}) if actual_backend == backend
		));
	}

	#[rstest]
	#[case(SqlDialect::Mysql, "mysql")]
	#[case(SqlDialect::Sqlite, "sqlite")]
	#[case(SqlDialect::Cockroachdb, "cockroachdb")]
	fn create_extension_migration_rejects_non_postgres_backends(
		#[case] dialect: SqlDialect,
		#[case] backend: &'static str,
	) {
		let operation = Operation::CreateExtension {
			name: "vector".to_string(),
			if_not_exists: true,
			schema: None,
		};

		let result = operation.try_to_sql(&dialect);

		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::UnsupportedBackendFeature {
				feature: "PostgreSQL extensions",
				backend: actual_backend,
			}) if actual_backend == backend
		));
	}

	#[test]
	fn create_extension_migration_quotes_identifiers_and_does_not_auto_drop_on_reverse() {
		let operation = Operation::CreateExtension {
			name: "vector\"; DROP TABLE documents; --".to_string(),
			if_not_exists: true,
			schema: Some("extension\"schema".to_string()),
		};

		assert_eq!(
			operation
				.try_to_sql(&SqlDialect::Postgres)
				.expect("PostgreSQL supports extension migrations"),
			"CREATE EXTENSION IF NOT EXISTS \"vector\"\"; DROP TABLE documents; --\" SCHEMA \"extension\"\"schema\";"
		);
		assert_eq!(
			operation
				.to_reverse_sql(&SqlDialect::Postgres, &ProjectState::new())
				.expect("reverse SQL generation should succeed"),
			None
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case("", "must not be empty")]
	#[case("documents\0embedding", "must not contain NUL")]
	fn named_vector_index_migration_rejects_unsafe_physical_name(
		#[case] name: &str,
		#[case] expected: &str,
	) {
		// Arrange
		let operation = Operation::CreateNamedIndex {
			table: "source".to_string(),
			name: name.to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		};

		// Act
		let result = operation.try_to_sql(&SqlDialect::Postgres);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message.contains(expected)
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn named_vector_index_migration_quotes_unusual_physical_name() {
		let operation = Operation::CreateNamedIndex {
			table: "source".to_string(),
			name: "select embedding-ann".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(IndexType::Ivfflat { lists: Some(100) }),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_l2_ops".to_string()),
		};

		assert_eq!(
			operation
				.try_to_sql(&SqlDialect::Postgres)
				.expect("unusual names must be identifier-quoted"),
			"CREATE INDEX \"select embedding-ann\" ON source USING ivfflat (embedding vector_l2_ops) WITH (lists = 100);"
		);
	}

	#[cfg(feature = "pgvector")]
	fn named_vector_index_drop(index_type: Option<IndexType>) -> Operation {
		Operation::DropNamedIndex {
			table: "source".to_string(),
			name: "source_embedding_ann".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		}
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case(SqlDialect::Mysql, "mysql")]
	#[case(SqlDialect::Sqlite, "sqlite")]
	fn named_vector_index_drop_rejects_unsupported_backends(
		#[case] dialect: SqlDialect,
		#[case] backend: &'static str,
	) {
		// Arrange
		let operation = named_vector_index_drop(Some(IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		}));

		// Act
		let result = operation.try_to_sql(&dialect);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::UnsupportedBackendFeature {
				feature: "approximate vector indexes",
				backend: actual_backend,
			}) if actual_backend == backend
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn named_vector_index_drop_renders_exact_postgres_sql() {
		let operation = named_vector_index_drop(Some(IndexType::Ivfflat { lists: Some(100) }));

		assert_eq!(
			operation
				.try_to_sql(&SqlDialect::Postgres)
				.expect("PostgreSQL supports approximate vector indexes"),
			"DROP INDEX source_embedding_ann;"
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case(SqlDialect::Mysql, "DROP INDEX source_embedding_ann ON source;")]
	#[case(SqlDialect::Sqlite, "DROP INDEX source_embedding_ann;")]
	fn legacy_named_index_drop_without_type_remains_backend_agnostic(
		#[case] dialect: SqlDialect,
		#[case] expected: &str,
	) {
		let operation = named_vector_index_drop(None);

		assert_eq!(
			operation
				.try_to_sql(&dialect)
				.expect("legacy named index drops do not identify a vector index"),
			expected
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case(
		IndexType::Hnsw {
			m: Some(1),
			ef_construction: Some(64),
		},
		"m must be in the range 2..=100"
	)]
	#[case(
		IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(1),
		},
		"ef_construction must be in the range 4..=1000"
	)]
	#[case(
		IndexType::Ivfflat { lists: Some(32769) },
		"lists must be in the range 1..=32768"
	)]
	#[case(
		IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(31),
		},
		"ef_construction must be at least twice m"
	)]
	#[case(
		IndexType::Hnsw {
			m: Some(100),
			ef_construction: None,
		},
		"ef_construction must be at least twice m"
	)]
	#[case(
		IndexType::Hnsw {
			m: None,
			ef_construction: Some(4),
		},
		"ef_construction must be at least twice m"
	)]
	fn vector_index_migration_rejects_invalid_options(
		#[case] index_type: IndexType,
		#[case] option: &str,
	) {
		// Arrange
		let operation = vector_index_operation(index_type);

		// Act
		let result = operation.try_to_sql(&SqlDialect::Postgres);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == format!("{option} for approximate vector indexes")
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case(true, vec!["embedding".to_string()], None, Some("vector_l2_ops".to_string()), "approximate vector indexes cannot be unique")]
	#[case(false, vec!["embedding".to_string(), "tenant_id".to_string()], None, Some("vector_l2_ops".to_string()), "approximate vector indexes require exactly one column or expression")]
	#[case(false, vec!["embedding".to_string()], None, None, "approximate vector indexes require a supported vector operator class")]
	#[case(false, vec!["embedding".to_string()], None, Some("gin_trgm_ops".to_string()), "approximate vector indexes require a supported vector operator class")]
	#[case(false, vec![], Some(vec!["embedding".to_string(), "tenant_id".to_string()]), Some("vector_l2_ops".to_string()), "approximate vector indexes require exactly one column or expression")]
	fn vector_index_migration_rejects_invalid_structure(
		#[case] unique: bool,
		#[case] columns: Vec<String>,
		#[case] expressions: Option<Vec<String>>,
		#[case] operator_class: Option<String>,
		#[case] expected: &str,
	) {
		// Arrange
		let operation = Operation::CreateIndex {
			table: "source".to_string(),
			columns,
			unique,
			index_type: Some(IndexType::Ivfflat { lists: None }),
			where_clause: None,
			concurrently: false,
			expressions,
			mysql_options: None,
			operator_class,
		};

		// Act
		let result = operation.try_to_sql(&SqlDialect::Postgres);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == expected
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_migration_rejects_column_and_expression_together() {
		// Arrange
		let operation = Operation::CreateIndex {
			table: "source".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: Some(IndexType::Hnsw {
				m: None,
				ef_construction: None,
			}),
			where_clause: None,
			concurrently: false,
			expressions: Some(vec!["normalize(embedding)".to_string()]),
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		};

		// Act
		let result = operation.try_to_sql(&SqlDialect::Postgres);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == "approximate vector indexes require exactly one column or expression"
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_migration_rejects_empty_target() {
		// Arrange
		let operation = Operation::CreateIndex {
			table: "source".to_string(),
			columns: vec![],
			unique: false,
			index_type: Some(IndexType::Ivfflat { lists: None }),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("vector_l2_ops".to_string()),
		};

		// Act
		let result = operation.try_to_sql(&SqlDialect::Postgres);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == "approximate vector indexes require exactly one column or expression"
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_migration_rejects_multiple_columns_individually() {
		// Arrange
		let mut operation = vector_index_operation(IndexType::Ivfflat { lists: None });
		if let Operation::CreateIndex { columns, .. } = &mut operation {
			columns.push("tenant_id".to_string());
		}

		// Act
		let result = operation.try_to_sql(&SqlDialect::Postgres);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == "approximate vector indexes require exactly one column or expression"
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_migration_rejects_multiple_expressions_individually() {
		// Arrange
		let operation = Operation::CreateIndex {
			table: "source".to_string(),
			columns: vec![],
			unique: false,
			index_type: Some(IndexType::Hnsw {
				m: None,
				ef_construction: None,
			}),
			where_clause: None,
			concurrently: false,
			expressions: Some(vec![
				"normalize(embedding)".to_string(),
				"tenant_id".to_string(),
			]),
			mysql_options: None,
			operator_class: Some("vector_cosine_ops".to_string()),
		};

		// Act
		let result = operation.try_to_sql(&SqlDialect::Postgres);

		// Assert
		assert!(matches!(
			result,
			Err(crate::migrations::MigrationError::InvalidMigration(message))
				if message == "approximate vector indexes require exactly one column or expression"
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_expression_migration_renders_operator_class() {
		// Arrange
		let operation = Operation::CreateIndex {
			table: "source".to_string(),
			columns: vec![],
			unique: false,
			index_type: Some(IndexType::Hnsw {
				m: None,
				ef_construction: None,
			}),
			where_clause: None,
			concurrently: false,
			expressions: Some(vec!["normalize(embedding)".to_string()]),
			mysql_options: None,
			operator_class: Some("vector_ip_ops".to_string()),
		};
		let state = ProjectState::default();

		// Act
		let forward_sql = operation.try_to_sql(&SqlDialect::Postgres).unwrap();
		let backward_sql = operation
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap();

		// Assert
		assert_eq!(
			forward_sql,
			"CREATE INDEX idx_source_expr ON source USING hnsw (normalize(embedding) vector_ip_ops);"
		);
		assert_eq!(backward_sql, vec!["DROP INDEX idx_source_expr;"]);
	}

	#[rstest]
	fn test_create_index_composite() {
		let op = Operation::CreateIndex {
			table: "users".to_string(),
			columns: vec!["first_name".to_string(), "last_name".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(
			sql.contains("first_name"),
			"SQL should include 'first_name' column, got: {}",
			sql
		);
		assert!(
			sql.contains("last_name"),
			"SQL should include 'last_name' column, got: {}",
			sql
		);
		assert!(
			sql.contains("idx_users_first_name_last_name"),
			"SQL should include composite index name, got: {}",
			sql
		);
	}

	#[test]
	fn test_alter_table_comment_with_quotes() {
		let op = Operation::AlterTableComment {
			table: "users".to_string(),
			comment: Some("User's account table".to_string()),
		};

		let stmt = op.to_statement();
		let sql = stmt.to_sql_string(crate::backends::types::DatabaseType::Postgres);
		assert!(
			sql.contains("COMMENT ON TABLE"),
			"SQL should contain COMMENT ON TABLE keywords, got: {}",
			sql
		);
		assert!(
			sql.contains("User''s account table"),
			"SQL should properly escape single quotes in comment, got: {}",
			sql
		);
	}

	#[test]
	fn test_state_forwards_alter_column() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new(
			"age".to_string(),
			FieldType::Integer,
			false,
		));
		state.add_model(model);

		let op = Operation::AlterColumn {
			table: "myapp_users".to_string(),
			column: "age".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition {
				name: "age".to_string(),
				type_definition: FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};

		op.state_forwards("myapp", &mut state);
		let model = state.get_model("myapp", "users").unwrap();
		let field = model.fields.get("age").unwrap();
		assert_eq!(
			field.field_type,
			FieldType::BigInteger,
			"Field type should be updated to BigInteger, got: {}",
			field.field_type
		);
	}

	#[test]
	fn test_state_forwards_create_inherited_table() {
		let mut state = ProjectState::new();
		let op = Operation::CreateInheritedTable {
			name: "admin_users".to_string(),
			columns: vec![ColumnDefinition {
				name: "admin_level".to_string(),
				type_definition: FieldType::Integer,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			}],
			base_table: "users".to_string(),
			join_column: "user_id".to_string(),
		};

		op.state_forwards("myapp", &mut state);
		let model = state.get_model("myapp", "admin_users");
		assert!(
			model.is_some(),
			"Inherited table 'admin_users' should exist in state"
		);
		let model = model.unwrap();
		assert_eq!(
			model.base_model,
			Some("users".to_string()),
			"base_model should be set to 'users'"
		);
		assert_eq!(
			model.inheritance_type,
			Some("joined_table".to_string()),
			"inheritance_type should be 'joined_table'"
		);
	}

	#[test]
	fn test_state_forwards_add_discriminator_column() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new("id".to_string(), FieldType::Integer, false));
		state.add_model(model);

		let op = Operation::AddDiscriminatorColumn {
			table: "myapp_users".to_string(),
			column_name: "user_type".to_string(),
			default_value: "regular".to_string(),
		};

		op.state_forwards("myapp", &mut state);
		let model = state.get_model("myapp", "users").unwrap();
		assert_eq!(
			model.discriminator_column,
			Some("user_type".to_string()),
			"discriminator_column should be set to 'user_type'"
		);
		assert_eq!(
			model.inheritance_type,
			Some("single_table".to_string()),
			"inheritance_type should be 'single_table'"
		);
	}

	#[test]
	fn state_forwards_adds_a_discriminator_after_a_table_rename() {
		// Arrange
		let mut state = ProjectState::new();
		let mut model = ModelState::new("auth", "User");
		model.table_name = "users".to_string();
		state.add_model(model);

		// Act
		Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "auth_user".to_string(),
		}
		.state_forwards("auth", &mut state);
		Operation::AddDiscriminatorColumn {
			table: "auth_user".to_string(),
			column_name: "kind".to_string(),
			default_value: "user".to_string(),
		}
		.state_forwards("auth", &mut state);

		// Assert
		let model = state
			.get_model("auth", "User")
			.expect("renamed model should remain in state");
		assert_eq!(model.discriminator_column.as_deref(), Some("kind"));
		assert!(model.has_field("kind"));
	}

	#[rstest]
	fn test_to_reverse_sql_create_table_quotes_identifiers() {
		// Arrange
		let op = Operation::CreateTable {
			name: "user-data".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert
		assert_eq!(
			sql, "DROP TABLE \"user-data\";",
			"Identifiers with special characters must be quoted"
		);
	}

	#[rstest]
	fn test_to_reverse_sql_add_column_quotes_identifiers() {
		// Arrange
		let op = Operation::AddColumn {
			table: "my table".to_string(),
			column: ColumnDefinition {
				name: "my column".to_string(),
				type_definition: FieldType::VarChar(255),
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
			mysql_options: None,
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert
		assert_eq!(
			sql, "ALTER TABLE \"my table\" DROP COLUMN \"my column\";",
			"Table and column names with spaces must be quoted"
		);
	}

	#[rstest]
	fn test_to_reverse_sql_rename_table_quotes_identifiers() {
		// Arrange: both names contain special characters requiring quoting
		let op = Operation::RenameTable {
			old_name: "old; DROP TABLE users;--".to_string(),
			new_name: "new-name".to_string(),
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert
		assert_eq!(
			sql, "ALTER TABLE \"new-name\" RENAME TO \"old; DROP TABLE users;--\";",
			"SQL injection attempt must be quoted as identifier"
		);
	}

	#[rstest]
	fn test_to_reverse_sql_rename_column_quotes_identifiers() {
		// Arrange: use identifiers with special characters to verify quoting
		let op = Operation::RenameColumn {
			table: "my table".to_string(),
			old_name: "old col".to_string(),
			new_name: "new col".to_string(),
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert
		assert_eq!(
			sql, "ALTER TABLE \"my table\" RENAME COLUMN \"new col\" TO \"old col\";",
			"Identifiers with spaces must be quoted"
		);
	}

	#[rstest]
	fn test_to_reverse_sql_create_index_quotes_identifiers() {
		// Arrange
		let op = Operation::CreateIndex {
			table: "my-table".to_string(),
			columns: vec!["col a".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert
		assert!(
			sql.contains("DROP INDEX \"idx_my-table_col a\""),
			"Index name must be quoted, got: {}",
			sql
		);
	}

	/// Regression test for kent8192/reinhardt-web#4583.
	///
	/// MySQL requires `DROP INDEX <name> ON <table>` when reversing a `CreateIndex`
	/// operation. The previous implementation emitted `DROP INDEX <name>;` for every
	/// dialect, which produced malformed SQL on MySQL (1064 syntax error). This test
	/// pins the MySQL-specific `ON <table>` clause to prevent regressions.
	#[rstest]
	fn test_to_reverse_sql_create_index_emits_on_table_clause_for_mysql() {
		// Arrange
		let op = Operation::CreateIndex {
			table: "users".to_string(),
			columns: vec!["email".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&SqlDialect::Mysql, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert: `quote_identifier` is `pg_escape::quote_identifier`, which only
		// adds quotes when the identifier contains reserved or non-lowercase
		// characters. For plain ASCII names, the output is unquoted. What matters
		// for regression is the presence of the `ON <table>` suffix.
		assert_eq!(
			sql, "DROP INDEX idx_users_email ON users;",
			"MySQL reverse SQL must include `ON <table>` clause"
		);
	}

	/// Verify Postgres / SQLite / CockroachDB continue to emit the bare
	/// `DROP INDEX <name>;` form without an `ON <table>` clause.
	#[rstest]
	#[case(SqlDialect::Postgres, "DROP INDEX idx_users_email;")]
	#[case(SqlDialect::Sqlite, "DROP INDEX idx_users_email;")]
	#[case(SqlDialect::Cockroachdb, "DROP INDEX idx_users_email;")]
	fn test_to_reverse_sql_create_index_omits_on_table_for_non_mysql(
		#[case] dialect: SqlDialect,
		#[case] expected: &str,
	) {
		// Arrange
		let op = Operation::CreateIndex {
			table: "users".to_string(),
			columns: vec!["email".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&dialect, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert
		assert_eq!(
			sql, expected,
			"Non-MySQL reverse SQL must remain unchanged for dialect {:?}",
			dialect
		);
	}

	#[rstest]
	fn test_to_reverse_sql_add_constraint_quotes_identifiers() {
		// Arrange: table name with special characters triggers quoting
		let op = Operation::AddConstraint {
			table: "my-table".to_string(),
			constraint_sql: "CONSTRAINT chk_positive CHECK (x > 0)".to_string(),
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert
		assert!(
			sql.contains("ALTER TABLE \"my-table\""),
			"Table name with special characters must be quoted, got: {}",
			sql
		);
		assert!(
			sql.contains("DROP CONSTRAINT"),
			"Should contain DROP CONSTRAINT, got: {}",
			sql
		);
	}

	fn project_state_with_long_enum_constraint() -> (ProjectState, String, String, String) {
		let table = "model_enum_jobs_with_a_name_that_exceeds_postgres_identifier_limits";
		let column = "job_status_with_a_name_that_exceeds_postgres_identifier_limits";
		let name = truncate_identifier_with_hash(&format!("{table}_{column}_model_enum_check"));
		let mut model = ModelState::new("tasks", "Job");
		model.table_name = table.to_string();
		model.constraints.push(
			crate::migrations::autodetector::ConstraintDefinition::enum_domain(
				name.clone(),
				column,
				crate::field_domain::FieldDomain::Enum {
					repr: crate::field_domain::ModelEnumRepr::String,
					values: vec![crate::field_domain::ModelEnumValue::String(
						"queued".to_string(),
					)],
				},
			),
		);
		let mut state = ProjectState::new();
		state.add_model(model);

		(state, table.to_string(), column.to_string(), name)
	}

	#[test]
	fn drop_enum_constraint_reverses_to_typed_constraint_operation() {
		let (state, table, _column, name) = project_state_with_long_enum_constraint();
		let operation = Operation::DropConstraint {
			table: table.clone(),
			constraint_name: name.clone(),
		};

		let reverse = operation
			.to_reverse_operation(&state)
			.expect("reverse operation should resolve")
			.expect("enum constraint should be reversible");

		assert!(matches!(
			reverse,
			Operation::AddConstraintDefinition {
				table: reverse_table,
				constraint: Constraint::EnumDomain {
					name: reverse_name,
					..
				},
			} if reverse_table == table && reverse_name == name
		));
	}

	#[test]
	fn drop_enum_constraint_reverse_sql_uses_mysql_identifier_quoting() {
		let (state, table, column, name) = project_state_with_long_enum_constraint();
		let operation = Operation::DropConstraint {
			table,
			constraint_name: name,
		};

		let sql = operation
			.to_reverse_sql(&SqlDialect::Mysql, &state)
			.expect("reverse SQL should resolve")
			.expect("enum constraint should be reversible")
			.join("\n");

		assert!(
			sql.contains(&format!("CHECK (BINARY `{column}` IN ('queued'))")),
			"{sql}"
		);
	}

	fn typed_constraints_with_reserved_mysql_identifiers() -> Vec<Constraint> {
		vec![
			Constraint::PrimaryKey {
				name: "primary-key".to_string(),
				columns: vec!["select".to_string()],
			},
			Constraint::Unique {
				name: "unique-key".to_string(),
				columns: vec!["group".to_string()],
			},
			Constraint::ForeignKey {
				name: "foreign-key".to_string(),
				columns: vec!["order".to_string()],
				referenced_table: "user-table".to_string(),
				referenced_columns: vec!["primary".to_string()],
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::NoAction,
				deferrable: None,
			},
			Constraint::OneToOne {
				name: "one-key".to_string(),
				column: "match".to_string(),
				referenced_table: "profile-table".to_string(),
				referenced_column: "primary".to_string(),
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::NoAction,
				deferrable: None,
			},
		]
	}

	#[test]
	fn drop_typed_constraints_reverse_to_typed_add_operations() {
		for constraint in typed_constraints_with_reserved_mysql_identifiers() {
			let operation = Operation::DropConstraintDefinition {
				table: "order-items".to_string(),
				constraint: constraint.clone(),
			};

			let reverse = operation
				.to_reverse_operation(&ProjectState::default())
				.expect("typed reverse operation should resolve")
				.expect("typed constraint should be reversible");

			assert_eq!(
				reverse,
				Operation::AddConstraintDefinition {
					table: "order-items".to_string(),
					constraint,
				}
			);
		}
	}

	#[test]
	fn drop_typed_constraints_reverse_sql_uses_mysql_identifier_quoting() {
		let expected_fragments = [
			"CONSTRAINT `primary-key` PRIMARY KEY (`select`)",
			"CONSTRAINT `unique-key` UNIQUE (`group`)",
			"CONSTRAINT `foreign-key` FOREIGN KEY (`order`) REFERENCES `user-table`(`primary`)",
			"CONSTRAINT `one-key` FOREIGN KEY (`match`) REFERENCES `profile-table`(`primary`)",
		];

		for (constraint, expected) in typed_constraints_with_reserved_mysql_identifiers()
			.into_iter()
			.zip(expected_fragments)
		{
			let operation = Operation::DropConstraintDefinition {
				table: "order-items".to_string(),
				constraint,
			};
			let sql = operation
				.to_reverse_sql(&SqlDialect::Mysql, &ProjectState::default())
				.expect("typed reverse SQL should resolve")
				.expect("typed constraint should be reversible")
				.join("\n");

			assert!(sql.starts_with("ALTER TABLE `order-items` ADD "), "{sql}");
			assert!(sql.contains(expected), "missing {expected} in {sql}");
		}
	}

	#[rstest]
	fn test_to_reverse_sql_bulk_load_quotes_identifiers() {
		// Arrange
		let op = Operation::BulkLoad {
			table: "user-data".to_string(),
			source: BulkLoadSource::Stdin,
			format: BulkLoadFormat::default(),
			options: BulkLoadOptions::default(),
		};
		let state = ProjectState::default();

		// Act
		let sql = op
			.to_reverse_sql(&SqlDialect::Postgres, &state)
			.unwrap()
			.unwrap()
			.join("\n");

		// Assert
		assert_eq!(
			sql, "TRUNCATE TABLE \"user-data\";",
			"Table name must be quoted"
		);
	}

	// ========================================================================
	// SetAutoIncrementValue — per-backend SQL rendering
	// ========================================================================

	#[rstest]
	#[case::postgres(SqlDialect::Postgres)]
	#[case::cockroachdb(SqlDialect::Cockroachdb)]
	fn test_set_auto_increment_postgres_uses_setval(#[case] dialect: SqlDialect) {
		// Arrange
		let op = Operation::SetAutoIncrementValue {
			table: "users".to_string(),
			column: "id".to_string(),
			value: 1000,
		};

		// Act
		let sql = op.to_sql(&dialect);

		// Assert
		assert_eq!(
			sql,
			"SELECT setval(pg_get_serial_sequence('users', 'id'), 1000, false);"
		);
	}

	#[test]
	fn test_set_auto_increment_mysql_alters_table() {
		// Arrange
		let op = Operation::SetAutoIncrementValue {
			table: "users".to_string(),
			column: "id".to_string(),
			value: 1000,
		};

		// Act
		let sql = op.to_sql(&SqlDialect::Mysql);

		// Assert: identifier quoting uses pg_escape's `quote_identifier`
		// uniformly across dialects (matches convention used elsewhere in
		// this module, e.g. AlterTableComment). pg_escape omits quotes when
		// the identifier needs no escaping.
		assert_eq!(sql, "ALTER TABLE users AUTO_INCREMENT = 1000;");
	}

	#[test]
	fn test_set_auto_increment_sqlite_upserts_sqlite_sequence() {
		// Arrange
		let op = Operation::SetAutoIncrementValue {
			table: "users".to_string(),
			column: "id".to_string(),
			value: 1000,
		};

		// Act
		let sql = op.to_sql(&SqlDialect::Sqlite);

		// Assert: INSERT OR REPLACE is robust vs. UPDATE which no-ops when
		// the sqlite_sequence row does not yet exist.
		assert_eq!(
			sql,
			"INSERT OR REPLACE INTO sqlite_sequence(name, seq) VALUES ('users', 1000);"
		);
	}

	#[test]
	fn test_set_auto_increment_postgres_escapes_literals() {
		// Arrange: embedded single quote must be doubled to avoid injection.
		let op = Operation::SetAutoIncrementValue {
			table: "user's".to_string(),
			column: "id".to_string(),
			value: 42,
		};

		// Act
		let sql = op.to_sql(&SqlDialect::Postgres);

		// Assert
		assert!(
			sql.contains("'user''s'"),
			"single quote in table name must be escaped: {}",
			sql
		);
	}

	// ========================================================================
	// CreateCompositePrimaryKey — SQL rendering and edge cases
	// ========================================================================

	#[rstest]
	#[case::postgres(SqlDialect::Postgres)]
	#[case::mysql(SqlDialect::Mysql)]
	#[case::sqlite(SqlDialect::Sqlite)]
	#[case::cockroachdb(SqlDialect::Cockroachdb)]
	fn test_composite_pk_default_name(#[case] dialect: SqlDialect) {
		// Arrange
		let op = Operation::CreateCompositePrimaryKey {
			table: "order_items".to_string(),
			columns: vec!["order_id".to_string(), "line_number".to_string()],
			constraint_name: None,
		};

		// Act
		let sql = op.to_sql(&dialect);

		// Assert
		assert!(
			sql.contains("ALTER TABLE"),
			"SQL should use ALTER TABLE: {}",
			sql
		);
		assert!(
			sql.contains("ADD CONSTRAINT"),
			"SQL should add a named constraint: {}",
			sql
		);
		assert!(
			sql.contains("PRIMARY KEY"),
			"SQL should add PRIMARY KEY: {}",
			sql
		);
		assert!(
			sql.contains("order_items_pkey"),
			"Default constraint name should be table_pkey: {}",
			sql
		);
		assert!(
			sql.contains("order_id") && sql.contains("line_number"),
			"Both PK columns must appear: {}",
			sql
		);
	}

	#[test]
	fn test_composite_pk_custom_name_and_quoting() {
		// Arrange
		let op = Operation::CreateCompositePrimaryKey {
			table: "tbl".to_string(),
			columns: vec!["a".to_string(), "b".to_string()],
			constraint_name: Some("my_pk".to_string()),
		};

		// Act
		let sql = op.to_sql(&SqlDialect::Postgres);

		// Assert: pg_escape omits quotes for identifiers that need no escaping.
		assert_eq!(
			sql,
			"ALTER TABLE tbl ADD CONSTRAINT my_pk PRIMARY KEY (a, b);"
		);
	}

	#[test]
	fn test_composite_pk_empty_columns_produces_failing_sql() {
		// Arrange: empty column list is invalid; we emit a deliberately
		// invalid SQL statement (a bare identifier) so every backend's
		// parser rejects it before execution, replacing the earlier
		// `SELECT 1/0` fallback that silently passed on SQLite and
		// lax-mode MySQL (reinhardt-web#4325).
		let op = Operation::CreateCompositePrimaryKey {
			table: "tbl".to_string(),
			columns: vec![],
			constraint_name: None,
		};

		// Act: verify behavior on every supported dialect.
		for dialect in [SqlDialect::Postgres, SqlDialect::Mysql, SqlDialect::Sqlite] {
			let sql = op.to_sql(&dialect);

			// Assert: the emitted statement encodes the diagnostic and is
			// not a syntactically valid SELECT/DDL on any backend.
			assert!(
				sql.starts_with("SYNTAX_ERROR_create_composite_pk_on_")
					&& sql.contains("requires_at_least_one_column"),
				"Empty column list must emit a syntax-error statement with diagnostic ({:?}): {}",
				dialect,
				sql
			);
			assert!(
				!sql.contains("SELECT 1/0"),
				"Must not fall back to SELECT 1/0 (silently passes on SQLite / lax MySQL): {}",
				sql
			);
		}
	}

	// ========================================================================
	// column_to_sql — SQLite AUTOINCREMENT type widening (Issue #4184)
	//
	// SQLite rejects `BIGINT PRIMARY KEY AUTOINCREMENT` at apply time with:
	//   "AUTOINCREMENT is only allowed on an INTEGER PRIMARY KEY"
	// The default `BigAutoField` from CoreSettings produces FieldType::BigInteger
	// + auto_increment, so the SQLite emitter must widen integer widths to the
	// literal `INTEGER` token.
	// ========================================================================

	#[rstest]
	#[case::big_integer(FieldType::BigInteger)]
	#[case::integer(FieldType::Integer)]
	#[case::small_integer(FieldType::SmallInteger)]
	fn test_column_to_sql_sqlite_auto_increment_pk_emits_integer(#[case] field_type: FieldType) {
		// Arrange: BigAutoField/AutoField/SmallAutoField PK with auto_increment.
		let mut col = ColumnDefinition::new("id", field_type);
		col.primary_key = true;
		col.auto_increment = true;
		col.not_null = true;

		// Act
		let sql = Operation::column_to_sql(&col, &SqlDialect::Sqlite);

		// Assert: must use the literal `INTEGER` token (not BIGINT/SMALLINT)
		// to satisfy SQLite's AUTOINCREMENT constraint.
		assert!(
			sql.contains("INTEGER PRIMARY KEY AUTOINCREMENT"),
			"SQLite auto_increment PK must emit `INTEGER PRIMARY KEY AUTOINCREMENT`: {}",
			sql
		);
		assert!(
			!sql.contains("BIGINT"),
			"SQLite auto_increment must not emit BIGINT (rejected by SQLite): {}",
			sql
		);
		assert!(
			!sql.contains("SMALLINT"),
			"SQLite auto_increment must not emit SMALLINT (rejected by SQLite): {}",
			sql
		);
	}

	#[test]
	fn test_column_to_sql_sqlite_big_integer_without_auto_increment_no_autoincrement() {
		// Arrange: plain BigInteger column without auto_increment must not emit
		// the AUTOINCREMENT keyword. SQLite represents all integer widths as
		// INTEGER (storage class), so emitting INTEGER (per to_sql_for_dialect)
		// is correct even without auto_increment.
		let mut col = ColumnDefinition::new("count", FieldType::BigInteger);
		col.not_null = true;

		// Act
		let sql = Operation::column_to_sql(&col, &SqlDialect::Sqlite);

		// Assert
		assert!(
			!sql.contains("AUTOINCREMENT"),
			"Non-auto_increment column must not emit AUTOINCREMENT: {}",
			sql
		);
		// SQLite accepts `BIGINT` declarations via type affinity, but our emitter
		// normalizes integer widths to `INTEGER` for consistency with the
		// auto_increment path. This assertion guards that normalization, not a
		// SQLite-level prohibition on BIGINT.
		assert!(
			!sql.contains("BIGINT"),
			"emitter is expected to normalize BigInteger to INTEGER for SQLite: {}",
			sql
		);
	}

	#[test]
	fn test_column_to_sql_postgres_big_integer_auto_increment_unchanged() {
		// Arrange: regression guard — Postgres path must remain GENERATED AS IDENTITY.
		let mut col = ColumnDefinition::new("id", FieldType::BigInteger);
		col.primary_key = true;
		col.auto_increment = true;
		col.not_null = true;

		// Act
		let sql = Operation::column_to_sql(&col, &SqlDialect::Postgres);

		// Assert
		assert!(
			sql.contains("BIGINT GENERATED BY DEFAULT AS IDENTITY"),
			"Postgres auto_increment BigInteger must emit identity syntax: {}",
			sql
		);
	}

	#[test]
	fn test_column_to_sql_sqlite_auto_increment_uuid_pk_omits_autoincrement() {
		// Arrange: a UUID primary key with auto_increment=true. The `#[model]`
		// macro previously emitted `auto_increment="true"` for every PK
		// regardless of type, which produced `"id" UUID PRIMARY KEY AUTOINCREMENT`
		// — rejected by SQLite with "AUTOINCREMENT is only allowed on an
		// INTEGER PRIMARY KEY". The emitter must defend against this combination
		// by omitting AUTOINCREMENT when the column type was not widened to
		// INTEGER. See reinhardt-web#4378.
		let mut col = ColumnDefinition::new("id", FieldType::Uuid);
		col.primary_key = true;
		col.auto_increment = true;
		col.not_null = true;

		// Act
		let sql = Operation::column_to_sql(&col, &SqlDialect::Sqlite);

		// Assert: PRIMARY KEY must be emitted, AUTOINCREMENT must NOT.
		assert!(
			sql.contains("PRIMARY KEY"),
			"UUID PK must still emit PRIMARY KEY: {}",
			sql
		);
		assert!(
			!sql.contains("AUTOINCREMENT"),
			"non-integer auto_increment PK must not emit AUTOINCREMENT (SQLite rejects it): {}",
			sql
		);
		// SQLite's `to_sql_for_dialect(Uuid)` returns `TEXT` (the storage class
		// SQLite actually uses for UUIDs); the important guarantee is that the
		// type was NOT silently widened to `INTEGER`, which would change the
		// column's semantics.
		assert!(
			!sql.contains("INTEGER"),
			"UUID column type must not be widened to INTEGER: {}",
			sql
		);
	}

	#[test]
	fn test_column_to_sql_without_pk_sqlite_auto_increment_emits_integer() {
		// Arrange: composite PK path also widens to INTEGER for SQLite.
		let mut col = ColumnDefinition::new("id", FieldType::BigInteger);
		col.auto_increment = true;
		col.not_null = true;

		// Act
		let sql = Operation::column_to_sql_without_pk(&col, &SqlDialect::Sqlite);

		// Assert
		assert!(
			sql.contains("INTEGER"),
			"SQLite auto_increment column (composite PK path) must emit INTEGER: {}",
			sql
		);
		assert!(
			!sql.contains("BIGINT"),
			"SQLite auto_increment must not emit BIGINT in composite PK path: {}",
			sql
		);
	}

	#[test]
	fn test_column_to_sql_postgres_typed_generated_column() {
		let mut col = ColumnDefinition::new("full_name", FieldType::VarChar(201));
		col.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			GeneratedStorage::Stored,
		));

		let sql = Operation::column_to_sql(&col, &SqlDialect::Postgres);

		assert_eq!(
			sql,
			"full_name VARCHAR(201) GENERATED ALWAYS AS (first_name || ' ' || last_name) STORED"
		);
	}

	#[test]
	fn test_column_to_sql_mysql_typed_generated_column() {
		let mut col = ColumnDefinition::new("full_name", FieldType::VarChar(201));
		col.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			GeneratedStorage::Stored,
		));

		let sql = Operation::column_to_sql(&col, &SqlDialect::Mysql);

		assert_eq!(
			sql,
			"full_name VARCHAR(201) GENERATED ALWAYS AS (CONCAT(`first_name`, ' ', `last_name`)) STORED"
		);
	}

	#[test]
	fn test_column_to_sql_mysql_empty_concat_generated_column() {
		let mut col = ColumnDefinition::new("empty_name", FieldType::VarChar(201));
		col.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::concat([]),
			"SchemaExpr::concat([])",
			GeneratedStorage::Stored,
		));

		let sql = Operation::column_to_sql(&col, &SqlDialect::Mysql);

		assert_eq!(
			sql,
			"empty_name VARCHAR(201) GENERATED ALWAYS AS ('') STORED"
		);
	}

	#[test]
	fn test_column_to_sql_sqlite_virtual_generated_column() {
		let mut col = ColumnDefinition::new("full_name", FieldType::VarChar(201));
		col.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			GeneratedStorage::Virtual,
		));

		let sql = Operation::column_to_sql(&col, &SqlDialect::Sqlite);

		assert_eq!(
			sql,
			"full_name VARCHAR(201) GENERATED ALWAYS AS (first_name || ' ' || last_name) VIRTUAL"
		);
	}

	#[test]
	fn test_column_to_sql_postgres_cast_generated_column_uses_sql_type() {
		let mut col = ColumnDefinition::new("amount_text", FieldType::VarChar(64));
		col.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("amount").cast(QueryColumnType::Decimal(Some((10, 2)))),
			"SchemaExpr::col(\"amount\").cast(ColumnType::Decimal(Some((10, 2))))",
			GeneratedStorage::Stored,
		));

		let sql = Operation::column_to_sql(&col, &SqlDialect::Postgres);

		assert_eq!(
			sql,
			"amount_text VARCHAR(64) GENERATED ALWAYS AS (CAST(amount AS NUMERIC(10, 2))) STORED"
		);
	}

	#[test]
	#[should_panic(
		expected = "PostgreSQL-compatible generated column `search_name` cannot reference generated column `full_name`"
	)]
	fn test_create_table_postgres_rejects_generated_column_chain() {
		let mut full_name = ColumnDefinition::new("full_name", FieldType::VarChar(201));
		full_name.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("first_name"),
			"SchemaExpr::col(\"first_name\")",
			GeneratedStorage::Stored,
		));
		let mut search_name = ColumnDefinition::new("search_name", FieldType::VarChar(201));
		search_name.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("full_name"),
			"SchemaExpr::col(\"full_name\")",
			GeneratedStorage::Stored,
		));
		let op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![full_name, search_name],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};

		let _ = op.to_sql(&SqlDialect::Postgres);
	}

	#[test]
	fn test_column_to_sql_mysql_cast_generated_string_uses_char_type() {
		let mut col = ColumnDefinition::new("amount_text", FieldType::VarChar(64));
		col.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("amount").cast(QueryColumnType::String(Some(64))),
			"SchemaExpr::col(\"amount\").cast(ColumnType::String(Some(64)))",
			GeneratedStorage::Stored,
		));

		let sql = Operation::column_to_sql(&col, &SqlDialect::Mysql);

		assert_eq!(
			sql,
			"amount_text VARCHAR(64) GENERATED ALWAYS AS (CAST(`amount` AS CHAR(64))) STORED"
		);
	}

	#[test]
	fn test_column_to_sql_mysql_cast_generated_integer_uses_signed_type() {
		let mut col = ColumnDefinition::new("age_int", FieldType::Integer);
		col.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("age").cast(QueryColumnType::Integer),
			"SchemaExpr::col(\"age\").cast(ColumnType::Integer)",
			GeneratedStorage::Stored,
		));

		let sql = Operation::column_to_sql(&col, &SqlDialect::Mysql);

		assert_eq!(
			sql,
			"age_int INTEGER GENERATED ALWAYS AS (CAST(`age` AS SIGNED)) STORED"
		);
	}

	#[test]
	fn generated_column_serde_rehydrates_typed_expr() {
		let source = r#"{
			"expr_tokens": "SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			"raw_sql": null,
			"storage": "stored"
		}"#;

		let generated: GeneratedColumnDefinition =
			serde_json::from_str(source).expect("generated column metadata should deserialize");

		assert_eq!(
			generated.expr.as_deref(),
			Some(&SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]))
		);
		assert!(generated.to_query_generated().is_some());
	}

	#[test]
	fn generated_column_tokens_canonicalize_schema_expr_aliases() {
		let generated = GeneratedColumnDefinition::typed(
			SchemaExpr::col("full_name"),
			"S::col(\"full_name\")",
			GeneratedStorage::Stored,
		);

		let tokens = quote::quote! { #generated }.to_string();

		assert!(tokens.contains("SchemaExpr :: col"));
		assert!(!tokens.contains("S :: col"));
	}

	#[test]
	fn generated_column_equality_prefers_typed_expr_over_token_spelling() {
		let alias_spelled = GeneratedColumnDefinition::typed(
			SchemaExpr::col("full_name"),
			"S::col(\"full_name\")",
			GeneratedStorage::Stored,
		);
		let canonical_spelled = GeneratedColumnDefinition::typed(
			SchemaExpr::col("full_name"),
			"SchemaExpr::col(\"full_name\")",
			GeneratedStorage::Stored,
		);

		assert_eq!(alias_spelled, canonical_spelled);
	}

	#[test]
	fn generated_column_equality_matches_typed_expr_to_sqlite_raw_sql() {
		let typed = GeneratedColumnDefinition::typed(
			SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			GeneratedStorage::Virtual,
		);
		let introspected = GeneratedColumnDefinition::raw_sql(
			r#""first_name" || ' ' || "last_name""#,
			GeneratedStorage::Virtual,
		);

		assert_eq!(typed, introspected);
	}

	#[test]
	fn generated_column_equality_matches_typed_binary_expr_to_parenthesized_raw_sql() {
		let typed = GeneratedColumnDefinition::typed(
			SchemaExpr::col("subtotal").binary(SchemaBinOper::Add, SchemaExpr::col("tax")),
			"SchemaExpr::col(\"subtotal\").binary(SchemaBinOper::Add, SchemaExpr::col(\"tax\"))",
			GeneratedStorage::Stored,
		);
		let introspected =
			GeneratedColumnDefinition::raw_sql("(subtotal + tax)", GeneratedStorage::Stored);

		assert_eq!(typed, introspected);
	}

	#[test]
	fn generated_column_raw_sql_equality_preserves_bracket_operators() {
		let array_access = GeneratedColumnDefinition::raw_sql("items[1]", GeneratedStorage::Stored);
		let flattened = GeneratedColumnDefinition::raw_sql("items1", GeneratedStorage::Stored);

		assert_ne!(array_access, flattened);
	}

	#[test]
	fn generated_column_dependency_uses_rehydrated_expr_before_tokens() {
		let generated = GeneratedColumnDefinition::typed(
			SchemaExpr::val("full_name"),
			"SchemaExpr::val(\"full_name\")",
			GeneratedStorage::Stored,
		);

		assert!(!Operation::generated_column_references_column(
			&generated,
			"full_name"
		));
	}

	#[test]
	#[should_panic(expected = "SQLite ADD COLUMN does not support stored generated columns")]
	fn test_add_column_sqlite_rejects_stored_generated_column() {
		let mut column = ColumnDefinition::new("full_name", FieldType::VarChar(201));
		column.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			GeneratedStorage::Stored,
		));
		let op = Operation::AddColumn {
			table: "users".to_string(),
			column,
			mysql_options: None,
		};

		let _ = op.to_sql(&SqlDialect::Sqlite);
	}

	fn generated_full_name_column() -> ColumnDefinition {
		let mut full_name = ColumnDefinition::new("full_name", FieldType::VarChar(201));
		full_name.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			GeneratedStorage::Stored,
		));
		full_name
	}

	#[test]
	fn test_sqlite_recreation_for_stored_generated_add_column_omits_generated_copy() {
		let id = ColumnDefinition::new("id", FieldType::Integer);
		let first_name = ColumnDefinition::new("first_name", FieldType::VarChar(100));
		let last_name = ColumnDefinition::new("last_name", FieldType::VarChar(100));
		let full_name = generated_full_name_column();

		let recreation = SqliteTableRecreation::for_add_column(
			"users",
			vec![id, first_name, last_name],
			full_name,
			vec![],
		);
		let sql = recreation.to_sql_statements();

		assert!(
			sql[0].contains("full_name VARCHAR(201) GENERATED ALWAYS AS"),
			"new schema must include the stored generated column: {}",
			sql[0]
		);
		assert_eq!(
			sql[1],
			"INSERT INTO \"users_new\" (\"id\", \"first_name\", \"last_name\") SELECT \"id\", \"first_name\", \"last_name\" FROM \"users\";"
		);
	}

	#[test]
	fn test_sqlite_recreation_places_column_collation_before_default() {
		let mut code = ColumnDefinition::new("code", FieldType::Text);
		code.default = Some("'alpha'".to_string());
		let recreation = SqliteTableRecreation::for_add_constraint(
			"jobs",
			vec![code],
			vec![],
			"CHECK (1 = 1)".to_string(),
		)
		.with_column_collations(vec![("code".to_string(), "NOCASE".to_string())]);

		let create_sql = &recreation.to_sql_statements()[0];

		assert!(
			create_sql.contains("code TEXT COLLATE \"NOCASE\" DEFAULT 'alpha'"),
			"{create_sql}"
		);
	}

	#[test]
	fn test_sqlite_recreation_recreates_indexes_after_rename() {
		let id = ColumnDefinition::new("id", FieldType::Integer);
		let title = ColumnDefinition::new("title", FieldType::VarChar(100));
		let full_name = generated_full_name_column();

		let recreation =
			SqliteTableRecreation::for_add_column("users", vec![id, title], full_name, vec![])
				.with_indexes(vec![SqliteRecreatedIndex {
					name: "idx_users_title".to_string(),
					columns: vec!["title".to_string()],
					unique: false,
					sql: None,
				}]);
		let sql = recreation.to_sql_statements();

		assert_eq!(
			sql.last().map(String::as_str),
			Some("CREATE INDEX \"idx_users_title\" ON \"users\" (\"title\");")
		);
	}

	#[test]
	fn test_sqlite_recreation_preserves_expression_index_sql() {
		let id = ColumnDefinition::new("id", FieldType::Integer);
		let title = ColumnDefinition::new("title", FieldType::VarChar(100));
		let full_name = generated_full_name_column();

		let recreation =
			SqliteTableRecreation::for_add_column("users", vec![id, title], full_name, vec![])
				.with_indexes(vec![SqliteRecreatedIndex {
					name: "idx_users_lower_title".to_string(),
					columns: Vec::new(),
					unique: false,
					sql: Some(
						"CREATE INDEX idx_users_lower_title ON users(lower(title))".to_string(),
					),
				}]);
		let sql = recreation.to_sql_statements();

		assert_eq!(
			sql.last().map(String::as_str),
			Some("CREATE INDEX idx_users_lower_title ON users(lower(title));")
		);
	}

	#[test]
	fn test_sqlite_recreation_preserves_partial_index_predicates() {
		let id = ColumnDefinition::new("id", FieldType::Integer);
		let title = ColumnDefinition::new("title", FieldType::VarChar(100));
		let full_name = generated_full_name_column();

		let recreation =
			SqliteTableRecreation::for_add_column("users", vec![id, title], full_name, vec![])
				.with_indexes(vec![SqliteRecreatedIndex {
					name: "idx_users_active_title".to_string(),
					columns: vec!["title".to_string()],
					unique: false,
					sql: Some(
						"CREATE INDEX \"idx_users_active_title\" ON \"users\" (\"title\") WHERE deleted_at IS NULL"
							.to_string(),
					),
				}]);
		let sql = recreation.to_sql_statements();

		assert_eq!(
			sql.last().map(String::as_str),
			Some(
				"CREATE INDEX \"idx_users_active_title\" ON \"users\" (\"title\") WHERE deleted_at IS NULL;"
			)
		);
	}

	#[test]
	fn test_sqlite_recreation_filters_indexes_referencing_dropped_column() {
		let id = ColumnDefinition::new("id", FieldType::Integer);
		let title = ColumnDefinition::new("title", FieldType::VarChar(100));
		let full_name = generated_full_name_column();

		let recreation = SqliteTableRecreation::for_drop_column(
			"users",
			vec![id, title, full_name],
			"full_name",
			vec![],
		)
		.with_indexes(vec![
			SqliteRecreatedIndex {
				name: "idx_users_title".to_string(),
				columns: vec!["title".to_string()],
				unique: false,
				sql: None,
			},
			SqliteRecreatedIndex {
				name: "idx_users_full_name".to_string(),
				columns: vec!["full_name".to_string()],
				unique: false,
				sql: None,
			},
			SqliteRecreatedIndex {
				name: "idx_users_title_partial".to_string(),
				columns: vec!["title".to_string()],
				unique: false,
				sql: Some(
					"CREATE INDEX \"idx_users_title_partial\" ON \"users\" (\"title\") WHERE \"full_name\" IS NOT NULL"
						.to_string(),
				),
			},
		])
		.without_indexes_referencing("full_name");
		let sql = recreation.to_sql_statements();

		assert!(
			sql.iter()
				.any(|statement| statement.contains("idx_users_title"))
		);
		assert!(
			sql.iter()
				.all(|statement| !statement.contains("idx_users_full_name"))
		);
		assert!(
			sql.iter()
				.all(|statement| !statement.contains("idx_users_title_partial"))
		);
	}

	#[test]
	fn test_sqlite_recreation_filters_raw_constraints_by_dropped_column() {
		let id = ColumnDefinition::new("id", FieldType::Integer);
		let title = ColumnDefinition::new("title", FieldType::VarChar(100));
		let slug = ColumnDefinition::new("slug", FieldType::VarChar(100));
		let recreation = SqliteTableRecreation::for_drop_column(
			"articles",
			vec![id, title, slug],
			"slug",
			vec![],
		)
		.with_raw_constraints(vec![
			SqliteRecreatedConstraint {
				name: Some("uq_articles_title".to_string()),
				physical_name: None,
				columns: vec!["title".to_string()],
				sql: "CONSTRAINT uq_articles_title UNIQUE (title)".to_string(),
			},
			SqliteRecreatedConstraint {
				name: Some("uq_articles_slug".to_string()),
				physical_name: None,
				columns: vec!["slug".to_string()],
				sql: "CONSTRAINT uq_articles_slug UNIQUE (slug)".to_string(),
			},
		])
		.without_raw_constraints_referencing("slug");

		let create_sql = &recreation.to_sql_statements()[0];
		assert!(create_sql.contains("uq_articles_title"), "{create_sql}");
		assert!(!create_sql.contains("uq_articles_slug"), "{create_sql}");
	}

	#[test]
	fn test_sqlite_recreation_filters_raw_constraints_by_name() {
		let recreation = SqliteTableRecreation::for_drop_constraint(
			"jobs",
			vec![ColumnDefinition::new("code", FieldType::Text)],
			vec![],
			"uq_jobs_nocase",
		)
		.with_raw_constraints(vec![
			SqliteRecreatedConstraint {
				name: Some("uq_jobs_nocase".to_string()),
				physical_name: None,
				columns: vec!["code".to_string()],
				sql: "CONSTRAINT uq_jobs_nocase UNIQUE (code COLLATE NOCASE)".to_string(),
			},
			SqliteRecreatedConstraint {
				name: Some("uq_jobs_binary".to_string()),
				physical_name: None,
				columns: vec!["code".to_string()],
				sql: "CONSTRAINT uq_jobs_binary UNIQUE (code COLLATE BINARY)".to_string(),
			},
		])
		.without_raw_constraint_named("uq_jobs_nocase");

		let create_sql = &recreation.to_sql_statements()[0];
		assert!(!create_sql.contains("uq_jobs_nocase"), "{create_sql}");
		assert!(create_sql.contains("uq_jobs_binary"), "{create_sql}");
	}

	#[test]
	fn test_sqlite_recreation_filters_unnamed_raw_constraint_by_physical_name() {
		let recreation = SqliteTableRecreation::for_drop_constraint(
			"jobs",
			vec![ColumnDefinition::new("code", FieldType::Text)],
			vec![],
			"sqlite_autoindex_jobs_1",
		)
		.with_raw_constraints(vec![SqliteRecreatedConstraint {
			name: None,
			physical_name: Some("sqlite_autoindex_jobs_1".to_string()),
			columns: vec!["code".to_string()],
			sql: "UNIQUE (code)".to_string(),
		}])
		.without_raw_constraint_named("sqlite_autoindex_jobs_1");

		let create_sql = &recreation.to_sql_statements()[0];
		assert!(!create_sql.contains("UNIQUE (code)"), "{create_sql}");
	}

	#[test]
	fn test_sqlite_recreation_copy_lists_omit_existing_generated_columns() {
		let id = ColumnDefinition::new("id", FieldType::Integer);
		let first_name = ColumnDefinition::new("first_name", FieldType::VarChar(100));
		let last_name = ColumnDefinition::new("last_name", FieldType::VarChar(100));
		let full_name = generated_full_name_column();
		let current_columns = vec![id, first_name, last_name, full_name];

		let drop_column = SqliteTableRecreation::for_drop_column(
			"users",
			current_columns.clone(),
			"last_name",
			vec![],
		);
		assert_eq!(drop_column.columns_to_copy, vec!["id", "first_name"]);

		let alter_column = SqliteTableRecreation::for_alter_column(
			"users",
			current_columns.clone(),
			"first_name",
			ColumnDefinition::new("first_name", FieldType::VarChar(150)),
			vec![],
		);
		assert_eq!(
			alter_column.columns_to_copy,
			vec!["id", "first_name", "last_name"]
		);

		let add_constraint = SqliteTableRecreation::for_add_constraint(
			"users",
			current_columns.clone(),
			vec![],
			"CONSTRAINT users_name_unique UNIQUE (first_name, last_name)".to_string(),
		);
		assert_eq!(
			add_constraint.columns_to_copy,
			vec!["id", "first_name", "last_name"]
		);

		let drop_constraint =
			SqliteTableRecreation::for_drop_constraint("users", current_columns, vec![], "old");
		assert_eq!(
			drop_constraint.columns_to_copy,
			vec!["id", "first_name", "last_name"]
		);
	}

	#[test]
	fn test_from_field_state_preserves_generated_metadata() {
		let generated = GeneratedColumnDefinition::typed(
			SchemaExpr::col("source_name"),
			"SchemaExpr::col(\"source_name\")",
			GeneratedStorage::Stored,
		);
		let mut field_state = FieldState::new("display_name", FieldType::VarChar(255), false);
		field_state.generated = Some(generated.clone());

		let col = ColumnDefinition::from_field_state("display_name", &field_state);

		assert_eq!(col.generated, Some(generated));
	}

	mod resolve_foreign_key_column_type_tests {
		use super::super::resolve_foreign_key_column_type_with;
		use super::FieldType;
		use crate::migrations::autodetector::FieldState;
		use crate::migrations::model_registry::{FieldMetadata, ModelMetadata, ModelRegistry};

		/// Helper: build a target model registered under `(app, name)`
		/// whose PK column is of `pk_type`.
		fn target_model(app: &str, name: &str, table: &str, pk_type: FieldType) -> ModelMetadata {
			let mut meta = ModelMetadata::new(app, name, table);
			meta.add_field(
				"id".to_string(),
				FieldMetadata::new(pk_type).with_param("primary_key", "true"),
			);
			meta
		}

		/// Helper: build a `ForeignKeyField`-style FieldState whose
		/// `fk_target` (and optionally `fk_target_app`) drive the
		/// resolver.
		fn fk_field_state(target_model: &str, target_app: Option<&str>) -> FieldState {
			let mut fs = FieldState::new("owner_id", FieldType::Uuid, false);
			fs.params
				.insert("fk_target".to_string(), target_model.to_string());
			if let Some(app) = target_app {
				fs.params
					.insert("fk_target_app".to_string(), app.to_string());
			}
			fs
		}

		#[test]
		fn qualified_hit_resolves_to_target_pk_type() {
			// Arrange
			let registry = ModelRegistry::new();
			registry.register_model(target_model(
				"auth",
				"User",
				"auth_user",
				FieldType::BigInteger,
			));
			let fs = fk_field_state("User", Some("auth"));

			// Act
			let resolved = resolve_foreign_key_column_type_with(&fs, &registry);

			// Assert: qualified lookup hits and returns the target's PK type.
			assert_eq!(resolved, Some(FieldType::BigInteger));
		}

		#[test]
		fn qualified_miss_falls_back_to_by_name_when_unambiguous() {
			// Arrange: target registered under a different app than the
			// macro emitted (simulates the `use`-import edge case).
			let registry = ModelRegistry::new();
			registry.register_model(target_model(
				"reinhardt_auth",
				"User",
				"auth_user",
				FieldType::Uuid,
			));
			// Macro emitted the current crate's app, which is wrong here.
			let fs = fk_field_state("User", Some("blog"));

			// Act
			let resolved = resolve_foreign_key_column_type_with(&fs, &registry);

			// Assert: by-name fallback resolves to the only registered
			// `User` model, preserving the pre-#4436 resolution path.
			assert_eq!(resolved, Some(FieldType::Uuid));
		}

		#[test]
		fn ambiguous_by_name_returns_none() {
			// Arrange: two apps register the same model name.
			let registry = ModelRegistry::new();
			registry.register_model(target_model(
				"auth",
				"User",
				"auth_user",
				FieldType::BigInteger,
			));
			registry.register_model(target_model(
				"billing",
				"User",
				"billing_user",
				FieldType::Uuid,
			));
			// No `fk_target_app` -> straight to by-name lookup.
			let fs = fk_field_state("User", None);

			// Act
			let resolved = resolve_foreign_key_column_type_with(&fs, &registry);

			// Assert: conservative `None` rather than silently picking
			// one of the two `User` models.
			assert_eq!(resolved, None);
		}

		#[test]
		fn path_typed_disambiguates_ambiguous_name() {
			// Arrange: two apps register `User`. The user wrote a
			// path-typed FK target (`ForeignKeyField<reinhardt_auth::User>`),
			// so the macro emits `fk_target_app="reinhardt_auth"` —
			// trusted as a user-explicit qualifier. The resolver must
			// use it to pick the correct `User`, not the unrelated
			// `blog.User`.
			let registry = ModelRegistry::new();
			registry.register_model(target_model(
				"blog",
				"User",
				"blog_user",
				FieldType::BigInteger,
			));
			registry.register_model(target_model(
				"reinhardt_auth",
				"User",
				"reinhardt_auth_user",
				FieldType::Uuid,
			));
			let fs = fk_field_state("User", Some("reinhardt_auth"));

			// Act
			let resolved = resolve_foreign_key_column_type_with(&fs, &registry);

			// Assert: qualified hit picks `reinhardt_auth.User`
			// (FieldType::Uuid), not `blog.User` (FieldType::BigInteger).
			assert_eq!(resolved, Some(FieldType::Uuid));
		}

		#[test]
		fn qualified_miss_with_ambiguous_by_name_returns_none() {
			// Arrange: qualified lookup misses AND the by-name fallback
			// is itself ambiguous. The resolver must still refuse to
			// guess.
			let registry = ModelRegistry::new();
			registry.register_model(target_model(
				"auth",
				"User",
				"auth_user",
				FieldType::BigInteger,
			));
			registry.register_model(target_model(
				"billing",
				"User",
				"billing_user",
				FieldType::Uuid,
			));
			let fs = fk_field_state("User", Some("blog")); // misses; falls back; ambiguous.

			// Act
			let resolved = resolve_foreign_key_column_type_with(&fs, &registry);

			// Assert
			assert_eq!(resolved, None);
		}

		#[test]
		fn no_fk_target_param_returns_none() {
			// Arrange: a non-FK field has no `fk_target` param.
			let registry = ModelRegistry::new();
			registry.register_model(target_model(
				"auth",
				"User",
				"auth_user",
				FieldType::BigInteger,
			));
			let fs = FieldState::new("name", FieldType::VarChar(64), false);

			// Act
			let resolved = resolve_foreign_key_column_type_with(&fs, &registry);

			// Assert
			assert_eq!(resolved, None);
		}
	}
}
