use std::path::Path;

use tera::{Context, Tera};

use crate::config::DeployConfig;
use crate::error::{DeployError, DeployResult};
use crate::providers::create_provider;

/// Embedded template assets from the `templates/` directory.
#[derive(rust_embed::RustEmbed)]
#[folder = "templates/"]
struct Templates;

/// HCL template generator that renders Terraform configuration files
/// from embedded Tera templates and deployment configuration.
pub struct HclGenerator {
	tera: Tera,
}

impl HclGenerator {
	/// Create a new `HclGenerator` by loading all embedded Tera templates.
	///
	/// # Errors
	///
	/// Returns [`DeployError::Template`] if any template fails to parse.
	pub fn new() -> DeployResult<Self> {
		let mut tera = Tera::default();

		for file_path in Templates::iter() {
			let file = Templates::get(&file_path).ok_or_else(|| DeployError::Template {
				message: format!("embedded template not found: {file_path}"),
			})?;
			let content =
				std::str::from_utf8(file.data.as_ref()).map_err(|e| DeployError::Template {
					message: format!("invalid UTF-8 in template {file_path}: {e}"),
				})?;
			tera.add_raw_template(&file_path, content)?;
		}

		Ok(Self { tera })
	}

	/// Render `versions.tf` with terraform version and required providers.
	///
	/// Uses the provider from `config.provider.provider_type` to determine
	/// which Terraform providers are required.
	///
	/// # Errors
	///
	/// Returns [`DeployError::Template`] if rendering fails.
	pub fn render_versions(&self, config: &DeployConfig) -> DeployResult<String> {
		let provider = create_provider(config.provider.provider_type.clone());
		let providers = provider.terraform_providers();

		let mut context = Context::new();
		context.insert("terraform_version", &config.terraform.version);

		// Build provider data for the template
		let provider_data: Vec<serde_json::Value> = providers
			.iter()
			.map(|p| {
				serde_json::json!({
					"name": p.name,
					"source": p.source,
					"version": p.version,
				})
			})
			.collect();
		context.insert("providers", &provider_data);

		let rendered = self.tera.render("common/versions.tf.tera", &context)?;
		Ok(rendered)
	}

	/// Render `variables.tf` with project configuration variables.
	///
	/// # Errors
	///
	/// Returns [`DeployError::Template`] if rendering fails.
	pub fn render_variables(&self, config: &DeployConfig) -> DeployResult<String> {
		let mut context = Context::new();
		context.insert("project_name", &config.project.name);
		context.insert("region", &config.project.region);
		context.insert("app_port", &config.app.port);
		context.insert("app_instances", &config.app.instances);

		let rendered = self.tera.render("common/variables.tf.tera", &context)?;
		Ok(rendered)
	}

	/// Render `outputs.tf` with output definitions.
	///
	/// Includes database endpoint output only when a database is configured.
	///
	/// # Errors
	///
	/// Returns [`DeployError::Template`] if rendering fails.
	pub fn render_outputs(&self, config: &DeployConfig) -> DeployResult<String> {
		let mut context = Context::new();
		context.insert("has_database", &config.database.is_some());

		let rendered = self.tera.render("common/outputs.tf.tera", &context)?;
		Ok(rendered)
	}

	/// Render all common templates and write them to the output directory.
	///
	/// Creates `versions.tf`, `variables.tf`, and `outputs.tf` in `output_dir`.
	///
	/// # Errors
	///
	/// Returns [`DeployError::Template`] if rendering fails, or
	/// [`DeployError::Io`] if file writing fails.
	pub fn render_all(&self, config: &DeployConfig, output_dir: &Path) -> DeployResult<()> {
		std::fs::create_dir_all(output_dir)?;

		let versions = self.render_versions(config)?;
		std::fs::write(output_dir.join("versions.tf"), versions)?;

		let variables = self.render_variables(config)?;
		std::fs::write(output_dir.join("variables.tf"), variables)?;

		let outputs = self.render_outputs(config)?;
		std::fs::write(output_dir.join("outputs.tf"), outputs)?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::{DeployConfig, ProviderType};
	use rstest::rstest;

	#[rstest]
	fn generate_versions_tf_for_docker() {
		// Arrange
		let mut config = DeployConfig::default();
		config.provider.provider_type = ProviderType::Docker;
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_versions(&config).unwrap();

		// Assert
		assert!(result.contains("required_version"));
		assert!(result.contains("kreuzwerker/docker"));
	}

	#[rstest]
	fn generate_versions_tf_for_aws() {
		// Arrange
		let mut config = DeployConfig::default();
		config.provider.provider_type = ProviderType::Aws;
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_versions(&config).unwrap();

		// Assert
		assert!(result.contains("required_version"));
		assert!(result.contains("hashicorp/aws"));
	}

	#[rstest]
	fn generate_versions_tf_for_gcp() {
		// Arrange
		let mut config = DeployConfig::default();
		config.provider.provider_type = ProviderType::Gcp;
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_versions(&config).unwrap();

		// Assert
		assert!(result.contains("required_version"));
		assert!(result.contains("hashicorp/google"));
	}

	#[rstest]
	fn generate_versions_tf_for_fly() {
		// Arrange
		let mut config = DeployConfig::default();
		config.provider.provider_type = ProviderType::FlyIo;
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_versions(&config).unwrap();

		// Assert
		assert!(result.contains("required_version"));
		assert!(result.contains("fly-apps/fly"));
	}

	#[rstest]
	fn generate_versions_tf_includes_terraform_version() {
		// Arrange
		let config = DeployConfig::default();
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_versions(&config).unwrap();

		// Assert
		assert!(result.contains(&config.terraform.version));
	}

	#[rstest]
	fn generate_variables_tf() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "testapp".into();
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_variables(&config).unwrap();

		// Assert
		assert!(result.contains("variable"));
		assert!(result.contains("project_name"));
		assert!(result.contains("testapp"));
	}

	#[rstest]
	fn generate_variables_tf_includes_port() {
		// Arrange
		let mut config = DeployConfig::default();
		config.app.port = 3000;
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_variables(&config).unwrap();

		// Assert
		assert!(result.contains("app_port"));
		assert!(result.contains("3000"));
	}

	#[rstest]
	fn generate_variables_tf_includes_instances() {
		// Arrange
		let mut config = DeployConfig::default();
		config.app.instances = 4;
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_variables(&config).unwrap();

		// Assert
		assert!(result.contains("app_instances"));
		assert!(result.contains("4"));
	}

	#[rstest]
	fn generate_variables_tf_with_region() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.region = Some("ap-northeast-1".into());
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_variables(&config).unwrap();

		// Assert
		assert!(result.contains("region"));
		assert!(result.contains("ap-northeast-1"));
	}

	#[rstest]
	fn generate_variables_tf_without_region() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.region = None;
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_variables(&config).unwrap();

		// Assert
		// Region variable block should not be present when region is None
		assert!(!result.contains("Deployment region"));
	}

	#[rstest]
	fn generate_outputs_tf_with_database() {
		// Arrange
		let mut config = DeployConfig::default();
		config.database = Some(crate::config::DatabaseConfig {
			engine: crate::config::DatabaseEngine::PostgreSql,
			version: Some("16".into()),
			instance_size: crate::config::InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		});
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_outputs(&config).unwrap();

		// Assert
		assert!(result.contains("app_url"));
		assert!(result.contains("database_endpoint"));
	}

	#[rstest]
	fn generate_outputs_tf_without_database() {
		// Arrange
		let mut config = DeployConfig::default();
		config.database = None;
		let generator = HclGenerator::new().unwrap();

		// Act
		let result = generator.render_outputs(&config).unwrap();

		// Assert
		assert!(result.contains("app_url"));
		assert!(!result.contains("database_endpoint"));
	}

	#[rstest]
	fn render_all_creates_files() {
		// Arrange
		let config = DeployConfig::default();
		let generator = HclGenerator::new().unwrap();
		let tmp = tempfile::tempdir().unwrap();

		// Act
		generator.render_all(&config, tmp.path()).unwrap();

		// Assert
		assert!(tmp.path().join("versions.tf").exists());
		assert!(tmp.path().join("variables.tf").exists());
		assert!(tmp.path().join("outputs.tf").exists());
	}

	#[rstest]
	fn render_all_writes_correct_content() {
		// Arrange
		let mut config = DeployConfig::default();
		config.project.name = "myapp".into();
		config.provider.provider_type = ProviderType::Docker;
		let generator = HclGenerator::new().unwrap();
		let tmp = tempfile::tempdir().unwrap();

		// Act
		generator.render_all(&config, tmp.path()).unwrap();

		// Assert
		let versions = std::fs::read_to_string(tmp.path().join("versions.tf")).unwrap();
		assert!(versions.contains("kreuzwerker/docker"));

		let variables = std::fs::read_to_string(tmp.path().join("variables.tf")).unwrap();
		assert!(variables.contains("myapp"));

		let outputs = std::fs::read_to_string(tmp.path().join("outputs.tf")).unwrap();
		assert!(outputs.contains("app_url"));
	}

	#[rstest]
	fn render_all_creates_output_dir_if_missing() {
		// Arrange
		let config = DeployConfig::default();
		let generator = HclGenerator::new().unwrap();
		let tmp = tempfile::tempdir().unwrap();
		let nested_dir = tmp.path().join("nested").join("terraform");

		// Act
		generator.render_all(&config, &nested_dir).unwrap();

		// Assert
		assert!(nested_dir.join("versions.tf").exists());
		assert!(nested_dir.join("variables.tf").exists());
		assert!(nested_dir.join("outputs.tf").exists());
	}
}
