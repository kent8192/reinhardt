//! Dependency injection support for migrations

/// Migration configuration
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationConfig {
	pub migrations_dir: String,
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
	pub fn new(config: MigrationConfig) -> Self {
		Self { config }
	}

	pub fn config(&self) -> &MigrationConfig {
		&self.config
	}
}

impl Default for MigrationService {
	fn default() -> Self {
		Self::new(MigrationConfig::default())
	}
}
