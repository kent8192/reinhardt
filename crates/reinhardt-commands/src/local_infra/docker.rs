//! Docker API adapter for local infrastructure.

use async_trait::async_trait;
use bollard::Docker;
use bollard::errors::Error as BollardError;
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{
	CreateContainerOptionsBuilder, CreateImageOptionsBuilder, ListContainersOptionsBuilder,
	RemoveContainerOptionsBuilder, StartContainerOptions,
};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Docker API operation error.
#[derive(Debug, thiserror::Error)]
pub enum DockerError {
	/// Docker backend returned an error.
	#[error("{0}")]
	Backend(String),
}

impl From<BollardError> for DockerError {
	fn from(err: BollardError) -> Self {
		Self::Backend(err.to_string())
	}
}

/// Result type for Docker API operations.
pub(crate) type DockerResult<T> = Result<T, DockerError>;

/// Runtime request for a detached local infrastructure container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerRunSpec {
	/// Container name.
	pub name: String,
	/// Image reference.
	pub image: String,
	/// Host port to bind.
	pub host_port: u16,
	/// Container port to expose.
	pub container_port: u16,
	/// Container environment variables.
	pub env: Vec<(String, String)>,
}

/// One Docker API operation captured by a fake engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockerCall {
	/// Container existence lookup.
	ContainerExists {
		/// Container name.
		name: String,
	},
	/// Container port binding lookup.
	ContainerPortBinding {
		/// Container name.
		name: String,
		/// Container port whose host binding is requested.
		container_port: u16,
	},
	/// Forced container removal.
	RemoveContainer {
		/// Container name.
		name: String,
	},
	/// Detached container creation and start.
	RunDetached {
		/// Detached container run request.
		spec: DockerRunSpec,
	},
}

/// Docker backend used by local infrastructure commands.
#[async_trait]
pub trait DockerEngine: Clone + Send + Sync + 'static {
	/// Return whether a container with this exact name exists.
	async fn container_exists(&self, name: &str) -> DockerResult<bool>;

	/// Return the loopback host port bound to a container port.
	async fn container_port_binding(
		&self,
		name: &str,
		container_port: u16,
	) -> DockerResult<Option<u16>> {
		Err(DockerError::Backend(format!(
			"Docker backend does not support inspecting `{name}` port {container_port}"
		)))
	}

	/// Remove a container by name, ignoring missing containers.
	async fn remove_container(&self, name: &str) -> DockerResult<()>;

	/// Create and start a detached container.
	async fn run_detached(&self, spec: DockerRunSpec) -> DockerResult<()>;
}

/// Bollard-backed Docker Engine API client.
#[derive(Clone)]
pub struct BollardDockerEngine {
	docker: Docker,
}

impl BollardDockerEngine {
	/// Connect to the local Docker backend using bollard's default discovery.
	pub fn local() -> DockerResult<Self> {
		let docker = Docker::connect_with_local_defaults()?;
		Ok(Self { docker })
	}

	async fn ensure_image_available(&self, image: &str) -> DockerResult<()> {
		match self.docker.inspect_image(image).await {
			Ok(_) => Ok(()),
			Err(err) if is_missing_image_error(&err) => self.pull_image(image).await,
			Err(err) => Err(DockerError::Backend(format!(
				"failed to inspect Docker image `{image}`: {err}"
			))),
		}
	}

	async fn pull_image(&self, image: &str) -> DockerResult<()> {
		let image_ref = ImagePullReference::parse(image);
		let mut options = CreateImageOptionsBuilder::default().from_image(image_ref.from_image);
		if let Some(tag) = image_ref.tag {
			options = options.tag(tag);
		}
		let mut pull = self.docker.create_image(Some(options.build()), None, None);

		while let Some(result) = pull.next().await {
			result.map_err(|err| {
				DockerError::Backend(format!("failed to pull Docker image `{image}`: {err}"))
			})?;
		}

		Ok(())
	}
}

#[derive(Debug, PartialEq, Eq)]
struct ImagePullReference<'a> {
	from_image: &'a str,
	tag: Option<&'a str>,
}

impl<'a> ImagePullReference<'a> {
	fn parse(image: &'a str) -> Self {
		if image.contains('@') {
			return Self {
				from_image: image,
				tag: None,
			};
		}

		let slash_index = image.rfind('/');
		let tag_separator = image
			.rfind(':')
			.filter(|colon_index| slash_index.is_none_or(|slash_index| *colon_index > slash_index));

		match tag_separator {
			Some(index) => Self {
				from_image: &image[..index],
				tag: Some(&image[index + 1..]),
			},
			None => Self {
				from_image: image,
				tag: Some("latest"),
			},
		}
	}
}

fn is_missing_image_error(err: &BollardError) -> bool {
	matches!(
		err,
		BollardError::DockerResponseServerError {
			status_code: 404,
			..
		}
	)
}

#[async_trait]
impl DockerEngine for BollardDockerEngine {
	async fn container_exists(&self, name: &str) -> DockerResult<bool> {
		let mut filters = HashMap::new();
		filters.insert("name", vec![name]);
		let containers = self
			.docker
			.list_containers(Some(
				ListContainersOptionsBuilder::default()
					.all(true)
					.filters(&filters)
					.build(),
			))
			.await
			.map_err(DockerError::from)?;

		Ok(containers.into_iter().any(|container| {
			container
				.names
				.unwrap_or_default()
				.iter()
				.any(|container_name| container_name == &format!("/{name}"))
		}))
	}

	async fn container_port_binding(
		&self,
		name: &str,
		container_port: u16,
	) -> DockerResult<Option<u16>> {
		let inspected = self
			.docker
			.inspect_container(name, None)
			.await
			.map_err(DockerError::from)?;
		let port = format!("{container_port}/tcp");
		let bindings = inspected
			.network_settings
			.and_then(|settings| settings.ports)
			.and_then(|mut ports| ports.remove(&port))
			.flatten()
			.unwrap_or_default();
		if bindings.len() != 1 {
			return Ok(None);
		}
		let binding = &bindings[0];
		if binding.host_ip.as_deref() != Some("127.0.0.1") {
			return Ok(None);
		}
		Ok(binding
			.host_port
			.as_deref()
			.and_then(|port| port.parse().ok()))
	}

	async fn remove_container(&self, name: &str) -> DockerResult<()> {
		if !self.container_exists(name).await? {
			return Ok(());
		}
		self.docker
			.remove_container(
				name,
				Some(
					RemoveContainerOptionsBuilder::default()
						.force(true)
						.v(true)
						.build(),
				),
			)
			.await
			.or_else(|err| match err {
				BollardError::DockerResponseServerError {
					status_code: 404, ..
				} => Ok(()),
				err => Err(DockerError::from(err)),
			})
	}

	async fn run_detached(&self, spec: DockerRunSpec) -> DockerResult<()> {
		self.ensure_image_available(&spec.image).await?;

		let exposed_port = format!("{}/tcp", spec.container_port);
		let mut port_bindings = HashMap::new();
		port_bindings.insert(
			exposed_port.clone(),
			Some(vec![PortBinding {
				host_ip: Some("127.0.0.1".to_string()),
				host_port: Some(spec.host_port.to_string()),
			}]),
		);

		let body = ContainerCreateBody {
			image: Some(spec.image.clone()),
			env: Some(
				spec.env
					.iter()
					.map(|(key, value)| format!("{key}={value}"))
					.collect(),
			),
			exposed_ports: Some(vec![exposed_port]),
			host_config: Some(HostConfig {
				auto_remove: Some(true),
				port_bindings: Some(port_bindings),
				..Default::default()
			}),
			..Default::default()
		};

		let options = CreateContainerOptionsBuilder::default()
			.name(&spec.name)
			.build();
		let container = self
			.docker
			.create_container(Some(options), body)
			.await
			.map_err(DockerError::from)?;
		self.docker
			.start_container(&container.id, None::<StartContainerOptions>)
			.await
			.map_err(DockerError::from)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn missing_image_error_is_pullable() {
		let err = BollardError::DockerResponseServerError {
			status_code: 404,
			message: "No such image: postgres:17-alpine".to_string(),
		};

		assert!(is_missing_image_error(&err));
	}

	#[test]
	fn non_missing_image_error_is_not_pullable() {
		let err = BollardError::DockerResponseServerError {
			status_code: 500,
			message: "Docker daemon failed".to_string(),
		};

		assert!(!is_missing_image_error(&err));
	}

	#[test]
	fn image_pull_reference_splits_tagged_image() {
		assert_eq!(
			ImagePullReference::parse("postgres:17-alpine"),
			ImagePullReference {
				from_image: "postgres",
				tag: Some("17-alpine")
			}
		);
	}

	#[test]
	fn image_pull_reference_defaults_untagged_image_to_latest() {
		assert_eq!(
			ImagePullReference::parse("redis"),
			ImagePullReference {
				from_image: "redis",
				tag: Some("latest")
			}
		);
	}

	#[test]
	fn image_pull_reference_defaults_namespaced_untagged_image_to_latest() {
		assert_eq!(
			ImagePullReference::parse("library/redis"),
			ImagePullReference {
				from_image: "library/redis",
				tag: Some("latest")
			}
		);
	}

	#[test]
	fn image_pull_reference_preserves_registry_port() {
		assert_eq!(
			ImagePullReference::parse("registry.example.com:5000/reinhardt/postgres:17-alpine"),
			ImagePullReference {
				from_image: "registry.example.com:5000/reinhardt/postgres",
				tag: Some("17-alpine")
			}
		);
	}

	#[test]
	fn image_pull_reference_keeps_digest_reference() {
		assert_eq!(
			ImagePullReference::parse("postgres@sha256:abcdef"),
			ImagePullReference {
				from_image: "postgres@sha256:abcdef",
				tag: None
			}
		);
	}
}

/// Fake Docker engine for tests.
#[derive(Debug, Clone)]
pub struct FakeDockerEngine {
	calls: Arc<Mutex<Vec<DockerCall>>>,
	exists: Arc<Mutex<Vec<bool>>>,
	port_bindings: Arc<Mutex<Vec<Option<u16>>>>,
}

impl FakeDockerEngine {
	/// Create a fake engine that returns existence checks in order.
	pub fn new(exists: Vec<bool>) -> Self {
		Self {
			calls: Arc::new(Mutex::new(Vec::new())),
			exists: Arc::new(Mutex::new(exists)),
			port_bindings: Arc::new(Mutex::new(Vec::new())),
		}
	}

	/// Configure container port binding results in lookup order.
	pub fn with_port_bindings(self, port_bindings: Vec<Option<u16>>) -> Self {
		*self.port_bindings.lock().expect("port bindings lock") = port_bindings;
		self
	}

	/// Return captured Docker API operations.
	pub fn calls(&self) -> Vec<DockerCall> {
		self.calls.lock().expect("calls lock").clone()
	}
}

#[async_trait]
impl DockerEngine for FakeDockerEngine {
	async fn container_exists(&self, name: &str) -> DockerResult<bool> {
		self.calls
			.lock()
			.expect("calls lock")
			.push(DockerCall::ContainerExists {
				name: name.to_string(),
			});
		let mut exists = self.exists.lock().expect("exists lock");
		Ok(if exists.is_empty() {
			false
		} else {
			exists.remove(0)
		})
	}

	async fn container_port_binding(
		&self,
		name: &str,
		container_port: u16,
	) -> DockerResult<Option<u16>> {
		self.calls
			.lock()
			.expect("calls lock")
			.push(DockerCall::ContainerPortBinding {
				name: name.to_string(),
				container_port,
			});
		let mut port_bindings = self.port_bindings.lock().expect("port bindings lock");
		Ok(if port_bindings.is_empty() {
			None
		} else {
			port_bindings.remove(0)
		})
	}

	async fn remove_container(&self, name: &str) -> DockerResult<()> {
		self.calls
			.lock()
			.expect("calls lock")
			.push(DockerCall::RemoveContainer {
				name: name.to_string(),
			});
		Ok(())
	}

	async fn run_detached(&self, spec: DockerRunSpec) -> DockerResult<()> {
		self.calls
			.lock()
			.expect("calls lock")
			.push(DockerCall::RunDetached { spec });
		Ok(())
	}
}
