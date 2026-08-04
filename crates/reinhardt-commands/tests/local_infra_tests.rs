use reinhardt_commands::local_infra::{
	DatabaseInfraInput, DockerCall, DockerEngine, FakeDockerEngine, InfraCommand, LocalInfraConfig,
	LocalInfraState, LocalServiceState, PortAllocator, RedisInfraInput, ServiceRuntimeStatus,
	StateStore,
};
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
		project_id(temp.path()),
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
	assert_eq!(state.services.len(), 1);
	assert_eq!(state.services[0].name, "postgres");
}
