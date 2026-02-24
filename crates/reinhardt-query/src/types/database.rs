//! Database operation types
//!
//! This module provides types for database operations, including database definition
//! for CREATE DATABASE and operations for ALTER DATABASE.
//!
//! # Overview
//!
//! - [`DatabaseDef`]: Database definition for CREATE DATABASE
//! - [`DatabaseOperation`]: Operations for ALTER DATABASE (RENAME, OWNER, multi-region)
//!
//! # Standard Operations
//!
//! ## Rename Database
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! let mut stmt = Query::alter_database();
//! stmt.name("old_db").rename_to("new_db");
//! // SQL: ALTER DATABASE "old_db" RENAME TO "new_db"
//! ```
//!
//! ## Change Owner
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! let mut stmt = Query::alter_database();
//! stmt.name("mydb").owner_to("new_owner");
//! // SQL: ALTER DATABASE "mydb" OWNER TO "new_owner"
//! ```
//!
//! # CockroachDB Multi-Region Operations
//!
//! ## Add Region
//!
//! Add a region to distribute data globally:
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! let mut stmt = Query::alter_database();
//! stmt.name("mydb")
//!     .add_region("us-east-1")
//!     .add_region("us-west-1");
//! // SQL: ALTER DATABASE "mydb"
//! //      ADD REGION 'us-east-1',
//! //      ADD REGION 'us-west-1'
//! ```
//!
//! ## Drop Region
//!
//! Remove a region when scaling down:
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! let mut stmt = Query::alter_database();
//! stmt.name("mydb").drop_region("us-west-1");
//! // SQL: ALTER DATABASE "mydb" DROP REGION 'us-west-1'
//! ```
//!
//! ## Set Primary Region
//!
//! Designate a primary region for low-latency access:
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! let mut stmt = Query::alter_database();
//! stmt.name("mydb").set_primary_region("us-east-1");
//! // SQL: ALTER DATABASE "mydb" PRIMARY REGION 'us-east-1'
//! ```
//!
//! ## Configure Zone
//!
//! Control replica placement and replication:
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//! use reinhardt_query::types::ZoneConfig;
//!
//! let zone = ZoneConfig::new()
//!     .num_replicas(5)
//!     .add_constraint("+region=us-east-1");
//!
//! let mut stmt = Query::alter_database();
//! stmt.name("mydb").configure_zone(zone);
//! // SQL: ALTER DATABASE "mydb" CONFIGURE ZONE USING
//! //      num_replicas = 5,
//! //      constraints = '[+region=us-east-1]'
//! ```

use crate::types::{DynIden, IntoIden, ZoneConfig};

/// Database definition for CREATE DATABASE
///
/// This struct represents a database definition, including its name
/// and various database-specific options for PostgreSQL and MySQL.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::database::DatabaseDef;
///
/// // CREATE DATABASE mydb
/// let db = DatabaseDef::new("mydb");
///
/// // CREATE DATABASE IF NOT EXISTS mydb
/// let db = DatabaseDef::new("mydb")
///     .if_not_exists(true);
///
/// // CREATE DATABASE mydb OWNER alice
/// let db = DatabaseDef::new("mydb")
///     .owner("alice");
///
/// // CREATE DATABASE mydb TEMPLATE template0 ENCODING 'UTF8' (PostgreSQL)
/// let db = DatabaseDef::new("mydb")
///     .template("template0")
///     .encoding("UTF8");
///
/// // CREATE DATABASE mydb CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci (MySQL)
/// let db = DatabaseDef::new("mydb")
///     .character_set("utf8mb4")
///     .collate("utf8mb4_unicode_ci");
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DatabaseDef {
	pub(crate) name: DynIden,
	pub(crate) if_not_exists: bool,
	pub(crate) owner: Option<DynIden>,
	/// Template database (PostgreSQL)
	pub(crate) template: Option<DynIden>,
	/// Encoding (PostgreSQL)
	pub(crate) encoding: Option<String>,
	/// LC_COLLATE (PostgreSQL)
	pub(crate) lc_collate: Option<String>,
	/// LC_CTYPE (PostgreSQL)
	pub(crate) lc_ctype: Option<String>,
	/// Character set (MySQL)
	pub(crate) character_set: Option<String>,
	/// Collation (MySQL/PostgreSQL)
	pub(crate) collate: Option<String>,
}

impl DatabaseDef {
	/// Create a new database definition
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb");
	/// ```
	pub fn new<N: IntoIden>(name: N) -> Self {
		Self {
			name: name.into_iden(),
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

	/// Set IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb")
	///     .if_not_exists(true);
	/// ```
	pub fn if_not_exists(mut self, if_not_exists: bool) -> Self {
		self.if_not_exists = if_not_exists;
		self
	}

	/// Set OWNER
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb")
	///     .owner("alice");
	/// ```
	pub fn owner<O: IntoIden>(mut self, owner: O) -> Self {
		self.owner = Some(owner.into_iden());
		self
	}

	/// Set TEMPLATE database (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb")
	///     .template("template0");
	/// ```
	pub fn template<T: IntoIden>(mut self, template: T) -> Self {
		self.template = Some(template.into_iden());
		self
	}

	/// Set ENCODING (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb")
	///     .encoding("UTF8");
	/// ```
	pub fn encoding<S: Into<String>>(mut self, encoding: S) -> Self {
		self.encoding = Some(encoding.into());
		self
	}

	/// Set LC_COLLATE (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb")
	///     .lc_collate("en_US.UTF-8");
	/// ```
	pub fn lc_collate<S: Into<String>>(mut self, lc_collate: S) -> Self {
		self.lc_collate = Some(lc_collate.into());
		self
	}

	/// Set LC_CTYPE (PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb")
	///     .lc_ctype("en_US.UTF-8");
	/// ```
	pub fn lc_ctype<S: Into<String>>(mut self, lc_ctype: S) -> Self {
		self.lc_ctype = Some(lc_ctype.into());
		self
	}

	/// Set CHARACTER SET (MySQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb")
	///     .character_set("utf8mb4");
	/// ```
	pub fn character_set<S: Into<String>>(mut self, charset: S) -> Self {
		self.character_set = Some(charset.into());
		self
	}

	/// Set COLLATE (MySQL/PostgreSQL)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::database::DatabaseDef;
	///
	/// let db = DatabaseDef::new("mydb")
	///     .collate("utf8mb4_unicode_ci");
	/// ```
	pub fn collate<S: Into<String>>(mut self, collate: S) -> Self {
		self.collate = Some(collate.into());
		self
	}
}

/// Database operations for ALTER DATABASE statement
///
/// This enum represents the different operations that can be performed on a database.
/// CockroachDB-specific operations include multi-region configuration.
///
/// # Examples
///
/// Basic usage (typically used via [`AlterDatabaseStatement`](crate::query::AlterDatabaseStatement)):
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // Rename database
/// let mut stmt = Query::alter_database();
/// stmt.name("old_db").rename_to("new_db");
///
/// // Add region (CockroachDB)
/// let mut stmt = Query::alter_database();
/// stmt.name("mydb").add_region("us-east-1");
/// ```
#[derive(Debug, Clone)]
pub enum DatabaseOperation {
	/// RENAME TO new_name
	RenameDatabase(DynIden),
	/// OWNER TO new_owner
	OwnerTo(DynIden),
	/// ADD REGION region_name (CockroachDB-specific)
	AddRegion(String),
	/// DROP REGION region_name (CockroachDB-specific)
	DropRegion(String),
	/// PRIMARY REGION region_name (CockroachDB-specific)
	SetPrimaryRegion(String),
	/// CONFIGURE ZONE USING ... (CockroachDB-specific)
	ConfigureZone(ZoneConfig),
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::IntoIden;
	use rstest::*;

	// DatabaseDef tests
	#[rstest]
	fn test_database_def_basic() {
		let db = DatabaseDef::new("mydb");
		assert_eq!(db.name.to_string(), "mydb");
		assert!(!db.if_not_exists);
		assert!(db.owner.is_none());
		assert!(db.template.is_none());
		assert!(db.encoding.is_none());
		assert!(db.lc_collate.is_none());
		assert!(db.lc_ctype.is_none());
		assert!(db.character_set.is_none());
		assert!(db.collate.is_none());
	}

	#[rstest]
	fn test_database_def_if_not_exists() {
		let db = DatabaseDef::new("mydb").if_not_exists(true);
		assert_eq!(db.name.to_string(), "mydb");
		assert!(db.if_not_exists);
	}

	#[rstest]
	fn test_database_def_with_owner() {
		let db = DatabaseDef::new("mydb").owner("alice");
		assert_eq!(db.name.to_string(), "mydb");
		assert_eq!(db.owner.as_ref().unwrap().to_string(), "alice");
	}

	#[rstest]
	fn test_database_def_with_template() {
		let db = DatabaseDef::new("mydb").template("template0");
		assert_eq!(db.name.to_string(), "mydb");
		assert_eq!(db.template.as_ref().unwrap().to_string(), "template0");
	}

	#[rstest]
	fn test_database_def_with_encoding() {
		let db = DatabaseDef::new("mydb").encoding("UTF8");
		assert_eq!(db.name.to_string(), "mydb");
		assert_eq!(db.encoding.as_ref().unwrap(), "UTF8");
	}

	#[rstest]
	fn test_database_def_with_lc_collate() {
		let db = DatabaseDef::new("mydb").lc_collate("en_US.UTF-8");
		assert_eq!(db.name.to_string(), "mydb");
		assert_eq!(db.lc_collate.as_ref().unwrap(), "en_US.UTF-8");
	}

	#[rstest]
	fn test_database_def_with_lc_ctype() {
		let db = DatabaseDef::new("mydb").lc_ctype("en_US.UTF-8");
		assert_eq!(db.name.to_string(), "mydb");
		assert_eq!(db.lc_ctype.as_ref().unwrap(), "en_US.UTF-8");
	}

	#[rstest]
	fn test_database_def_with_character_set() {
		let db = DatabaseDef::new("mydb").character_set("utf8mb4");
		assert_eq!(db.name.to_string(), "mydb");
		assert_eq!(db.character_set.as_ref().unwrap(), "utf8mb4");
	}

	#[rstest]
	fn test_database_def_with_collate() {
		let db = DatabaseDef::new("mydb").collate("utf8mb4_unicode_ci");
		assert_eq!(db.name.to_string(), "mydb");
		assert_eq!(db.collate.as_ref().unwrap(), "utf8mb4_unicode_ci");
	}

	#[rstest]
	fn test_database_def_postgresql_full() {
		let db = DatabaseDef::new("mydb")
			.if_not_exists(true)
			.owner("alice")
			.template("template0")
			.encoding("UTF8")
			.lc_collate("en_US.UTF-8")
			.lc_ctype("en_US.UTF-8");
		assert_eq!(db.name.to_string(), "mydb");
		assert!(db.if_not_exists);
		assert_eq!(db.owner.as_ref().unwrap().to_string(), "alice");
		assert_eq!(db.template.as_ref().unwrap().to_string(), "template0");
		assert_eq!(db.encoding.as_ref().unwrap(), "UTF8");
		assert_eq!(db.lc_collate.as_ref().unwrap(), "en_US.UTF-8");
		assert_eq!(db.lc_ctype.as_ref().unwrap(), "en_US.UTF-8");
	}

	#[rstest]
	fn test_database_def_mysql_full() {
		let db = DatabaseDef::new("mydb")
			.if_not_exists(true)
			.character_set("utf8mb4")
			.collate("utf8mb4_unicode_ci");
		assert_eq!(db.name.to_string(), "mydb");
		assert!(db.if_not_exists);
		assert_eq!(db.character_set.as_ref().unwrap(), "utf8mb4");
		assert_eq!(db.collate.as_ref().unwrap(), "utf8mb4_unicode_ci");
	}

	// DatabaseOperation tests
	#[rstest]
	fn test_rename_database_operation() {
		let op = DatabaseOperation::RenameDatabase("new_db".into_iden());
		assert!(matches!(op, DatabaseOperation::RenameDatabase(_)));
	}

	#[rstest]
	fn test_owner_to_operation() {
		let op = DatabaseOperation::OwnerTo("new_owner".into_iden());
		assert!(matches!(op, DatabaseOperation::OwnerTo(_)));
	}

	#[rstest]
	fn test_add_region_operation() {
		let op = DatabaseOperation::AddRegion("us-east-1".to_string());
		match op {
			DatabaseOperation::AddRegion(region) => {
				assert_eq!(region, "us-east-1");
			}
			_ => panic!("Expected AddRegion operation"),
		}
	}

	#[rstest]
	fn test_drop_region_operation() {
		let op = DatabaseOperation::DropRegion("us-west-1".to_string());
		match op {
			DatabaseOperation::DropRegion(region) => {
				assert_eq!(region, "us-west-1");
			}
			_ => panic!("Expected DropRegion operation"),
		}
	}

	#[rstest]
	fn test_set_primary_region_operation() {
		let op = DatabaseOperation::SetPrimaryRegion("us-east-1".to_string());
		match op {
			DatabaseOperation::SetPrimaryRegion(region) => {
				assert_eq!(region, "us-east-1");
			}
			_ => panic!("Expected SetPrimaryRegion operation"),
		}
	}

	#[rstest]
	fn test_configure_zone_operation() {
		let zone = ZoneConfig::new()
			.num_replicas(3)
			.add_constraint("+region=us-east-1");
		let op = DatabaseOperation::ConfigureZone(zone);
		match op {
			DatabaseOperation::ConfigureZone(config) => {
				assert_eq!(config.num_replicas, Some(3));
				assert_eq!(config.constraints.len(), 1);
			}
			_ => panic!("Expected ConfigureZone operation"),
		}
	}
}
