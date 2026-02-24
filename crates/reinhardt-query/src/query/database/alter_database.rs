//! ALTER DATABASE statement builder
//!
//! This module provides the `AlterDatabaseStatement` type for building SQL ALTER DATABASE queries.

use crate::{
	backend::QueryBuilder,
	types::{DatabaseOperation, DynIden, IntoIden, ZoneConfig},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ALTER DATABASE statement builder
///
/// This struct provides a fluent API for constructing ALTER DATABASE queries.
/// It supports standard PostgreSQL operations and CockroachDB-specific multi-region configuration.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // ALTER DATABASE mydb RENAME TO newdb
/// let query = Query::alter_database()
///     .name("mydb")
///     .rename_to("newdb");
///
/// // ALTER DATABASE mydb ADD REGION "us-east-1" (CockroachDB)
/// let query = Query::alter_database()
///     .name("mydb")
///     .add_region("us-east-1");
///
/// // ALTER DATABASE mydb PRIMARY REGION "us-east-1" (CockroachDB)
/// let query = Query::alter_database()
///     .name("mydb")
///     .set_primary_region("us-east-1");
/// ```
#[derive(Debug, Clone)]
pub struct AlterDatabaseStatement {
	pub(crate) database_name: Option<DynIden>,
	pub(crate) operations: Vec<DatabaseOperation>,
}

impl AlterDatabaseStatement {
	/// Create a new ALTER DATABASE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_database();
	/// ```
	pub fn new() -> Self {
		Self {
			database_name: None,
			operations: Vec::new(),
		}
	}

	/// Take the ownership of data in the current [`AlterDatabaseStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			database_name: self.database_name.take(),
			operations: std::mem::take(&mut self.operations),
		}
	}

	/// Set the database name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_database()
	///     .name("mydb");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.database_name = Some(name.into_iden());
		self
	}

	/// Rename the database to a new name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_database()
	///     .name("old_db")
	///     .rename_to("new_db");
	/// ```
	pub fn rename_to<N>(&mut self, new_name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.operations
			.push(DatabaseOperation::RenameDatabase(new_name.into_iden()));
		self
	}

	/// Change the owner of the database
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_database()
	///     .name("mydb")
	///     .owner_to("new_owner");
	/// ```
	pub fn owner_to<O>(&mut self, new_owner: O) -> &mut Self
	where
		O: IntoIden,
	{
		self.operations
			.push(DatabaseOperation::OwnerTo(new_owner.into_iden()));
		self
	}

	/// Add a region to the database (CockroachDB-specific)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_database()
	///     .name("mydb")
	///     .add_region("us-east-1");
	/// ```
	pub fn add_region<S>(&mut self, region: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.operations
			.push(DatabaseOperation::AddRegion(region.into()));
		self
	}

	/// Drop a region from the database (CockroachDB-specific)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_database()
	///     .name("mydb")
	///     .drop_region("us-west-1");
	/// ```
	pub fn drop_region<S>(&mut self, region: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.operations
			.push(DatabaseOperation::DropRegion(region.into()));
		self
	}

	/// Set the primary region for the database (CockroachDB-specific)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_database()
	///     .name("mydb")
	///     .set_primary_region("us-east-1");
	/// ```
	pub fn set_primary_region<S>(&mut self, region: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.operations
			.push(DatabaseOperation::SetPrimaryRegion(region.into()));
		self
	}

	/// Configure zone settings for the database (CockroachDB-specific)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ZoneConfig;
	///
	/// let zone = ZoneConfig::new()
	///     .num_replicas(3)
	///     .add_constraint("+region=us-east-1");
	///
	/// let query = Query::alter_database()
	///     .name("mydb")
	///     .configure_zone(zone);
	/// ```
	pub fn configure_zone(&mut self, zone: ZoneConfig) -> &mut Self {
		self.operations.push(DatabaseOperation::ConfigureZone(zone));
		self
	}
}

impl Default for AlterDatabaseStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterDatabaseStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_alter_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_alter_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_alter_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_alter_database(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AlterDatabaseStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_alter_database_new() {
		let stmt = AlterDatabaseStatement::new();
		assert!(stmt.database_name.is_none());
		assert!(stmt.operations.is_empty());
	}

	#[rstest]
	fn test_alter_database_with_name() {
		let mut stmt = AlterDatabaseStatement::new();
		stmt.name("mydb");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
	}

	#[rstest]
	fn test_alter_database_rename_to() {
		let mut stmt = AlterDatabaseStatement::new();
		stmt.name("old_db").rename_to("new_db");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "old_db");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			DatabaseOperation::RenameDatabase(name) => {
				assert_eq!(name.to_string(), "new_db");
			}
			_ => panic!("Expected RenameDatabase operation"),
		}
	}

	#[rstest]
	fn test_alter_database_owner_to() {
		let mut stmt = AlterDatabaseStatement::new();
		stmt.name("mydb").owner_to("new_owner");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			DatabaseOperation::OwnerTo(owner) => {
				assert_eq!(owner.to_string(), "new_owner");
			}
			_ => panic!("Expected OwnerTo operation"),
		}
	}

	#[rstest]
	fn test_alter_database_add_region() {
		let mut stmt = AlterDatabaseStatement::new();
		stmt.name("mydb").add_region("us-east-1");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			DatabaseOperation::AddRegion(region) => {
				assert_eq!(region, "us-east-1");
			}
			_ => panic!("Expected AddRegion operation"),
		}
	}

	#[rstest]
	fn test_alter_database_drop_region() {
		let mut stmt = AlterDatabaseStatement::new();
		stmt.name("mydb").drop_region("us-west-1");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			DatabaseOperation::DropRegion(region) => {
				assert_eq!(region, "us-west-1");
			}
			_ => panic!("Expected DropRegion operation"),
		}
	}

	#[rstest]
	fn test_alter_database_set_primary_region() {
		let mut stmt = AlterDatabaseStatement::new();
		stmt.name("mydb").set_primary_region("us-east-1");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			DatabaseOperation::SetPrimaryRegion(region) => {
				assert_eq!(region, "us-east-1");
			}
			_ => panic!("Expected SetPrimaryRegion operation"),
		}
	}

	#[rstest]
	fn test_alter_database_multiple_operations() {
		let mut stmt = AlterDatabaseStatement::new();
		stmt.name("mydb")
			.add_region("us-east-1")
			.add_region("us-west-1")
			.set_primary_region("us-east-1");
		assert_eq!(stmt.operations.len(), 3);
	}

	#[rstest]
	fn test_alter_database_take() {
		let mut stmt = AlterDatabaseStatement::new();
		stmt.name("mydb").add_region("us-east-1");
		let taken = stmt.take();
		assert!(stmt.database_name.is_none());
		assert!(stmt.operations.is_empty());
		assert_eq!(taken.database_name.as_ref().unwrap().to_string(), "mydb");
		assert_eq!(taken.operations.len(), 1);
	}

	#[rstest]
	fn test_alter_database_configure_zone() {
		let mut stmt = AlterDatabaseStatement::new();
		let zone = ZoneConfig::new()
			.num_replicas(3)
			.add_constraint("+region=us-east-1");
		stmt.name("mydb").configure_zone(zone);
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			DatabaseOperation::ConfigureZone(config) => {
				assert_eq!(config.num_replicas, Some(3));
				assert_eq!(config.constraints.len(), 1);
			}
			_ => panic!("Expected ConfigureZone operation"),
		}
	}

	#[rstest]
	fn test_alter_database_configure_zone_multiple_options() {
		let mut stmt = AlterDatabaseStatement::new();
		let zone = ZoneConfig::new()
			.num_replicas(5)
			.add_constraint("+region=us-east-1")
			.add_constraint("+zone=a")
			.add_lease_preference("+region=us-east-1");
		stmt.name("mydb").configure_zone(zone);
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			DatabaseOperation::ConfigureZone(config) => {
				assert_eq!(config.num_replicas, Some(5));
				assert_eq!(config.constraints.len(), 2);
				assert_eq!(config.lease_preferences.len(), 1);
			}
			_ => panic!("Expected ConfigureZone operation"),
		}
	}
}
