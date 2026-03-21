//! Dependency injection support for migrations

/// Migration configuration
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationConfig {
	/// The migrations dir.
	pub migrations_dir: String,
	/// The auto migrate.
	pub auto_migrate: bool,
}

impl Default for MigrationConfig {
	fn default() -> Self {
		Self {
			migrations_dir: "migrations".to_string(),
			auto_migrate: false,
		}
	}
}

/// Migration service for dependency injection
#[derive(Clone)]
pub struct MigrationService {
	config: MigrationConfig,
}

impl MigrationService {
	/// Creates a new instance.
	pub fn new(config: MigrationConfig) -> Self {
		Self { config }
	}

	/// Performs the config operation.
	pub fn config(&self) -> &MigrationConfig {
		&self.config
	}
}

impl Default for MigrationService {
	fn default() -> Self {
		Self::new(MigrationConfig::default())
	}
}
