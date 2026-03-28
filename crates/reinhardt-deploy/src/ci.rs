use std::path::Path;

use tera::{Context, Tera};

use crate::config::DeployConfig;
use crate::error::{DeployError, DeployResult};

/// Embedded CI/CD Tera templates.
#[derive(rust_embed::RustEmbed)]
#[folder = "templates/ci/"]
struct CiTemplates;

/// Generated CI/CD workflow content.
#[derive(Debug, Clone)]
pub struct CiWorkflow {
	/// CI provider name (e.g., "github-actions").
	pub provider: String,
	/// Rendered workflow file content.
	pub content: String,
	/// Relative output path for the workflow file.
	pub output_path: String,
}

/// Generate a GitHub Actions workflow from deployment configuration.
///
/// Loads the embedded `github-actions.yml.tera` template and renders it
/// with the production branch and Terraform version from config.
///
/// # Errors
///
/// Returns [`DeployError::Template`] if template loading or rendering fails.
pub fn generate_github_actions(config: &DeployConfig) -> DeployResult<CiWorkflow> {
	let tera = load_ci_templates()?;

	let production_branch = config
		.git
		.as_ref()
		.map(|g| g.production_branch.as_str())
		.unwrap_or("main");

	let terraform_version = &config.terraform.version;

	let mut ctx = Context::new();
	ctx.insert("production_branch", production_branch);
	ctx.insert("terraform_version", terraform_version);

	let content = tera.render("github-actions.yml.tera", &ctx)?;

	Ok(CiWorkflow {
		provider: "github-actions".into(),
		content,
		output_path: ".github/workflows/reinhardt-deploy.yml".into(),
	})
}

/// Write a generated workflow file to the project directory.
///
/// Creates the parent directories (e.g., `.github/workflows/`) if they
/// do not already exist, then writes the workflow content.
///
/// # Errors
///
/// Returns [`DeployError::Io`] if directory creation or file writing fails.
pub fn write_workflow(project_root: &Path, workflow: &CiWorkflow) -> DeployResult<()> {
	let full_path = project_root.join(&workflow.output_path);
	if let Some(parent) = full_path.parent() {
		std::fs::create_dir_all(parent)?;
	}
	std::fs::write(&full_path, &workflow.content)?;
	Ok(())
}

/// Load all CI Tera templates from embedded resources.
fn load_ci_templates() -> DeployResult<Tera> {
	let mut tera = Tera::default();

	for file_path in CiTemplates::iter() {
		let file = CiTemplates::get(&file_path).ok_or_else(|| DeployError::Template {
			message: format!("embedded CI template not found: {file_path}"),
		})?;
		let content =
			std::str::from_utf8(file.data.as_ref()).map_err(|e| DeployError::Template {
				message: format!("invalid UTF-8 in CI template {file_path}: {e}"),
			})?;
		tera.add_raw_template(&file_path, content)?;
	}

	Ok(tera)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::{DeployConfig, GitConfig};
	use rstest::rstest;

	/// Helper: create a default config with optional git settings.
	fn config_with_branch(branch: Option<&str>) -> DeployConfig {
		let mut config = DeployConfig::default();
		if let Some(b) = branch {
			config.git = Some(GitConfig {
				auto_deploy: false,
				production_branch: b.into(),
				preview_branches: vec![],
				ignored_branches: vec![],
			});
		}
		config
	}

	#[rstest]
	fn generate_workflow_default_branch() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		assert!(workflow.content.contains("branches: [main]"));
		assert!(workflow.content.contains("refs/heads/main"));
	}

	#[rstest]
	fn generate_workflow_custom_branch() {
		// Arrange
		let config = config_with_branch(Some("develop"));

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		assert!(workflow.content.contains("branches: [develop]"));
		assert!(workflow.content.contains("refs/heads/develop"));
	}

	#[rstest]
	fn generate_workflow_contains_dry_run_job() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		assert!(workflow.content.contains("dry-run:"));
		assert!(workflow.content.contains("name: Dry Run"));
		assert!(workflow.content.contains("deploy --dry-run"));
	}

	#[rstest]
	fn generate_workflow_contains_preview_job() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		assert!(workflow.content.contains("preview:"));
		assert!(workflow.content.contains("name: Preview Deploy"));
		assert!(workflow.content.contains("deploy --preview"));
		assert!(workflow.content.contains("PR_NUMBER:"));
	}

	#[rstest]
	fn generate_workflow_contains_deploy_job() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		assert!(workflow.content.contains("deploy:"));
		assert!(workflow.content.contains("name: Deploy to Production"));
		assert!(workflow.content.contains("environment: production"));
		assert!(workflow.content.contains("deploy --env production"));
	}

	#[rstest]
	fn generate_workflow_contains_terraform_version() {
		// Arrange
		let mut config = DeployConfig::default();
		config.terraform.version = "1.12".into();

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		assert!(workflow.content.contains("TERRAFORM_VERSION: \"1.12\""));
	}

	#[rstest]
	fn generate_workflow_output_path_is_correct() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		assert_eq!(
			workflow.output_path,
			".github/workflows/reinhardt-deploy.yml"
		);
	}

	#[rstest]
	fn write_workflow_creates_file() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let workflow = CiWorkflow {
			provider: "github-actions".into(),
			content: "name: Test Workflow\n".into(),
			output_path: ".github/workflows/reinhardt-deploy.yml".into(),
		};

		// Act
		write_workflow(tmp.path(), &workflow).unwrap();

		// Assert
		let expected_path = tmp.path().join(".github/workflows/reinhardt-deploy.yml");
		assert!(expected_path.exists());
		let written = std::fs::read_to_string(&expected_path).unwrap();
		assert_eq!(written, "name: Test Workflow\n");
	}

	#[rstest]
	fn generate_workflow_provider_is_github_actions() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		assert_eq!(workflow.provider, "github-actions");
	}

	#[rstest]
	fn generate_workflow_renders_github_actions_expressions() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let workflow = generate_github_actions(&config).unwrap();

		// Assert
		// Verify that Tera escaping produces valid GitHub Actions expressions
		assert!(workflow.content.contains("${{ env.TERRAFORM_VERSION }}"));
		assert!(
			workflow
				.content
				.contains("${{ github.event.pull_request.number }}")
		);
	}
}
