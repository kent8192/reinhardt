//! Database selection shared by database-backed management commands.

use crate::{CommandError, CommandResult};
use reinhardt_conf::HasCommonSettings;
use reinhardt_db::backends::{DatabaseConnection, DatabaseType};
use std::fmt;

pub(crate) struct DatabaseSelector {
	pub alias: String,
	pub url_override: Option<String>,
}

impl fmt::Debug for DatabaseSelector {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("DatabaseSelector")
			.field("alias", &self.alias)
			.finish()
	}
}

pub(crate) struct ResolvedDatabase {
	alias: String,
	backend: DatabaseType,
	url: String,
}

impl ResolvedDatabase {
	pub(crate) fn alias(&self) -> &str {
		&self.alias
	}

	pub(crate) fn backend(&self) -> DatabaseType {
		self.backend
	}

	pub(crate) fn url(&self) -> &str {
		&self.url
	}

	// Used by the database-backed command implementations that consume this selector.
	#[allow(dead_code)]
	pub(crate) async fn connect(&self) -> CommandResult<DatabaseConnection> {
		match self.backend {
			DatabaseType::Postgres => {
				#[cfg(feature = "postgres")]
				{
					DatabaseConnection::connect_postgres(&self.url)
						.await
						.map_err(|_| {
							CommandError::ExecutionError(
								"Failed to connect to PostgreSQL database.".to_string(),
							)
						})
				}
				#[cfg(not(feature = "postgres"))]
				{
					Err(CommandError::FeatureDisabled(
						"The selected PostgreSQL database requires the `postgres` feature."
							.to_string(),
					))
				}
			}
			DatabaseType::Mysql => {
				#[cfg(feature = "mysql")]
				{
					DatabaseConnection::connect_mysql(&self.url)
						.await
						.map_err(|_| {
							CommandError::ExecutionError(
								"Failed to connect to MySQL database.".to_string(),
							)
						})
				}
				#[cfg(not(feature = "mysql"))]
				{
					Err(CommandError::FeatureDisabled(
						"The selected MySQL database requires the `mysql` feature.".to_string(),
					))
				}
			}
			DatabaseType::Sqlite => {
				#[cfg(feature = "sqlite")]
				{
					DatabaseConnection::connect_sqlite(&self.url)
						.await
						.map_err(|_| {
							CommandError::ExecutionError(
								"Failed to connect to SQLite database.".to_string(),
							)
						})
				}
				#[cfg(not(feature = "sqlite"))]
				{
					Err(CommandError::FeatureDisabled(
						"The selected SQLite database requires the `sqlite` feature.".to_string(),
					))
				}
			}
		}
	}
}

impl fmt::Debug for ResolvedDatabase {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ResolvedDatabase")
			.field("alias", &self.alias)
			.field("backend", &self.backend)
			.finish()
	}
}

pub(crate) fn resolve_database(
	selector: &DatabaseSelector,
	settings: Option<&dyn HasCommonSettings>,
) -> CommandResult<ResolvedDatabase> {
	let url = match &selector.url_override {
		Some(url_override) => url_override.clone(),
		None => settings
			.ok_or_else(|| {
				CommandError::ExecutionError(
					"Database settings are required when no database URL override is provided."
						.to_string(),
				)
			})?
			.core()
			.databases
			.get(&selector.alias)
			.ok_or_else(|| {
				CommandError::InvalidArguments(format!(
					"Database alias `{}` was not found in settings.",
					selector.alias
				))
			})?
			.to_url(),
	};

	let backend = backend_from_url(&url)?;

	Ok(ResolvedDatabase {
		alias: selector.alias.clone(),
		backend,
		url,
	})
}

fn backend_from_url(url: &str) -> CommandResult<DatabaseType> {
	match url.split_once(':').map(|(scheme, _)| scheme) {
		Some("postgres") | Some("postgresql") => Ok(DatabaseType::Postgres),
		Some("mysql") => Ok(DatabaseType::Mysql),
		Some("sqlite") => Ok(DatabaseType::Sqlite),
		Some(scheme) if !scheme.is_empty() => Err(CommandError::InvalidArguments(format!(
			"Unsupported database URL scheme `{scheme}`."
		))),
		_ => Err(CommandError::InvalidArguments(
			"Unsupported database URL scheme `unknown`.".to_string(),
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::{DatabaseSelector, resolve_database};
	use reinhardt_conf::settings::DatabaseConfig;
	use reinhardt_conf::settings::contacts::ContactSettings;
	use reinhardt_conf::settings::core_settings::CoreSettings;
	use reinhardt_conf::settings::fragment::HasSettings;
	use reinhardt_db::backends::DatabaseType;
	use std::collections::HashMap;

	struct StubProjectSettings {
		core: CoreSettings,
		contacts: ContactSettings,
	}

	impl HasSettings<CoreSettings> for StubProjectSettings {
		fn get_settings(&self) -> &CoreSettings {
			&self.core
		}
	}

	impl HasSettings<ContactSettings> for StubProjectSettings {
		fn get_settings(&self) -> &ContactSettings {
			&self.contacts
		}
	}

	fn settings() -> StubProjectSettings {
		let mut databases = HashMap::new();
		databases.insert(
			"default".to_string(),
			DatabaseConfig::postgresql("primary", "admin", "default-secret", "localhost", 5432),
		);
		databases.insert(
			"replica".to_string(),
			DatabaseConfig::mysql("replica", "reader", "replica-secret", "localhost", 3306),
		);

		StubProjectSettings {
			core: CoreSettings {
				secret_key: "test-secret".to_string(),
				databases,
				..Default::default()
			},
			contacts: ContactSettings::default(),
		}
	}

	#[test]
	fn resolve_database_uses_default_alias_from_settings() {
		let settings = settings();
		let selector = DatabaseSelector {
			alias: "default".to_string(),
			url_override: None,
		};

		let resolved =
			resolve_database(&selector, Some(&settings)).expect("default database resolves");

		assert_eq!(resolved.alias(), "default");
		assert_eq!(resolved.backend(), DatabaseType::Postgres);
		assert_eq!(
			resolved.url(),
			"postgresql://admin:default-secret@localhost:5432/primary"
		);
	}

	#[test]
	fn resolve_database_uses_explicit_alias_from_settings() {
		let settings = settings();
		let selector = DatabaseSelector {
			alias: "replica".to_string(),
			url_override: None,
		};

		let resolved =
			resolve_database(&selector, Some(&settings)).expect("replica database resolves");

		assert_eq!(resolved.alias(), "replica");
		assert_eq!(resolved.backend(), DatabaseType::Mysql);
		assert_eq!(
			resolved.url(),
			"mysql://reader:replica-secret@localhost:3306/replica"
		);
	}

	#[test]
	fn resolve_database_prefers_url_override_over_selected_alias() {
		let settings = settings();
		let selector = DatabaseSelector {
			alias: "replica".to_string(),
			url_override: Some("sqlite::memory:".to_string()),
		};

		let resolved = resolve_database(&selector, Some(&settings)).expect("override resolves");

		assert_eq!(resolved.alias(), "replica");
		assert_eq!(resolved.backend(), DatabaseType::Sqlite);
		assert_eq!(resolved.url(), "sqlite::memory:");
	}

	#[test]
	fn resolve_database_rejects_unknown_alias_without_disclosing_urls() {
		let settings = settings();
		let selector = DatabaseSelector {
			alias: "archive".to_string(),
			url_override: None,
		};

		let error = resolve_database(&selector, Some(&settings)).expect_err("unknown alias fails");
		let diagnostic = error.to_string();

		assert!(diagnostic.contains("archive"));
		assert!(!diagnostic.contains("default-secret"));
		assert!(!diagnostic.contains("replica-secret"));
	}

	#[test]
	fn resolve_database_requires_settings_without_url_override() {
		let selector = DatabaseSelector {
			alias: "default".to_string(),
			url_override: None,
		};

		let error = resolve_database(&selector, None).expect_err("missing settings fails");

		assert!(error.to_string().contains("settings"));
	}

	#[test]
	fn resolve_database_rejects_unsupported_url_scheme_without_disclosing_url() {
		let selector = DatabaseSelector {
			alias: "default".to_string(),
			url_override: Some("oracle://admin:unsupported-secret@db.example/app".to_string()),
		};

		let error = resolve_database(&selector, None).expect_err("unsupported scheme fails");
		let diagnostic = error.to_string();

		assert!(diagnostic.contains("oracle"));
		assert!(!diagnostic.contains("unsupported-secret"));
		assert!(!diagnostic.contains("db.example"));
	}

	#[test]
	fn resolved_database_debug_redacts_url() {
		let settings = settings();
		let selector = DatabaseSelector {
			alias: "default".to_string(),
			url_override: None,
		};

		let resolved =
			resolve_database(&selector, Some(&settings)).expect("default database resolves");
		let debug = format!("{resolved:?}");

		assert!(debug.contains("default"));
		assert!(debug.contains("Postgres"));
		assert!(!debug.contains("default-secret"));
		assert!(!debug.contains("localhost"));
		assert!(!debug.contains("primary"));
	}
}
