use async_trait::async_trait;
use reinhardt_commands::local_infra::{
	DatabaseInfraInput, DockerCall, DockerEngine, DockerError, DockerRunSpec, FakeDockerEngine,
	InfraCommand, InfraSubcommand, LocalInfraConfig, LocalInfraState, LocalServiceState,
	PortAllocator, RedisInfraInput, ServiceRuntimeStatus, StateStore,
};
use rstest::rstest;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn state_store_round_trips_local_infra_state() {
	let temp = TempDir::new().unwrap();
	let store = StateStore::new(temp.path());
	let state = LocalInfraState {
		project_id: project_id(temp.path()),
		profile: "local".to_string(),
		services: vec![LocalServiceState {
			name: "postgres".to_string(),
			container_name: format!("reinhardt-{}-local-postgres", project_id(temp.path())),
			image: "postgres:17-alpine".to_string(),
			host: "127.0.0.1".to_string(),
			host_port: 55432,
			container_port: 5432,
			status: ServiceRuntimeStatus::Running,
			metadata: serde_json::json!({"database": "app", "user": "postgres"}),
		}],
	};

	store.save(&state).unwrap();
	let loaded = store.load().unwrap().expect("state should exist");

	assert_eq!(loaded.project_id, project_id(temp.path()));
	assert_eq!(loaded.profile, "local");
	assert_eq!(loaded.services.len(), 1);
	assert_eq!(loaded.services[0].host_port, 55432);
}

#[test]
fn state_store_missing_file_returns_none() {
	let temp = TempDir::new().unwrap();
	let store = StateStore::new(temp.path());

	let loaded = store.load().unwrap();

	assert!(loaded.is_none());
}

#[test]
fn infra_run_environment_maps_postgres_and_redis_state_to_process_env() {
	let state = LocalInfraState {
		project_id: "project123".to_string(),
		profile: "local".to_string(),
		services: vec![
			LocalServiceState {
				name: "postgres".to_string(),
				container_name: "pg".to_string(),
				image: "postgres:17-alpine".to_string(),
				host: "127.0.0.1".to_string(),
				host_port: 55432,
				container_port: 5432,
				status: ServiceRuntimeStatus::Running,
				metadata: serde_json::json!({
					"database": "app",
					"user": "postgres",
					"password": "postgres"
				}),
			},
			LocalServiceState {
				name: "redis".to_string(),
				container_name: "redis".to_string(),
				image: "redis:7-alpine".to_string(),
				host: "127.0.0.1".to_string(),
				host_port: 56379,
				container_port: 6379,
				status: ServiceRuntimeStatus::Running,
				metadata: serde_json::json!({"database": 1}),
			},
		],
	};

	let env = InfraCommand::environment_from_state(&state, None).unwrap();

	assert_eq!(
		env.iter()
			.find(|(key, _)| key == "DATABASE_URL")
			.map(|(_, value)| value.as_str()),
		Some("postgresql://postgres:postgres@127.0.0.1:55432/app")
	);
	assert_eq!(
		env.iter()
			.find(|(key, _)| key == "REDIS_URL")
			.map(|(_, value)| value.as_str()),
		Some("redis://127.0.0.1:56379/1")
	);
}

#[test]
fn infra_run_rejects_runserver_target() {
	let args = vec!["runserver".to_string()];

	let result = InfraCommand::validate_run_command(&args);

	assert!(result.is_err());
	assert!(
		result.unwrap_err().to_string().contains("manage runserver"),
		"error should direct users to the separate runserver command"
	);
}

#[test]
fn port_allocator_uses_fallback_when_requested_port_is_occupied() {
	let allocator = PortAllocator;
	let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
	let occupied = listener.local_addr().unwrap().port();

	let selected = allocator.select_port(occupied).unwrap();

	assert_ne!(selected, occupied);
}

#[tokio::test]
async fn docker_engine_records_container_existence_checks() {
	let docker = FakeDockerEngine::new(vec![true]);

	let exists = docker.container_exists("reinhardt-test").await.unwrap();

	assert!(exists);
	let calls = docker.calls();
	assert_eq!(calls.len(), 1);
	assert_eq!(
		calls[0],
		DockerCall::ContainerExists {
			name: "reinhardt-test".to_string()
		}
	);
}

#[test]
fn local_infra_config_derives_postgres_and_redis_services() {
	let config = LocalInfraConfig::derive(
		"project123",
		"local",
		Some(DatabaseInfraInput {
			engine: "postgresql".to_string(),
			host: "localhost".to_string(),
			port: 5432,
			name: "app".to_string(),
			user: "postgres".to_string(),
			password: Some("postgres".to_string()),
		}),
		Some(RedisInfraInput {
			url: "redis://localhost:6379/1".to_string(),
		}),
	)
	.unwrap();

	assert_eq!(config.project_id, "project123");
	assert_eq!(config.profile, "local");
	assert_eq!(config.services.len(), 2);
	assert_eq!(config.services[0].name(), "postgres");
	assert_eq!(config.services[1].name(), "redis");
}

#[test]
fn local_infra_config_ignores_sqlite_database() {
	let config = LocalInfraConfig::derive(
		"project123",
		"local",
		Some(DatabaseInfraInput {
			engine: "sqlite".to_string(),
			host: "localhost".to_string(),
			port: 0,
			name: "db.sqlite3".to_string(),
			user: String::new(),
			password: None,
		}),
		None,
	)
	.unwrap();

	assert!(config.services.is_empty());
}

#[tokio::test]
async fn infra_down_removes_state_even_when_containers_are_missing() {
	let temp = TempDir::new().unwrap();
	let store = StateStore::new(temp.path());
	store
		.save(&LocalInfraState {
			project_id: project_id(temp.path()),
			profile: "local".to_string(),
			services: vec![],
		})
		.unwrap();
	let docker = FakeDockerEngine::new(vec![]);

	InfraCommand::execute_with_runner(
		reinhardt_commands::local_infra::InfraSubcommand::Down { profile: None },
		temp.path(),
		docker,
	)
	.await
	.unwrap();

	assert!(store.load().unwrap().is_none());
}

#[test]
fn state_store_rejects_tampered_remote_service_endpoint() {
	let temp = TempDir::new().unwrap();
	let state_path = temp.path().join(".reinhardt/local-infra.json");
	std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
	std::fs::write(
		&state_path,
		serde_json::to_vec(&LocalInfraState {
			project_id: project_id(temp.path()),
			profile: "local".to_string(),
			services: vec![LocalServiceState {
				name: "postgres".to_string(),
				container_name: format!("reinhardt-{}-local-postgres", project_id(temp.path())),
				image: "postgres:17-alpine".to_string(),
				host: "203.0.113.10".to_string(),
				host_port: 5432,
				container_port: 5432,
				status: ServiceRuntimeStatus::Running,
				metadata: serde_json::json!({"database": "app", "user": "postgres"}),
			}],
		})
		.unwrap(),
	)
	.unwrap();

	let error = StateStore::new(temp.path()).load().unwrap_err();

	assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[tokio::test]
async fn infra_down_rejects_tampered_container_name() {
	let temp = TempDir::new().unwrap();
	let state_path = temp.path().join(".reinhardt/local-infra.json");
	std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
	std::fs::write(
		&state_path,
		serde_json::to_vec(&LocalInfraState {
			project_id: project_id(temp.path()),
			profile: "local".to_string(),
			services: vec![LocalServiceState {
				name: "redis".to_string(),
				container_name: "unrelated-container".to_string(),
				image: "redis:7-alpine".to_string(),
				host: "127.0.0.1".to_string(),
				host_port: 6379,
				container_port: 6379,
				status: ServiceRuntimeStatus::Running,
				metadata: serde_json::json!({"database": 0}),
			}],
		})
		.unwrap(),
	)
	.unwrap();
	let docker = FakeDockerEngine::new(vec![]);

	let result = InfraCommand::execute_with_runner(
		reinhardt_commands::local_infra::InfraSubcommand::Down { profile: None },
		temp.path(),
		docker.clone(),
	)
	.await;

	assert!(result.is_err());
	assert_eq!(docker.calls(), Vec::new());
}

#[tokio::test]
async fn infra_down_rejects_state_for_another_profile() {
	let temp = TempDir::new().unwrap();
	let store = StateStore::new(temp.path());
	store
		.save(&LocalInfraState {
			project_id: project_id(temp.path()),
			profile: "development".to_string(),
			services: vec![],
		})
		.unwrap();
	let docker = FakeDockerEngine::new(vec![]);

	let result = InfraCommand::execute_with_runner(
		reinhardt_commands::local_infra::InfraSubcommand::Down {
			profile: Some("local".to_string()),
		},
		temp.path(),
		docker,
	)
	.await;

	assert!(result.is_err());
	assert!(store.load().unwrap().is_some());
}

fn project_id(path: &std::path::Path) -> String {
	use sha2::{Digest, Sha256};

	let mut hasher = Sha256::new();
	hasher.update(path.to_string_lossy().as_bytes());
	let digest = hasher.finalize();
	format!("{digest:x}")[..12].to_string()
}

#[tokio::test]
async fn infra_up_writes_state_for_started_services() {
	let temp = TempDir::new().unwrap();
	let docker = FakeDockerEngine::new(vec![]);

	let config = LocalInfraConfig::derive(
		"caller-supplied-project",
		"local",
		Some(DatabaseInfraInput {
			engine: "postgresql".to_string(),
			host: "localhost".to_string(),
			port: 5432,
			name: "app".to_string(),
			user: "postgres".to_string(),
			password: Some("postgres".to_string()),
		}),
		None,
	)
	.unwrap();

	InfraCommand::up_with_config(temp.path(), config, docker)
		.await
		.unwrap();

	let state = StateStore::new(temp.path()).load().unwrap().unwrap();
	assert_ne!(state.project_id, "caller-supplied-project");
	assert_eq!(state.services.len(), 1);
	assert_eq!(state.services[0].name, "postgres");
}

#[tokio::test]
async fn infra_up_rejects_invalid_profile_before_docker_operations() {
	let temp = TempDir::new().unwrap();
	let docker = FakeDockerEngine::new(vec![]);
	let config = LocalInfraConfig::derive(
		"caller-supplied-project",
		"qa.eu",
		Some(DatabaseInfraInput {
			engine: "postgresql".to_string(),
			host: "localhost".to_string(),
			port: 5432,
			name: "app".to_string(),
			user: "postgres".to_string(),
			password: Some("postgres".to_string()),
		}),
		None,
	)
	.unwrap();

	let result = InfraCommand::up_with_config(temp.path(), config, docker.clone()).await;

	assert!(result.is_err());
	assert_eq!(docker.calls(), Vec::new());
}

#[tokio::test]
async fn infra_status_rejects_host_port_that_differs_from_docker_binding() {
	let temp = TempDir::new().unwrap();
	let store = StateStore::new(temp.path());
	store
		.save(&LocalInfraState {
			project_id: project_id(temp.path()),
			profile: "local".to_string(),
			services: vec![LocalServiceState {
				name: "postgres".to_string(),
				container_name: format!("reinhardt-{}-local-postgres", project_id(temp.path())),
				image: "postgres:17-alpine".to_string(),
				host: "127.0.0.1".to_string(),
				host_port: 55432,
				container_port: 5432,
				status: ServiceRuntimeStatus::Running,
				metadata: serde_json::json!({"database": "app", "user": "postgres"}),
			}],
		})
		.unwrap();
	let docker = FakeDockerEngine::new(vec![]).with_port_bindings(vec![Some(55433)]);

	let result = InfraCommand::execute_with_runner(
		reinhardt_commands::local_infra::InfraSubcommand::Status {
			profile: None,
			json: false,
		},
		temp.path(),
		docker,
	)
	.await;

	assert!(result.is_err());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailingDockerOperation {
	Remove,
	Run,
	Binding,
}

#[derive(Debug, Clone)]
struct FailingDockerEngine {
	calls: Arc<Mutex<Vec<DockerCall>>>,
	failing_operation: FailingDockerOperation,
}

impl FailingDockerEngine {
	fn new(failing_operation: FailingDockerOperation) -> Self {
		Self {
			calls: Arc::new(Mutex::new(Vec::new())),
			failing_operation,
		}
	}

	fn calls(&self) -> Vec<DockerCall> {
		self.calls.lock().expect("calls lock").clone()
	}

	fn record(&self, call: DockerCall) {
		self.calls.lock().expect("calls lock").push(call);
	}
}

#[async_trait]
impl DockerEngine for FailingDockerEngine {
	async fn container_exists(&self, name: &str) -> Result<bool, DockerError> {
		self.record(DockerCall::ContainerExists {
			name: name.to_string(),
		});
		Ok(false)
	}

	async fn container_port_binding(
		&self,
		name: &str,
		container_port: u16,
	) -> Result<Option<u16>, DockerError> {
		self.record(DockerCall::ContainerPortBinding {
			name: name.to_string(),
			container_port,
		});
		if self.failing_operation == FailingDockerOperation::Binding {
			return Err(DockerError::Backend("scripted binding failure".to_string()));
		}
		Ok(Some(55432))
	}

	async fn remove_container(&self, name: &str) -> Result<(), DockerError> {
		self.record(DockerCall::RemoveContainer {
			name: name.to_string(),
		});
		if self.failing_operation == FailingDockerOperation::Remove {
			return Err(DockerError::Backend("scripted removal failure".to_string()));
		}
		Ok(())
	}

	async fn run_detached(&self, spec: DockerRunSpec) -> Result<(), DockerError> {
		self.record(DockerCall::RunDetached { spec });
		if self.failing_operation == FailingDockerOperation::Run {
			return Err(DockerError::Backend("scripted run failure".to_string()));
		}
		Ok(())
	}
}

#[tokio::test]
async fn infra_up_stops_after_remove_failure_without_persisting_state() {
	let temp = TempDir::new().expect("create temporary project");
	let docker = FailingDockerEngine::new(FailingDockerOperation::Remove);

	let error = InfraCommand::up_with_config(temp.path(), postgres_config(), docker.clone())
		.await
		.expect_err("removal failures must stop provisioning");

	assert_eq!(error.to_string(), "scripted removal failure");
	assert_eq!(
		docker.calls(),
		vec![DockerCall::RemoveContainer {
			name: postgres_container_name(temp.path()),
		}]
	);
	assert!(
		StateStore::new(temp.path())
			.load()
			.expect("read state")
			.is_none()
	);
}

#[tokio::test]
async fn infra_up_stops_after_run_failure_without_persisting_state() {
	let temp = TempDir::new().expect("create temporary project");
	let docker = FailingDockerEngine::new(FailingDockerOperation::Run);

	let error = InfraCommand::up_with_config(temp.path(), postgres_config(), docker.clone())
		.await
		.expect_err("run failures must stop provisioning");

	assert_eq!(error.to_string(), "scripted run failure");
	assert_eq!(
		docker.calls(),
		vec![
			DockerCall::RemoveContainer {
				name: postgres_container_name(temp.path()),
			},
			DockerCall::RunDetached {
				spec: DockerRunSpec {
					name: postgres_container_name(temp.path()),
					image: "postgres:17-alpine".to_string(),
					host_port: 55432,
					container_port: 5432,
					env: vec![
						("POSTGRES_USER".to_string(), "postgres".to_string()),
						("POSTGRES_PASSWORD".to_string(), "postgres".to_string()),
						("POSTGRES_DB".to_string(), "app".to_string()),
					],
				},
			},
		]
	);
	assert!(
		StateStore::new(temp.path())
			.load()
			.expect("read state")
			.is_none()
	);
}

#[tokio::test]
async fn infra_down_stops_after_first_removal_failure_and_retains_state() {
	let temp = TempDir::new().expect("create temporary project");
	let state = valid_state(temp.path());
	StateStore::new(temp.path())
		.save(&state)
		.expect("save state");
	let docker = FailingDockerEngine::new(FailingDockerOperation::Remove);

	let error = InfraCommand::execute_with_runner(
		InfraSubcommand::Down { profile: None },
		temp.path(),
		docker.clone(),
	)
	.await
	.expect_err("removal failure must stop down");

	assert_eq!(error.to_string(), "scripted removal failure");
	assert_eq!(
		docker.calls(),
		vec![DockerCall::RemoveContainer {
			name: postgres_container_name(temp.path()),
		}]
	);
	assert_eq!(
		StateStore::new(temp.path()).load().expect("read state"),
		Some(state)
	);
}

#[tokio::test]
async fn infra_status_reports_the_service_when_binding_lookup_fails() {
	let temp = TempDir::new().expect("create temporary project");
	StateStore::new(temp.path())
		.save(&valid_state(temp.path()))
		.expect("save state");
	let docker = FailingDockerEngine::new(FailingDockerOperation::Binding);

	let error = InfraCommand::execute_with_runner(
		InfraSubcommand::Status {
			profile: None,
			json: true,
		},
		temp.path(),
		docker.clone(),
	)
	.await
	.expect_err("binding lookup failures must name the service");

	assert_eq!(
		error.to_string(),
		"failed to inspect local infrastructure service `postgres` binding: scripted binding failure"
	);
	assert_eq!(
		docker.calls(),
		vec![DockerCall::ContainerPortBinding {
			name: postgres_container_name(temp.path()),
			container_port: 5432,
		}]
	);
}

#[rstest]
#[case::wrong_project_id(
	|state: &mut LocalInfraState| state.project_id = "other-project".to_string(),
	"project identifier does not match this workspace"
)]
#[case::invalid_profile(
	|state: &mut LocalInfraState| state.profile = "qa.eu".to_string(),
	"profile is invalid"
)]
#[case::unknown_service(
	|state: &mut LocalInfraState| state.services[0].name = "mysql".to_string(),
	"service name is invalid"
)]
#[case::duplicate_service(
	|state: &mut LocalInfraState| state.services.push(state.services[0].clone()),
	"service is duplicated"
)]
#[case::wrong_image(
	|state: &mut LocalInfraState| state.services[0].image = "postgres:latest".to_string(),
	"service does not match local infrastructure configuration"
)]
#[case::remote_host(
	|state: &mut LocalInfraState| state.services[0].host = "203.0.113.10".to_string(),
	"service does not match local infrastructure configuration"
)]
#[case::zero_host_port(
	|state: &mut LocalInfraState| state.services[0].host_port = 0,
	"service does not match local infrastructure configuration"
)]
#[case::wrong_container_port(
	|state: &mut LocalInfraState| state.services[0].container_port = 5433,
	"service does not match local infrastructure configuration"
)]
#[case::unstable_container_name(
	|state: &mut LocalInfraState| state.services[0].container_name = "unrelated-container".to_string(),
	"service does not match local infrastructure configuration"
)]
#[case::missing_postgres_database(
	|state: &mut LocalInfraState| state.services[0].metadata = serde_json::json!({"user": "postgres"}),
	"service does not match local infrastructure configuration"
)]
#[case::missing_postgres_user(
	|state: &mut LocalInfraState| state.services[0].metadata = serde_json::json!({"database": "app"}),
	"service does not match local infrastructure configuration"
)]
#[case::redis_database_above_u16_max(
	|state: &mut LocalInfraState| {
		state.services[0] = LocalServiceState {
			name: "redis".to_string(),
			container_name: format!("reinhardt-{}-local-redis", state.project_id),
			image: "redis:7-alpine".to_string(),
			host: "127.0.0.1".to_string(),
			host_port: 56379,
			container_port: 6379,
			status: ServiceRuntimeStatus::Running,
			metadata: serde_json::json!({"database": u32::from(u16::MAX) + 1}),
		};
	},
	"service does not match local infrastructure configuration"
)]
fn state_store_rejects_every_invalid_persisted_state_mutation(
	#[case] mutate: fn(&mut LocalInfraState),
	#[case] category: &str,
) {
	let temp = TempDir::new().expect("create temporary project");
	let store = StateStore::new(temp.path());
	let mut state = valid_state(temp.path());
	mutate(&mut state);
	std::fs::create_dir_all(store.path().parent().expect("state directory"))
		.expect("create state directory");
	std::fs::write(
		store.path(),
		serde_json::to_vec(&state).expect("serialize state"),
	)
	.expect("write tampered state");

	let error = store.load().expect_err("tampered state must be rejected");

	assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
	assert_eq!(
		error.to_string(),
		format!("local infrastructure state {category}")
	);
}

#[test]
fn state_store_save_failure_preserves_existing_valid_state() {
	let temp = TempDir::new().expect("create temporary project");
	let store = StateStore::new(temp.path());
	let original = valid_state(temp.path());
	store.save(&original).expect("save original state");
	std::fs::create_dir(store.path().with_extension("json.tmp"))
		.expect("create temporary-file failure sentinel");
	let mut replacement = valid_state(temp.path());
	replacement.profile = "staging".to_string();
	replacement.services[0].container_name =
		format!("reinhardt-{}-staging-postgres", project_id(temp.path()));

	let error = store
		.save(&replacement)
		.expect_err("temporary write must fail");

	assert_eq!(error.kind(), std::io::ErrorKind::IsADirectory);
	assert_eq!(store.load().expect("reload original state"), Some(original));
}

fn postgres_config() -> LocalInfraConfig {
	LocalInfraConfig::derive(
		"caller-supplied-project",
		"local",
		Some(DatabaseInfraInput {
			engine: "postgresql".to_string(),
			host: "localhost".to_string(),
			port: 55432,
			name: "app".to_string(),
			user: "postgres".to_string(),
			password: Some("postgres".to_string()),
		}),
		None,
	)
	.expect("derive postgres configuration")
}

fn valid_state(project_root: &std::path::Path) -> LocalInfraState {
	LocalInfraState {
		project_id: project_id(project_root),
		profile: "local".to_string(),
		services: vec![LocalServiceState {
			name: "postgres".to_string(),
			container_name: postgres_container_name(project_root),
			image: "postgres:17-alpine".to_string(),
			host: "127.0.0.1".to_string(),
			host_port: 55432,
			container_port: 5432,
			status: ServiceRuntimeStatus::Running,
			metadata: serde_json::json!({"database": "app", "user": "postgres"}),
		}],
	}
}

fn postgres_container_name(project_root: &std::path::Path) -> String {
	format!("reinhardt-{}-local-postgres", project_id(project_root))
}
