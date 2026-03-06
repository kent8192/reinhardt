//! Fly.io deployment provider with Machine and Postgres resources.

use std::collections::HashMap;

use tera::{Context, Tera};

use crate::config::DeployConfig;
use crate::error::{DeployError, DeployResult};

/// Embedded Fly.io-specific Tera templates.
#[derive(rust_embed::RustEmbed)]
#[folder = "templates/fly_io/"]
struct FlyIoTemplates;

/// Generate Fly.io-specific HCL files from deployment configuration.
///
/// Returns a map of filename -> rendered HCL content.
/// Always generates `main.tf` and `app.tf`.
/// Generates `database.tf` only when a database is configured
/// (only PostgreSQL is supported on Fly.io).
///
/// # Errors
///
/// Returns [`DeployError::Template`] if template loading or rendering fails.
pub fn generate_fly_hcl(config: &DeployConfig) -> DeployResult<HashMap<String, String>> {
	let tera = load_fly_templates()?;
	let mut files = HashMap::new();

	let region = config.project.region.as_deref().unwrap_or("iad");
	let (fly_cpus, fly_memory_mb) = map_fly_resources(config.app.cpu);

	let mut ctx = Context::new();
	ctx.insert("project_name", &config.project.name);
	ctx.insert("region", region);
	ctx.insert("app_port", &config.app.port);
	ctx.insert("app_instances", &config.app.instances);
	ctx.insert("fly_cpus", &fly_cpus);
	ctx.insert("fly_memory_mb", &fly_memory_mb);

	// main.tf - Fly provider
	let main_tf = tera.render("main.tf.tera", &ctx)?;
	files.insert("main.tf".into(), main_tf);

	// app.tf - Fly app + machine
	let app_tf = tera.render("app.tf.tera", &ctx)?;
	files.insert("app.tf".into(), app_tf);

	// database.tf - Fly Postgres (conditional)
	if config.database.is_some() {
		let db_version = config
			.database
			.as_ref()
			.and_then(|d| d.version.as_deref())
			.unwrap_or("16");
		ctx.insert("db_version", db_version);
		let db_tf = tera.render("database.tf.tera", &ctx)?;
		files.insert("database.tf".into(), db_tf);
	}

	Ok(files)
}

/// Load all Fly.io Tera templates from embedded resources.
fn load_fly_templates() -> DeployResult<Tera> {
	let mut tera = Tera::default();

	for file_path in FlyIoTemplates::iter() {
		let file = FlyIoTemplates::get(&file_path).ok_or_else(|| DeployError::Template {
			message: format!("embedded Fly.io template not found: {file_path}"),
		})?;
		let content =
			std::str::from_utf8(file.data.as_ref()).map_err(|e| DeployError::Template {
				message: format!("invalid UTF-8 in Fly.io template {file_path}: {e}"),
			})?;
		tera.add_raw_template(&file_path, content)?;
	}

	Ok(tera)
}

/// Map CPU millicores to Fly.io machine resources (cpus, memory_mb).
///
/// Fly.io machines use discrete CPU and memory values. This function
/// maps the abstract millicore-based CPU configuration to appropriate
/// Fly.io machine sizing.
fn map_fly_resources(cpu_millicores: u32) -> (u32, u32) {
	match cpu_millicores {
		0..=256 => (1, 256),
		257..=512 => (1, 512),
		513..=1024 => (1, 1024),
		1025..=2048 => (2, 2048),
		_ => (4, 4096),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::*;
	use rstest::rstest;

	fn minimal_config() -> DeployConfig {
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.project.region = Some("nrt".into());
		config.provider.provider_type = ProviderType::FlyIo;
		config
	}

	#[rstest]
	fn fly_generate_hcl_minimal() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("main.tf"));
		assert!(files.contains_key("app.tf"));
		assert!(!files.contains_key("database.tf"));
	}

	#[rstest]
	fn fly_generate_hcl_with_database() {
		// Arrange
		let mut config = minimal_config();
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: None,
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		});

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("database.tf"));
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("flyio/postgres:16"));
		assert!(db_tf.contains("testapp-db"));
	}

	#[rstest]
	fn fly_generate_hcl_without_database() {
		// Arrange
		let mut config = minimal_config();
		config.database = None;

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		assert!(!files.contains_key("database.tf"));
		assert_eq!(files.len(), 2);
	}

	#[rstest]
	fn fly_main_tf_contains_provider() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		let main_tf = &files["main.tf"];
		assert!(main_tf.contains("provider \"fly\""));
		assert!(main_tf.contains("fly-apps/fly"));
	}

	#[rstest]
	fn fly_app_tf_contains_project_name() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		let app_tf = &files["app.tf"];
		assert!(app_tf.contains("\"testapp\""));
		assert!(app_tf.contains("testapp-app"));
		assert!(app_tf.contains("fly_app"));
		assert!(app_tf.contains("fly_machine"));
	}

	#[rstest]
	fn fly_app_tf_contains_port() {
		// Arrange
		let mut config = minimal_config();
		config.app.port = 3000;

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		let app_tf = &files["app.tf"];
		assert!(app_tf.contains("internal_port = 3000"));
	}

	#[rstest]
	fn fly_default_region() {
		// Arrange
		let mut config = minimal_config();
		config.project.region = None;

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		let app_tf = &files["app.tf"];
		assert!(app_tf.contains("iad"));
	}

	#[rstest]
	fn fly_map_resources_micro() {
		// Arrange & Act
		let (cpus, memory) = map_fly_resources(256);

		// Assert
		assert_eq!(cpus, 1);
		assert_eq!(memory, 256);
	}

	#[rstest]
	fn fly_map_resources_large() {
		// Arrange & Act
		let (cpus, memory) = map_fly_resources(2048);

		// Assert
		assert_eq!(cpus, 2);
		assert_eq!(memory, 2048);
	}

	#[rstest]
	fn fly_map_resources_xlarge() {
		// Arrange & Act
		let (cpus, memory) = map_fly_resources(4096);

		// Assert
		assert_eq!(cpus, 4);
		assert_eq!(memory, 4096);
	}

	#[rstest]
	fn fly_database_with_custom_version() {
		// Arrange
		let mut config = minimal_config();
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: Some("15".into()),
			instance_size: InstanceSize::Medium,
			storage_gb: 50,
			backup_retention_days: 14,
			high_availability: false,
		});

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("flyio/postgres:15"));
	}

	#[rstest]
	fn fly_app_tf_contains_region() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_fly_hcl(&config).unwrap();

		// Assert
		let app_tf = &files["app.tf"];
		assert!(app_tf.contains("nrt"));
	}
}
