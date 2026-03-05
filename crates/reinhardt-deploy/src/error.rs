use std::path::PathBuf;

use thiserror::Error;

/// Result type for deploy operations.
pub type DeployResult<T> = std::result::Result<T, DeployError>;

/// Errors that can occur during deployment.
#[derive(Debug, Error)]
pub enum DeployError {
	#[error("deploy.toml not found at: {path}")]
	ConfigNotFound { path: PathBuf },

	#[error("failed to parse deploy.toml: {message}")]
	ConfigParse { message: String },

	#[error("invalid configuration: {message}")]
	ConfigValidation { message: String },

	#[error("terraform not found in PATH (>= 1.11 required)")]
	TerraformNotFound,

	#[error("terraform version {found} is below minimum required {required}")]
	TerraformVersion { found: String, required: String },

	#[error("terraform execution failed: {message}")]
	TerraformExecution { message: String },

	#[error("provider error ({provider}): {message}")]
	Provider { provider: String, message: String },

	#[error("docker error: {message}")]
	Docker { message: String },

	#[error("build error: {message}")]
	Build { message: String },

	#[error("pre-flight check failed: {check}")]
	PreflightFailed { check: String },

	#[error("template rendering error: {message}")]
	Template { message: String },

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("detection error: {message}")]
	Detection { message: String },

	#[error("rollback error: {message}")]
	Rollback { message: String },
}

impl From<toml::de::Error> for DeployError {
	fn from(e: toml::de::Error) -> Self {
		DeployError::ConfigParse {
			message: e.to_string(),
		}
	}
}

impl From<tera::Error> for DeployError {
	fn from(e: tera::Error) -> Self {
		DeployError::Template {
			message: e.to_string(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn deploy_error_display_config_not_found() {
		// Arrange
		let error = DeployError::ConfigNotFound {
			path: "/app/deploy.toml".into(),
		};

		// Act
		let display = format!("{}", error);

		// Assert
		assert_eq!(display, "deploy.toml not found at: /app/deploy.toml");
	}

	#[rstest]
	fn deploy_error_display_config_parse() {
		// Arrange
		let error = DeployError::ConfigParse {
			message: "invalid key".to_string(),
		};

		// Act
		let display = format!("{}", error);

		// Assert
		assert_eq!(display, "failed to parse deploy.toml: invalid key");
	}

	#[rstest]
	fn deploy_error_display_terraform_not_found() {
		// Arrange
		let error = DeployError::TerraformNotFound;

		// Act
		let display = format!("{}", error);

		// Assert
		assert_eq!(display, "terraform not found in PATH (>= 1.11 required)");
	}

	#[rstest]
	fn deploy_error_from_io_error() {
		// Arrange
		let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");

		// Act
		let deploy_err: DeployError = io_err.into();

		// Assert
		assert!(matches!(deploy_err, DeployError::Io(_)));
	}
}
