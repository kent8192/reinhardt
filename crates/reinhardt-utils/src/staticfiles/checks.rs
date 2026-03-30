//! System checks for static files configuration
//!
//! Validates static files configuration and provides warnings/errors
//! for common misconfigurations, similar to Django's check framework.

use super::StaticFilesConfig;

/// Severity level for a configuration check message.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckLevel {
	/// Detailed diagnostic information.
	Debug,
	/// General informational note about the configuration.
	Info,
	/// A potential misconfiguration that may cause issues.
	Warning,
	/// A configuration error that should be fixed.
	Error,
	/// A severe misconfiguration that prevents proper operation.
	Critical,
}

/// A diagnostic message produced by a configuration check.
#[derive(Debug, Clone)]
pub struct CheckMessage {
	/// The severity level of this message.
	pub level: CheckLevel,
	/// A unique identifier for this check (e.g., `"static.E001"`).
	pub id: String,
	/// A human-readable description of the issue.
	pub message: String,
	/// An optional suggestion for how to fix the issue.
	pub hint: Option<String>,
}

impl CheckMessage {
	/// Documentation for `error`
	///
	pub fn error(id: impl Into<String>, message: impl Into<String>) -> Self {
		Self {
			level: CheckLevel::Error,
			id: id.into(),
			message: message.into(),
			hint: None,
		}
	}
	/// Documentation for `warning`
	///
	pub fn warning(id: impl Into<String>, message: impl Into<String>) -> Self {
		Self {
			level: CheckLevel::Warning,
			id: id.into(),
			message: message.into(),
			hint: None,
		}
	}
	/// Add a hint to the check message
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_utils::staticfiles::checks::CheckMessage;
	/// let msg = CheckMessage::error("E001", "Error").with_hint("Try this");
	/// ```
	pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
		self.hint = Some(hint.into());
		self
	}
}

/// Run all static files configuration checks
///
/// Returns a list of check messages indicating any configuration issues.
///
/// # Example
///
/// ```rust
/// use reinhardt_utils::staticfiles::checks::check_static_files_config;
/// use reinhardt_utils::staticfiles::storage::StaticFilesConfig;
/// use std::path::PathBuf;
///
/// let config = StaticFilesConfig {
///     static_root: PathBuf::from("/var/www/static"),
///     static_url: "/static/".to_string(),
///     staticfiles_dirs: vec![PathBuf::from("/app/static")],
///     media_url: None,
/// };
///
/// let messages = check_static_files_config(&config);
/// for message in messages {
///     println!("[{}] {}", message.id, message.message);
/// }
/// ```
pub fn check_static_files_config(config: &StaticFilesConfig) -> Vec<CheckMessage> {
	let mut messages = Vec::new();

	messages.extend(check_static_root(config));
	messages.extend(check_static_url(config));
	messages.extend(check_staticfiles_dirs(config));
	messages.extend(check_media_url_conflict(config));

	messages
}

/// Check STATIC_ROOT configuration
fn check_static_root(config: &StaticFilesConfig) -> Vec<CheckMessage> {
	let mut messages = Vec::new();

	// E001: STATIC_ROOT is not set
	if config.static_root.as_os_str().is_empty() {
		messages.push(
			CheckMessage::error("static.E001", "STATIC_ROOT setting is not set").with_hint(
				"Set STATIC_ROOT to a directory path where static files will be collected",
			),
		);
	}

	// W001: STATIC_ROOT is in STATICFILES_DIRS
	for dir in &config.staticfiles_dirs {
		if dir == &config.static_root {
			messages.push(
				CheckMessage::error(
					"static.E002",
					format!(
						"STATIC_ROOT ({}) is in STATICFILES_DIRS",
						config.static_root.display()
					),
				)
				.with_hint("STATIC_ROOT should be a separate directory from source directories"),
			);
		}

		// Check if STATIC_ROOT is a subdirectory of any STATICFILES_DIRS
		if config.static_root.starts_with(dir) {
			messages.push(
				CheckMessage::warning(
					"static.W001",
					format!(
						"STATIC_ROOT ({}) is a subdirectory of STATICFILES_DIRS entry ({})",
						config.static_root.display(),
						dir.display()
					),
				)
				.with_hint("This may cause files to be collected recursively"),
			);
		}
	}

	messages
}

/// Check STATIC_URL configuration
fn check_static_url(config: &StaticFilesConfig) -> Vec<CheckMessage> {
	let mut messages = Vec::new();

	// E003: STATIC_URL is empty
	if config.static_url.is_empty() {
		messages.push(
			CheckMessage::error("static.E003", "STATIC_URL setting is empty")
				.with_hint("Set STATIC_URL to a URL path like '/static/'"),
		);
	}

	// W002: STATIC_URL doesn't start with /
	if !config.static_url.is_empty() && !config.static_url.starts_with('/') {
		messages.push(
			CheckMessage::warning(
				"static.W002",
				format!(
					"STATIC_URL ('{}') doesn't start with '/'",
					config.static_url
				),
			)
			.with_hint("STATIC_URL should start with '/' for local serving"),
		);
	}

	// W003: STATIC_URL doesn't end with /
	if !config.static_url.is_empty() && !config.static_url.ends_with('/') {
		messages.push(
			CheckMessage::warning(
				"static.W003",
				format!("STATIC_URL ('{}') doesn't end with '/'", config.static_url),
			)
			.with_hint("STATIC_URL should end with '/' to avoid path issues"),
		);
	}

	messages
}

/// Check STATICFILES_DIRS configuration
fn check_staticfiles_dirs(config: &StaticFilesConfig) -> Vec<CheckMessage> {
	let mut messages = Vec::new();

	// W004: Empty STATICFILES_DIRS
	if config.staticfiles_dirs.is_empty() {
		messages.push(
			CheckMessage::warning("static.W004", "STATICFILES_DIRS is empty")
				.with_hint("Add source directories containing static files"),
		);
	}

	// W005: Directory doesn't exist
	for dir in &config.staticfiles_dirs {
		if !dir.exists() {
			messages.push(
				CheckMessage::warning(
					"static.W005",
					format!("STATICFILES_DIRS entry does not exist: {}", dir.display()),
				)
				.with_hint("Create the directory or remove it from STATICFILES_DIRS"),
			);
		} else if !dir.is_dir() {
			messages.push(CheckMessage::error(
				"static.E004",
				format!(
					"STATICFILES_DIRS entry is not a directory: {}",
					dir.display()
				),
			));
		}
	}

	// W006: Duplicate entries
	for (i, dir1) in config.staticfiles_dirs.iter().enumerate() {
		for dir2 in config.staticfiles_dirs.iter().skip(i + 1) {
			if dir1 == dir2 {
				messages.push(
					CheckMessage::warning(
						"static.W006",
						format!(
							"STATICFILES_DIRS contains duplicate entry: {}",
							dir1.display()
						),
					)
					.with_hint("Remove duplicate directory entries"),
				);
			}
		}
	}

	messages
}

/// Check for conflicts with MEDIA_URL (if present)
fn check_media_url_conflict(config: &StaticFilesConfig) -> Vec<CheckMessage> {
	let mut messages = Vec::new();

	// Check if MEDIA_URL is configured
	if let Some(media_url) = &config.media_url {
		// E004: STATIC_URL and MEDIA_URL are the same
		if config.static_url == *media_url {
			messages.push(
				CheckMessage::error("static.E004", "STATIC_URL and MEDIA_URL cannot be the same")
					.with_hint(
						"Use different URL paths for static and media files (e.g., '/static/' and '/media/')",
					),
			);
		}

		// W007: MEDIA_URL is not empty but doesn't start with /
		if !media_url.is_empty() && !media_url.starts_with('/') {
			messages.push(
				CheckMessage::warning("static.W007", "MEDIA_URL should start with a slash")
					.with_hint(format!(
						"Change MEDIA_URL from '{}' to '/{}'",
						media_url, media_url
					)),
			);
		}

		// W008: MEDIA_URL is not empty but doesn't end with /
		if !media_url.is_empty() && !media_url.ends_with('/') {
			messages.push(
				CheckMessage::warning("static.W008", "MEDIA_URL should end with a slash")
					.with_hint(format!(
						"Change MEDIA_URL from '{}' to '{}'",
						media_url,
						if media_url.ends_with('/') {
							media_url.to_string()
						} else {
							format!("{}/", media_url)
						}
					)),
			);
		}

		// W009: MEDIA_URL is a prefix of STATIC_URL or vice versa
		if config.static_url.starts_with(media_url) || media_url.starts_with(&config.static_url) {
			messages.push(
				CheckMessage::warning(
					"static.W009",
					"MEDIA_URL should not be a prefix of STATIC_URL or vice versa",
				)
				.with_hint("Use distinct URL paths to avoid routing conflicts"),
			);
		}
	}

	messages
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;
	use tempfile::TempDir;

	#[test]
	fn test_check_static_root_not_set() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from(""),
			static_url: "/static/".to_string(),
			staticfiles_dirs: vec![],
			media_url: None,
		};

		let messages = check_static_root(&config);
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0].id, "static.E001");
		assert_eq!(messages[0].level, CheckLevel::Error);
	}

	#[test]
	fn test_check_static_root_in_staticfiles_dirs() {
		let root = PathBuf::from("/var/www/static");
		let config = StaticFilesConfig {
			static_root: root.clone(),
			static_url: "/static/".to_string(),
			staticfiles_dirs: vec![root],
			media_url: None,
		};

		let messages = check_static_root(&config);
		assert!(messages.iter().any(|m| m.id == "static.E002"));
	}

	#[test]
	fn test_check_static_root_subdirectory() {
		let temp_dir = TempDir::new().unwrap();
		let parent = temp_dir.path().to_path_buf();
		let child = parent.join("collected");

		let config = StaticFilesConfig {
			static_root: child,
			static_url: "/static/".to_string(),
			staticfiles_dirs: vec![parent],
			media_url: None,
		};

		let messages = check_static_root(&config);
		assert!(messages.iter().any(|m| m.id == "static.W001"));
	}

	#[test]
	fn test_check_static_url_empty() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: String::new(),
			staticfiles_dirs: vec![],
			media_url: None,
		};

		let messages = check_static_url(&config);
		assert!(messages.iter().any(|m| m.id == "static.E003"));
	}

	#[test]
	fn test_check_static_url_no_leading_slash() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "static/".to_string(),
			staticfiles_dirs: vec![],
			media_url: None,
		};

		let messages = check_static_url(&config);
		assert!(messages.iter().any(|m| m.id == "static.W002"));
	}

	#[test]
	fn test_check_static_url_no_trailing_slash() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static".to_string(),
			staticfiles_dirs: vec![],
			media_url: None,
		};

		let messages = check_static_url(&config);
		assert!(messages.iter().any(|m| m.id == "static.W003"));
	}

	#[test]
	fn test_check_staticfiles_dirs_empty() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: vec![],
			media_url: None,
		};

		let messages = check_staticfiles_dirs(&config);
		assert!(messages.iter().any(|m| m.id == "static.W004"));
	}

	#[test]
	fn test_check_staticfiles_dirs_not_exist() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: vec![PathBuf::from("/nonexistent/path")],
			media_url: None,
		};

		let messages = check_staticfiles_dirs(&config);
		assert!(messages.iter().any(|m| m.id == "static.W005"));
	}

	#[test]
	fn test_check_staticfiles_dirs_duplicate() {
		let dir = PathBuf::from("/app/static");
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: vec![dir.clone(), dir],
			media_url: None,
		};

		let messages = check_staticfiles_dirs(&config);
		assert!(messages.iter().any(|m| m.id == "static.W006"));
	}

	#[test]
	fn test_valid_configuration() {
		let temp_dir = TempDir::new().unwrap();
		let source_dir = temp_dir.path().join("source");
		std::fs::create_dir(&source_dir).unwrap();

		let config = StaticFilesConfig {
			static_root: temp_dir.path().join("collected"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: vec![source_dir],
			media_url: None,
		};

		let messages = check_static_files_config(&config);
		// Should have no errors, only the "collected doesn't exist" warning is acceptable
		let errors: Vec<_> = messages
			.iter()
			.filter(|m| m.level == CheckLevel::Error || m.level == CheckLevel::Critical)
			.collect();
		assert_eq!(errors.len(), 0);
	}

	#[test]
	fn test_media_url_same_as_static_url() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: Vec::new(),
			media_url: Some("/static/".to_string()),
		};

		let messages = check_media_url_conflict(&config);
		assert!(messages.iter().any(|m| m.id == "static.E004"));
	}

	#[test]
	fn test_media_url_no_leading_slash() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: Vec::new(),
			media_url: Some("media/".to_string()),
		};

		let messages = check_media_url_conflict(&config);
		assert!(messages.iter().any(|m| m.id == "static.W007"));
	}

	#[test]
	fn test_media_url_no_trailing_slash() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: Vec::new(),
			media_url: Some("/media".to_string()),
		};

		let messages = check_media_url_conflict(&config);
		assert!(messages.iter().any(|m| m.id == "static.W008"));
	}

	#[test]
	fn test_media_url_prefix_conflict() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: Vec::new(),
			media_url: Some("/static/media/".to_string()),
		};

		let messages = check_media_url_conflict(&config);
		assert!(messages.iter().any(|m| m.id == "static.W009"));
	}

	#[test]
	fn test_media_url_valid() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: Vec::new(),
			media_url: Some("/media/".to_string()),
		};

		let messages = check_media_url_conflict(&config);
		assert_eq!(messages.len(), 0);
	}

	#[test]
	fn test_media_url_none() {
		let config = StaticFilesConfig {
			static_root: PathBuf::from("/var/www/static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: Vec::new(),
			media_url: None,
		};

		let messages = check_media_url_conflict(&config);
		assert_eq!(messages.len(), 0);
	}
}
