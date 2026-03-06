use std::collections::HashMap;

use tera::{Context, Tera};

use crate::config::{DatabaseEngine, DeployConfig};
use crate::error::{DeployError, DeployResult};

/// Embedded Docker-specific Tera templates.
#[derive(rust_embed::RustEmbed)]
#[folder = "templates/docker/"]
struct DockerTemplates;

/// Generate Docker-specific HCL files from deployment configuration.
///
/// Returns a map of filename -> rendered HCL content.
/// Always generates `main.tf`, `network.tf`, and `app.tf`.
/// Generates `database.tf` only when a database is configured.
///
/// # Errors
///
/// Returns [`DeployError::Template`] if template loading or rendering fails.
pub fn generate_docker_hcl(config: &DeployConfig) -> DeployResult<HashMap<String, String>> {
	let tera = load_docker_templates()?;
	let mut files = HashMap::new();

	// Render main.tf (Docker provider config)
	let main_tf = tera.render("main.tf.tera", &Context::new())?;
	files.insert("main.tf".into(), main_tf);

	// Render network.tf
	let mut network_ctx = Context::new();
	network_ctx.insert("project_name", &config.project.name);
	let network_tf = tera.render("network.tf.tera", &network_ctx)?;
	files.insert("network.tf".into(), network_tf);

	// Render app.tf
	let mut app_ctx = Context::new();
	app_ctx.insert("project_name", &config.project.name);
	app_ctx.insert("app_port", &config.app.port);
	app_ctx.insert("app_memory", &config.app.memory);
	let app_tf = tera.render("app.tf.tera", &app_ctx)?;
	files.insert("app.tf".into(), app_tf);

	// Render database.tf only when database is configured
	if let Some(db) = &config.database {
		let db_tf = render_database_tf(&tera, config, db)?;
		files.insert("database.tf".into(), db_tf);
	}

	Ok(files)
}

/// Load all Docker Tera templates from embedded resources.
fn load_docker_templates() -> DeployResult<Tera> {
	let mut tera = Tera::default();

	for file_path in DockerTemplates::iter() {
		let file = DockerTemplates::get(&file_path).ok_or_else(|| DeployError::Template {
			message: format!("embedded docker template not found: {file_path}"),
		})?;
		let content =
			std::str::from_utf8(file.data.as_ref()).map_err(|e| DeployError::Template {
				message: format!("invalid UTF-8 in docker template {file_path}: {e}"),
			})?;
		tera.add_raw_template(&file_path, content)?;
	}

	Ok(tera)
}

/// Render the database.tf template with engine-specific context.
fn render_database_tf(
	tera: &Tera,
	config: &DeployConfig,
	db: &crate::config::DatabaseConfig,
) -> DeployResult<String> {
	let mut ctx = Context::new();
	ctx.insert("project_name", &config.project.name);

	let (db_image, db_engine_name, default_version, db_port) = match db.engine {
		DatabaseEngine::PostgreSql => ("postgres", "postgresql", "16", 5432u16),
		DatabaseEngine::MySql => ("mysql", "mysql", "8", 3306u16),
	};

	ctx.insert("db_image", db_image);
	ctx.insert("db_engine", db_engine_name);
	ctx.insert(
		"db_version",
		db.version.as_deref().unwrap_or(default_version),
	);
	ctx.insert("db_port", &db_port);

	let rendered = tera.render("database.tf.tera", &ctx)?;
	Ok(rendered)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::{DatabaseConfig, DatabaseEngine, DeployConfig, InstanceSize};
	use rstest::rstest;

	#[rstest]
	fn docker_generate_hcl_minimal() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("main.tf"));
		assert!(files.contains_key("network.tf"));
		assert!(files.contains_key("app.tf"));
	}

	#[rstest]
	fn docker_generate_hcl_with_database() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: Some("16".into()),
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		});

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("database.tf"));
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("postgres"));
	}

	#[rstest]
	fn docker_generate_hcl_without_database() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		assert!(!files.contains_key("database.tf"));
	}

	#[rstest]
	fn docker_app_tf_contains_port() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.app.port = 3000;

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		let app_tf = &files["app.tf"];
		assert!(app_tf.contains("3000"));
	}

	#[rstest]
	fn docker_main_tf_contains_docker_provider() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		let main_tf = &files["main.tf"];
		assert!(main_tf.contains("provider \"docker\""));
		assert!(main_tf.contains("unix:///var/run/docker.sock"));
	}

	#[rstest]
	fn docker_network_tf_contains_project_name() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "myproject".into();

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		let network_tf = &files["network.tf"];
		assert!(network_tf.contains("myproject_network"));
	}

	#[rstest]
	fn docker_app_tf_contains_memory() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.app.memory = 1024;

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		let app_tf = &files["app.tf"];
		assert!(app_tf.contains("1024"));
	}

	#[rstest]
	fn docker_database_tf_uses_custom_version() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: Some("15".into()),
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		});

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("postgres:15"));
	}

	#[rstest]
	fn docker_database_tf_uses_default_version() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: None,
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		});

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("postgres:16"));
	}

	#[rstest]
	fn docker_database_tf_mysql_engine() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::MySql,
			version: Some("8".into()),
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		});

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("mysql:8"));
		assert!(db_tf.contains("MYSQL_DATABASE"));
	}

	#[rstest]
	fn docker_database_tf_postgresql_env_vars() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: Some("16".into()),
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		});

		// Act
		let files = generate_docker_hcl(&config).unwrap();

		// Assert
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("POSTGRES_DB=testapp"));
		assert!(db_tf.contains("POSTGRES_USER=testapp"));
		assert!(db_tf.contains("POSTGRES_PASSWORD"));
	}
}
