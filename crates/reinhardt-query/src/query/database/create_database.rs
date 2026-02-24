//! CREATE DATABASE statement builder
//!
//! This module provides the `CreateDatabaseStatement` type for building SQL CREATE DATABASE queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE DATABASE statement builder
///
/// This struct provides a fluent API for constructing CREATE DATABASE queries.
/// It supports both PostgreSQL and MySQL database creation options.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // CREATE DATABASE mydb
/// let query = Query::create_database()
///     .name("mydb");
///
/// // CREATE DATABASE IF NOT EXISTS mydb
/// let query = Query::create_database()
///     .name("mydb")
///     .if_not_exists();
///
/// // CREATE DATABASE mydb OWNER alice (PostgreSQL)
/// let query = Query::create_database()
///     .name("mydb")
///     .owner("alice");
///
/// // CREATE DATABASE mydb TEMPLATE template0 ENCODING 'UTF8' (PostgreSQL)
/// let query = Query::create_database()
///     .name("mydb")
///     .template("template0")
///     .encoding("UTF8");
///
/// // CREATE DATABASE mydb CHARACTER SET utf8mb4 (MySQL)
/// let query = Query::create_database()
///     .name("mydb")
///     .character_set("utf8mb4");
/// ```
#[derive(Debug, Clone)]
pub struct CreateDatabaseStatement {
	pub(crate) database_name: Option<DynIden>,
	pub(crate) if_not_exists: bool,
	pub(crate) owner: Option<DynIden>,
	pub(crate) template: Option<DynIden>,
	pub(crate) encoding: Option<String>,
	pub(crate) lc_collate: Option<String>,
	pub(crate) lc_ctype: Option<String>,
	pub(crate) character_set: Option<String>,
	pub(crate) collate: Option<String>,
}

impl CreateDatabaseStatement {
	/// Create a new CREATE DATABASE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database();
	/// ```
	pub fn new() -> Self {
		Self {
			database_name: None,
			if_not_exists: false,
			owner: None,
			template: None,
			encoding: None,
			lc_collate: None,
			lc_ctype: None,
			character_set: None,
			collate: None,
		}
	}

	/// Take the ownership of data in the current [`CreateDatabaseStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			database_name: self.database_name.take(),
			if_not_exists: self.if_not_exists,
			owner: self.owner.take(),
			template: self.template.take(),
			encoding: self.encoding.take(),
			lc_collate: self.lc_collate.take(),
			lc_ctype: self.lc_ctype.take(),
			character_set: self.character_set.take(),
			collate: self.collate.take(),
		}
	}

	/// Set the database name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.database_name = Some(name.into_iden());
		self
	}

	/// Add IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb")
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.if_not_exists = true;
		self
	}

	/// Set OWNER (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb")
	///     .owner("alice");
	/// ```
	pub fn owner<O>(&mut self, owner: O) -> &mut Self
	where
		O: IntoIden,
	{
		self.owner = Some(owner.into_iden());
		self
	}

	/// Set TEMPLATE database (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb")
	///     .template("template0");
	/// ```
	pub fn template<T>(&mut self, template: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.template = Some(template.into_iden());
		self
	}

	/// Set ENCODING (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb")
	///     .encoding("UTF8");
	/// ```
	pub fn encoding<S>(&mut self, encoding: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.encoding = Some(encoding.into());
		self
	}

	/// Set LC_COLLATE (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb")
	///     .lc_collate("en_US.UTF-8");
	/// ```
	pub fn lc_collate<S>(&mut self, lc_collate: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.lc_collate = Some(lc_collate.into());
		self
	}

	/// Set LC_CTYPE (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb")
	///     .lc_ctype("en_US.UTF-8");
	/// ```
	pub fn lc_ctype<S>(&mut self, lc_ctype: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.lc_ctype = Some(lc_ctype.into());
		self
	}

	/// Set CHARACTER SET (MySQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb")
	///     .character_set("utf8mb4");
	/// ```
	pub fn character_set<S>(&mut self, charset: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.character_set = Some(charset.into());
		self
	}

	/// Set COLLATE (MySQL/PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("mydb")
	///     .collate("utf8mb4_unicode_ci");
	/// ```
	pub fn collate<S>(&mut self, collate: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.collate = Some(collate.into());
		self
	}
}

impl Default for CreateDatabaseStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateDatabaseStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_create_database(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateDatabaseStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_create_database_new() {
		let stmt = CreateDatabaseStatement::new();
		assert!(stmt.database_name.is_none());
		assert!(!stmt.if_not_exists);
		assert!(stmt.owner.is_none());
		assert!(stmt.template.is_none());
		assert!(stmt.encoding.is_none());
		assert!(stmt.lc_collate.is_none());
		assert!(stmt.lc_ctype.is_none());
		assert!(stmt.character_set.is_none());
		assert!(stmt.collate.is_none());
	}

	#[rstest]
	fn test_create_database_with_name() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
	}

	#[rstest]
	fn test_create_database_if_not_exists() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").if_not_exists();
		assert!(stmt.if_not_exists);
	}

	#[rstest]
	fn test_create_database_with_owner() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").owner("alice");
		assert_eq!(stmt.owner.as_ref().unwrap().to_string(), "alice");
	}

	#[rstest]
	fn test_create_database_with_template() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").template("template0");
		assert_eq!(stmt.template.as_ref().unwrap().to_string(), "template0");
	}

	#[rstest]
	fn test_create_database_with_encoding() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").encoding("UTF8");
		assert_eq!(stmt.encoding.as_ref().unwrap(), "UTF8");
	}

	#[rstest]
	fn test_create_database_with_lc_collate() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").lc_collate("en_US.UTF-8");
		assert_eq!(stmt.lc_collate.as_ref().unwrap(), "en_US.UTF-8");
	}

	#[rstest]
	fn test_create_database_with_lc_ctype() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").lc_ctype("en_US.UTF-8");
		assert_eq!(stmt.lc_ctype.as_ref().unwrap(), "en_US.UTF-8");
	}

	#[rstest]
	fn test_create_database_with_character_set() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").character_set("utf8mb4");
		assert_eq!(stmt.character_set.as_ref().unwrap(), "utf8mb4");
	}

	#[rstest]
	fn test_create_database_with_collate() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").collate("utf8mb4_unicode_ci");
		assert_eq!(stmt.collate.as_ref().unwrap(), "utf8mb4_unicode_ci");
	}

	#[rstest]
	fn test_create_database_postgresql_full() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb")
			.if_not_exists()
			.owner("alice")
			.template("template0")
			.encoding("UTF8")
			.lc_collate("en_US.UTF-8")
			.lc_ctype("en_US.UTF-8");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
		assert!(stmt.if_not_exists);
		assert_eq!(stmt.owner.as_ref().unwrap().to_string(), "alice");
		assert_eq!(stmt.template.as_ref().unwrap().to_string(), "template0");
		assert_eq!(stmt.encoding.as_ref().unwrap(), "UTF8");
		assert_eq!(stmt.lc_collate.as_ref().unwrap(), "en_US.UTF-8");
		assert_eq!(stmt.lc_ctype.as_ref().unwrap(), "en_US.UTF-8");
	}

	#[rstest]
	fn test_create_database_mysql_full() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb")
			.if_not_exists()
			.character_set("utf8mb4")
			.collate("utf8mb4_unicode_ci");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
		assert!(stmt.if_not_exists);
		assert_eq!(stmt.character_set.as_ref().unwrap(), "utf8mb4");
		assert_eq!(stmt.collate.as_ref().unwrap(), "utf8mb4_unicode_ci");
	}

	#[rstest]
	fn test_create_database_take() {
		let mut stmt = CreateDatabaseStatement::new();
		stmt.name("mydb").owner("alice");
		let taken = stmt.take();
		assert!(stmt.database_name.is_none());
		assert!(stmt.owner.is_none());
		assert_eq!(taken.database_name.as_ref().unwrap().to_string(), "mydb");
		assert_eq!(taken.owner.as_ref().unwrap().to_string(), "alice");
	}
}
