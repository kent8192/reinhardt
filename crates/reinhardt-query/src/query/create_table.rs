//! CREATE TABLE statement builder
//!
//! This module provides the `CreateTableStatement` type for building SQL CREATE TABLE queries.

use crate::{
	backend::QueryBuilder,
	types::{ColumnDef, IndexDef, IntoIden, IntoTableRef, TableConstraint, TableRef},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE TABLE statement builder
///
/// This struct provides a fluent API for constructing CREATE TABLE queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
/// use reinhardt_query::types::ddl::ColumnType;
///
/// let query = Query::create_table()
///     .table("users")
///     .if_not_exists()
///     .col(
///         ColumnDef::new("id")
///             .column_type(ColumnType::Integer)
///             .primary_key(true)
///             .auto_increment(true)
///     )
///     .col(
///         ColumnDef::new("name")
///             .column_type(ColumnType::String(Some(100)))
///             .not_null(true)
///     );
/// ```
#[derive(Debug, Clone)]
pub struct CreateTableStatement {
	pub(crate) table: Option<TableRef>,
	pub(crate) columns: Vec<ColumnDef>,
	pub(crate) constraints: Vec<TableConstraint>,
	pub(crate) indexes: Vec<IndexDef>,
	pub(crate) if_not_exists: bool,
	pub(crate) comment: Option<String>,
}

impl CreateTableStatement {
	/// Create a new CREATE TABLE statement
	pub fn new() -> Self {
		Self {
			table: None,
			columns: Vec::new(),
			constraints: Vec::new(),
			indexes: Vec::new(),
			if_not_exists: false,
			comment: None,
		}
	}

	/// Take the ownership of data in the current [`CreateTableStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			table: self.table.take(),
			columns: std::mem::take(&mut self.columns),
			constraints: std::mem::take(&mut self.constraints),
			indexes: std::mem::take(&mut self.indexes),
			if_not_exists: self.if_not_exists,
			comment: self.comment.take(),
		}
	}

	/// Set the table to create
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_table()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(tbl.into_table_ref());
		self
	}

	/// Add IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.if_not_exists = true;
		self
	}

	/// Add a column definition
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::{ColumnDef, ColumnType};
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .col(
	///         ColumnDef::new("id")
	///             .column_type(ColumnType::Integer)
	///             .primary_key(true)
	///     );
	/// ```
	pub fn col(&mut self, column: ColumnDef) -> &mut Self {
		self.columns.push(column);
		self
	}

	/// Add multiple column definitions
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::{ColumnDef, ColumnType};
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .cols(vec![
	///         ColumnDef::new("id").column_type(ColumnType::Integer),
	///         ColumnDef::new("name").column_type(ColumnType::String(Some(100))),
	///     ]);
	/// ```
	pub fn cols<I>(&mut self, columns: I) -> &mut Self
	where
		I: IntoIterator<Item = ColumnDef>,
	{
		for col in columns {
			self.columns.push(col);
		}
		self
	}

	/// Add a table constraint
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::TableConstraint;
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .constraint(TableConstraint::PrimaryKey {
	///         name: Some("pk_users".into()),
	///         columns: vec!["id".into()],
	///     });
	/// ```
	pub fn constraint(&mut self, constraint: TableConstraint) -> &mut Self {
		self.constraints.push(constraint);
		self
	}

	/// Add multiple table constraints
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::TableConstraint;
	///
	/// let query = Query::create_table()
	///     .table("order_items")
	///     .constraints(vec![
	///         TableConstraint::PrimaryKey {
	///             name: None,
	///             columns: vec!["order_id".into(), "item_id".into()],
	///         },
	///         TableConstraint::ForeignKey {
	///             name: Some("fk_order".into()),
	///             columns: vec!["order_id".into()],
	///             ref_table: Box::new("orders".into()),
	///             ref_columns: vec!["id".into()],
	///             on_delete: None,
	///             on_update: None,
	///         },
	///     ]);
	/// ```
	pub fn constraints<I>(&mut self, constraints: I) -> &mut Self
	where
		I: IntoIterator<Item = TableConstraint>,
	{
		for constraint in constraints {
			self.constraints.push(constraint);
		}
		self
	}

	/// Add an index definition
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::IndexDef;
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .index(
	///         IndexDef::new("idx_email", "users")
	///             .column("email")
	///             .unique(true)
	///     );
	/// ```
	pub fn index(&mut self, index: IndexDef) -> &mut Self {
		self.indexes.push(index);
		self
	}

	/// Add multiple index definitions
	pub fn indexes<I>(&mut self, indexes: I) -> &mut Self
	where
		I: IntoIterator<Item = IndexDef>,
	{
		for index in indexes {
			self.indexes.push(index);
		}
		self
	}

	/// Set table comment
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .comment("User accounts table");
	/// ```
	pub fn comment<S: Into<String>>(&mut self, comment: S) -> &mut Self {
		self.comment = Some(comment.into());
		self
	}

	/// Add a primary key constraint
	///
	/// This is a convenience method for adding a PRIMARY KEY constraint.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .primary_key(vec!["id"]);
	/// ```
	pub fn primary_key<I, C>(&mut self, columns: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		self.constraints.push(TableConstraint::PrimaryKey {
			name: None,
			columns: columns.into_iter().map(|c| c.into_iden()).collect(),
		});
		self
	}

	/// Add a unique constraint
	///
	/// This is a convenience method for adding a UNIQUE constraint.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .unique(vec!["email"]);
	/// ```
	pub fn unique<I, C>(&mut self, columns: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		self.constraints.push(TableConstraint::Unique {
			name: None,
			columns: columns.into_iter().map(|c| c.into_iden()).collect(),
		});
		self
	}

	/// Add a foreign key constraint
	///
	/// This is a convenience method for adding a FOREIGN KEY constraint.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::ForeignKeyAction;
	///
	/// let query = Query::create_table()
	///     .table("posts")
	///     .foreign_key(
	///         vec!["user_id"],
	///         "users",
	///         vec!["id"],
	///         Some(ForeignKeyAction::Cascade),
	///         None,
	///     );
	/// ```
	pub fn foreign_key<I1, C1, T, I2, C2>(
		&mut self,
		columns: I1,
		ref_table: T,
		ref_columns: I2,
		on_delete: Option<crate::types::ForeignKeyAction>,
		on_update: Option<crate::types::ForeignKeyAction>,
	) -> &mut Self
	where
		I1: IntoIterator<Item = C1>,
		C1: IntoIden,
		T: IntoTableRef,
		I2: IntoIterator<Item = C2>,
		C2: IntoIden,
	{
		self.constraints.push(TableConstraint::ForeignKey {
			name: None,
			columns: columns.into_iter().map(|c| c.into_iden()).collect(),
			ref_table: Box::new(ref_table.into_table_ref()),
			ref_columns: ref_columns.into_iter().map(|c| c.into_iden()).collect(),
			on_delete,
			on_update,
		});
		self
	}

	/// Add a foreign key constraint from a `ForeignKeyCreateStatement` builder.
	///
	/// This method accepts the builder-pattern style used by
	/// [`ForeignKey::create()`](super::ForeignKey::create).
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let mut fk = ForeignKey::create();
	/// fk.from_tbl(Alias::new("posts"))
	///     .from_col(Alias::new("user_id"))
	///     .to_tbl(Alias::new("users"))
	///     .to_col(Alias::new("id"));
	///
	/// let mut stmt = Query::create_table();
	/// stmt.table("posts")
	///     .foreign_key_from_builder(&mut fk);
	/// ```
	pub fn foreign_key_from_builder(
		&mut self,
		fk: &mut super::ForeignKeyCreateStatement,
	) -> &mut Self {
		let ref_table = fk
			.to_tbl
			.take()
			.expect("ForeignKeyCreateStatement: to_tbl is required");
		self.constraints.push(TableConstraint::ForeignKey {
			name: fk.name.take(),
			columns: std::mem::take(&mut fk.from_cols),
			ref_table: Box::new(ref_table),
			ref_columns: std::mem::take(&mut fk.to_cols),
			on_delete: fk.on_delete.take(),
			on_update: fk.on_update.take(),
		});
		self
	}
}

impl Default for CreateTableStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateTableStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_table(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateTableStatement {}
