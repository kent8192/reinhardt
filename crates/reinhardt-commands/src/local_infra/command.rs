//! Command definitions for local infrastructure management.

use clap::Subcommand;
use reinhardt_conf::HasCommonSettings;
use std::error::Error;
use std::path::Path;
use std::process::Command;

use super::state::project_id;
use super::{
	BollardDockerEngine, DatabaseInfraInput, DockerEngine, DockerRunSpec, LocalInfraConfig,
	LocalInfraState, PortAllocator, ServiceSpec, StateStore,
};

/// Management subcommands for local infrastructure.
#[derive(Debug, Clone, Subcommand)]
pub enum InfraSubcommand {
	/// Start local infrastructure containers.
	Up {
		/// Settings profile to resolve before deriving services.
		#[arg(long)]
		profile: Option<String>,
		/// Print machine-readable JSON output.
		#[arg(long)]
		json: bool,
		/// Print shell-compatible environment assignments.
		#[arg(long = "print-env")]
		print_env: bool,
	},
	/// Stop local infrastructure containers.
	Down {
		/// Settings profile whose state should be stopped.
		#[arg(long)]
		profile: Option<String>,
	},
	/// Stop and recreate local infrastructure containers.
	Reset {
		/// Settings profile to reset.
		#[arg(long)]
		profile: Option<String>,
	},
	/// Show local infrastructure status.
	Status {
		/// Settings profile whose state should be inspected.
		#[arg(long)]
		profile: Option<String>,
		/// Print machine-readable JSON output.
		#[arg(long)]
		json: bool,
	},
	/// Run a management command with local infrastructure settings applied.
	Run {
		/// Command and arguments to dispatch after `--`.
		#[arg(last = true, required = true)]
		command: Vec<String>,
	},
}

/// Executor for the `manage infra` command group.
#[derive(Debug, Default, Clone, Copy)]
pub struct InfraCommand;

impl InfraCommand {
	/// Execute an infrastructure command with the local Docker Engine API.
	pub async fn execute(
		command: InfraSubcommand,
		project_root: &Path,
		settings: Option<&dyn HasCommonSettings>,
	) -> Result<(), Box<dyn Error>> {
		let docker = BollardDockerEngine::local()?;
		match command {
			InfraSubcommand::Up {
				profile,
				json,
				print_env,
			} => {
				let config = derive_config(project_root, profile, settings)?;
				let state = Self::up_with_config(project_root, config, docker).await?;
				print_up_result(&state, json, print_env, settings)?;
				Ok(())
			}
			InfraSubcommand::Reset { profile } => {
				Self::execute_with_runner(
					InfraSubcommand::Down {
						profile: profile.clone(),
					},
					project_root,
					docker.clone(),
				)
				.await?;
				let config = derive_config(project_root, profile, settings)?;
				Self::up_with_config(project_root, config, docker)
					.await
					.map(|_| ())
			}
			InfraSubcommand::Run { command } => {
				Self::run_with_local_env(project_root, command, settings)
			}
			other => Self::execute_with_runner(other, project_root, docker).await,
		}
	}

	/// Execute an infrastructure command with an injected Docker engine.
	pub async fn execute_with_runner<R>(
		command: InfraSubcommand,
		project_root: &Path,
		docker: R,
	) -> Result<(), Box<dyn Error>>
	where
		R: DockerEngine,
	{
		let store = StateStore::new(project_root);

		match command {
			InfraSubcommand::Down { profile } => {
				if let Some(state) =
					store.load_for_profile(&resolved_profile(profile.as_deref()))?
				{
					for service in state.services {
						docker.remove_container(&service.container_name).await?;
					}
				}
				store.remove()?;
				Ok(())
			}
			InfraSubcommand::Status { profile, json } => {
				let state = store.load_for_profile(&resolved_profile(profile.as_deref()))?;
				if json {
					println!("{}", serde_json::to_string_pretty(&state)?);
				} else if let Some(state) = state {
					for service in state.services {
						let status = if docker.container_exists(&service.container_name).await? {
							"running"
						} else {
							"missing"
						};
						println!("{} {} {}", service.name, service.container_name, status);
					}
				} else {
					println!("No local infrastructure state found.");
				}
				Ok(())
			}
			InfraSubcommand::Up { .. } | InfraSubcommand::Reset { .. } => {
				Err("infra up/reset require resolved settings".into())
			}
			InfraSubcommand::Run { .. } => {
				Err("infra run requires resolved settings for secret interpolation".into())
			}
		}
	}

	/// Start services from a pre-derived local infrastructure config.
	pub async fn up_with_config<R>(
		project_root: &Path,
		config: LocalInfraConfig,
		runner: R,
	) -> Result<LocalInfraState, Box<dyn Error>>
	where
		R: DockerEngine,
	{
		let docker = runner;
		let ports = PortAllocator;
		let mut states = Vec::new();

		for service in &config.services {
			let host_port = ports.select_port(service.requested_port())?;
			let container_name = format!(
				"reinhardt-{}-{}-{}",
				config.project_id,
				config.profile,
				service.name()
			);
			let env = match service {
				ServiceSpec::Postgres(pg) => vec![
					("POSTGRES_USER", pg.user.as_str()),
					(
						"POSTGRES_PASSWORD",
						pg.password.as_deref().unwrap_or("postgres"),
					),
					("POSTGRES_DB", pg.database.as_str()),
				],
				ServiceSpec::Redis(_) => Vec::new(),
			};
			docker.remove_container(&container_name).await?;
			docker
				.run_detached(DockerRunSpec {
					name: container_name.clone(),
					image: service.image().to_string(),
					host_port,
					container_port: service.container_port(),
					env: env
						.into_iter()
						.map(|(key, value)| (key.to_string(), value.to_string()))
						.collect(),
				})
				.await?;
			states.push(service.to_state(container_name, host_port));
		}

		let state = LocalInfraState {
			project_id: config.project_id,
			profile: config.profile,
			services: states,
		};
		StateStore::new(project_root).save(&state)?;
		Ok(state)
	}

	fn run_with_local_env(
		project_root: &Path,
		args: Vec<String>,
		settings: Option<&dyn HasCommonSettings>,
	) -> Result<(), Box<dyn Error>> {
		Self::validate_run_command(&args)?;
		let state = StateStore::new(project_root)
			.load_for_profile(&resolved_profile(None))?
			.ok_or("local infrastructure state does not exist; run `manage infra up` first")?;
		let current_exe = std::env::current_exe()?;
		let status = Command::new(current_exe)
			.args(args)
			.envs(Self::environment_from_state(&state, settings)?)
			.status()?;

		if status.success() {
			Ok(())
		} else {
			Err(format!("local infrastructure command exited with {status}").into())
		}
	}

	/// Build process environment overrides from persisted local infrastructure state.
	pub fn environment_from_state(
		state: &LocalInfraState,
		settings: Option<&dyn HasCommonSettings>,
	) -> Result<Vec<(String, String)>, Box<dyn Error>> {
		local_infra_env(state, settings)
	}

	/// Validate a command targeted by `infra run`.
	pub fn validate_run_command(args: &[String]) -> Result<(), Box<dyn Error>> {
		match args.first().map(String::as_str) {
			Some("runserver") => Err(
				"`manage infra run -- runserver` is intentionally unsupported. Run `manage infra up --print-env`, export the printed variables, then run `manage runserver` separately."
					.into(),
			),
			_ => Ok(()),
		}
	}
}

fn local_infra_env(
	state: &LocalInfraState,
	settings: Option<&dyn HasCommonSettings>,
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
	let mut env = Vec::new();

	for service in &state.services {
		match service.name.as_str() {
			"postgres" => {
				let database = service
					.metadata
					.get("database")
					.and_then(serde_json::Value::as_str)
					.unwrap_or("postgres");
				let user = service
					.metadata
					.get("user")
					.and_then(serde_json::Value::as_str)
					.unwrap_or("postgres");
				let password = settings
					.and_then(|settings| settings.core().databases.get("default"))
					.and_then(|database| database.password.as_ref())
					.map(|password| password.expose_secret());
				// codeql[rust/hard-coded-cryptographic-value] -- Local Docker fallback for #5300, not a production credential.
				let password = password.unwrap_or("postgres");
				env.push((
					"DATABASE_URL".to_string(),
					postgres_url(user, password, &service.host, service.host_port, database)?,
				));
			}
			"redis" => {
				let database = service
					.metadata
					.get("database")
					.and_then(serde_json::Value::as_u64)
					.unwrap_or(0);
				let url = format!(
					"redis://{}:{}/{}",
					service.host, service.host_port, database
				);
				env.push(("REDIS_URL".to_string(), url.clone()));
				env.push(("REINHARDT_REDIS_URL".to_string(), url));
			}
			_ => {}
		}
	}

	Ok(env)
}

fn postgres_url(
	user: &str,
	password: &str,
	host: &str,
	port: u16,
	database: &str,
) -> Result<String, Box<dyn Error>> {
	let mut url = url::Url::parse("postgresql://localhost/")?;
	url.set_username(user)
		.map_err(|_| "postgres URL rejected username")?;
	url.set_password(Some(password))
		.map_err(|_| "postgres URL rejected password")?;
	url.set_host(Some(host))?;
	url.set_port(Some(port))
		.map_err(|_| "postgres URL rejected port")?;
	url.set_path(database);
	Ok(url.to_string())
}

fn derive_config(
	project_root: &Path,
	profile: Option<String>,
	settings: Option<&dyn HasCommonSettings>,
) -> Result<LocalInfraConfig, Box<dyn Error>> {
	let project_id = project_id(project_root);
	let profile = resolved_profile(profile.as_deref());
	let database = settings
		.and_then(|settings| settings.core().databases.get("default"))
		.map(|database| DatabaseInfraInput {
			engine: database.engine.clone(),
			host: database
				.host
				.clone()
				.unwrap_or_else(|| "127.0.0.1".to_string()),
			port: database.port.unwrap_or(5432),
			name: database.name.clone(),
			user: database
				.user
				.clone()
				.unwrap_or_else(|| "postgres".to_string()),
			password: database
				.password
				.as_ref()
				.map(|password| password.expose_secret().to_string()),
		});

	LocalInfraConfig::derive(project_id, profile, database, None).map_err(Into::into)
}

fn resolved_profile(profile: Option<&str>) -> String {
	profile
		.map(ToOwned::to_owned)
		.or_else(|| std::env::var("REINHARDT_ENV").ok())
		.unwrap_or_else(|| "local".to_string())
}

fn print_up_result(
	state: &LocalInfraState,
	json: bool,
	print_env: bool,
	settings: Option<&dyn HasCommonSettings>,
) -> Result<(), Box<dyn Error>> {
	if json {
		println!("{}", serde_json::to_string_pretty(state)?);
	}
	if print_env {
		for (key, value) in local_infra_env(state, settings)? {
			println!("{key}={}", shell_quote(&value));
		}
	}
	if !json && !print_env {
		for service in &state.services {
			println!(
				"{} {}:{} -> {}",
				service.name, service.host, service.host_port, service.container_port
			);
		}
	}
	Ok(())
}

fn shell_quote(value: &str) -> String {
	if value.is_empty() {
		return "''".to_string();
	}
	if value
		.chars()
		.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '@'))
	{
		return value.to_string();
	}
	format!("'{}'", value.replace('\'', "'\\''"))
}
