//! Deterministic contract verification and Cargo replay.

use crate::{CommandError, CommandResult};
use reinhardt_conf::settings::schema::SettingsPathSegment;
use reinhardt_conf::settings::{
	ComposedSettings, PendingSettings, SettingsViolation, verify_settings_contract,
};
use reinhardt_core::endpoint::{EndpointSecurityViolation, collect_endpoint_security_violations};
use reinhardt_db::migrations::{SchemaCheckError, SchemaFinding, verify_schema_contract};
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;

/// The Cargo profile used for the replay check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CargoProfile {
	/// The normal development profile.
	Dev,
	/// Cargo's optimized release profile.
	Release,
	/// A named profile declared by the consumer project.
	Named(String),
}

impl CargoProfile {
	/// Convert Cargo's `PROFILE` value into the replay profile.
	pub fn from_name(name: impl AsRef<str>) -> Self {
		match name.as_ref() {
			"debug" | "dev" => Self::Dev,
			"release" => Self::Release,
			name => Self::Named(name.to_owned()),
		}
	}
}

/// A reason why Cargo configuration cannot be replayed safely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CargoReplayUnsupported {
	/// A required launcher value was not emitted.
	MissingContext,
	/// The launcher recorded an unsupported Cargo configuration.
	UnsupportedConfiguration,
}

/// Cargo configuration replay captured by the consumer build script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CargoConfigReplay {
	/// The encoded rustflags can be applied exactly.
	Exact {
		/// Effective rustflags supplied by Cargo, when present.
		encoded_rustflags: Option<String>,
	},
	/// The build used a configuration this command must not guess.
	Unsupported {
		/// Stable reason for the refusal.
		reason: CargoReplayUnsupported,
	},
}

/// Compile-time Cargo context supplied by a generated management launcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CargoCheckContext {
	/// Every feature enabled for the management binary.
	pub enabled_features: Vec<String>,
	/// Target triple used by the consumer build.
	pub target: Option<String>,
	/// Effective Cargo profile.
	pub profile: CargoProfile,
	/// Consumer manifest path.
	pub manifest_path: PathBuf,
	/// Consumer package, when the manifest contains multiple packages.
	pub package: Option<String>,
	/// Management binary name.
	pub binary: Option<String>,
	/// Effective Cargo configuration replay.
	pub config_replay: CargoConfigReplay,
}

impl CargoCheckContext {
	/// Read launcher-provided `REINHARDT_*` values without guessing project data.
	pub fn from_launcher(
		manifest_path: impl Into<PathBuf>,
		package: Option<String>,
		binary: Option<String>,
	) -> Self {
		let mut enabled_features: Vec<_> = env::var("REINHARDT_ENABLED_FEATURES")
			.unwrap_or_default()
			.split(',')
			.filter(|feature| !feature.is_empty())
			.map(str::to_owned)
			.collect();
		enabled_features.sort();
		enabled_features.dedup();
		let config_replay = match env::var("REINHARDT_CARGO_REPLAY") {
			Ok(value) if value == "unsupported" => CargoConfigReplay::Unsupported {
				reason: CargoReplayUnsupported::UnsupportedConfiguration,
			},
			_ => CargoConfigReplay::Exact {
				encoded_rustflags: env::var("REINHARDT_ENCODED_RUSTFLAGS").ok(),
			},
		};
		Self {
			enabled_features,
			target: env::var("REINHARDT_TARGET").ok(),
			profile: CargoProfile::from_name(
				env::var("REINHARDT_PROFILE").unwrap_or_else(|_| "debug".to_owned()),
			),
			manifest_path: manifest_path.into(),
			package,
			binary,
			config_replay,
		}
	}
}

/// A pure process invocation plan for `cargo check`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CargoCheckPlan {
	/// Executable to launch.
	pub program: String,
	/// Exact Cargo arguments in order.
	pub args: Vec<String>,
	/// Environment overrides to apply to the child.
	pub environment: Vec<(String, String)>,
}

/// Build a deterministic Cargo check invocation without spawning a process.
pub fn plan_cargo_check(context: &CargoCheckContext) -> CommandResult<CargoCheckPlan> {
	if context.manifest_path.as_os_str().is_empty() {
		return Err(CommandError::ExecutionError(
			"Cargo replay context is incomplete".to_owned(),
		));
	}
	let CargoConfigReplay::Exact { encoded_rustflags } = &context.config_replay else {
		return Err(CommandError::ExecutionError(
			"Cargo replay configuration is unsupported".to_owned(),
		));
	};
	let mut features = context.enabled_features.clone();
	features.sort();
	features.dedup();
	let mut args = vec!["check".to_owned(), "--no-default-features".to_owned()];
	if !features.is_empty() {
		args.extend(["--features".to_owned(), features.join(",")]);
	}
	if let Some(target) = &context.target {
		args.extend(["--target".to_owned(), target.clone()]);
	}
	match &context.profile {
		CargoProfile::Dev => {}
		CargoProfile::Release => args.push("--release".to_owned()),
		CargoProfile::Named(profile) => {
			args.extend(["--profile".to_owned(), profile.clone()]);
		}
	}
	args.extend([
		"--manifest-path".to_owned(),
		context.manifest_path.to_string_lossy().into_owned(),
	]);
	if let Some(package) = &context.package {
		args.extend(["--package".to_owned(), package.clone()]);
	}
	if let Some(binary) = &context.binary {
		args.extend(["--bin".to_owned(), binary.clone()]);
	}
	let environment = encoded_rustflags
		.as_ref()
		.map(|flags| vec![("CARGO_ENCODED_RUSTFLAGS".to_owned(), flags.clone())])
		.unwrap_or_default();
	Ok(CargoCheckPlan {
		program: "cargo".to_owned(),
		args,
		environment,
	})
}

/// A finding from one of the independent contract validators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationFinding {
	/// Schema drift or unapplied migration.
	Schema(SchemaFinding),
	/// Endpoint without an explicit authentication decision.
	Authorization(EndpointSecurityViolation),
	/// Value-free settings violation.
	Settings(SettingsViolation),
}

/// A check that could not be completed safely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationCheckError {
	/// Schema validator could not infer a safe result.
	Schema(SchemaCheckError),
	/// Contract aggregate resolution failed.
	Resolution(crate::ContractResolutionErrorKind),
}

/// Collected verification output before rendering.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerificationRun {
	/// Deterministically sorted findings.
	pub findings: Vec<VerificationFinding>,
	/// Deterministically sorted check errors.
	pub check_errors: Vec<VerificationCheckError>,
}

impl VerificationRun {
	/// Sort findings and errors without using runtime values as ordering keys.
	pub fn sort_canonical(&mut self) {
		self.findings.sort_by_key(verification_finding_key);
		self.check_errors.sort_by_key(verification_error_key);
	}
}

fn verification_finding_key(finding: &VerificationFinding) -> (u8, String) {
	match finding {
		VerificationFinding::Schema(value) => (0, format!("{value:?}")),
		VerificationFinding::Authorization(value) => (
			1,
			format!("{} {} {}", value.method, value.path, value.function_name),
		),
		VerificationFinding::Settings(value) => (2, {
			let path = value
				.path
				.segments()
				.iter()
				.map(|segment| match segment {
					SettingsPathSegment::DynamicKey(_) => "*",
					SettingsPathSegment::Key(key) => key,
					SettingsPathSegment::AnyKey => "*",
					SettingsPathSegment::AnyIndex => "*",
				})
				.collect::<Vec<_>>()
				.join(".");
			format!("{}:{}", value.kind.code(), path)
		}),
	}
}

fn verification_error_key(error: &VerificationCheckError) -> (u8, String) {
	match error {
		VerificationCheckError::Schema(error) => (0, format!("{error:?}")),
		VerificationCheckError::Resolution(error) => (1, format!("{error:?}")),
	}
}

/// Run Cargo replay and all independent contract validators.
pub async fn execute_verify<S: ComposedSettings>(
	cargo: &CargoCheckContext,
	pending: &PendingSettings<S>,
	stdout: &mut dyn Write,
	_stderr: &mut dyn Write,
) -> CommandResult<()> {
	let plan = plan_cargo_check(cargo)?;
	let mut command = tokio::process::Command::new(&plan.program);
	command
		.args(&plan.args)
		.stdout(Stdio::null())
		.stderr(Stdio::null());
	for (key, value) in &plan.environment {
		command.env(key, value);
	}
	let status = command
		.status()
		.await
		.map_err(|_| CommandError::ExecutionError("cargo check could not be started".to_owned()))?;
	if !status.success() {
		return Err(CommandError::ExecutionError(
			"cargo check failed; contract verification was not run".to_owned(),
		));
	}

	let mut run = VerificationRun::default();
	let aggregate = match crate::resolve_contract_state(pending, None).await {
		Ok(aggregate) => aggregate,
		Err(error) => {
			run.check_errors
				.push(VerificationCheckError::Resolution(error.kind));
			run.sort_canonical();
			render_verification(&run, stdout)?;
			return Err(CommandError::ExecutionError(
				"contract verification could not complete".to_owned(),
			));
		}
	};
	let schema = verify_schema_contract(&aggregate.schema);
	run.findings
		.extend(schema.findings.into_iter().map(VerificationFinding::Schema));
	run.check_errors.extend(
		schema
			.check_errors
			.into_iter()
			.map(VerificationCheckError::Schema),
	);
	run.findings.extend(
		collect_endpoint_security_violations(&aggregate.registered_endpoints)
			.into_iter()
			.map(VerificationFinding::Authorization),
	);
	run.findings.extend(
		verify_settings_contract(
			&aggregate.settings.root_schema,
			&aggregate.settings.merged,
			aggregate.settings.typed_coercion,
		)
		.into_iter()
		.map(VerificationFinding::Settings),
	);
	run.sort_canonical();
	render_verification(&run, stdout)?;
	if run.findings.is_empty() && run.check_errors.is_empty() {
		stdout.write_all(b"Verification passed.\n")?;
		return Ok(());
	}
	Err(CommandError::ExecutionError(
		"contract verification found issues".to_owned(),
	))
}

fn render_verification(run: &VerificationRun, stdout: &mut dyn Write) -> CommandResult<()> {
	for error in &run.check_errors {
		writeln!(stdout, "error: {}", render_check_error(error))?;
	}
	for finding in &run.findings {
		writeln!(stdout, "finding: {}", render_finding(finding))?;
	}
	Ok(())
}

fn render_check_error(error: &VerificationCheckError) -> &'static str {
	match error {
		VerificationCheckError::Schema(SchemaCheckError::OpaqueMigrationState) => {
			"schema check unavailable (opaque migration state)"
		}
		VerificationCheckError::Schema(SchemaCheckError::Autodetector { .. }) => {
			"schema check unavailable (autodetector)"
		}
		VerificationCheckError::Resolution(_) => "contract state resolution unavailable",
	}
}

fn render_finding(finding: &VerificationFinding) -> String {
	match finding {
		VerificationFinding::Schema(SchemaFinding::MissingMigration {
			app_label,
			name_fragment,
			description,
		}) => format!("schema missing migration {app_label}:{name_fragment} ({description})"),
		VerificationFinding::Schema(SchemaFinding::UnappliedMigration { migration }) => {
			format!(
				"schema unapplied migration {}:{}",
				migration.app_label, migration.name
			)
		}
		VerificationFinding::Authorization(value) => format!(
			"authorization {} {} ({}/{})",
			value.method, value.path, value.module_path, value.function_name
		),
		VerificationFinding::Settings(value) => {
			let path = value
				.path
				.segments()
				.iter()
				.map(|segment| match segment {
					SettingsPathSegment::DynamicKey(_) => "*".to_owned(),
					SettingsPathSegment::Key(key) => (*key).to_owned(),
					SettingsPathSegment::AnyKey | SettingsPathSegment::AnyIndex => "*".to_owned(),
				})
				.collect::<Vec<_>>()
				.join(".");
			format!("{} at {}", value.kind.code(), path)
		}
	}
}
