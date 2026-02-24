//! Data Control Language (DCL) support for reinhardt-query
//!
//! This module provides type-safe builders for DCL statements including:
//! - GRANT and REVOKE privileges
//! - CREATE, DROP, and ALTER for roles and users
//! - Session management (SET ROLE, RESET ROLE, SET DEFAULT ROLE)
//! - User renaming (MySQL only)
//!
//! # Role and User Management
//!
//! ## Creating Roles and Users
//!
//! ```
//! use reinhardt_query::dcl::{CreateRoleStatement, RoleAttribute};
//! use reinhardt_query::backend::{PostgresQueryBuilder, QueryBuilder};
//!
//! // PostgreSQL: Create a role with attributes
//! let stmt = CreateRoleStatement::new()
//!     .role("app_user")
//!     .attribute(RoleAttribute::Login)
//!     .attribute(RoleAttribute::CreateDb);
//!
//! let builder = PostgresQueryBuilder::new();
//! let (sql, _values) = builder.build_create_role(&stmt);
//! // Generates: CREATE ROLE "app_user" WITH LOGIN CREATEDB
//! ```
//!
//! ## Session Management
//!
//! ```
//! use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
//! use reinhardt_query::backend::{PostgresQueryBuilder, QueryBuilder};
//!
//! // Set current session role
//! let stmt = SetRoleStatement::new()
//!     .role(RoleTarget::Named("admin".to_string()));
//!
//! let builder = PostgresQueryBuilder::new();
//! let (sql, _values) = builder.build_set_role(&stmt);
//! // Generates: SET ROLE "admin"
//! ```
//!
//! # Privileges and Permissions
//!
//! ```
//! use reinhardt_query::dcl::{Privilege, ObjectType, Grantee};
//!
//! // Create privilege
//! let privilege = Privilege::Select;
//! assert_eq!(privilege.as_sql(), "SELECT");
//!
//! // Create object type
//! let object_type = ObjectType::Table;
//! assert_eq!(object_type.as_sql(), "TABLE");
//!
//! // Create grantee
//! let grantee = Grantee::role("app_user");
//! ```
//!
//! # Security Considerations
//!
//! ## Password Handling
//!
//! **IMPORTANT**: Passwords are stored in plain text within statement structures
//! and are only escaped during SQL generation. For production use:
//!
//! - Never log or display statement structures containing passwords
//! - Use encrypted password variants when possible (`` `RoleAttribute::EncryptedPassword` ``)
//! - Ensure secure transmission of SQL statements to the database
//! - Consider using external authentication plugins (`` `UserOption::AuthPlugin` ``)
//! - Rotate passwords regularly using ALTER ROLE/USER statements
//!
//! ## SQL Injection Prevention
//!
//! This module automatically prevents SQL injection through:
//!
//! - **Identifier escaping**: All role names, user names, and identifiers are
//!   properly quoted using backend-specific quoting (e.g., `"role"` for PostgreSQL,
//!   `` `role` `` for MySQL)
//! - **Value parameterization**: All values (passwords, timestamps, etc.) are
//!   converted to parameterized placeholders (`$1`, `$2` for PostgreSQL; `?` for MySQL)
//! - **Type safety**: Rust's type system prevents invalid combinations of attributes
//!   and options
//!
//! However, you should still:
//!
//! - Validate user input before constructing statements
//! - Use the provided builder methods instead of constructing raw SQL
//! - Review generated SQL in development to ensure correctness
//!
//! # Database Support
//!
//! | Feature | PostgreSQL | MySQL | SQLite |
//! |---------|-----------|-------|--------|
//! | CREATE/DROP/ALTER ROLE | ✓ | ✓ | ✗ (panics) |
//! | CREATE/DROP/ALTER USER | ✓ | ✓ | ✗ (panics) |
//! | RENAME USER | ✗ (use ALTER ROLE) | ✓ | ✗ (panics) |
//! | SET ROLE | ✓ | ✓ | ✗ (panics) |
//! | RESET ROLE | ✓ | ✗ (panics) | ✗ (panics) |
//! | SET DEFAULT ROLE | ✗ (panics) | ✓ | ✗ (panics) |
//! | GRANT/REVOKE | ✓ | ✓ | ✗ (panics) |
//!
//! SQLite does not support DCL operations. Attempting to use DCL builders
//! with `` `SqliteQueryBuilder` `` will panic with a descriptive error message.

mod alter_role;
mod alter_user;
mod create_role;
mod create_user;
mod drop_role;
mod drop_user;
mod grant;
mod grant_role;
mod grantee;
mod object;
mod privilege;
mod rename_user;
mod reset_role;
mod revoke;
mod revoke_role;
mod role_attributes;
mod role_specification;
mod set_default_role;
mod set_role;
mod user_options;

/// Validate that a name is not empty or whitespace only.
pub(crate) fn validate_name(name: &str, field_description: &str) -> Result<(), String> {
	if name.trim().is_empty() {
		return Err(format!(
			"{} cannot be empty or whitespace only",
			field_description
		));
	}
	Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod create_role_tests;

#[cfg(test)]
mod drop_role_tests;

#[cfg(test)]
mod alter_role_tests;

#[cfg(test)]
mod create_user_tests;

#[cfg(test)]
mod drop_user_tests;

#[cfg(test)]
mod alter_user_tests;

#[cfg(test)]
mod rename_user_tests;

#[cfg(test)]
mod set_role_tests;

#[cfg(test)]
mod reset_role_tests;

#[cfg(test)]
mod set_default_role_tests;

pub use alter_role::AlterRoleStatement;
pub use alter_user::AlterUserStatement;
pub use create_role::CreateRoleStatement;
pub use create_user::CreateUserStatement;
pub use drop_role::DropRoleStatement;
pub use drop_user::DropUserStatement;
pub use grant::GrantStatement;
pub use grant_role::GrantRoleStatement;
pub use grantee::Grantee;
pub use object::ObjectType;
pub use privilege::Privilege;
pub use rename_user::RenameUserStatement;
pub use reset_role::ResetRoleStatement;
pub use revoke::RevokeStatement;
pub use revoke_role::RevokeRoleStatement;
pub use role_attributes::RoleAttribute;
pub use role_specification::{DropBehavior, RoleSpecification};
pub use set_default_role::{DefaultRoleSpec, SetDefaultRoleStatement};
pub use set_role::{RoleTarget, SetRoleStatement};
pub use user_options::UserOption;
