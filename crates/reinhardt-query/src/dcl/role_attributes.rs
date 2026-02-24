//! PostgreSQL role attribute specifications
//!
//! This module provides type-safe representations of PostgreSQL role attributes
//! used in CREATE ROLE, ALTER ROLE, and CREATE USER statements.
//!
//! # Examples
//!
//! ```
//! use reinhardt_query::dcl::RoleAttribute;
//!
//! // Create role with SUPERUSER privilege
//! let attr = RoleAttribute::SuperUser;
//!
//! // Create role with LOGIN capability
//! let login_attr = RoleAttribute::Login;
//!
//! // Create role with connection limit
//! let conn_limit = RoleAttribute::ConnectionLimit(10);
//! ```

/// PostgreSQL role attribute specifications
///
/// These attributes control various privileges and settings for database roles.
/// They are used in CREATE ROLE, ALTER ROLE, and CREATE USER statements.
///
/// # Privilege Attributes
///
/// - `` `SuperUser` ``/`` `NoSuperUser` `` - Superuser privilege
/// - `` `CreateDb` ``/`` `NoCreateDb` `` - Database creation privilege
/// - `` `CreateRole` ``/`` `NoCreateRole` `` - Role creation privilege
/// - `` `Inherit` ``/`` `NoInherit` `` - Privilege inheritance
/// - `` `Login` ``/`` `NoLogin` `` - Login capability
/// - `` `Replication` ``/`` `NoReplication` `` - Replication privilege
/// - `` `BypassRls` ``/`` `NoBypassRls` `` - Row-level security bypass
///
/// # Configuration Attributes
///
/// - `` `ConnectionLimit` `` - Maximum concurrent connections (-1 = unlimited)
/// - `` `Password` `` - Set role password (automatically encrypted)
/// - `` `EncryptedPassword` `` - Set pre-encrypted password
/// - `` `UnencryptedPassword` `` - Set unencrypted password (not recommended)
/// - `` `ValidUntil` `` - Password expiration timestamp
///
/// # Role Membership Attributes
///
/// - `` `InRole` `` - Add role to specified roles
/// - `` `Role` `` - Grant specified roles to this role
/// - `` `Admin` `` - Grant specified roles with ADMIN OPTION
#[derive(Debug, Clone, PartialEq)]
pub enum RoleAttribute {
	/// SUPERUSER privilege - can override all access restrictions
	SuperUser,
	/// NOSUPERUSER - explicitly deny superuser privilege
	NoSuperUser,

	/// CREATEDB privilege - can create databases
	CreateDb,
	/// NOCREATEDB - cannot create databases
	NoCreateDb,

	/// CREATEROLE privilege - can create roles
	CreateRole,
	/// NOCREATEROLE - cannot create roles
	NoCreateRole,

	/// INHERIT - automatically inherit privileges of roles it is a member of
	Inherit,
	/// NOINHERIT - do not automatically inherit privileges
	NoInherit,

	/// LOGIN - role can log in (required for users)
	Login,
	/// NOLOGIN - role cannot log in (typical for group roles)
	NoLogin,

	/// REPLICATION - role can initiate streaming replication
	Replication,
	/// NOREPLICATION - role cannot initiate replication
	NoReplication,

	/// BYPASSRLS - role bypasses row-level security policies
	BypassRls,
	/// NOBYPASSRLS - role is subject to row-level security
	NoBypassRls,

	/// CONNECTION LIMIT - maximum concurrent connections (-1 = unlimited)
	ConnectionLimit(i32),

	/// PASSWORD - set role password (will be encrypted by PostgreSQL)
	Password(String),
	/// ENCRYPTED PASSWORD - set pre-encrypted password
	EncryptedPassword(String),
	/// UNENCRYPTED PASSWORD - set unencrypted password (deprecated, not recommended)
	UnencryptedPassword(String),

	/// VALID UNTIL - password expiration timestamp (ISO 8601 format recommended)
	ValidUntil(String),

	/// IN ROLE - add this role to the specified roles
	InRole(Vec<String>),
	/// ROLE - grant the specified roles to this role
	Role(Vec<String>),
	/// ADMIN - grant the specified roles with ADMIN OPTION
	Admin(Vec<String>),
}
