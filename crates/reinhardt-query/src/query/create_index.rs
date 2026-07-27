//! CREATE INDEX statement builder
//!
//! This module provides the `CreateIndexStatement` type for building SQL CREATE INDEX queries.

use crate::{
	backend::QueryBuilder,
	expr::SimpleExpr,
	types::{DynIden, IntoIden, IntoTableRef, Order, TableRef},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE INDEX statement builder
///
/// This struct provides a fluent API for constructing CREATE INDEX queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::create_index()
///     .name("idx_email")
///     .table("users")
///     .col("email")
///     .unique();
/// ```
#[derive(Debug, Clone)]
pub struct CreateIndexStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) table: Option<TableRef>,
	pub(crate) columns: Vec<IndexColumn>,
	pub(crate) unique: bool,
	pub(crate) if_not_exists: bool,
	pub(crate) r#where: Option<SimpleExpr>,
	pub(crate) using: Option<IndexMethod>,
	pub(crate) options: Option<IndexOptions>,
}

/// Index column specification
///
/// This struct represents a column in an index, including its name and sort order.
#[derive(Debug, Clone)]
pub struct IndexColumn {
	pub(crate) name: DynIden,
	pub(crate) order: Option<Order>,
	pub(crate) operator_class: Option<String>,
}

/// Index method (PostgreSQL and MySQL)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IndexMethod {
	/// BTREE - B-Tree index (default for most databases)
	BTree,
	/// HASH - Hash index
	Hash,
	/// GIST - Generalized Search Tree (PostgreSQL)
	Gist,
	/// GIN - Generalized Inverted Index (PostgreSQL)
	Gin,
	/// BRIN - Block Range Index (PostgreSQL)
	Brin,
	/// FULLTEXT - Full-text index (MySQL)
	FullText,
	/// SPATIAL - Spatial index (MySQL)
	Spatial,
	/// HNSW - Hierarchical Navigable Small World vector index (PostgreSQL)
	Hnsw,
	/// IVFFlat - Inverted file vector index (PostgreSQL)
	Ivfflat,
}

/// Method-specific options for approximate vector indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IndexOptions {
	/// HNSW construction parameters.
	Hnsw {
		/// Maximum number of connections per layer.
		m: Option<u16>,
		/// Candidate list size used while constructing the index.
		ef_construction: Option<u16>,
	},
	/// IVFFlat construction parameters.
	Ivfflat {
		/// Number of inverted lists.
		lists: Option<u32>,
	},
}

impl IndexMethod {
	/// Get the SQL keyword for this index method
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::BTree => "BTREE",
			Self::Hash => "HASH",
			Self::Gist => "GIST",
			Self::Gin => "GIN",
			Self::Brin => "BRIN",
			Self::FullText => "FULLTEXT",
			Self::Spatial => "SPATIAL",
			Self::Hnsw => "HNSW",
			Self::Ivfflat => "IVFFLAT",
		}
	}
}

impl CreateIndexStatement {
	/// Create a new CREATE INDEX statement
	pub fn new() -> Self {
		Self {
			name: None,
			table: None,
			columns: Vec::new(),
			unique: false,
			if_not_exists: false,
			r#where: None,
			using: None,
			options: None,
		}
	}

	/// Take the ownership of data in the current [`CreateIndexStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			table: self.table.take(),
			columns: std::mem::take(&mut self.columns),
			unique: self.unique,
			if_not_exists: self.if_not_exists,
			r#where: self.r#where.take(),
			using: self.using.take(),
			options: self.options.take(),
		}
	}

	/// Set the index name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_email");
	/// ```
	pub fn name<T>(&mut self, name: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Set the table
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_email")
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(tbl.into_table_ref());
		self
	}

	/// Add a column to the index
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_name_email")
	///     .table("users")
	///     .col("name")
	///     .col("email");
	/// ```
	pub fn col<C>(&mut self, column: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.columns.push(IndexColumn {
			name: column.into_iden(),
			order: None,
			operator_class: None,
		});
		self
	}

	/// Add a column with a PostgreSQL operator class.
	pub fn col_with_operator_class<C, O>(&mut self, column: C, operator_class: O) -> &mut Self
	where
		C: IntoIden,
		O: Into<String>,
	{
		self.columns.push(IndexColumn {
			name: column.into_iden(),
			order: None,
			operator_class: Some(operator_class.into()),
		});
		self
	}

	/// Add a column with sort order
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::Order;
	///
	/// let query = Query::create_index()
	///     .name("idx_created_at")
	///     .table("posts")
	///     .col_order("created_at", Order::Desc);
	/// ```
	pub fn col_order<C>(&mut self, column: C, order: Order) -> &mut Self
	where
		C: IntoIden,
	{
		self.columns.push(IndexColumn {
			name: column.into_iden(),
			order: Some(order),
			operator_class: None,
		});
		self
	}

	/// Add multiple columns to the index
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_name_email")
	///     .table("users")
	///     .cols(vec!["name", "email"]);
	/// ```
	pub fn cols<I, C>(&mut self, columns: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		for col in columns {
			self.col(col);
		}
		self
	}

	/// Set UNIQUE attribute
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_email")
	///     .table("users")
	///     .col("email")
	///     .unique();
	/// ```
	pub fn unique(&mut self) -> &mut Self {
		self.unique = true;
		self
	}

	/// Add IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_email")
	///     .table("users")
	///     .col("email")
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.if_not_exists = true;
		self
	}

	/// Add WHERE clause for partial index
	///
	/// Partial indexes are supported by PostgreSQL and SQLite.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_active_users")
	///     .table("users")
	///     .col("email")
	///     .r#where(Expr::col("active").eq(true));
	/// ```
	pub fn r#where(&mut self, condition: SimpleExpr) -> &mut Self {
		self.r#where = Some(condition);
		self
	}

	/// Set index method using USING clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::query::IndexMethod;
	///
	/// let query = Query::create_index()
	///     .name("idx_email")
	///     .table("users")
	///     .col("email")
	///     .using(IndexMethod::Hash);
	/// ```
	pub fn using(&mut self, method: IndexMethod) -> &mut Self {
		self.using = Some(method);
		self
	}

	/// Set method-specific approximate vector index options.
	pub fn options(&mut self, options: IndexOptions) -> &mut Self {
		self.options = Some(options);
		self
	}

	pub(crate) fn validate_for_backend(
		&self,
		backend: &'static str,
		supports_approximate_vector_indexes: bool,
	) -> Result<(), crate::QueryBuildError> {
		let approximate_method =
			matches!(self.using, Some(IndexMethod::Hnsw | IndexMethod::Ivfflat));

		if approximate_method && !supports_approximate_vector_indexes {
			return Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "approximate vector indexes",
				backend,
			});
		}

		if !supports_approximate_vector_indexes
			&& self
				.columns
				.iter()
				.any(|column| column.operator_class.is_some())
		{
			return Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "index operator classes",
				backend,
			});
		}

		if supports_approximate_vector_indexes
			&& self.columns.iter().any(|column| {
				column
					.operator_class
					.as_deref()
					.is_some_and(|operator_class| !is_valid_postgres_operator_class(operator_class))
			}) {
			return Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "valid PostgreSQL operator class",
				backend,
			});
		}

		let options_match_method = matches!(
			(self.using, self.options),
			(_, None)
				| (Some(IndexMethod::Hnsw), Some(IndexOptions::Hnsw { .. }))
				| (
					Some(IndexMethod::Ivfflat),
					Some(IndexOptions::Ivfflat { .. })
				)
		);
		if !options_match_method {
			return Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "matching approximate vector index method and options",
				backend,
			});
		}

		if !approximate_method {
			return Ok(());
		}

		if self.unique {
			return Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "non-unique approximate vector indexes",
				backend,
			});
		}
		if self.columns.len() != 1 {
			return Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "single-column approximate vector indexes",
				backend,
			});
		}
		if !self.columns[0]
			.operator_class
			.as_deref()
			.is_some_and(is_supported_vector_operator_class)
		{
			return Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "supported vector operator class",
				backend,
			});
		}

		match self.options {
			Some(IndexOptions::Hnsw {
				m: Some(m),
				ef_construction: _,
			}) if !(2..=100).contains(&m) => Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "HNSW m in 2..=100",
				backend,
			}),
			Some(IndexOptions::Hnsw {
				m: _,
				ef_construction: Some(ef_construction),
			}) if !(4..=1000).contains(&ef_construction) => {
				Err(crate::QueryBuildError::UnsupportedBackendFeature {
					feature: "HNSW ef_construction in 4..=1000",
					backend,
				})
			}
			Some(IndexOptions::Hnsw {
				m: Some(m),
				ef_construction: Some(ef_construction),
			}) if ef_construction < 2 * m => Err(crate::QueryBuildError::UnsupportedBackendFeature {
				feature: "HNSW ef_construction at least twice m",
				backend,
			}),
			Some(IndexOptions::Ivfflat { lists: Some(lists) }) if !(1..=32768).contains(&lists) => {
				Err(crate::QueryBuildError::UnsupportedBackendFeature {
					feature: "IVFFlat lists in 1..=32768",
					backend,
				})
			}
			_ => Ok(()),
		}
	}
}

pub(crate) fn is_supported_vector_operator_class(operator_class: &str) -> bool {
	matches!(
		operator_class,
		"vector_l2_ops" | "vector_ip_ops" | "vector_cosine_ops"
	)
}

fn is_valid_postgres_operator_class(operator_class: &str) -> bool {
	let identifiers = operator_class.split('.').collect::<Vec<_>>();
	(1..=2).contains(&identifiers.len())
		&& identifiers.into_iter().all(|identifier| {
			let mut characters = identifier.chars();
			characters
				.next()
				.is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
				&& characters.all(|character| {
					character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
				})
		})
}

impl Default for CreateIndexStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateIndexStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_index(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_index(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_index(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateIndexStatement {}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::{CreateIndexStatement, IndexMethod, IndexOptions};
	use crate::{
		QueryBuildError,
		backend::{MySqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder},
		prelude::Query,
	};

	fn hnsw_statement() -> CreateIndexStatement {
		let mut statement = Query::create_index();
		statement
			.name("source_embedding_cosine_hnsw")
			.table("source")
			.col_with_operator_class("embedding", "vector_cosine_ops")
			.using(IndexMethod::Hnsw)
			.options(IndexOptions::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			});
		statement
	}

	fn ivfflat_statement() -> CreateIndexStatement {
		let mut statement = Query::create_index();
		statement
			.name("source_embedding_l2_ivfflat")
			.table("source")
			.col_with_operator_class("embedding", "vector_l2_ops")
			.using(IndexMethod::Ivfflat)
			.options(IndexOptions::Ivfflat { lists: Some(100) });
		statement
	}

	#[rstest]
	fn create_vector_index_hnsw_renders_exact_postgres_sql() {
		// Arrange
		let statement = hnsw_statement();

		// Act
		let (sql, values) = PostgresQueryBuilder::new()
			.build_create_index_checked(&statement)
			.unwrap();

		// Assert
		assert_eq!(
			sql,
			r#"CREATE INDEX "source_embedding_cosine_hnsw" ON "source" USING HNSW ("embedding" vector_cosine_ops) WITH (m = 16, ef_construction = 64)"#
		);
		assert!(values.is_empty());
	}

	#[rstest]
	fn create_vector_index_ivfflat_renders_exact_postgres_sql() {
		// Arrange
		let statement = ivfflat_statement();

		// Act
		let (sql, values) = PostgresQueryBuilder::new()
			.build_create_index_checked(&statement)
			.unwrap();

		// Assert
		assert_eq!(
			sql,
			r#"CREATE INDEX "source_embedding_l2_ivfflat" ON "source" USING IVFFLAT ("embedding" vector_l2_ops) WITH (lists = 100)"#
		);
		assert!(values.is_empty());
	}

	#[rstest]
	fn create_vector_index_hnsw_option_order_is_deterministic() {
		// Arrange
		let mut using_first = Query::create_index();
		using_first
			.name("idx_embedding")
			.table("source")
			.col_with_operator_class("embedding", "vector_ip_ops")
			.using(IndexMethod::Hnsw)
			.options(IndexOptions::Hnsw {
				m: Some(8),
				ef_construction: Some(32),
			});
		let mut options_first = Query::create_index();
		options_first
			.name("idx_embedding")
			.table("source")
			.col_with_operator_class("embedding", "vector_ip_ops")
			.options(IndexOptions::Hnsw {
				m: Some(8),
				ef_construction: Some(32),
			})
			.using(IndexMethod::Hnsw);

		// Act
		let using_first_sql = PostgresQueryBuilder::new()
			.build_create_index_checked(&using_first)
			.unwrap()
			.0;
		let options_first_sql = PostgresQueryBuilder::new()
			.build_create_index_checked(&options_first)
			.unwrap()
			.0;

		// Assert
		assert_eq!(using_first_sql, options_first_sql);
		assert_eq!(
			using_first_sql,
			r#"CREATE INDEX "idx_embedding" ON "source" USING HNSW ("embedding" vector_ip_ops) WITH (m = 8, ef_construction = 32)"#
		);
	}

	#[rstest]
	#[case("MySQL")]
	#[case("SQLite")]
	fn create_vector_index_rejects_non_postgres_backends(#[case] backend: &'static str) {
		// Arrange
		let statement = hnsw_statement();

		// Act
		let result = match backend {
			"MySQL" => MySqlQueryBuilder::new().build_create_index_checked(&statement),
			"SQLite" => SqliteQueryBuilder::new().build_create_index_checked(&statement),
			_ => unreachable!(),
		};

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "approximate vector indexes",
				backend: actual_backend,
			}) if actual_backend == backend
		));
	}

	#[rstest]
	#[case(
		IndexMethod::Hnsw,
		IndexOptions::Hnsw {
			m: Some(1),
			ef_construction: Some(64),
		},
		"HNSW m in 2..=100"
	)]
	#[case(
		IndexMethod::Hnsw,
		IndexOptions::Hnsw {
			m: Some(16),
			ef_construction: Some(1),
		},
		"HNSW ef_construction in 4..=1000"
	)]
	#[case(
		IndexMethod::Ivfflat,
		IndexOptions::Ivfflat { lists: Some(32769) },
		"IVFFlat lists in 1..=32768"
	)]
	#[case(
		IndexMethod::Hnsw,
		IndexOptions::Hnsw {
			m: Some(16),
			ef_construction: Some(31),
		},
		"HNSW ef_construction at least twice m"
	)]
	fn create_vector_index_rejects_invalid_options(
		#[case] method: IndexMethod,
		#[case] options: IndexOptions,
		#[case] feature: &'static str,
	) {
		// Arrange
		let mut statement = Query::create_index();
		statement
			.name("idx_embedding")
			.table("source")
			.col_with_operator_class("embedding", "vector_l2_ops")
			.using(method)
			.options(options);

		// Act
		let result = PostgresQueryBuilder::new().build_create_index_checked(&statement);

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: actual_feature,
				backend: "PostgreSQL",
			}) if actual_feature == feature
		));
	}

	#[rstest]
	fn create_vector_index_rejects_method_options_mismatch() {
		// Arrange
		let mut statement = Query::create_index();
		statement
			.name("idx_embedding")
			.table("source")
			.col_with_operator_class("embedding", "vector_l2_ops")
			.using(IndexMethod::Hnsw)
			.options(IndexOptions::Ivfflat { lists: Some(100) });

		// Act
		let result = PostgresQueryBuilder::new().build_create_index_checked(&statement);

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "matching approximate vector index method and options",
				backend: "PostgreSQL",
			})
		));
	}

	#[rstest]
	fn create_vector_index_omits_empty_hnsw_options_clause() {
		// Arrange
		let mut statement = Query::create_index();
		statement
			.name("idx_embedding")
			.table("source")
			.col_with_operator_class("embedding", "vector_l2_ops")
			.using(IndexMethod::Hnsw)
			.options(IndexOptions::Hnsw {
				m: None,
				ef_construction: None,
			});

		// Act
		let sql = PostgresQueryBuilder::new()
			.build_create_index_checked(&statement)
			.unwrap()
			.0;

		// Assert
		assert_eq!(
			sql,
			r#"CREATE INDEX "idx_embedding" ON "source" USING HNSW ("embedding" vector_l2_ops)"#
		);
		assert!(!sql.contains("WITH ()"));
	}

	#[rstest]
	#[case(None, "supported vector operator class")]
	#[case(Some("gin_trgm_ops"), "supported vector operator class")]
	fn create_vector_index_rejects_missing_or_unsupported_operator_class(
		#[case] operator_class: Option<&str>,
		#[case] feature: &'static str,
	) {
		// Arrange
		let mut statement = Query::create_index();
		statement.name("idx_embedding").table("source");
		if let Some(operator_class) = operator_class {
			statement.col_with_operator_class("embedding", operator_class);
		} else {
			statement.col("embedding");
		}
		statement
			.using(IndexMethod::Hnsw)
			.options(IndexOptions::Hnsw {
				m: None,
				ef_construction: None,
			});

		// Act
		let result = PostgresQueryBuilder::new().build_create_index_checked(&statement);

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: actual_feature,
				backend: "PostgreSQL",
			}) if actual_feature == feature
		));
	}

	#[rstest]
	fn create_vector_index_rejects_multiple_columns() {
		// Arrange
		let mut statement = hnsw_statement();
		statement.col("tenant_id");

		// Act
		let result = PostgresQueryBuilder::new().build_create_index_checked(&statement);

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "single-column approximate vector indexes",
				backend: "PostgreSQL",
			})
		));
	}

	#[rstest]
	fn create_vector_index_rejects_unique_indexes() {
		// Arrange
		let mut statement = ivfflat_statement();
		statement.unique();

		// Act
		let result = PostgresQueryBuilder::new().build_create_index_checked(&statement);

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "non-unique approximate vector indexes",
				backend: "PostgreSQL",
			})
		));
	}

	#[rstest]
	fn create_vector_index_rejects_malicious_generic_operator_class() {
		// Arrange
		let mut statement = Query::create_index();
		statement
			.name("idx_document_search")
			.table("document")
			.col_with_operator_class("search", "gin_trgm_ops) WHERE true; --")
			.using(IndexMethod::Gin);

		// Act
		let result = PostgresQueryBuilder::new().build_create_index_checked(&statement);

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "valid PostgreSQL operator class",
				backend: "PostgreSQL",
			})
		));
	}

	#[rstest]
	fn create_vector_index_rejects_overqualified_generic_operator_class() {
		// Arrange
		let mut statement = Query::create_index();
		statement
			.name("idx_document_search")
			.table("document")
			.col_with_operator_class("search", "database.public.gin_trgm_ops")
			.using(IndexMethod::Gin);

		// Act
		let result = PostgresQueryBuilder::new().build_create_index_checked(&statement);

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "valid PostgreSQL operator class",
				backend: "PostgreSQL",
			})
		));
	}

	#[rstest]
	fn create_vector_index_accepts_safe_qualified_generic_operator_class() {
		// Arrange
		let mut statement = Query::create_index();
		statement
			.name("idx_document_search")
			.table("document")
			.col_with_operator_class("search", "public.gin_trgm_ops")
			.using(IndexMethod::Gin);

		// Act
		let sql = PostgresQueryBuilder::new()
			.build_create_index_checked(&statement)
			.unwrap()
			.0;

		// Assert
		assert_eq!(
			sql,
			r#"CREATE INDEX "idx_document_search" ON "document" USING GIN ("search" public.gin_trgm_ops)"#
		);
	}

	#[rstest]
	fn create_vector_index_rejects_qualified_vector_operator_class() {
		// Arrange
		let mut statement = hnsw_statement();
		statement.columns[0].operator_class = Some("public.vector_cosine_ops".to_string());

		// Act
		let result = PostgresQueryBuilder::new().build_create_index_checked(&statement);

		// Assert
		assert!(matches!(
			result,
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "supported vector operator class",
				backend: "PostgreSQL",
			})
		));
	}
}
