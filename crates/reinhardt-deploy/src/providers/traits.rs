use std::collections::HashMap;

use crate::config::{DeployConfig, InstanceSize};
use crate::error::DeployResult;

/// Terraform provider requirement for HCL generation.
#[derive(Debug, Clone)]
pub struct TerraformProvider {
	pub name: String,
	pub source: String,
	pub version: String,
}

/// Pre-flight check definition.
#[derive(Debug, Clone)]
pub struct PreflightCheck {
	pub name: String,
	pub description: String,
	pub command: String,
	pub expected_exit_code: i32,
}

/// Trait for cloud provider implementations.
pub trait DeployProvider: Send + Sync {
	/// Provider name (e.g., "docker", "aws").
	fn name(&self) -> &str;

	/// Required CLI tools that must be installed.
	fn required_tools(&self) -> Vec<&str>;

	/// Terraform providers needed for this deployment target.
	fn terraform_providers(&self) -> Vec<TerraformProvider>;

	/// Map a generic instance size to provider-specific sizing.
	fn map_instance_size(&self, component: &str, size: &InstanceSize) -> String;

	/// Map a generic region name to provider-specific region.
	fn map_region(&self, region: &str) -> String;

	/// Pre-flight checks specific to this provider.
	fn preflight_checks(&self, config: &DeployConfig) -> Vec<PreflightCheck>;

	/// Generate provider-specific HCL files from configuration.
	/// Returns a map of filename -> rendered HCL content.
	fn generate_hcl(&self, config: &DeployConfig) -> DeployResult<HashMap<String, String>>;
}

/// Docker-based deployment provider for local/single-server deployments.
pub struct DockerProvider;

impl DeployProvider for DockerProvider {
	fn name(&self) -> &str {
		"docker"
	}

	fn required_tools(&self) -> Vec<&str> {
		vec!["docker"]
	}

	fn terraform_providers(&self) -> Vec<TerraformProvider> {
		vec![TerraformProvider {
			name: "docker".into(),
			source: "kreuzwerker/docker".into(),
			version: "~> 3.0".into(),
		}]
	}

	fn map_instance_size(&self, _component: &str, size: &InstanceSize) -> String {
		// Docker uses memory limits directly
		match size {
			InstanceSize::Micro => "256m".into(),
			InstanceSize::Small => "512m".into(),
			InstanceSize::Medium => "1g".into(),
			InstanceSize::Large => "2g".into(),
			InstanceSize::Xlarge => "4g".into(),
		}
	}

	fn map_region(&self, region: &str) -> String {
		// Docker runs locally; region is a pass-through
		region.into()
	}

	fn preflight_checks(&self, _config: &DeployConfig) -> Vec<PreflightCheck> {
		Vec::new()
	}

	fn generate_hcl(&self, config: &DeployConfig) -> DeployResult<HashMap<String, String>> {
		super::docker::generate_docker_hcl(config)
	}
}

/// Fly.io deployment provider.
pub struct FlyIoProvider;

impl DeployProvider for FlyIoProvider {
	fn name(&self) -> &str {
		"fly"
	}

	fn required_tools(&self) -> Vec<&str> {
		vec!["flyctl"]
	}

	fn terraform_providers(&self) -> Vec<TerraformProvider> {
		vec![TerraformProvider {
			name: "fly".into(),
			source: "fly-apps/fly".into(),
			version: "~> 0.1".into(),
		}]
	}

	fn map_instance_size(&self, _component: &str, size: &InstanceSize) -> String {
		match size {
			InstanceSize::Micro => "shared-cpu-1x".into(),
			InstanceSize::Small => "shared-cpu-1x".into(),
			InstanceSize::Medium => "dedicated-cpu-1x".into(),
			InstanceSize::Large => "dedicated-cpu-2x".into(),
			InstanceSize::Xlarge => "dedicated-cpu-4x".into(),
		}
	}

	fn map_region(&self, region: &str) -> String {
		// Fly.io uses short region codes; pass through as-is
		region.into()
	}

	fn preflight_checks(&self, _config: &DeployConfig) -> Vec<PreflightCheck> {
		Vec::new()
	}

	fn generate_hcl(&self, config: &DeployConfig) -> DeployResult<HashMap<String, String>> {
		super::fly_io::generate_fly_hcl(config)
	}
}

/// AWS deployment provider.
pub struct AwsProvider;

impl DeployProvider for AwsProvider {
	fn name(&self) -> &str {
		"aws"
	}

	fn required_tools(&self) -> Vec<&str> {
		vec!["aws"]
	}

	fn terraform_providers(&self) -> Vec<TerraformProvider> {
		vec![TerraformProvider {
			name: "aws".into(),
			source: "hashicorp/aws".into(),
			version: "~> 5.0".into(),
		}]
	}

	fn map_instance_size(&self, component: &str, size: &InstanceSize) -> String {
		match component {
			"database" => match size {
				InstanceSize::Micro => "db.t4g.micro".into(),
				InstanceSize::Small => "db.t4g.small".into(),
				InstanceSize::Medium => "db.t4g.medium".into(),
				InstanceSize::Large => "db.r6g.large".into(),
				InstanceSize::Xlarge => "db.r6g.xlarge".into(),
			},
			"cache" => match size {
				InstanceSize::Micro => "cache.t4g.micro".into(),
				InstanceSize::Small => "cache.t4g.small".into(),
				InstanceSize::Medium => "cache.t4g.medium".into(),
				InstanceSize::Large => "cache.r6g.large".into(),
				InstanceSize::Xlarge => "cache.r6g.xlarge".into(),
			},
			// Default to ECS/Fargate CPU+memory sizing
			_ => match size {
				InstanceSize::Micro => "256/512".into(),
				InstanceSize::Small => "512/1024".into(),
				InstanceSize::Medium => "1024/2048".into(),
				InstanceSize::Large => "2048/4096".into(),
				InstanceSize::Xlarge => "4096/8192".into(),
			},
		}
	}

	fn map_region(&self, region: &str) -> String {
		// AWS region codes are used directly
		region.into()
	}

	fn preflight_checks(&self, _config: &DeployConfig) -> Vec<PreflightCheck> {
		Vec::new()
	}

	fn generate_hcl(&self, config: &DeployConfig) -> DeployResult<HashMap<String, String>> {
		super::aws::generate_aws_hcl(config)
	}
}

/// GCP deployment provider.
pub struct GcpProvider;

impl DeployProvider for GcpProvider {
	fn name(&self) -> &str {
		"gcp"
	}

	fn required_tools(&self) -> Vec<&str> {
		vec!["gcloud"]
	}

	fn terraform_providers(&self) -> Vec<TerraformProvider> {
		vec![TerraformProvider {
			name: "google".into(),
			source: "hashicorp/google".into(),
			version: "~> 5.0".into(),
		}]
	}

	fn map_instance_size(&self, component: &str, size: &InstanceSize) -> String {
		match component {
			"database" => match size {
				InstanceSize::Micro => "db-f1-micro".into(),
				InstanceSize::Small => "db-g1-small".into(),
				InstanceSize::Medium => "db-custom-2-4096".into(),
				InstanceSize::Large => "db-custom-4-8192".into(),
				InstanceSize::Xlarge => "db-custom-8-16384".into(),
			},
			// Default to Cloud Run CPU+memory sizing
			_ => match size {
				InstanceSize::Micro => "1/256Mi".into(),
				InstanceSize::Small => "1/512Mi".into(),
				InstanceSize::Medium => "2/1Gi".into(),
				InstanceSize::Large => "4/2Gi".into(),
				InstanceSize::Xlarge => "8/4Gi".into(),
			},
		}
	}

	fn map_region(&self, region: &str) -> String {
		// GCP region codes are used directly
		region.into()
	}

	fn preflight_checks(&self, _config: &DeployConfig) -> Vec<PreflightCheck> {
		Vec::new()
	}

	fn generate_hcl(&self, config: &DeployConfig) -> DeployResult<HashMap<String, String>> {
		super::gcp::generate_gcp_hcl(config)
	}
}
