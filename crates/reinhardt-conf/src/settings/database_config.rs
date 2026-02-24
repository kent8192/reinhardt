//! Database configuration for settings
//!
//! This module provides the `DatabaseConfig` struct and its methods for
//! configuring database connections in Reinhardt settings files.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::settings::secret_types::SecretString;

/// Characters that must be percent-encoded in URL userinfo components.
/// RFC 3986 Section 3.2.1 defines userinfo = *( unreserved / pct-encoded / sub-delims / ":" )
/// We encode everything except unreserved characters to be safe.
const USERINFO_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
	.remove(b'-')
	.remove(b'.')
	.remove(b'_')
	.remove(b'~');

/// Database configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
	/// Database engine/backend
	pub engine: String,

	/// Database name or path
	pub name: String,

	/// Database user (if applicable)
	pub user: Option<String>,

	/// Database password (if applicable) - stored as `SecretString` to prevent accidental exposure
	pub password: Option<SecretString>,

	/// Database host (if applicable)
	pub host: Option<String>,

	/// Database port (if applicable)
	pub port: Option<u16>,

	/// Additional options
	pub options: HashMap<String, String>,
}

impl fmt::Debug for DatabaseConfig {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("DatabaseConfig")
			.field("engine", &self.engine)
			.field("name", &self.name)
			.field("user", &self.user)
			.field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
			.field("host", &self.host)
			.field("port", &self.port)
			.field("options", &self.options)
			.finish()
	}
}

impl DatabaseConfig {
	/// Create a SQLite database configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::DatabaseConfig;
	///
	/// let db = DatabaseConfig::sqlite("myapp.db");
	///
	/// assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
	/// assert_eq!(db.name, "myapp.db");
	/// assert!(db.user.is_none());
	/// assert!(db.password.is_none());
	/// ```
	pub fn sqlite(name: impl Into<String>) -> Self {
		Self {
			engine: "reinhardt.db.backends.sqlite3".to_string(),
			name: name.into(),
			user: None,
			password: None,
			host: None,
			port: None,
			options: HashMap::new(),
		}
	}
	/// Create a PostgreSQL database configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::DatabaseConfig;
	///
	/// let db = DatabaseConfig::postgresql("mydb", "admin", "password123", "localhost", 5432);
	///
	/// assert_eq!(db.engine, "reinhardt.db.backends.postgresql");
	/// assert_eq!(db.name, "mydb");
	/// assert_eq!(db.user, Some("admin".to_string()));
	/// assert_eq!(db.password.as_ref().map(|p| p.expose_secret()), Some("password123"));
	/// assert_eq!(db.host, Some("localhost".to_string()));
	/// assert_eq!(db.port, Some(5432));
	/// ```
	pub fn postgresql(
		name: impl Into<String>,
		user: impl Into<String>,
		password: impl Into<String>,
		host: impl Into<String>,
		port: u16,
	) -> Self {
		Self {
			engine: "reinhardt.db.backends.postgresql".to_string(),
			name: name.into(),
			user: Some(user.into()),
			password: Some(SecretString::new(password.into())),
			host: Some(host.into()),
			port: Some(port),
			options: HashMap::new(),
		}
	}
	/// Create a MySQL database configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::DatabaseConfig;
	///
	/// let db = DatabaseConfig::mysql("mydb", "root", "password123", "localhost", 3306);
	///
	/// assert_eq!(db.engine, "reinhardt.db.backends.mysql");
	/// assert_eq!(db.name, "mydb");
	/// assert_eq!(db.user, Some("root".to_string()));
	/// assert_eq!(db.password.as_ref().map(|p| p.expose_secret()), Some("password123"));
	/// assert_eq!(db.host, Some("localhost".to_string()));
	/// assert_eq!(db.port, Some(3306));
	/// ```
	pub fn mysql(
		name: impl Into<String>,
		user: impl Into<String>,
		password: impl Into<String>,
		host: impl Into<String>,
		port: u16,
	) -> Self {
		Self {
			engine: "reinhardt.db.backends.mysql".to_string(),
			name: name.into(),
			user: Some(user.into()),
			password: Some(SecretString::new(password.into())),
			host: Some(host.into()),
			port: Some(port),
			options: HashMap::new(),
		}
	}

	/// Convert `DatabaseConfig` to DATABASE_URL string
	///
	/// Credentials and query parameter values are percent-encoded per RFC 3986
	/// to prevent URL injection and parsing errors from special characters.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::DatabaseConfig;
	///
	/// let db = DatabaseConfig::sqlite("db.sqlite3");
	/// assert_eq!(db.to_url(), "sqlite:db.sqlite3");
	///
	/// let db = DatabaseConfig::postgresql("mydb", "user", "p@ss:word", "localhost", 5432);
	/// assert_eq!(db.to_url(), "postgresql://user:p%40ss%3Aword@localhost:5432/mydb");
	/// ```
	pub fn to_url(&self) -> String {
		// Determine the database scheme from engine
		// Handle both short names (e.g., "sqlite") and full backend paths (e.g., "reinhardt.db.backends.sqlite3")
		let scheme = if self.engine == "sqlite" || self.engine.contains("sqlite") {
			"sqlite"
		} else if self.engine == "postgresql"
			|| self.engine == "postgres"
			|| self.engine.contains("postgresql")
			|| self.engine.contains("postgres")
		{
			"postgresql"
		} else if self.engine == "mysql" || self.engine.contains("mysql") {
			"mysql"
		} else {
			// Default to sqlite for unknown engines
			"sqlite"
		};

		match scheme {
			"sqlite" => {
				if self.name == ":memory:" {
					"sqlite::memory:".to_string()
				} else {
					// Use sqlite: format for relative paths (will be converted to absolute in connect_database)
					// sqlite:/// is for absolute paths
					use std::path::Path;
					let path = Path::new(&self.name);
					if path.is_absolute() {
						// Absolute path: sqlite:///path/to/db.sqlite3
						format!("sqlite:///{}", self.name)
					} else {
						// Relative path: sqlite:db.sqlite3 (will be converted to absolute in connect_database)
						format!("sqlite:{}", self.name)
					}
				}
			}
			"postgresql" | "mysql" => {
				let mut url = format!("{}://", scheme);

				// Add user and password if available, percent-encoded per RFC 3986
				if let Some(user) = &self.user {
					let encoded_user = utf8_percent_encode(user, USERINFO_ENCODE_SET).to_string();
					url.push_str(&encoded_user);
					if let Some(password) = &self.password {
						url.push(':');
						let encoded_password =
							utf8_percent_encode(password.expose_secret(), USERINFO_ENCODE_SET)
								.to_string();
						url.push_str(&encoded_password);
					}
					url.push('@');
				}

				// Add host (default to localhost if not specified)
				let host = self.host.as_deref().unwrap_or("localhost");
				url.push_str(host);

				// Add port if available
				if let Some(port) = self.port {
					url.push(':');
					url.push_str(&port.to_string());
				}

				// Add database name
				url.push('/');
				url.push_str(&self.name);

				// Add query parameters if any, with percent-encoded values
				if !self.options.is_empty() {
					let mut query_parts = Vec::new();
					for (key, value) in &self.options {
						let encoded_key = utf8_percent_encode(key, USERINFO_ENCODE_SET).to_string();
						let encoded_value =
							utf8_percent_encode(value, USERINFO_ENCODE_SET).to_string();
						query_parts.push(format!("{}={}", encoded_key, encoded_value));
					}
					url.push('?');
					url.push_str(&query_parts.join("&"));
				}

				url
			}
			_ => format!("sqlite://{}", self.name),
		}
	}
}

impl fmt::Display for DatabaseConfig {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// Display a sanitized representation that never exposes credentials
		let scheme = if self.engine.contains("sqlite") {
			"sqlite"
		} else if self.engine.contains("postgresql") || self.engine.contains("postgres") {
			"postgresql"
		} else if self.engine.contains("mysql") {
			"mysql"
		} else {
			"unknown"
		};

		match scheme {
			"sqlite" => write!(f, "sqlite:{}", self.name),
			_ => {
				write!(f, "{}://", scheme)?;
				if self.user.is_some() || self.password.is_some() {
					write!(f, "***@")?;
				}
				if let Some(host) = &self.host {
					write!(f, "{}", host)?;
				}
				if let Some(port) = self.port {
					write!(f, ":{}", port)?;
				}
				write!(f, "/{}", self.name)
			}
		}
	}
}

impl Default for DatabaseConfig {
	fn default() -> Self {
		Self::sqlite("db.sqlite3".to_string())
	}
}

/// Recognized database URL schemes for connection validation.
pub(crate) const VALID_DATABASE_SCHEMES: &[&str] = &[
	"postgres://",
	"postgresql://",
	"sqlite://",
	"sqlite:",
	"mysql://",
	"mariadb://",
];

/// Validate that a database URL starts with a recognized scheme.
///
/// Returns `Ok(())` if the URL starts with one of the supported schemes,
/// or `Err` with a descriptive message listing the accepted schemes.
pub(crate) fn validate_database_url_scheme(url: &str) -> Result<(), String> {
	if VALID_DATABASE_SCHEMES.iter().any(|s| url.starts_with(s)) {
		Ok(())
	} else {
		Err(format!(
			"Invalid database URL: unrecognized scheme. Expected one of: {}",
			VALID_DATABASE_SCHEMES.join(", ")
		))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_settings_db_config_sqlite() {
		// Arrange
		let db = DatabaseConfig::sqlite("test.db");

		// Assert
		assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
		assert_eq!(db.name, "test.db");
		assert!(db.user.is_none());
		assert!(db.password.is_none());
	}

	#[rstest]
	fn test_settings_db_config_postgresql() {
		// Arrange
		let db = DatabaseConfig::postgresql("testdb", "user", "pass", "localhost", 5432);

		// Assert
		assert_eq!(db.engine, "reinhardt.db.backends.postgresql");
		assert_eq!(db.name, "testdb");
		assert_eq!(db.user, Some("user".to_string()));
		assert_eq!(
			db.password.as_ref().map(|p| p.expose_secret()),
			Some("pass")
		);
		assert_eq!(db.port, Some(5432));
	}

	#[rstest]
	fn test_debug_output_redacts_password() {
		// Arrange
		let db = DatabaseConfig::postgresql("testdb", "user", "s3cr3t!", "localhost", 5432);

		// Act
		let debug_output = format!("{:?}", db);

		// Assert
		assert!(!debug_output.contains("s3cr3t!"));
		assert!(debug_output.contains("[REDACTED]"));
	}

	#[rstest]
	fn test_debug_output_without_password() {
		// Arrange
		let db = DatabaseConfig::sqlite("test.db");

		// Act
		let debug_output = format!("{:?}", db);

		// Assert
		assert!(debug_output.contains("None"));
		assert!(debug_output.contains("DatabaseConfig"));
	}

	#[rstest]
	fn test_to_url_encodes_special_chars_in_username() {
		// Arrange
		let mut db = DatabaseConfig::postgresql("mydb", "user@domain", "pass", "localhost", 5432);
		db.user = Some("user@domain".to_string());

		// Act
		let url = db.to_url();

		// Assert
		assert!(url.contains("user%40domain"));
		assert!(!url.contains("user@domain:"));
	}

	#[rstest]
	fn test_to_url_encodes_special_chars_in_password() {
		// Arrange
		let db = DatabaseConfig::postgresql("mydb", "user", "p@ss:w/rd#", "localhost", 5432);

		// Act
		let url = db.to_url();

		// Assert
		assert!(url.contains("p%40ss%3Aw%2Frd%23"));
		assert!(!url.contains("p@ss:w/rd#"));
	}

	#[rstest]
	fn test_to_url_prevents_host_injection() {
		// Arrange - malicious username that attempts to redirect to a different host
		let db = DatabaseConfig::postgresql(
			"mydb",
			"admin@evil.com:9999/fake",
			"pass",
			"localhost",
			5432,
		);

		// Act
		let url = db.to_url();

		// Assert - the @ in username should be encoded, preventing host injection
		assert!(url.contains("admin%40evil.com%3A9999%2Ffake"));
		assert!(url.contains("@localhost:5432"));
	}

	#[rstest]
	fn test_to_url_encodes_query_parameter_values() {
		// Arrange
		let mut db = DatabaseConfig::postgresql("mydb", "user", "pass", "localhost", 5432);
		db.options
			.insert("sslmode".to_string(), "require&inject=true".to_string());

		// Act
		let url = db.to_url();

		// Assert
		assert!(url.contains("require%26inject%3Dtrue"));
		assert!(!url.contains("require&inject=true"));
	}

	#[rstest]
	fn test_to_url_simple_credentials() {
		// Arrange
		let db = DatabaseConfig::postgresql("mydb", "user", "pass", "localhost", 5432);

		// Act
		let url = db.to_url();

		// Assert
		assert_eq!(url, "postgresql://user:pass@localhost:5432/mydb");
	}

	#[rstest]
	fn test_display_output_masks_credentials() {
		// Arrange
		let db = DatabaseConfig::postgresql("mydb", "admin", "s3cr3t!", "db.example.com", 5432);

		// Act
		let display_output = format!("{}", db);

		// Assert
		assert!(!display_output.contains("admin"));
		assert!(!display_output.contains("s3cr3t!"));
		assert!(display_output.contains("***@"));
		assert!(display_output.contains("db.example.com"));
		assert!(display_output.contains("mydb"));
	}

	#[rstest]
	fn test_display_output_sqlite() {
		// Arrange
		let db = DatabaseConfig::sqlite("app.db");

		// Act
		let display_output = format!("{}", db);

		// Assert
		assert_eq!(display_output, "sqlite:app.db");
	}

	#[rstest]
	fn test_password_stored_as_secret_string() {
		// Arrange
		let db = DatabaseConfig::postgresql("mydb", "user", "my-secret-pw", "localhost", 5432);

		// Act
		let password = db.password.as_ref().unwrap();

		// Assert
		assert_eq!(password.expose_secret(), "my-secret-pw");
		// Display should not reveal the password
		assert_eq!(format!("{}", password), "[REDACTED]");
	}

	#[rstest]
	#[case("postgres://localhost/db")]
	#[case("postgresql://user:pass@localhost:5432/db")]
	#[case("sqlite::memory:")]
	#[case("sqlite:///path/to/db")]
	#[case("mysql://root@localhost/db")]
	#[case("mariadb://root@localhost/db")]
	fn test_valid_database_url_schemes(#[case] url: &str) {
		// Act / Assert
		assert!(validate_database_url_scheme(url).is_ok());
	}

	#[rstest]
	#[case("http://localhost/db")]
	#[case("ftp://localhost/db")]
	#[case("redis://localhost")]
	#[case("")]
	#[case("not-a-url")]
	fn test_invalid_database_url_schemes(#[case] url: &str) {
		// Act
		let result = validate_database_url_scheme(url);

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().contains("Invalid database URL"));
	}
}
