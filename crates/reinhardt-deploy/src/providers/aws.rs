//! AWS deployment provider with ECS Fargate, RDS, and related resources.

use std::collections::HashMap;

use tera::{Context, Tera};

use crate::config::{DatabaseEngine, DeployConfig, InstanceSize};
use crate::error::{DeployError, DeployResult};

/// Embedded AWS-specific Tera templates.
#[derive(rust_embed::RustEmbed)]
#[folder = "templates/aws/"]
struct AwsTemplates;

/// Generate AWS-specific HCL files from deployment configuration.
///
/// Returns a map of filename -> rendered HCL content.
/// Always generates `main.tf`, `vpc.tf`, `ecs.tf`, and `ecr.tf`.
/// Generates `database.tf` only when a database is configured.
/// Generates `cache.tf` only when a cache is configured.
///
/// # Errors
///
/// Returns [`DeployError::Template`] if template loading or rendering fails.
pub fn generate_aws_hcl(config: &DeployConfig) -> DeployResult<HashMap<String, String>> {
	let tera = load_aws_templates()?;
	let mut files = HashMap::new();

	let mut ctx = Context::new();
	ctx.insert("project_name", &config.project.name);
	ctx.insert(
		"region",
		config.project.region.as_deref().unwrap_or("us-east-1"),
	);
	ctx.insert("app_port", &config.app.port);
	ctx.insert("app_instances", &config.app.instances);
	ctx.insert("app_cpu", &config.app.cpu);
	ctx.insert("app_memory", &config.app.memory);

	// main.tf - AWS provider + backend
	let main_tf = tera.render("main.tf.tera", &ctx)?;
	files.insert("main.tf".into(), main_tf);

	// vpc.tf - VPC + subnets + security groups
	let vpc_tf = tera.render("vpc.tf.tera", &ctx)?;
	files.insert("vpc.tf".into(), vpc_tf);

	// ecs.tf - ECS cluster + task definition + service
	let ecs_tf = tera.render("ecs.tf.tera", &ctx)?;
	files.insert("ecs.tf".into(), ecs_tf);

	// ecr.tf - Container registry
	let ecr_tf = tera.render("ecr.tf.tera", &ctx)?;
	files.insert("ecr.tf".into(), ecr_tf);

	// database.tf - RDS (conditional)
	if let Some(ref db_config) = config.database {
		let db_tf = render_database_tf(&tera, &ctx, db_config)?;
		files.insert("database.tf".into(), db_tf);
	}

	// cache.tf - ElastiCache (conditional)
	if config.cache.is_some() {
		let cache_tf = tera.render("cache.tf.tera", &ctx)?;
		files.insert("cache.tf".into(), cache_tf);
	}

	Ok(files)
}

/// Load all AWS Tera templates from embedded resources.
fn load_aws_templates() -> DeployResult<Tera> {
	let mut tera = Tera::default();

	for file_path in AwsTemplates::iter() {
		let file = AwsTemplates::get(&file_path).ok_or_else(|| DeployError::Template {
			message: format!("embedded AWS template not found: {file_path}"),
		})?;
		let content =
			std::str::from_utf8(file.data.as_ref()).map_err(|e| DeployError::Template {
				message: format!("invalid UTF-8 in AWS template {file_path}: {e}"),
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

	let (engine, engine_version, port) = match db.engine {
		DatabaseEngine::PostgreSql => ("postgres", db.version.as_deref().unwrap_or("16"), 5432u16),
		DatabaseEngine::MySql => ("mysql", db.version.as_deref().unwrap_or("8.0"), 3306u16),
	};

	ctx.insert("db_engine", engine);
	ctx.insert("db_engine_version", engine_version);
	ctx.insert("db_port", &port);
	ctx.insert(
		"db_instance_class",
		&format!("db.t4g.{}", size_to_rds(&db.instance_size)),
	);
	ctx.insert("db_storage_gb", &db.storage_gb);
	ctx.insert("db_ha", &db.high_availability);

	let rendered = tera.render("database.tf.tera", &ctx)?;
	Ok(rendered)
}

/// Map `InstanceSize` to RDS instance size suffix.
fn size_to_rds(size: &InstanceSize) -> &'static str {
	match size {
		InstanceSize::Micro => "micro",
		InstanceSize::Small => "small",
		InstanceSize::Medium => "medium",
		InstanceSize::Large => "large",
		InstanceSize::Xlarge => "xlarge",
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
		config.project.region = Some("us-east-1".into());
		config.provider.provider_type = ProviderType::Aws;
		config
	}

	#[rstest]
	fn aws_generate_hcl_minimal() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("main.tf"));
		assert!(files.contains_key("vpc.tf"));
		assert!(files.contains_key("ecs.tf"));
		assert!(files.contains_key("ecr.tf"));
		assert!(!files.contains_key("database.tf"));
		assert!(!files.contains_key("cache.tf"));
	}

	#[rstest]
	fn aws_generate_hcl_with_database() {
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
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("database.tf"));
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("postgres"));
		assert!(db_tf.contains("db.t4g.small"));
	}

	#[rstest]
	fn aws_generate_hcl_with_cache() {
		// Arrange
		let mut config = minimal_config();
		config.cache = Some(CacheConfig {
			engine: default_cache_engine(),
			version: None,
			instance_size: InstanceSize::Micro,
		});

		// Act
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		assert!(files.contains_key("cache.tf"));
		let cache_tf = &files["cache.tf"];
		assert!(cache_tf.contains("redis"));
	}

	#[rstest]
	fn aws_main_tf_contains_provider() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		let main_tf = &files["main.tf"];
		assert!(main_tf.contains("provider \"aws\""));
		assert!(main_tf.contains("hashicorp/aws"));
	}

	#[rstest]
	fn aws_vpc_tf_contains_project_name() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		let vpc_tf = &files["vpc.tf"];
		assert!(vpc_tf.contains("testapp-vpc"));
		assert!(vpc_tf.contains("testapp-app-sg"));
	}

	#[rstest]
	fn aws_ecs_tf_contains_config_values() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		let ecs_tf = &files["ecs.tf"];
		assert!(ecs_tf.contains("testapp-cluster"));
		assert!(ecs_tf.contains("FARGATE"));
	}

	#[rstest]
	fn aws_ecr_tf_contains_repository() {
		// Arrange
		let config = minimal_config();

		// Act
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		let ecr_tf = &files["ecr.tf"];
		assert!(ecr_tf.contains("testapp"));
		assert!(ecr_tf.contains("IMMUTABLE"));
	}

	#[rstest]
	fn aws_database_mysql_engine() {
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
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		let db_tf = &files["database.tf"];
		assert!(db_tf.contains("mysql"));
		assert!(db_tf.contains("8.0"));
		assert!(db_tf.contains("db.t4g.medium"));
		assert!(db_tf.contains("true")); // multi_az
	}

	#[rstest]
	fn aws_size_to_rds_mapping() {
		// Arrange & Act & Assert
		assert_eq!(size_to_rds(&InstanceSize::Micro), "micro");
		assert_eq!(size_to_rds(&InstanceSize::Small), "small");
		assert_eq!(size_to_rds(&InstanceSize::Medium), "medium");
		assert_eq!(size_to_rds(&InstanceSize::Large), "large");
		assert_eq!(size_to_rds(&InstanceSize::Xlarge), "xlarge");
	}

	#[rstest]
	fn aws_default_region() {
		// Arrange
		let mut config = minimal_config();
		config.project.region = None;

		// Act
		let files = generate_aws_hcl(&config).unwrap();

		// Assert
		let main_tf = &files["main.tf"];
		assert!(main_tf.contains("us-east-1"));
	}

	/// Helper to provide default cache engine value for tests.
	fn default_cache_engine() -> String {
		"redis".to_string()
	}
}
