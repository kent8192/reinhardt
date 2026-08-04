//! State persistence for local infrastructure containers.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const LOCAL_HOST: &str = "127.0.0.1";

/// Persisted state for a project's local infrastructure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalInfraState {
	/// Stable project identifier used for local infrastructure names.
	pub project_id: String,
	/// Local infrastructure profile name.
	pub profile: String,
	/// Services tracked for this project and profile.
	pub services: Vec<LocalServiceState>,
}

/// Persisted runtime state for one local service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalServiceState {
	/// Logical service name.
	pub name: String,
	/// Runtime container name.
	pub container_name: String,
	/// Container image reference.
	pub image: String,
	/// Host address used to reach the service.
	pub host: String,
	/// Host port exposed for the service.
	pub host_port: u16,
	/// Port exposed inside the container.
	pub container_port: u16,
	/// Last observed runtime status.
	pub status: ServiceRuntimeStatus,
	/// Service-specific persisted metadata.
	pub metadata: serde_json::Value,
}

/// Runtime status recorded for a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceRuntimeStatus {
	/// The service is running.
	Running,
	/// The service is stopped.
	Stopped,
	/// The service container is missing.
	Missing,
	/// The persisted state no longer matches runtime state.
	Stale,
}

/// Project-local state store.
#[derive(Debug, Clone)]
pub struct StateStore {
	path: PathBuf,
	project_root: PathBuf,
}

impl StateStore {
	/// Create a state store rooted at a project directory.
	pub fn new(project_root: impl AsRef<Path>) -> Self {
		let project_root = project_root.as_ref();
		Self {
			path: project_root.join(".reinhardt").join("local-infra.json"),
			project_root: project_root.to_path_buf(),
		}
	}

	/// Return the state file path.
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Load persisted state, returning `None` when the state file does not exist.
	pub fn load(&self) -> io::Result<Option<LocalInfraState>> {
		if !self.path.exists() {
			return Ok(None);
		}
		let bytes = fs::read(&self.path)?;
		let state = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
		validate_state(&state, &self.project_root)?;
		Ok(Some(state))
	}

	/// Load persisted state for an expected local infrastructure profile.
	pub fn load_for_profile(&self, profile: &str) -> io::Result<Option<LocalInfraState>> {
		let state = self.load()?;
		if let Some(state) = &state
			&& state.profile != profile
		{
			return Err(invalid_state(
				"profile does not match the requested profile",
			));
		}
		Ok(state)
	}

	/// Save state atomically through a temporary file in the state directory.
	pub fn save(&self, state: &LocalInfraState) -> io::Result<()> {
		if let Some(parent) = self.path.parent() {
			fs::create_dir_all(parent)?;
		}
		let tmp = self.path.with_extension("json.tmp");
		let bytes = serde_json::to_vec_pretty(state).map_err(io::Error::other)?;
		fs::write(&tmp, bytes)?;
		fs::rename(tmp, &self.path)?;
		Ok(())
	}

	/// Remove the state file if it exists.
	pub fn remove(&self) -> io::Result<()> {
		match fs::remove_file(&self.path) {
			Ok(()) => Ok(()),
			Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
			Err(err) => Err(err),
		}
	}
}

fn validate_state(state: &LocalInfraState, project_root: &Path) -> io::Result<()> {
	if state.project_id != project_id(project_root) {
		return Err(invalid_state(
			"project identifier does not match this workspace",
		));
	}
	if !is_valid_profile(&state.profile) {
		return Err(invalid_state("profile is invalid"));
	}

	let mut postgres_seen = false;
	let mut redis_seen = false;
	for service in &state.services {
		let (expected_image, expected_container_port, metadata_is_valid, already_seen) =
			match service.name.as_str() {
				"postgres" => (
					"postgres:17-alpine",
					5432,
					service
						.metadata
						.get("database")
						.and_then(serde_json::Value::as_str)
						.is_some_and(|value| !value.is_empty())
						&& service
							.metadata
							.get("user")
							.and_then(serde_json::Value::as_str)
							.is_some_and(|value| !value.is_empty()),
					postgres_seen,
				),
				"redis" => (
					"redis:7-alpine",
					6379,
					service
						.metadata
						.get("database")
						.and_then(serde_json::Value::as_u64)
						.is_some_and(|value| u16::try_from(value).is_ok()),
					redis_seen,
				),
				_ => return Err(invalid_state("service name is invalid")),
			};

		if already_seen {
			return Err(invalid_state("service is duplicated"));
		}
		if service.image != expected_image
			|| service.host != LOCAL_HOST
			|| service.host_port == 0
			|| service.container_port != expected_container_port
			|| service.container_name
				!= stable_container_name(&state.project_id, &state.profile, &service.name)
			|| !metadata_is_valid
		{
			return Err(invalid_state(
				"service does not match local infrastructure configuration",
			));
		}

		match service.name.as_str() {
			"postgres" => postgres_seen = true,
			"redis" => redis_seen = true,
			_ => unreachable!("service names are validated above"),
		}
	}
	Ok(())
}

pub(crate) fn project_id(project_root: &Path) -> String {
	use sha2::{Digest, Sha256};

	let mut hasher = Sha256::new();
	hasher.update(project_root.to_string_lossy().as_bytes());
	let digest = hasher.finalize();
	format!("{digest:x}")[..12].to_string()
}

fn stable_container_name(project_id: &str, profile: &str, service: &str) -> String {
	format!("reinhardt-{project_id}-{profile}-{service}")
}

fn is_valid_profile(profile: &str) -> bool {
	!profile.is_empty()
		&& profile
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn invalid_state(message: &str) -> io::Error {
	io::Error::new(
		io::ErrorKind::InvalidData,
		format!("local infrastructure state {message}"),
	)
}
