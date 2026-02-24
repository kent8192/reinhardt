//! Grantee types for DCL statements

/// Grantee types for GRANT and REVOKE statements
///
/// This enum represents the various types of privilege recipients
/// in SQL databases.
///
/// # Examples
///
/// ```
/// use reinhardt_query::dcl::Grantee;
///
/// let grantee = Grantee::role("app_user");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Grantee {
	/// PostgreSQL: role_name | MySQL: simple user
	Role(String),
	/// MySQL: 'user'@'host'
	User(String, String),
	/// PostgreSQL: PUBLIC keyword
	Public,
	/// PostgreSQL: CURRENT_ROLE
	CurrentRole,
	/// PostgreSQL: CURRENT_USER (also supported in MySQL)
	CurrentUser,
	/// PostgreSQL: SESSION_USER
	SessionUser,
}

impl Grantee {
	/// Create a role grantee
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::Grantee;
	///
	/// let grantee = Grantee::role("app_user");
	/// ```
	pub fn role(name: impl Into<String>) -> Self {
		Grantee::Role(name.into())
	}

	/// Create a MySQL user@host grantee
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::Grantee;
	///
	/// let grantee = Grantee::user("app_user", "localhost");
	/// ```
	pub fn user(username: impl Into<String>, hostname: impl Into<String>) -> Self {
		Grantee::User(username.into(), hostname.into())
	}

	/// Checks if this grantee is PostgreSQL-specific
	///
	/// Returns `true` for grantees that are only available in PostgreSQL.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::Grantee;
	///
	/// assert!(!Grantee::role("app_user").is_postgres_only());
	/// assert!(Grantee::Public.is_postgres_only());
	/// ```
	pub fn is_postgres_only(&self) -> bool {
		matches!(
			self,
			Grantee::Public | Grantee::CurrentRole | Grantee::SessionUser
		)
	}

	/// Checks if this grantee is MySQL-specific
	///
	/// Returns `true` for grantees that are only available in MySQL.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::Grantee;
	///
	/// assert!(Grantee::user("app_user", "localhost").is_mysql_specific());
	/// assert!(!Grantee::role("app_user").is_mysql_specific());
	/// ```
	pub fn is_mysql_specific(&self) -> bool {
		matches!(self, Grantee::User(_, _))
	}
}
