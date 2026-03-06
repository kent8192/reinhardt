//! GCP deployment provider with Cloud Run, Cloud SQL, and related resources.

use std::collections::HashMap;

use tera::{Context, Tera};

use crate::config::{DatabaseEngine, DeployConfig, InstanceSize};
use crate::error::{DeployError, DeployResult};

/// Embedded GCP-specific Tera templates.
#[derive(rust_embed::RustEmbed)]
#[folder = "templates/gcp/"]
struct GcpTemplates;

/// Generate GCP-specific HCL files from deployment configuration.
///
/// Returns a map of filename -> rendered HCL content.
/// Always generates `main.tf`, `cloud_run.tf`, and `artifact_registry.tf`.
/// Generates `database.tf` only when a database is configured.
/// Generates `cache.tf` only when a cache is configured.
///
/// # Errors
///
/// Returns [`DeployError::Template`] if template loading or rendering fails.
pub fn generate_gcp_hcl(config: &DeployConfig) -> DeployResult<HashMap<String, String>> {
	let tera = load_gcp_templates()?;
	let mut files = HashMap::new();

	let mut ctx = Context::new();
	ctx.insert("project_name", &config.project.name);
	ctx.insert(
		"region",
		config.project.region.as_deref().unwrap_or("us-central1"),
	);
	ctx.insert("app_port", &config.app.port);
	ctx.insert("app_instances", &config.app.instances);
	ctx.insert("app_cpu_limit", &cpu_to_cloud_run(config.app.cpu));
	ctx.insert("app_memory_limit", &memory_to_cloud_run(config.app.memory));

	// main.tf - Google provider + backend
	let main_tf = tera.render("main.tf.tera", &ctx)?;
	files.insert("main.tf".into(), main_tf);

	// cloud_run.tf - Cloud Run service + IAM
	let cloud_run_tf = tera.render("cloud_run.tf.tera", &ctx)?;
	files.insert("cloud_run.tf".into(), cloud_run_tf);

	// artifact_registry.tf - Container registry
	let artifact_registry_tf = tera.render("artifact_registry.tf.tera", &ctx)?;
	files.insert("artifact_registry.tf".into(), artifact_registry_tf);

	// database.tf - Cloud SQL (conditional)
	if let Some(ref db_config) = config.database {
		let db_tf = render_database_tf(&tera, &ctx, db_config)?;
		files.insert("database.tf".into(), db_tf);
	}

	// cache.tf - Memorystore Redis (conditional)
	if config.cache.is_some() {
		let cache_tf = tera.render("cache.tf.tera", &ctx)?;
		files.insert("cache.tf".into(), cache_tf);
	}

	Ok(files)
}

/// Load all GCP Tera templates from embedded resources.
fn load_gcp_templates() -> DeployResult<Tera> {
	let mut tera = Tera::default();

	for file_path in GcpTemplates::iter() {
		let file = GcpTemplates::get(&file_path).ok_or_else(|| DeployError::Template {
			message: format!("embedded GCP template not found: {file_path}"),
		})?;
		let content =
			std::str::from_utf8(file.data.as_ref()).map_err(|e| DeployError::Template {
				message: format!("invalid UTF-8 in GCP template {file_path}: {e}"),
			})?;
		tera.add_raw_template(&file_path, content)?;
	}

	Ok(tera)
}

/// Render the database.tf template with engine-specific context.
fn render_database_tf(
	tera: &Tera,
	base_ctx: &Context,
	db: &crate::config::DatabaseConfig,
) -> DeployResult<String> {
	let mut ctx = base_ctx.clone();

	let db_version_string = match db.engine {
		DatabaseEngine::PostgreSql => {
			format!("POSTGRES_{}", db.version.as_deref().unwrap_or("16"))
		}
		DatabaseEngine::MySql => {
			format!(
				"MYSQL_{}",
				db.version.as_deref().unwrap_or("8_0").replace('.', "_")
			)
		}
	};

	let db_tier = size_to_cloud_sql(&db.instance_size);
	let db_availability = if db.high_availability {
		"REGIONAL"
	} else {
		"ZONAL"
	};

	ctx.insert("db_version_string", &db_version_string);
	ctx.insert("db_tier", db_tier);
	ctx.insert("db_availability", db_availability);
	ctx.insert("db_storage_gb", &db.storage_gb);

	let rendered = tera.render("database.tf.tera", &ctx)?;
	Ok(rendered)
}

/// Map `InstanceSize` to Cloud SQL machine tier.
fn size_to_cloud_sql(size: &InstanceSize) -> &'static str {
	match size {
		InstanceSize::Micro => "db-f1-micro",
		InstanceSize::Small => "db-g1-small",
		InstanceSize::Medium => "db-custom-2-4096",
		InstanceSize::Large => "db-custom-4-8192",
		InstanceSize::Xlarge => "db-custom-8-16384",
	}
}

/// Convert CPU value (in AWS-style milliCPU units) to Cloud Run CPU string.
///
/// Cloud Run requires at least 1 CPU core. Values below 1000 map to "1".
fn cpu_to_cloud_run(cpu: u32) -> String {
	let cores = std::cmp::max(1, cpu / 1000);
	cores.to_string()
}

/// Convert memory value (in MB) to Cloud Run memory string with Mi suffix.
fn memory_to_cloud_run(memory: u32) -> String {
	format!("{memory}Mi")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::*;
	use rstest::rstest;

	fn minimal_config() -> DeployConfig {
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		config.project.region = Some("us-central1".into());
		config.provider.provider_type = ProviderType::Gcp;
		config
	}

	#[rstest]
	fn gcp_generate_hcl_minimal() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_gcp_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("main.tf"));
		assert!(files.contains_key("cloud_run.tf"));
		assert!(files.contains_key("artifact_registry.tf"));
		assert!(!files.contains_key("database.tf"));
		assert!(!files.contains_key("cache.tf"));
	}

	#[rstest]
	fn gcp_generate_hcl_with_database() {
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
		let files = generate_gcp_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("database.tf"));
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("POSTGRES_16"));
		assert!(db_tf.contains("db-g1-small"));
	}

	#[rstest]
	fn gcp_generate_hcl_with_cache() {
		// Arrange
		let mut config = minimal_config();
		config.cache = Some(CacheConfig {
			engine: default_cache_engine(),
			version: None,
			instance_size: InstanceSize::Micro,
		});

		// Act
		let files = generate_gcp_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("cache.tf"));
		let cache_tf = &files["cache.tf"];
		assert!(cache_tf.contains("google_redis_instance"));
		assert!(cache_tf.contains("testapp-cache"));
	}

	#[rstest]
	fn gcp_main_tf_contains_provider() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_gcp_hcl(&config).unwrap();

		// Assert
		let main_tf = &files["main.tf"];
		assert!(main_tf.contains("provider \"google\""));
		assert!(main_tf.contains("hashicorp/google"));
	}

	#[rstest]
	fn gcp_cloud_run_contains_project_name() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_gcp_hcl(&config).unwrap();

		// Assert
		let cloud_run_tf = &files["cloud_run.tf"];
		assert!(cloud_run_tf.contains("testapp"));
		assert!(cloud_run_tf.contains("google_cloud_run_v2_service"));
	}

	#[rstest]
	fn gcp_artifact_registry_contains_project_name() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_gcp_hcl(&config).unwrap();

		// Assert
		let artifact_registry_tf = &files["artifact_registry.tf"];
		assert!(artifact_registry_tf.contains("testapp"));
		assert!(artifact_registry_tf.contains("google_artifact_registry_repository"));
	}

	#[rstest]
	fn gcp_database_mysql_engine() {
		// Arrange
		let mut config = minimal_config();
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::MySql,
			version: Some("8.0".into()),
			instance_size: InstanceSize::Medium,
			storage_gb: 50,
			backup_retention_days: 14,
			high_availability: true,
		});

		// Act
		let files = generate_gcp_hcl(&config).unwrap();

		// Assert
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("MYSQL_8_0"));
		assert!(db_tf.contains("db-custom-2-4096"));
		assert!(db_tf.contains("REGIONAL"));
	}

	#[rstest]
	fn gcp_default_region() {
		// Arrange
		let mut config = minimal_config();
		config.project.region = None;

		// Act
		let files = generate_gcp_hcl(&config).unwrap();

		// Assert
		let main_tf = &files["main.tf"];
		assert!(main_tf.contains("us-central1"));
	}

	#[rstest]
	fn gcp_size_to_cloud_sql_mapping() {
		// Arrange & Act & Assert
		assert_eq!(size_to_cloud_sql(&InstanceSize::Micro), "db-f1-micro");
		assert_eq!(size_to_cloud_sql(&InstanceSize::Small), "db-g1-small");
		assert_eq!(size_to_cloud_sql(&InstanceSize::Medium), "db-custom-2-4096");
		assert_eq!(size_to_cloud_sql(&InstanceSize::Large), "db-custom-4-8192");
		assert_eq!(
			size_to_cloud_sql(&InstanceSize::Xlarge),
			"db-custom-8-16384"
		);
	}

	#[rstest]
	fn gcp_cpu_to_cloud_run_conversion() {
		// Arrange & Act & Assert
		assert_eq!(cpu_to_cloud_run(256), "1");
		assert_eq!(cpu_to_cloud_run(512), "1");
		assert_eq!(cpu_to_cloud_run(1024), "1");
		assert_eq!(cpu_to_cloud_run(2048), "2");
		assert_eq!(cpu_to_cloud_run(4096), "4");
	}

	#[rstest]
	fn gcp_memory_to_cloud_run_conversion() {
		// Arrange & Act & Assert
		assert_eq!(memory_to_cloud_run(256), "256Mi");
		assert_eq!(memory_to_cloud_run(512), "512Mi");
		assert_eq!(memory_to_cloud_run(1024), "1024Mi");
		assert_eq!(memory_to_cloud_run(2048), "2048Mi");
	}

	/// Helper to provide default cache engine value for tests.
	fn default_cache_engine() -> String {
		"redis".to_string()
	}
}
