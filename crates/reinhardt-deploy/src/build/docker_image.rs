//! Docker image building utilities for Reinhardt deploy.
//!
//! Provides Dockerfile generation with multi-stage builds optimized for Rust
//! applications, along with helpers for constructing Docker CLI arguments.

use regex::Regex;
use rust_embed::RustEmbed;
use tera::{Context, Tera};

use crate::error::{DeployError, DeployResult};

/// Options for building a Docker image of a Reinhardt application.
#[derive(Debug, Clone)]
pub struct DockerBuildOptions {
	/// Application name used for labeling and default binary name.
	pub app_name: String,
	/// Port to expose in the container.
	pub port: u16,
	/// Health check endpoint path (e.g., "/health/").
	pub health_check: String,
	/// Override the binary name copied from the build stage.
	/// Falls back to `app_name` when `None`.
	pub binary_name: Option<String>,
}

#[derive(RustEmbed)]
#[folder = "templates/build/"]
struct BuildTemplates;

/// Validate that a binary name contains only safe characters.
fn validate_binary_name(name: &str) -> DeployResult<()> {
	let re = Regex::new(r"^[a-zA-Z0-9_-]+$").expect("binary name regex must compile");
	if !re.is_match(name) {
		return Err(DeployError::Build {
			message: format!("invalid binary name '{}': must match [a-zA-Z0-9_-]+", name),
		});
	}
	Ok(())
}

/// Validate that a health check path is a safe URL path.
fn validate_health_check(path: &str) -> DeployResult<()> {
	let re = Regex::new(r"^/[a-zA-Z0-9/_.-]*$").expect("health check regex must compile");
	if !re.is_match(path) {
		return Err(DeployError::Build {
			message: format!(
				"invalid health check path '{}': must start with / and contain only [a-zA-Z0-9/_.-]",
				path
			),
		});
	}
	Ok(())
}

/// Generate a multi-stage Dockerfile for a Rust application.
///
/// Stage 1 (builder): compiles the project in release mode.
/// Stage 2 (runtime): copies the binary into a minimal Debian image with
/// health check and port exposure configured.
pub fn generate_dockerfile(opts: &DockerBuildOptions) -> DeployResult<String> {
	let binary = opts.binary_name.as_deref().unwrap_or(&opts.app_name);
	validate_binary_name(binary)?;
	validate_health_check(&opts.health_check)?;

	let template_content =
		BuildTemplates::get("Dockerfile.backend.tera").ok_or_else(|| DeployError::Template {
			message: "Dockerfile.backend.tera template not found".to_string(),
		})?;
	let template_str =
		std::str::from_utf8(template_content.data.as_ref()).map_err(|e| DeployError::Template {
			message: format!("failed to read template as UTF-8: {e}"),
		})?;

	let mut tera = Tera::default();
	tera.add_raw_template("Dockerfile.backend", template_str)?;

	let mut context = Context::new();
	context.insert("binary", binary);
	context.insert("port", &opts.port);
	context.insert("health_check", &opts.health_check);

	let rendered = tera.render("Dockerfile.backend", &context)?;
	Ok(rendered)
}

/// Options for building a WASM frontend Docker image.
#[derive(Debug, Clone)]
pub struct WasmDockerBuildOptions {
	/// Port to expose in the container (nginx listen port).
	pub port: u16,
	/// Health check endpoint path (e.g., "/").
	pub health_check: String,
	/// Output directory from trunk build (defaults to "dist").
	pub dist_dir: Option<String>,
	/// WASM compilation target (defaults to "wasm32-unknown-unknown").
	pub wasm_target: Option<String>,
}

/// Generate a Dockerfile for a WASM frontend application.
///
/// Stage 1 (builder): installs `wasm32-unknown-unknown` target and `trunk`,
/// then builds the frontend with `trunk build --release`.
/// Stage 2 (runtime): serves the built assets via nginx.
pub fn generate_wasm_dockerfile(opts: &WasmDockerBuildOptions) -> DeployResult<String> {
	let template_content =
		BuildTemplates::get("Dockerfile.wasm.tera").ok_or_else(|| DeployError::Template {
			message: "Dockerfile.wasm.tera template not found".to_string(),
		})?;
	let template_str =
		std::str::from_utf8(template_content.data.as_ref()).map_err(|e| DeployError::Template {
			message: format!("failed to read template as UTF-8: {e}"),
		})?;

	let mut tera = Tera::default();
	tera.add_raw_template("Dockerfile.wasm", template_str)?;

	validate_health_check(&opts.health_check)?;

	let mut context = Context::new();
	context.insert("port", &opts.port);
	context.insert("health_check", &opts.health_check);
	if let Some(ref dist_dir) = opts.dist_dir {
		context.insert("dist_dir", dist_dir);
	}
	if let Some(ref wasm_target) = opts.wasm_target {
		context.insert("wasm_target", wasm_target);
	}

	let rendered = tera.render("Dockerfile.wasm", &context)?;
	Ok(rendered)
}

/// Construct a Docker image tag from registry, application name, and commit hash.
///
/// Returns a tag in the format `registry/app_name:commit`.
pub fn build_image_tag(registry: &str, app_name: &str, commit: &str) -> String {
	format!("{registry}/{app_name}:{commit}")
}

/// Build the argument list for a `docker build` command.
///
/// Returns args suitable for passing to a process builder:
/// `["build", "-t", tag, context_dir]`.
pub fn build_docker_args(tag: &str, context_dir: &str) -> Vec<String> {
	vec![
		"build".to_string(),
		"-t".to_string(),
		tag.to_string(),
		context_dir.to_string(),
	]
}

/// Build the argument list for a `docker push` command.
///
/// Returns args suitable for passing to a process builder:
/// `["push", tag]`.
pub fn push_docker_args(tag: &str) -> Vec<String> {
	vec!["push".to_string(), tag.to_string()]
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn dockerfile_generation_minimal() {
		// Arrange
		let opts = DockerBuildOptions {
			app_name: "myapp".to_string(),
			port: 8000,
			health_check: "/health/".to_string(),
			binary_name: None,
		};

		// Act
		let dockerfile = generate_dockerfile(&opts).unwrap();

		// Assert
		assert!(dockerfile.contains("FROM rust:"));
		assert!(dockerfile.contains("EXPOSE 8000"));
		assert!(dockerfile.contains("HEALTHCHECK"));
		assert!(dockerfile.contains("/health/"));
	}

	#[rstest]
	fn dockerfile_generation_with_binary_name() {
		// Arrange
		let opts = DockerBuildOptions {
			app_name: "myapp".to_string(),
			port: 8000,
			health_check: "/health/".to_string(),
			binary_name: Some("myapp-server".to_string()),
		};

		// Act
		let dockerfile = generate_dockerfile(&opts).unwrap();

		// Assert
		assert!(dockerfile.contains("myapp-server"));
	}

	#[rstest]
	fn image_tag_from_commit() {
		// Arrange
		let registry = "myregistry";
		let app_name = "myapp";
		let commit = "abc1234";

		// Act
		let tag = build_image_tag(registry, app_name, commit);

		// Assert
		assert_eq!(tag, "myregistry/myapp:abc1234");
	}

	#[rstest]
	fn build_args_correct() {
		// Arrange & Act
		let args = build_docker_args("myregistry/myapp:abc1234", ".");

		// Assert
		assert_eq!(args, vec!["build", "-t", "myregistry/myapp:abc1234", "."]);
	}

	#[rstest]
	fn push_args_correct() {
		// Arrange & Act
		let args = push_docker_args("myregistry/myapp:abc1234");

		// Assert
		assert_eq!(args, vec!["push", "myregistry/myapp:abc1234"]);
	}

	#[rstest]
	fn dockerfile_includes_runtime_stage() {
		// Arrange
		let opts = DockerBuildOptions {
			app_name: "testapp".to_string(),
			port: 3000,
			health_check: "/api/health".to_string(),
			binary_name: None,
		};

		// Act
		let dockerfile = generate_dockerfile(&opts).unwrap();

		// Assert
		assert!(dockerfile.contains("debian:bookworm-slim"));
		assert!(dockerfile.contains("EXPOSE 3000"));
	}

	#[rstest]
	fn dockerfile_rejects_invalid_binary_name() {
		// Arrange
		let opts = DockerBuildOptions {
			app_name: "my app; rm -rf /".to_string(),
			port: 8000,
			health_check: "/health/".to_string(),
			binary_name: None,
		};

		// Act
		let result = generate_dockerfile(&opts);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn dockerfile_rejects_invalid_health_check() {
		// Arrange
		let opts = DockerBuildOptions {
			app_name: "myapp".to_string(),
			port: 8000,
			health_check: "'; DROP TABLE users; --".to_string(),
			binary_name: None,
		};

		// Act
		let result = generate_dockerfile(&opts);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn dockerfile_generated_from_template() {
		// Arrange
		let opts = DockerBuildOptions {
			app_name: "myapp".to_string(),
			port: 8000,
			health_check: "/health/".to_string(),
			binary_name: None,
		};

		// Act
		let dockerfile = generate_dockerfile(&opts).unwrap();

		// Assert
		assert!(dockerfile.contains("FROM rust:"));
		assert!(dockerfile.contains("cargo build --release --bin myapp"));
		assert!(dockerfile.contains("EXPOSE 8000"));
		assert!(dockerfile.contains("/health/"));
	}

	#[rstest]
	fn wasm_dockerfile_generation() {
		// Arrange
		let opts = WasmDockerBuildOptions {
			port: 8080,
			health_check: "/".to_string(),
			dist_dir: None,
			wasm_target: None,
		};

		// Act
		let dockerfile = generate_wasm_dockerfile(&opts).unwrap();

		// Assert
		assert!(dockerfile.contains("wasm32-unknown-unknown"));
		assert!(dockerfile.contains("trunk build --release"));
		assert!(dockerfile.contains("nginx:alpine"));
		assert!(dockerfile.contains("EXPOSE 8080"));
		assert!(dockerfile.contains("HEALTHCHECK"));
	}

	#[rstest]
	fn wasm_dockerfile_serves_from_dist() {
		// Arrange
		let opts = WasmDockerBuildOptions {
			port: 3000,
			health_check: "/".to_string(),
			dist_dir: None,
			wasm_target: None,
		};

		// Act
		let dockerfile = generate_wasm_dockerfile(&opts).unwrap();

		// Assert
		assert!(dockerfile.contains("/app/dist"));
		assert!(dockerfile.contains("/usr/share/nginx/html"));
		assert!(dockerfile.contains("EXPOSE 3000"));
	}

	#[rstest]
	fn wasm_dockerfile_with_custom_dist_dir() {
		// Arrange
		let opts = WasmDockerBuildOptions {
			port: 80,
			health_check: "/".to_string(),
			dist_dir: Some("build/output".to_string()),
			wasm_target: None,
		};

		// Act
		let dockerfile = generate_wasm_dockerfile(&opts).unwrap();

		// Assert
		assert!(dockerfile.contains("/app/build/output"));
	}

	#[rstest]
	fn wasm_dockerfile_with_custom_target() {
		// Arrange
		let opts = WasmDockerBuildOptions {
			port: 80,
			health_check: "/".to_string(),
			dist_dir: None,
			wasm_target: Some("wasm32-wasi".to_string()),
		};

		// Act
		let dockerfile = generate_wasm_dockerfile(&opts).unwrap();

		// Assert
		assert!(dockerfile.contains("wasm32-wasi"));
	}
}
