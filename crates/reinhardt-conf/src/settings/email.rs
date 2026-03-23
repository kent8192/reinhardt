//! Email settings fragment
//!
//! Provides composable email configuration as a [`SettingsFragment`].

use super::fragment::{HasSettings, SettingsFragment};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Email configuration fragment.
///
/// Controls email backend, SMTP connection, and notification settings.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailSettings {
	/// Email backend type (e.g., `"smtp"`, `"console"`, `"file"`, `"memory"`).
	pub backend: String,
	/// SMTP server hostname.
	pub host: String,
	/// SMTP server port number.
	pub port: u16,
	/// Optional SMTP authentication username.
	pub username: Option<String>,
	/// Optional SMTP authentication password.
	pub password: Option<String>,
	/// Whether to use STARTTLS for the SMTP connection.
	pub use_tls: bool,
	/// Whether to use direct TLS/SSL for the SMTP connection.
	pub use_ssl: bool,
	/// Default sender email address for outgoing emails.
	pub from_email: String,

	/// List of (name, email) tuples for site administrators.
	/// Used by mail_admins() helper.
	#[serde(default)]
	pub admins: Vec<(String, String)>,

	/// List of (name, email) tuples for site managers.
	/// Used by mail_managers() helper.
	#[serde(default)]
	pub managers: Vec<(String, String)>,

	/// Email address for server error notifications.
	#[serde(default = "default_server_email")]
	pub server_email: String,

	/// Prefix for email subjects (e.g., `"[Reinhardt]"`).
	#[serde(default)]
	pub subject_prefix: String,

	/// Connection timeout in seconds.
	pub timeout: Option<u64>,

	/// Path to SSL certificate file.
	pub ssl_certfile: Option<PathBuf>,

	/// Path to SSL key file.
	pub ssl_keyfile: Option<PathBuf>,

	/// Directory path for file-based email backend.
	/// Required when backend is `"file"`.
	#[serde(default)]
	pub file_path: Option<PathBuf>,
}

fn default_server_email() -> String {
	"root@localhost".to_string()
}

impl Default for EmailSettings {
	fn default() -> Self {
		Self {
			backend: "console".to_string(),
			host: "localhost".to_string(),
			port: 25,
			username: None,
			password: None,
			use_tls: false,
			use_ssl: false,
			from_email: "noreply@example.com".to_string(),
			admins: Vec::new(),
			managers: Vec::new(),
			server_email: default_server_email(),
			subject_prefix: String::new(),
			timeout: None,
			ssl_certfile: None,
			ssl_keyfile: None,
			file_path: None,
		}
	}
}

impl SettingsFragment for EmailSettings {
	type Accessor = dyn HasEmailSettings;

	fn section() -> &'static str {
		"email"
	}
}

/// Trait for settings containers that include email configuration.
pub trait HasEmailSettings {
	/// Returns a reference to the email settings.
	fn email(&self) -> &EmailSettings;
}

impl<T: HasSettings<EmailSettings>> HasEmailSettings for T {
	fn email(&self) -> &EmailSettings {
		self.get_settings()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_email_section_name() {
		// Arrange / Act
		let section = EmailSettings::section();

		// Assert
		assert_eq!(section, "email");
	}

	#[rstest]
	fn test_email_default_values() {
		// Arrange / Act
		let settings = EmailSettings::default();

		// Assert
		assert_eq!(settings.backend, "console");
		assert_eq!(settings.host, "localhost");
		assert_eq!(settings.port, 25);
		assert!(settings.username.is_none());
		assert!(settings.password.is_none());
		assert!(!settings.use_tls);
		assert!(!settings.use_ssl);
		assert_eq!(settings.from_email, "noreply@example.com");
		assert!(settings.admins.is_empty());
		assert!(settings.managers.is_empty());
		assert_eq!(settings.server_email, "root@localhost");
		assert!(settings.subject_prefix.is_empty());
		assert!(settings.timeout.is_none());
		assert!(settings.ssl_certfile.is_none());
		assert!(settings.ssl_keyfile.is_none());
		assert!(settings.file_path.is_none());
	}
}
