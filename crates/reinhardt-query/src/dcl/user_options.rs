//! MySQL user/role option specifications
//!
//! This module provides type-safe representations of MySQL user and role options
//! used in CREATE ROLE, ALTER ROLE, CREATE USER, and ALTER USER statements.
//!
//! # Examples
//!
//! ```
//! use reinhardt_query::dcl::UserOption;
//!
//! // Set user password
//! let pass_opt = UserOption::Password("secret".to_string());
//!
//! // Lock user account
//! let lock_opt = UserOption::AccountLock;
//!
//! // Set password expiration
//! let expire_opt = UserOption::PasswordExpireInterval(90);
//! ```

/// MySQL user/role option specifications
///
/// These options control authentication, account locking, password policies,
/// and metadata for MySQL users and roles.
///
/// # Authentication Options
///
/// - `` `Password` `` - Set user password (IDENTIFIED BY)
/// - `` `AuthPlugin` `` - Use authentication plugin (IDENTIFIED WITH)
///
/// # Account Locking Options
///
/// - `` `AccountLock` `` - Lock the account (prevent login)
/// - `` `AccountUnlock` `` - Unlock the account
///
/// # Password Expiration Options
///
/// - `` `PasswordExpire` `` - Expire password immediately
/// - `` `PasswordExpireDefault` `` - Use default expiration policy
/// - `` `PasswordExpireNever` `` - Password never expires
/// - `` `PasswordExpireInterval` `` - Expire after N days
///
/// # Password History Options
///
/// - `` `PasswordHistory` `` - Prevent reuse of last N passwords
/// - `` `PasswordHistoryDefault` `` - Use default history policy
///
/// # Password Reuse Options
///
/// - `` `PasswordReuseInterval` `` - Allow reuse after N days
/// - `` `PasswordReuseIntervalDefault` `` - Use default reuse policy
///
/// # Password Requirement Options
///
/// - `` `PasswordRequireCurrent` `` - Require current password to change
/// - `` `PasswordRequireCurrentOptional` `` - Current password optional
/// - `` `PasswordRequireCurrentDefault` `` - Use default requirement policy
///
/// # Failed Login Handling Options
///
/// - `` `FailedLoginAttempts` `` - Lock after N failed attempts
/// - `` `PasswordLockTime` `` - Lock for N days after failed attempts
/// - `` `PasswordLockTimeUnbounded` `` - Lock indefinitely
///
/// # Metadata Options
///
/// - `` `Comment` `` - User comment/description
/// - `` `Attribute` `` - User attribute (JSON format)
#[derive(Debug, Clone, PartialEq)]
pub enum UserOption {
	/// IDENTIFIED BY - set user password
	Password(String),

	/// IDENTIFIED WITH - use authentication plugin
	AuthPlugin {
		/// Plugin name
		plugin: String,
		/// BY clause (authentication string)
		by: Option<String>,
		/// AS clause (hashed authentication string)
		as_: Option<String>,
	},

	/// ACCOUNT LOCK - prevent user from logging in
	AccountLock,
	/// ACCOUNT UNLOCK - allow user to log in
	AccountUnlock,

	/// PASSWORD EXPIRE - expire password immediately
	PasswordExpire,
	/// PASSWORD EXPIRE DEFAULT - use default expiration policy
	PasswordExpireDefault,
	/// PASSWORD EXPIRE NEVER - password never expires
	PasswordExpireNever,
	/// PASSWORD EXPIRE INTERVAL N DAY - expire after N days
	PasswordExpireInterval(u32),

	/// PASSWORD HISTORY N - prevent reuse of last N passwords
	PasswordHistory(u32),
	/// PASSWORD HISTORY DEFAULT - use default history policy
	PasswordHistoryDefault,

	/// PASSWORD REUSE INTERVAL N DAY - allow password reuse after N days
	PasswordReuseInterval(u32),
	/// PASSWORD REUSE INTERVAL DEFAULT - use default reuse policy
	PasswordReuseIntervalDefault,

	/// PASSWORD REQUIRE CURRENT - require current password to change password
	PasswordRequireCurrent,
	/// PASSWORD REQUIRE CURRENT OPTIONAL - current password optional
	PasswordRequireCurrentOptional,
	/// PASSWORD REQUIRE CURRENT DEFAULT - use default requirement policy
	PasswordRequireCurrentDefault,

	/// FAILED_LOGIN_ATTEMPTS N - lock account after N failed login attempts
	FailedLoginAttempts(u32),
	/// PASSWORD_LOCK_TIME N - lock account for N days after failed attempts
	PasswordLockTime(u32),
	/// PASSWORD_LOCK_TIME UNBOUNDED - lock account indefinitely
	PasswordLockTimeUnbounded,

	/// COMMENT - user comment/description
	Comment(String),
	/// ATTRIBUTE - user attribute in JSON format
	Attribute(String),
}
