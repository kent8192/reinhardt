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
use super::{FieldState, FieldType, ModelState, ProjectState};
use pg_escape::quote_identifier;
use reinhardt_query::prelude::{
	Alias, AlterTableStatement, ColumnDef, CreateIndexStatement, CreateTableStatement,
	DropIndexStatement, DropTableStatement, Query, SimpleExpr, Value,
};
use serde::{Deserialize, Serialize};

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
	Instant,
	Inplace,
	Copy,
	#[default]
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
	None,
	Shared,
	Exclusive,
	#[default]
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
	pub algorithm: Option<MySqlAlgorithm>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub lock: Option<MySqlLock>,
}

impl AlterTableOptions {
	pub fn new() -> Self {
		Self::default()
	}
	pub fn with_algorithm(mut self, algorithm: MySqlAlgorithm) -> Self {
		self.algorithm = Some(algorithm);
		self
	}
	pub fn with_lock(mut self, lock: MySqlLock) -> Self {
		self.lock = Some(lock);
		self
	}
	pub fn is_empty(&self) -> bool {
		self.algorithm.is_none() && self.lock.is_none()
	}
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
	Range,
	List,
	Hash,
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
	LessThan(String),
	In(Vec<String>),
	ModuloCount(u32),
}

/// Individual partition definition
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartitionDef {
	pub name: String,
	pub values: PartitionValues,
}

impl PartitionDef {
	pub fn new(name: impl Into<String>, values: PartitionValues) -> Self {
		Self {
			name: name.into(),
			values,
		}
	}
	pub fn less_than(name: impl Into<String>, value: impl Into<String>) -> Self {
		Self::new(name, PartitionValues::LessThan(value.into()))
	}
	pub fn maxvalue(name: impl Into<String>) -> Self {
		Self::new(name, PartitionValues::LessThan("MAXVALUE".to_string()))
	}
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
	pub partition_type: PartitionType,
	pub column: String,
	pub partitions: Vec<PartitionDef>,
}

impl PartitionOptions {
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
	pub fn range(column: impl Into<String>, partitions: Vec<PartitionDef>) -> Self {
		Self::new(PartitionType::Range, column, partitions)
	}
	pub fn list(column: impl Into<String>, partitions: Vec<PartitionDef>) -> Self {
		Self::new(PartitionType::List, column, partitions)
	}
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
	PrimaryKey { name: String, columns: Vec<String> },
	/// ForeignKey constraint
	ForeignKey {
		name: String,
		columns: Vec<String>,
		referenced_table: String,
		referenced_columns: Vec<String>,
		on_delete: super::ForeignKeyAction,
		on_update: super::ForeignKeyAction,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		deferrable: Option<DeferrableOption>,
	},
	/// Unique constraint
	Unique { name: String, columns: Vec<String> },
	/// Check constraint
	Check { name: String, expression: String },
	/// OneToOne constraint (ForeignKey + Unique combination)
	OneToOne {
		name: String,
		column: String,
		referenced_table: String,
		referenced_column: String,
		on_delete: super::ForeignKeyAction,
		on_update: super::ForeignKeyAction,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		deferrable: Option<DeferrableOption>,
	},
	/// ManyToMany relationship metadata (intermediate table reference)
	ManyToMany {
		name: String,
		through_table: String,
		source_column: String,
		target_column: String,
		target_table: String,
	},
	/// Exclude constraint (PostgreSQL only)
	Exclude {
		name: String,
		elements: Vec<(String, String)>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		using: Option<String>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		where_clause: Option<String>,
	},
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Operation {
	CreateTable {
		name: String,
		columns: Vec<ColumnDefinition>,
		#[serde(default)]
		constraints: Vec<Constraint>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		without_rowid: Option<bool>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		interleave_in_parent: Option<InterleaveSpec>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		partition: Option<PartitionOptions>,
	},
	DropTable {
		name: String,
	},
	AddColumn {
		table: String,
		column: ColumnDefinition,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		mysql_options: Option<AlterTableOptions>,
	},
	DropColumn {
		table: String,
		column: String,
	},
	AlterColumn {
		table: String,
		column: String,
		/// Original column definition (before alteration).
		/// This is required for generating accurate rollback SQL.
		/// If None, rollback will attempt to reconstruct from ProjectState.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		old_definition: Option<ColumnDefinition>,
		new_definition: ColumnDefinition,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		mysql_options: Option<AlterTableOptions>,
	},
	RenameTable {
		old_name: String,
		new_name: String,
	},
	RenameColumn {
		table: String,
		old_name: String,
		new_name: String,
	},
	AddConstraint {
		table: String,
		constraint_sql: String,
	},
	DropConstraint {
		table: String,
		constraint_name: String,
	},
	CreateIndex {
		table: String,
		columns: Vec<String>,
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
	DropIndex {
		table: String,
		columns: Vec<String>,
	},
	RunSQL {
		sql: String,
		reverse_sql: Option<String>,
	},
	RunRust {
		code: String,
		reverse_code: Option<String>,
	},
	AlterTableComment {
		table: String,
		comment: Option<String>,
	},
	AlterUniqueTogether {
		table: String,
		unique_together: Vec<Vec<String>>,
	},
	AlterModelOptions {
		table: String,
		options: std::collections::HashMap<String, String>,
	},
	CreateInheritedTable {
		name: String,
		columns: Vec<ColumnDefinition>,
		base_table: String,
		join_column: String,
	},
	AddDiscriminatorColumn {
		table: String,
		column_name: String,
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
}

/// Default value provider for serde (returns true)
const fn default_true() -> bool {
	true
}

impl Operation {
	/// Apply this operation to the project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		match self {
			Operation::CreateTable { name, columns, .. } => {
				let mut model = ModelState::new(app_label, name.clone());
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
			Operation::DropTable { name } => {
				state.remove_model(app_label, name);
			}
			Operation::AddColumn { table, column, .. } => {
				if let Some(model) = state.get_model_mut(app_label, table) {
					let field = FieldState::new(
						column.name.to_string(),
						column.type_definition.clone(),
						false,
					);
					model.add_field(field);
				}
			}
			Operation::DropColumn { table, column } => {
				if let Some(model) = state.get_model_mut(app_label, table) {
					model.remove_field(column);
				}
			}
			Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} => {
				if let Some(model) = state.get_model_mut(app_label, table) {
					let field = FieldState::new(
						column.to_string(),
						new_definition.type_definition.clone(),
						false,
					);
					model.alter_field(column, field);
				}
			}
			Operation::RenameTable { old_name, new_name } => {
				state.rename_model(app_label, old_name, new_name.to_string());
			}
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => {
				if let Some(model) = state.get_model_mut(app_label, table) {
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
				if let Some(model) = state.get_model_mut(app_label, table) {
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
			Operation::AddConstraint { .. }
			| Operation::DropConstraint { .. }
			| Operation::CreateIndex { .. }
			| Operation::DropIndex { .. }
			| Operation::RunSQL { .. }
			| Operation::RunRust { .. }
			| Operation::AlterTableComment { .. }
			| Operation::AlterUniqueTogether { .. }
			| Operation::AlterModelOptions { .. } => {}
			Operation::MoveModel {
				model_name,
				from_app,
				to_app,
				rename_table,
				old_table_name,
				new_table_name,
			} => {
				// Move the model from one app to another in the project state
				// First get the model, then remove it from the old location
				if let Some(model) = state.get_model(from_app, model_name).cloned() {
					state.remove_model(from_app, model_name);

					// Create a new model with updated app label
					let mut new_model = model;
					new_model.app_label = to_app.to_string();

					// Update table name if rename_table is true
					if *rename_table
						&& let (Some(_old_name), Some(new_name)) = (old_table_name, new_table_name)
					{
						new_model.table_name = new_name.to_string();
					}

					state.add_model(new_model);
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
					parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
					// For SQLite, if part of composite PK, we don't add AUTOINCREMENT here
					// It will be handled by the table-level PRIMARY KEY constraint
				}
			}
		} else {
			parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
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

	/// Generate column SQL with all constraints
	fn column_to_sql(col: &ColumnDefinition, dialect: &SqlDialect) -> String {
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
					parts.push(col.type_definition.to_sql_for_dialect(dialect).into());
					// SQLite: INTEGER PRIMARY KEY is implicitly AUTOINCREMENT
					// But we need explicit AUTOINCREMENT keyword for tests
					if col.primary_key {
						parts.push("PRIMARY KEY AUTOINCREMENT".to_string().into());
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
					if has_composite_pk {
						parts.push(format!(
							"  {}",
							Self::column_to_sql_without_pk(col, dialect)
						));
					} else {
						parts.push(format!("  {}", Self::column_to_sql(col, dialect)));
					}
				}

				// Add composite primary key constraint if detected
				if has_composite_pk {
					let pk_constraint_name = format!("{}_pkey", name);
					let quoted_pk_columns = pk_columns
						.iter()
						.map(|s| quote_identifier(s))
						.collect::<Vec<_>>()
						.join(", ");
					let pk_constraint = format!(
						"  CONSTRAINT {} PRIMARY KEY ({})",
						quote_identifier(&pk_constraint_name),
						quoted_pk_columns
					);
					parts.push(pk_constraint);
				}

				for constraint in constraints {
					parts.push(format!("  {}", constraint));
				}
				let mut sql = format!(
					"CREATE TABLE {} (\n{}\n)",
					quote_identifier(name),
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
			Operation::DropColumn { table, column } => {
				format!(
					"ALTER TABLE {} DROP COLUMN {};",
					quote_identifier(table),
					quote_identifier(column)
				)
			}
			Operation::AlterColumn {
				table,
				column,
				new_definition,
				mysql_options,
				..
			} => {
				let sql_type = new_definition.type_definition.to_sql_for_dialect(dialect);
				match dialect {
					SqlDialect::Postgres | SqlDialect::Cockroachdb => {
						format!(
							"ALTER TABLE {} ALTER COLUMN {} TYPE {};",
							quote_identifier(table),
							quote_identifier(column),
							sql_type
						)
					}
					SqlDialect::Mysql => {
						let base_sql = format!(
							"ALTER TABLE {} MODIFY COLUMN {} {}",
							quote_identifier(table),
							quote_identifier(column),
							sql_type
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
			} => {
				format!(
					"ALTER TABLE {} ADD {};",
					quote_identifier(table),
					constraint_sql
				)
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
						let content = exprs.join(", ");
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

				let idx_name = format!("idx_{}_{}", table, name_suffix);

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
		}
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
	/// * `Ok(Some(sql))` - Reverse SQL generated successfully
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
	) -> super::Result<Option<String>> {
		match self {
			Operation::CreateTable { name, .. } => Ok(Some(format!("DROP TABLE {};", name))),
			Operation::AddColumn { table, column, .. } => Ok(Some(format!(
				"ALTER TABLE {} DROP COLUMN {};",
				table, column.name
			))),
			Operation::RunSQL { reverse_sql, .. } => {
				Ok(reverse_sql.as_ref().map(|s| s.to_string()))
			}
			Operation::RunRust { reverse_code, .. } => Ok(reverse_code.as_ref().map(|code| {
				format!(
					"-- RunRust (reverse): {}",
					code.lines().next().unwrap_or("")
				)
			})),
			// Phase 1: Simple reverse operations
			Operation::RenameTable { old_name, new_name } => Ok(Some(format!(
				"ALTER TABLE {} RENAME TO {};",
				new_name, old_name
			))),
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => Ok(Some(format!(
				"ALTER TABLE {} RENAME COLUMN {} TO {};",
				table, new_name, old_name
			))),
			Operation::CreateIndex { table, columns, .. } => {
				// Use the same naming convention as to_sql(): idx_{table}_{columns_joined}
				// This ensures the rollback DROP INDEX targets the correct index name
				let columns_joined = columns.join("_");
				let index_name = format!("idx_{}_{}", table, columns_joined);
				Ok(Some(format!("DROP INDEX {};", index_name)))
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
				Ok(Some(format!(
					"ALTER TABLE {} DROP CONSTRAINT {};",
					table, constraint_name
				)))
			}
			// Phase 2: Complex reverse operations using ProjectState
			Operation::DropColumn { table, column } => {
				// Retrieve original column definition from ProjectState
				if let Some(model) = project_state.find_model_by_table(table)
					&& let Some(field) = model.get_field(column)
				{
					let col_def = ColumnDefinition::from_field_state(column.clone(), field);
					let col_sql = Self::column_to_sql(&col_def, dialect);
					return Ok(Some(format!(
						"ALTER TABLE {} ADD COLUMN {};",
						table, col_sql
					)));
				}
				// Cannot reconstruct without state
				Ok(None)
			}
			Operation::AlterColumn {
				table,
				column,
				old_definition,
				new_definition: _,
				..
			} => {
				// Prioritize old_definition if available (accurate rollback)
				if let Some(old_def) = old_definition {
					let type_sql = old_def.type_definition.to_sql_for_dialect(dialect);
					let null_clause = if old_def.not_null { " NOT NULL" } else { "" };
					return Ok(Some(format!(
						"ALTER TABLE {} ALTER COLUMN {} TYPE {}{};",
						table, column, type_sql, null_clause
					)));
				}

				// Fallback: Retrieve original column definition from ProjectState
				if let Some(model) = project_state.find_model_by_table(table)
					&& let Some(field) = model.get_field(column)
				{
					let col_def = ColumnDefinition::from_field_state(column.clone(), field);
					// Generate ALTER COLUMN to restore original definition
					let type_sql = col_def.type_definition.to_sql_for_dialect(dialect);
					let null_clause = if col_def.not_null { " NOT NULL" } else { "" };
					return Ok(Some(format!(
						"ALTER TABLE {} ALTER COLUMN {} TYPE {}{};",
						table, column, type_sql, null_clause
					)));
				}
				// Cannot reconstruct without state
				Ok(None)
			}
			Operation::DropIndex { table, columns } => {
				// Enhancement opportunity: Full index reconstruction would preserve
				// index_type, where_clause, operator_class, and other advanced properties.
				// The current implementation generates a basic CREATE INDEX statement.
				let columns_joined = columns.join("_");
				let index_name = format!("idx_{}_{}", table, columns_joined);
				let columns_list = columns.join(", ");
				Ok(Some(format!(
					"CREATE INDEX {} ON {} ({});",
					index_name, table, columns_list
				)))
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
					return Ok(Some(format!("ALTER TABLE {} ADD {};", table, constraint)));
				}
				// Cannot reconstruct without state
				Ok(None)
			}
			Operation::DropTable { name } => {
				// Retrieve table definition from ProjectState and reconstruct CREATE TABLE
				if let Some(model) = project_state.find_model_by_table(name) {
					let mut parts = Vec::new();

					// Convert fields to column definitions
					for (field_name, field) in &model.fields {
						let col_def = ColumnDefinition::from_field_state(field_name.clone(), field);
						parts.push(format!("  {}", Self::column_to_sql(&col_def, dialect)));
					}

					// Add constraints
					for constraint_def in &model.constraints {
						let constraint = constraint_def.to_constraint();
						parts.push(format!("  {}", constraint));
					}

					return Ok(Some(format!(
						"CREATE TABLE {} (\n{}\n);",
						name,
						parts.join(",\n")
					)));
				}
				// Cannot reconstruct without state
				Ok(None)
			}
			Operation::BulkLoad { table, .. } => {
				// Reverse of bulk load is to truncate the table (remove loaded data)
				// Note: This removes ALL data, not just the data loaded by this operation
				Ok(Some(format!("TRUNCATE TABLE {};", table)))
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
				// Reverse: Rename back from new_name to old_name
				if let Some(mut model) = state
					.models
					.remove(&(app_label.to_string(), new_name.to_string()))
				{
					model.table_name = old_name.to_string();
					state
						.models
						.insert((app_label.to_string(), old_name.to_string()), model);
				}
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
			Operation::DropConstraint {
				table: _,
				constraint_name: _,
			} => {
				// Cannot reconstruct constraint definition without snapshot.
				// For proper rollback, use to_reverse_sql with pre-operation ProjectState.
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
	pub name: String,
	pub type_definition: FieldType,
	#[serde(default)]
	pub not_null: bool,
	#[serde(default)]
	pub unique: bool,
	#[serde(default)]
	pub primary_key: bool,
	#[serde(default)]
	pub auto_increment: bool,
	#[serde(default)]
	pub default: Option<String>,
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
		}
	}

	/// Create a ColumnDefinition from FieldState with attribute parsing
	///
	/// This method reads field attributes (primary_key, not_null, unique, etc.) from
	/// the FieldState.params HashMap and properly initializes ColumnDefinition fields.
	///
	/// # Arguments
	///
	/// * `name` - Column name
	/// * `field_state` - FieldState containing field metadata and params
	///
	/// # Notes
	///
	/// - If `primary_key` is true, `not_null` is automatically set to true
	/// - Default values are false/None for unspecified attributes
	pub fn from_field_state(name: impl Into<String>, field_state: &FieldState) -> Self {
		let name_str = name.into();
		let params = &field_state.params;

		// Parse attributes from params HashMap
		let primary_key = params
			.get("primary_key")
			.and_then(|v| v.parse::<bool>().ok())
			.unwrap_or(false);

		let not_null = params
			.get("not_null")
			.and_then(|v| v.parse::<bool>().ok())
			.or(if primary_key { Some(true) } else { None })
			.unwrap_or(false);

		let unique = params
			.get("unique")
			.and_then(|v| v.parse::<bool>().ok())
			.unwrap_or(false);

		let auto_increment = params
			.get("auto_increment")
			.and_then(|v| v.parse::<bool>().ok())
			.unwrap_or(false);

		let default = params.get("default").cloned();

		Self {
			name: name_str,
			type_definition: field_state.field_type.clone(),
			not_null,
			unique,
			primary_key,
			auto_increment,
			default,
		}
	}
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
	Sqlite,
	Postgres,
	Mysql,
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
	/// WITHOUT ROWID option
	pub without_rowid: bool,
}

impl SqliteTableRecreation {
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
		let columns_to_copy: Vec<_> = new_columns.iter().map(|c| c.name.to_string()).collect();

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
			without_rowid: false,
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
		let columns_to_copy: Vec<_> = new_columns.iter().map(|c| c.name.to_string()).collect();

		Self {
			table_name,
			new_columns,
			columns_to_copy,
			constraints: current_constraints,
			raw_constraint_sqls: Vec::new(),
			without_rowid: false,
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
		let columns_to_copy: Vec<_> = current_columns.iter().map(|c| c.name.to_string()).collect();

		Self {
			table_name,
			new_columns: current_columns,
			columns_to_copy,
			constraints: current_constraints,
			raw_constraint_sqls: vec![constraint_sql],
			without_rowid: false,
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
		let columns_to_copy: Vec<_> = current_columns.iter().map(|c| c.name.to_string()).collect();

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
			without_rowid: false,
		}
	}

	/// Generate the 4-step SQL statements for table recreation
	pub fn to_sql_statements(&self) -> Vec<String> {
		let temp_table = format!("{}_new", self.table_name);

		// Step 1: CREATE TABLE with new schema
		let column_defs: Vec<String> = self
			.new_columns
			.iter()
			.map(|c| Operation::column_to_sql(c, &SqlDialect::Sqlite))
			.collect();

		let constraint_defs: Vec<String> = self.constraints.iter().map(|c| c.to_string()).collect();

		let mut create_parts = column_defs;
		create_parts.extend(constraint_defs);
		// Include raw constraint SQLs (from AddConstraint operations)
		create_parts.extend(self.raw_constraint_sqls.clone());

		let mut create_sql = format!(
			"CREATE TABLE \"{}\" (\n  {}\n)",
			temp_table,
			create_parts.join(",\n  ")
		);
		if self.without_rowid {
			create_sql.push_str(" WITHOUT ROWID");
		}
		create_sql.push(';');

		// Step 2: Copy data
		let columns_list = self
			.columns_to_copy
			.iter()
			.map(|c| format!("\"{}\"", c))
			.collect::<Vec<_>>()
			.join(", ");
		let insert_sql = format!(
			"INSERT INTO \"{}\" SELECT {} FROM \"{}\";",
			temp_table, columns_list, self.table_name
		);

		// Step 3: Drop old table
		let drop_sql = format!("DROP TABLE \"{}\";", self.table_name);

		// Step 4: Rename new table
		let rename_sql = format!(
			"ALTER TABLE \"{}\" RENAME TO \"{}\";",
			temp_table, self.table_name
		);

		vec![create_sql, insert_sql, drop_sql, rename_sql]
	}

	/// Check if a constraint references a specific column
	fn constraint_references_column(constraint: &Constraint, column_name: &str) -> bool {
		match constraint {
			Constraint::PrimaryKey { columns, .. } => columns.iter().any(|c| c == column_name),
			Constraint::ForeignKey { columns, .. } => columns.iter().any(|c| c == column_name),
			Constraint::Unique { columns, .. } => columns.iter().any(|c| c == column_name),
			Constraint::Check { expression, .. } => expression.contains(column_name),
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
				| Operation::DropConstraint { .. }
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
				// DropConstraint → Reverse AddConstraint (requires recreation)
				| Operation::DropConstraint { .. }
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
					let columns: Vec<ColumnDefinition> = model
						.fields
						.iter()
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
			})),
			Operation::DropColumn { table, column } => {
				// Reconstruct AddColumn from ProjectState
				if let Some(model) = project_state.find_model_by_table(table)
					&& let Some(field) = model.get_field(column)
				{
					let col_def = ColumnDefinition::from_field_state(column.clone(), field);
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
				new_definition: _,
				..
			} => {
				// Reconstruct AlterColumn with original definition from ProjectState
				if let Some(model) = project_state.find_model_by_table(table)
					&& let Some(field) = model.get_field(column)
				{
					let col_def = ColumnDefinition::from_field_state(column.clone(), field);
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
					return Ok(Some(Operation::AddConstraint {
						table: table.clone(),
						constraint_sql: format!("{}", constraint),
					}));
				}
				Ok(None)
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
	TableCreate(CreateTableStatement),
	TableDrop(DropTableStatement),
	TableAlter(AlterTableStatement),
	TableRename(AlterTableStatement),
	IndexCreate(CreateIndexStatement),
	IndexDrop(DropIndexStatement),
	/// Sanitized raw SQL (identifiers escaped with pg_escape::quote_identifier)
	RawSql(String),
}

impl OperationStatement {
	/// Execute the operation statement
	pub async fn execute<'c, E>(&self, executor: E) -> Result<(), sqlx::Error>
	where
		E: sqlx::Executor<'c, Database = sqlx::Postgres>,
	{
		use crate::backends::sql_build_helpers;
		use crate::backends::types::DatabaseType;
		let db_type = DatabaseType::Postgres;
		match self {
			OperationStatement::TableCreate(stmt) => {
				let sql = sql_build_helpers::build_create_table_sql(db_type, stmt);
				sqlx::query(&sql).execute(executor).await?;
			}
			OperationStatement::TableDrop(stmt) => {
				let sql = sql_build_helpers::build_drop_table_sql(db_type, stmt);
				sqlx::query(&sql).execute(executor).await?;
			}
			OperationStatement::TableAlter(stmt) => {
				let sql = sql_build_helpers::build_alter_table_sql(db_type, stmt);
				sqlx::query(&sql).execute(executor).await?;
			}
			OperationStatement::TableRename(stmt) => {
				let sql = sql_build_helpers::build_alter_table_sql(db_type, stmt);
				sqlx::query(&sql).execute(executor).await?;
			}
			OperationStatement::IndexCreate(stmt) => {
				let sql = sql_build_helpers::build_create_index_sql(db_type, stmt);
				sqlx::query(&sql).execute(executor).await?;
			}
			OperationStatement::IndexDrop(stmt) => {
				let sql = sql_build_helpers::build_drop_index_sql(db_type, stmt);
				sqlx::query(&sql).execute(executor).await?;
			}
			OperationStatement::RawSql(sql) => {
				// Already sanitized with pg_escape::quote_identifier
				sqlx::query(sql).execute(executor).await?;
			}
		}
		Ok(())
	}

	/// Convert to SQL string for logging/debugging
	///
	/// # Arguments
	///
	/// * `db_type` - Database type to generate SQL for (PostgreSQL, MySQL, SQLite)
	pub fn to_sql_string(&self, db_type: crate::backends::types::DatabaseType) -> String {
		use crate::backends::sql_build_helpers;

		match self {
			OperationStatement::TableCreate(stmt) => {
				sql_build_helpers::build_create_table_sql(db_type, stmt)
			}
			OperationStatement::TableDrop(stmt) => {
				sql_build_helpers::build_drop_table_sql(db_type, stmt)
			}
			OperationStatement::TableAlter(stmt) => {
				sql_build_helpers::build_alter_table_sql(db_type, stmt)
			}
			OperationStatement::TableRename(stmt) => {
				sql_build_helpers::build_alter_table_sql(db_type, stmt)
			}
			OperationStatement::IndexCreate(stmt) => {
				sql_build_helpers::build_create_index_sql(db_type, stmt)
			}
			OperationStatement::IndexDrop(stmt) => {
				sql_build_helpers::build_drop_index_sql(db_type, stmt)
			}
			OperationStatement::RawSql(sql) => sql.clone(),
		}
	}
}

impl Operation {
	/// Convert Operation to reinhardt-query statement or sanitized raw SQL
	pub fn to_statement(&self) -> OperationStatement {
		match self {
			Operation::CreateTable {
				name,
				columns,
				constraints,
				..
			} => {
				OperationStatement::TableCreate(self.build_create_table(name, columns, constraints))
			}
			Operation::DropTable { name } => {
				OperationStatement::TableDrop(self.build_drop_table(name))
			}
			Operation::AddColumn { table, column, .. } => {
				OperationStatement::TableAlter(self.build_add_column(table, column))
			}
			Operation::DropColumn { table, column } => {
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
			Operation::AddConstraint {
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
			Operation::DropConstraint {
				table,
				constraint_name,
			} => OperationStatement::RawSql(format!(
				"ALTER TABLE {} DROP CONSTRAINT {}",
				quote_identifier(table),
				quote_identifier(constraint_name)
			)),
			Operation::CreateIndex {
				table,
				columns,
				unique,
				..
			} => {
				let idx_name = format!("idx_{}_{}", table, columns.join("_"));
				OperationStatement::IndexCreate(
					self.build_create_index(&idx_name, table, columns, *unique),
				)
			}
			Operation::DropIndex { table, columns } => {
				let idx_name = format!("idx_{}_{}", table, columns.join("_"));
				OperationStatement::IndexDrop(self.build_drop_index(&idx_name))
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
			FieldType::JsonBinary => col_def.json_binary(),
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
			FieldType::Custom(custom_type) => col_def.custom(Alias::new(custom_type)),
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

		// SQL function calls (e.g., NOW(), CURRENT_TIMESTAMP)
		if trimmed.ends_with("()") || trimmed.contains('(') {
			// Return as custom SQL expression
			return Value::String(Some(Box::new(trimmed.to_string())));
		}

		// Default: treat as string
		Value::String(Some(Box::new(trimmed.to_string())))
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
			Operation::DropColumn { table, column } => Some(format!(
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
			Operation::AddConstraint { table, .. } => {
				Some(format!("add_constraint_{}", table.to_lowercase()))
			}
			Operation::DropConstraint {
				table: _,
				constraint_name,
			} => Some(format!(
				"drop_constraint_{}",
				constraint_name.to_lowercase()
			)),
			Operation::CreateIndex { table, unique, .. } => {
				if *unique {
					Some(format!("create_unique_index_{}", table.to_lowercase()))
				} else {
					Some(format!("create_index_{}", table.to_lowercase()))
				}
			}
			Operation::DropIndex { table, .. } => {
				Some(format!("drop_index_{}", table.to_lowercase()))
			}
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
		}
	}

	fn describe(&self) -> String {
		match self {
			Operation::CreateTable { name, .. } => format!("Create table {}", name),
			Operation::DropTable { name } => format!("Drop table {}", name),
			Operation::AddColumn { table, column, .. } => {
				format!("Add column {} to {}", column.name, table)
			}
			Operation::DropColumn { table, column } => {
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
			Operation::AddConstraint { table, .. } => format!("Add constraint on {}", table),
			Operation::DropConstraint {
				table,
				constraint_name,
			} => format!("Drop constraint {} from {}", constraint_name, table),
			Operation::CreateIndex { table, unique, .. } => {
				if *unique {
					format!("Create unique index on {}", table)
				} else {
					format!("Create index on {}", table)
				}
			}
			Operation::DropIndex { table, .. } => format!("Drop index on {}", table),
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
			// DropIndex: Sort columns
			Operation::DropIndex { table, columns } => {
				let mut sorted_columns = columns.clone();
				sorted_columns.sort();

				Operation::DropIndex {
					table: table.clone(),
					columns: sorted_columns,
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
	use FieldType;

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
				},
				ColumnDefinition {
					name: "name".to_string(),
					type_definition: FieldType::VarChar(100),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
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
				},
				ColumnDefinition {
					name: "name".to_string(),
					type_definition: FieldType::VarChar(100),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
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

	#[test]
	fn test_state_forwards_drop_table() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new("id".to_string(), FieldType::Integer, false));
		state.add_model(model);

		let op = Operation::DropTable {
			name: "users".to_string(),
		};

		op.state_forwards("myapp", &mut state);
		assert!(
			state.get_model("myapp", "users").is_none(),
			"Model 'users' should be removed from state after drop"
		);
	}

	#[test]
	fn test_state_forwards_add_column() {
		let mut state = ProjectState::new();
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new("id".to_string(), FieldType::Integer, false));
		state.add_model(model);

		let op = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition {
				name: "email".to_string(),
				type_definition: FieldType::VarChar(255),
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
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
			table: "users".to_string(),
			column: "email".to_string(),
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
		let mut model = ModelState::new("myapp", "users");
		model.add_field(FieldState::new("id".to_string(), FieldType::Integer, false));
		state.add_model(model);

		let op = Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "accounts".to_string(),
		};

		op.state_forwards("myapp", &mut state);
		assert!(
			state.get_model("myapp", "users").is_none(),
			"Old model name 'users' should not exist after rename"
		);
		assert!(
			state.get_model("myapp", "accounts").is_some(),
			"New model name 'accounts' should exist after rename"
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
			table: "users".to_string(),
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
		let sql = reverse.unwrap().unwrap();
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
			},
			mysql_options: None,
		};

		let state = ProjectState::default();
		let reverse = op.to_reverse_sql(&SqlDialect::Postgres, &state);
		assert!(
			reverse.is_ok() && reverse.as_ref().ok().unwrap().is_some(),
			"AddColumn should have reverse SQL operation"
		);
		let sql = reverse.unwrap().unwrap();
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
		let sql = reverse.unwrap().unwrap();
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

	#[test]
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
			table: "users".to_string(),
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
}
