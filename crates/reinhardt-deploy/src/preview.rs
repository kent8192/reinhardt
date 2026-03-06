//! Preview deployment support.
//!
//! Provides per-PR preview environments with isolated Terraform workspaces,
//! scaled-down resources, unique subdomains, and TTL-based cleanup.

use crate::config::{DeployConfig, InstanceSize};
use crate::error::DeployResult;

/// Default TTL in hours when no preview configuration is provided.
const DEFAULT_TTL_HOURS: u32 = 72;

/// Default domain used when no domain is configured.
const DEFAULT_DOMAIN: &str = "localhost";

/// Scaled-down application configuration for preview environments.
///
/// Preview environments use minimal resources to reduce cost while
/// providing a functional deployment for PR review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaledAppConfig {
	pub instances: u32,
	pub cpu: u32,
	pub memory: u32,
	pub instance_size: InstanceSize,
}

/// A preview environment associated with a specific pull request.
///
/// Each PR gets its own isolated Terraform workspace, a unique subdomain,
/// and scaled-down resources. Environments are automatically cleaned up
/// after the configured TTL expires.
#[derive(Debug, Clone)]
pub struct PreviewEnvironment {
	pub pr_number: u32,
	pub workspace_name: String,
	pub subdomain: String,
	pub ttl_hours: u32,
	pub scaled_config: ScaledAppConfig,
}

/// Generate a Terraform workspace name for a given PR number.
///
/// Format: `preview-pr-{N}`
pub fn preview_workspace_name(pr_number: u32) -> String {
	format!("preview-pr-{pr_number}")
}

/// Generate a preview subdomain for a given PR number and domain.
///
/// Format: `pr-{N}.preview.{domain}`
pub fn preview_subdomain(pr_number: u32, domain: &str) -> String {
	format!("pr-{pr_number}.preview.{domain}")
}

/// Create a preview environment for the given PR number using deploy configuration.
///
/// Resources are scaled down to micro instances with a single replica.
/// TTL and domain are read from the config, falling back to defaults
/// (72 hours TTL, "localhost" domain) when not configured.
pub fn create_preview_environment(
	pr_number: u32,
	config: &DeployConfig,
) -> DeployResult<PreviewEnvironment> {
	let domain = config.network.domain.as_deref().unwrap_or(DEFAULT_DOMAIN);

	let ttl_hours = config
		.preview
		.as_ref()
		.map(|p| p.ttl_hours)
		.unwrap_or(DEFAULT_TTL_HOURS);

	let scaled_config = ScaledAppConfig {
		instances: 1,
		cpu: 128,
		memory: 256,
		instance_size: InstanceSize::Micro,
	};

	Ok(PreviewEnvironment {
		pr_number,
		workspace_name: preview_workspace_name(pr_number),
		subdomain: preview_subdomain(pr_number, domain),
		ttl_hours,
		scaled_config,
	})
}

/// Check whether a preview environment has exceeded its TTL.
///
/// Compares the elapsed time since deployment (in seconds) against
/// the environment's TTL converted to seconds.
pub fn is_preview_expired(
	env: &PreviewEnvironment,
	deployed_at_epoch: u64,
	current_epoch: u64,
) -> bool {
	let ttl_seconds = u64::from(env.ttl_hours) * 3600;
	current_epoch.saturating_sub(deployed_at_epoch) >= ttl_seconds
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::{NetworkConfig, PreviewConfig};
	use rstest::rstest;

	#[rstest]
	fn workspace_name_generation() {
		// Arrange
		let pr_number = 42;

		// Act
		let name = preview_workspace_name(pr_number);

		// Assert
		assert_eq!(name, "preview-pr-42");
	}

	#[rstest]
	fn subdomain_generation() {
		// Arrange
		let pr_number = 99;
		let domain = "example.com";

		// Act
		let subdomain = preview_subdomain(pr_number, domain);

		// Assert
		assert_eq!(subdomain, "pr-99.preview.example.com");
	}

	#[rstest]
	fn create_preview_with_full_config() {
		// Arrange
		let config = DeployConfig {
			network: NetworkConfig {
				domain: Some("myapp.example.com".to_string()),
				..NetworkConfig::default()
			},
			preview: Some(PreviewConfig {
				enabled: true,
				auto_deploy: true,
				branch_subdomains: true,
				ttl_hours: 48,
				shared_database: false,
				seed_data: true,
			}),
			..DeployConfig::default()
		};

		// Act
		let env = create_preview_environment(123, &config).unwrap();

		// Assert
		assert_eq!(env.pr_number, 123);
		assert_eq!(env.workspace_name, "preview-pr-123");
		assert_eq!(env.subdomain, "pr-123.preview.myapp.example.com");
		assert_eq!(env.ttl_hours, 48);
		assert_eq!(env.scaled_config.instances, 1);
		assert_eq!(env.scaled_config.instance_size, InstanceSize::Micro);
	}

	#[rstest]
	fn create_preview_with_default_config() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let env = create_preview_environment(7, &config).unwrap();

		// Assert
		assert_eq!(env.pr_number, 7);
		assert_eq!(env.workspace_name, "preview-pr-7");
		assert_eq!(env.subdomain, "pr-7.preview.localhost");
		assert_eq!(env.ttl_hours, DEFAULT_TTL_HOURS);
	}

	#[rstest]
	fn create_preview_with_custom_ttl() {
		// Arrange
		let config = DeployConfig {
			preview: Some(PreviewConfig {
				enabled: true,
				auto_deploy: false,
				branch_subdomains: false,
				ttl_hours: 24,
				shared_database: false,
				seed_data: false,
			}),
			..DeployConfig::default()
		};

		// Act
		let env = create_preview_environment(5, &config).unwrap();

		// Assert
		assert_eq!(env.ttl_hours, 24);
	}

	#[rstest]
	fn scaled_down_resources_are_minimal() {
		// Arrange
		let config = DeployConfig::default();

		// Act
		let env = create_preview_environment(1, &config).unwrap();

		// Assert
		assert_eq!(env.scaled_config.instances, 1);
		assert_eq!(env.scaled_config.cpu, 128);
		assert_eq!(env.scaled_config.memory, 256);
		assert_eq!(env.scaled_config.instance_size, InstanceSize::Micro);
	}

	#[rstest]
	fn ttl_expired() {
		// Arrange
		let config = DeployConfig::default();
		let env = create_preview_environment(10, &config).unwrap();
		let deployed_at = 1_000_000u64;
		// 72 hours = 259200 seconds; deployed_at + 259200 = 1_259_200
		let current = deployed_at + u64::from(env.ttl_hours) * 3600;

		// Act
		let expired = is_preview_expired(&env, deployed_at, current);

		// Assert
		assert!(expired);
	}

	#[rstest]
	fn ttl_not_expired() {
		// Arrange
		let config = DeployConfig::default();
		let env = create_preview_environment(10, &config).unwrap();
		let deployed_at = 1_000_000u64;
		// One second before TTL expires
		let current = deployed_at + u64::from(env.ttl_hours) * 3600 - 1;

		// Act
		let expired = is_preview_expired(&env, deployed_at, current);

		// Assert
		assert!(!expired);
	}

	#[rstest]
	fn preview_with_no_domain_uses_localhost() {
		// Arrange
		let config = DeployConfig {
			network: NetworkConfig {
				domain: None,
				..NetworkConfig::default()
			},
			..DeployConfig::default()
		};

		// Act
		let env = create_preview_environment(15, &config).unwrap();

		// Assert
		assert_eq!(env.subdomain, "pr-15.preview.localhost");
	}

	#[rstest]
	fn workspace_name_with_large_pr_number() {
		// Arrange
		let pr_number = 99999;

		// Act
		let name = preview_workspace_name(pr_number);

		// Assert
		assert_eq!(name, "preview-pr-99999");
	}
}
