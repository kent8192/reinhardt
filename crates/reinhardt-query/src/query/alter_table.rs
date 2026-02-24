//! ALTER TABLE statement builder
//!
//! This module provides the `AlterTableStatement` type for building SQL ALTER TABLE queries.

use crate::{
	backend::QueryBuilder,
	types::{ColumnDef, DynIden, ForeignKeyAction, IntoIden, IntoTableRef, TableRef},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ALTER TABLE statement builder
///
/// This struct provides a fluent API for constructing ALTER TABLE queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
/// use reinhardt_query::types::ddl::{ColumnDef, ColumnType};
///
/// let query = Query::alter_table()
///     .table("users")
///     .add_column(
///         ColumnDef::new("age")
///             .column_type(ColumnType::Integer)
///     );
/// ```
#[derive(Debug, Clone)]
pub struct AlterTableStatement {
	pub(crate) table: Option<TableRef>,
	pub(crate) operations: Vec<AlterTableOperation>,
}

/// ALTER TABLE operation
///
/// This enum represents the various operations that can be performed with ALTER TABLE.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AlterTableOperation {
	/// ADD COLUMN
	AddColumn(ColumnDef),
	/// DROP COLUMN
	DropColumn {
		/// Column name to drop
		name: DynIden,
		/// IF EXISTS clause
		if_exists: bool,
	},
	/// RENAME COLUMN
	RenameColumn {
		/// Old column name
		old: DynIden,
		/// New column name
		new: DynIden,
	},
	/// MODIFY COLUMN / ALTER COLUMN (type or constraints)
	ModifyColumn(ColumnDef),
	/// ADD CONSTRAINT
	AddConstraint(crate::types::TableConstraint),
	/// DROP CONSTRAINT
	DropConstraint {
		/// Constraint name
		name: DynIden,
		/// IF EXISTS clause
		if_exists: bool,
	},
	/// RENAME TABLE
	RenameTable(DynIden),
}

impl AlterTableStatement {
	/// Create a new ALTER TABLE statement
	pub fn new() -> Self {
		Self {
			table: None,
			operations: Vec::new(),
		}
	}

	/// Take the ownership of data in the current [`AlterTableStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			table: self.table.take(),
			operations: std::mem::take(&mut self.operations),
		}
	}

	/// Set the table to alter
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_table()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(tbl.into_table_ref());
		self
	}

	/// Add a column
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::{ColumnDef, ColumnType};
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .add_column(
	///         ColumnDef::new("age")
	///             .column_type(ColumnType::Integer)
	///             .not_null(false)
	///     );
	/// ```
	pub fn add_column(&mut self, column: ColumnDef) -> &mut Self {
		self.operations.push(AlterTableOperation::AddColumn(column));
		self
	}

	/// Drop a column
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .drop_column("age");
	/// ```
	pub fn drop_column<C>(&mut self, column: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.operations.push(AlterTableOperation::DropColumn {
			name: column.into_iden(),
			if_exists: false,
		});
		self
	}

	/// Drop a column with IF EXISTS
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .drop_column_if_exists("age");
	/// ```
	pub fn drop_column_if_exists<C>(&mut self, column: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.operations.push(AlterTableOperation::DropColumn {
			name: column.into_iden(),
			if_exists: true,
		});
		self
	}

	/// Rename a column
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .rename_column("old_name", "new_name");
	/// ```
	pub fn rename_column<C1, C2>(&mut self, old: C1, new: C2) -> &mut Self
	where
		C1: IntoIden,
		C2: IntoIden,
	{
		self.operations.push(AlterTableOperation::RenameColumn {
			old: old.into_iden(),
			new: new.into_iden(),
		});
		self
	}

	/// Modify a column (type or constraints)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::{ColumnDef, ColumnType};
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .modify_column(
	///         ColumnDef::new("age")
	///             .column_type(ColumnType::BigInteger)
	///     );
	/// ```
	pub fn modify_column(&mut self, column: ColumnDef) -> &mut Self {
		self.operations
			.push(AlterTableOperation::ModifyColumn(column));
		self
	}

	/// Add a constraint
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::TableConstraint;
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .add_constraint(TableConstraint::Unique {
	///         name: Some("uq_email".into()),
	///         columns: vec!["email".into()],
	///     });
	/// ```
	pub fn add_constraint(&mut self, constraint: crate::types::TableConstraint) -> &mut Self {
		self.operations
			.push(AlterTableOperation::AddConstraint(constraint));
		self
	}

	/// Drop a constraint
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .drop_constraint("uq_email");
	/// ```
	pub fn drop_constraint<C>(&mut self, constraint: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.operations.push(AlterTableOperation::DropConstraint {
			name: constraint.into_iden(),
			if_exists: false,
		});
		self
	}

	/// Drop a constraint with IF EXISTS
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .drop_constraint_if_exists("uq_email");
	/// ```
	pub fn drop_constraint_if_exists<C>(&mut self, constraint: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.operations.push(AlterTableOperation::DropConstraint {
			name: constraint.into_iden(),
			if_exists: true,
		});
		self
	}

	/// Rename table
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .rename_table("accounts");
	/// ```
	pub fn rename_table<T>(&mut self, new_name: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.operations
			.push(AlterTableOperation::RenameTable(new_name.into_iden()));
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
	/// let query = Query::alter_table()
	///     .table("users")
	///     .add_primary_key(vec!["id"]);
	/// ```
	pub fn add_primary_key<I, C>(&mut self, columns: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		self.operations.push(AlterTableOperation::AddConstraint(
			crate::types::TableConstraint::PrimaryKey {
				name: None,
				columns: columns.into_iter().map(|c| c.into_iden()).collect(),
			},
		));
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
	/// let query = Query::alter_table()
	///     .table("users")
	///     .add_unique(vec!["email"]);
	/// ```
	pub fn add_unique<I, C>(&mut self, columns: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		self.operations.push(AlterTableOperation::AddConstraint(
			crate::types::TableConstraint::Unique {
				name: None,
				columns: columns.into_iter().map(|c| c.into_iden()).collect(),
			},
		));
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
	/// let query = Query::alter_table()
	///     .table("posts")
	///     .add_foreign_key(
	///         vec!["user_id"],
	///         "users",
	///         vec!["id"],
	///         Some(ForeignKeyAction::Cascade),
	///         None,
	///     );
	/// ```
	pub fn add_foreign_key<I1, C1, T, I2, C2>(
		&mut self,
		columns: I1,
		ref_table: T,
		ref_columns: I2,
		on_delete: Option<ForeignKeyAction>,
		on_update: Option<ForeignKeyAction>,
	) -> &mut Self
	where
		I1: IntoIterator<Item = C1>,
		C1: IntoIden,
		T: IntoTableRef,
		I2: IntoIterator<Item = C2>,
		C2: IntoIden,
	{
		self.operations.push(AlterTableOperation::AddConstraint(
			crate::types::TableConstraint::ForeignKey {
				name: None,
				columns: columns.into_iter().map(|c| c.into_iden()).collect(),
				ref_table: Box::new(ref_table.into_table_ref()),
				ref_columns: ref_columns.into_iter().map(|c| c.into_iden()).collect(),
				on_delete,
				on_update,
			},
		));
		self
	}
}

impl Default for AlterTableStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterTableStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_alter_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_alter_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_alter_table(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AlterTableStatement {}
