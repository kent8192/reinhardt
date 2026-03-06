pub mod aws;
pub mod docker;
pub mod fly_io;
pub mod gcp;
pub mod traits;

pub use traits::{DeployProvider, PreflightCheck, TerraformProvider};

use crate::config::ProviderType;

/// Create a provider implementation for the given provider type.
pub fn create_provider(provider_type: ProviderType) -> Box<dyn DeployProvider> {
	match provider_type {
		ProviderType::Docker => Box::new(traits::DockerProvider),
		ProviderType::FlyIo => Box::new(traits::FlyIoProvider),
		ProviderType::Aws => Box::new(traits::AwsProvider),
		ProviderType::Gcp => Box::new(traits::GcpProvider),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::ProviderType;
	use rstest::rstest;

	#[rstest]
	fn create_provider_for_docker() {
		// Arrange & Act
		let provider = create_provider(ProviderType::Docker);

		// Assert
		assert_eq!(provider.name(), "docker");
	}

	#[rstest]
	fn create_provider_for_aws() {
		// Arrange & Act
		let provider = create_provider(ProviderType::Aws);

		// Assert
		assert_eq!(provider.name(), "aws");
	}

	#[rstest]
	fn create_provider_for_gcp() {
		// Arrange & Act
		let provider = create_provider(ProviderType::Gcp);

		// Assert
		assert_eq!(provider.name(), "gcp");
	}

	#[rstest]
	fn create_provider_for_fly() {
		// Arrange & Act
		let provider = create_provider(ProviderType::FlyIo);

		// Assert
		assert_eq!(provider.name(), "fly");
	}

	#[rstest]
	fn docker_required_tools() {
		// Arrange
		let provider = create_provider(ProviderType::Docker);

		// Act
		let tools = provider.required_tools();

		// Assert
		assert!(tools.contains(&"docker"));
	}

	#[rstest]
	fn aws_required_tools() {
		// Arrange
		let provider = create_provider(ProviderType::Aws);

		// Act
		let tools = provider.required_tools();

		// Assert
		assert!(tools.contains(&"aws"));
	}
}
