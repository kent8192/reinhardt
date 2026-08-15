//! Deterministic contract verification and Cargo replay.

use crate::{CommandError, CommandResult, ContractResolutionErrorKind, SafeContractTarget};
use reinhardt_conf::settings::schema::{JsonKind, SettingsPathBuf, SettingsPathSegment};
use reinhardt_conf::settings::{
	ComposedSettings, PendingSettings, SettingsViolation, verify_settings_contract,
};
use reinhardt_core::endpoint::{EndpointSecurityViolation, collect_endpoint_security_violations};
use reinhardt_db::migrations::{
	SchemaCheckError, SchemaFinding, verification::SchemaContractState, verify_schema_contract,
};
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
	/// Every effective build override can be applied exactly.
	Exact {
		/// Effective rustflags supplied by Cargo, when present.
		encoded_rustflags: Option<String>,
		/// Effective Rust compiler wrapper, when present.
		rustc_wrapper: Option<String>,
		/// Effective workspace Rust compiler wrapper, when present.
		rustc_workspace_wrapper: Option<String>,
		/// Effective target linker, when present.
		rustc_linker: Option<String>,
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
		let enabled_features_value = env::var("REINHARDT_ENABLED_FEATURES").ok();
		let target = env::var("REINHARDT_TARGET").ok();
		let profile = env::var("REINHARDT_PROFILE").ok();
		let replay = env::var("REINHARDT_CARGO_REPLAY").ok();
		let mut enabled_features: Vec<_> = enabled_features_value
			.clone()
			.unwrap_or_default()
			.split(',')
			.filter(|feature| !feature.is_empty())
			.map(str::to_owned)
			.collect();
		enabled_features.sort();
		enabled_features.dedup();
		let missing_context = enabled_features_value.is_none()
			|| target.is_none()
			|| profile.is_none()
			|| replay.is_none();
		let config_replay = match replay.as_deref() {
			Some("unsupported") => CargoConfigReplay::Unsupported {
				reason: CargoReplayUnsupported::UnsupportedConfiguration,
			},
			Some("exact") if !missing_context => CargoConfigReplay::Exact {
				encoded_rustflags: env::var("REINHARDT_ENCODED_RUSTFLAGS").ok(),
				rustc_wrapper: env::var("REINHARDT_RUSTC_WRAPPER").ok(),
				rustc_workspace_wrapper: env::var("REINHARDT_RUSTC_WORKSPACE_WRAPPER").ok(),
				rustc_linker: env::var("REINHARDT_RUSTC_LINKER").ok(),
			},
			_ => CargoConfigReplay::Unsupported {
				reason: CargoReplayUnsupported::MissingContext,
			},
		};
		Self {
			enabled_features,
			target,
			profile: CargoProfile::from_name(profile.unwrap_or_default()),
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
	/// Consumer project root used for Cargo configuration discovery.
	pub working_directory: PathBuf,
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
	if context
		.manifest_path
		.file_name()
		.and_then(|name| name.to_str())
		!= Some("Cargo.toml")
	{
		return Err(CommandError::ExecutionError(
			"Cargo replay manifest path must name Cargo.toml".to_owned(),
		));
	}
	let CargoConfigReplay::Exact {
		encoded_rustflags,
		rustc_wrapper,
		rustc_workspace_wrapper,
		rustc_linker,
	} = &context.config_replay
	else {
		return Err(CommandError::ExecutionError(
			"Cargo replay configuration is unsupported".to_owned(),
		));
	};
	let mut features = context.enabled_features.clone();
	features.sort();
	features.dedup();
	let mut args = vec![
		"check".to_owned(),
		"--quiet".to_owned(),
		"--no-default-features".to_owned(),
	];
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
	let working_directory = context
		.manifest_path
		.parent()
		.filter(|path| !path.as_os_str().is_empty())
		.ok_or_else(|| {
			CommandError::ExecutionError("Cargo replay project root is unavailable".to_owned())
		})?
		.to_path_buf();
	let mut environment = vec![(
		"CARGO_TARGET_DIR".to_owned(),
		working_directory
			.join("target/reinhardt-contract-verify")
			.to_string_lossy()
			.into_owned(),
	)];
	for (key, value) in [
		("CARGO_ENCODED_RUSTFLAGS", encoded_rustflags),
		("RUSTC_WRAPPER", rustc_wrapper),
		("RUSTC_WORKSPACE_WRAPPER", rustc_workspace_wrapper),
		("RUSTC_LINKER", rustc_linker),
	] {
		if let Some(value) = value {
			environment.push((key.to_owned(), value.clone()));
		}
	}
	Ok(CargoCheckPlan {
		program: "cargo".to_owned(),
		args,
		working_directory,
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
	Resolution {
		/// Stable failure category.
		kind: crate::ContractResolutionErrorKind,
		/// Redacted target metadata, when available.
		safe_target: Option<SafeContractTarget>,
	},
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

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct VerificationSortKey {
	class: u8,
	code: &'static str,
	target: String,
	ordinal: usize,
}

fn verification_finding_key(finding: &VerificationFinding) -> VerificationSortKey {
	match finding {
		VerificationFinding::Schema(SchemaFinding::MissingMigration {
			app_label,
			name_fragment,
			description,
		}) => VerificationSortKey {
			class: 0,
			code: "schema.missing_migration",
			target: format!("{app_label}\0{name_fragment}\0{description}"),
			ordinal: 0,
		},
		VerificationFinding::Schema(SchemaFinding::UnappliedMigration { migration }) => {
			VerificationSortKey {
				class: 0,
				code: "schema.unapplied_migration",
				target: format!("{}\0{}", migration.app_label, migration.name),
				ordinal: 0,
			}
		}
		VerificationFinding::Authorization(value) => VerificationSortKey {
			class: 1,
			code: "authorization.missing_declaration",
			target: format!(
				"{}\0{}\0{}\0{}",
				value.method, value.path, value.module_path, value.function_name
			),
			ordinal: 0,
		},
		VerificationFinding::Settings(value) => VerificationSortKey {
			class: 2,
			code: value.kind.code(),
			target: format!(
				"{}\0{}\0{}",
				redacted_settings_path(&value.path),
				value.expected,
				json_kind_rank(value.actual)
			),
			ordinal: value.ordinal,
		},
	}
}

fn verification_error_key(error: &VerificationCheckError) -> VerificationSortKey {
	match error {
		VerificationCheckError::Schema(SchemaCheckError::OpaqueMigrationState) => {
			VerificationSortKey {
				class: 0,
				code: "schema.opaque_migration_state",
				target: String::new(),
				ordinal: 0,
			}
		}
		VerificationCheckError::Schema(SchemaCheckError::Autodetector { app_label }) => {
			VerificationSortKey {
				class: 0,
				code: "schema.autodetector",
				target: app_label.clone().unwrap_or_default(),
				ordinal: 0,
			}
		}
		VerificationCheckError::Resolution { kind, safe_target } => VerificationSortKey {
			class: resolution_class(*kind),
			code: resolution_code(*kind),
			target: match safe_target {
				Some(SafeContractTarget::SettingsSection(section)) => (*section).to_owned(),
				Some(SafeContractTarget::Migrations) => "migrations".to_owned(),
				None => String::new(),
			},
			ordinal: 0,
		},
	}
}

fn resolution_class(kind: ContractResolutionErrorKind) -> u8 {
	match kind {
		ContractResolutionErrorKind::MigrationCatalog
		| ContractResolutionErrorKind::ModelRegistry => 0,
		ContractResolutionErrorKind::RouteTopology => 1,
		ContractResolutionErrorKind::SettingsSource
		| ContractResolutionErrorKind::SettingsSection
		| ContractResolutionErrorKind::SettingsSchema => 2,
		ContractResolutionErrorKind::CargoContext => 3,
	}
}

fn resolution_code(kind: ContractResolutionErrorKind) -> &'static str {
	match kind {
		ContractResolutionErrorKind::CargoContext => "cargo_context",
		ContractResolutionErrorKind::SettingsSource => "settings_source",
		ContractResolutionErrorKind::SettingsSection => "settings_section",
		ContractResolutionErrorKind::SettingsSchema => "settings_schema",
		ContractResolutionErrorKind::MigrationCatalog => "migration_catalog",
		ContractResolutionErrorKind::ModelRegistry => "model_registry",
		ContractResolutionErrorKind::RouteTopology => "route_topology",
	}
}

fn json_kind_rank(kind: Option<JsonKind>) -> u8 {
	match kind {
		None => 0,
		Some(JsonKind::Null) => 1,
		Some(JsonKind::Boolean) => 2,
		Some(JsonKind::Number) => 3,
		Some(JsonKind::String) => 4,
		Some(JsonKind::Sequence) => 5,
		Some(JsonKind::Map) => 6,
	}
}

fn redacted_settings_path(path: &SettingsPathBuf) -> String {
	path.segments()
		.iter()
		.map(|segment| match segment {
			SettingsPathSegment::Key(key) => *key,
			SettingsPathSegment::DynamicKey(_)
			| SettingsPathSegment::AnyKey
			| SettingsPathSegment::AnyIndex => "*",
		})
		.collect::<Vec<_>>()
		.join(".")
}

fn append_schema_checks(run: &mut VerificationRun, schema: &SchemaContractState) {
	let verification = verify_schema_contract(schema);
	run.findings.extend(
		verification
			.findings
			.into_iter()
			.map(VerificationFinding::Schema),
	);
	run.check_errors.extend(
		verification
			.check_errors
			.into_iter()
			.map(VerificationCheckError::Schema),
	);
}

/// Run Cargo replay and all independent contract validators.
pub async fn execute_verify<S: ComposedSettings>(
	cargo: &CargoCheckContext,
	pending: &PendingSettings<S>,
	stdout: &mut dyn Write,
	stderr: &mut dyn Write,
) -> CommandResult<()> {
	execute_cargo_check(cargo, stdout, stderr).await?;
	execute_contract_checks(pending, stdout).await
}

pub(crate) async fn execute_verify_with_provider<S, F>(
	cargo: &CargoCheckContext,
	provider: F,
	stdout: &mut dyn Write,
	stderr: &mut dyn Write,
) -> CommandResult<()>
where
	S: ComposedSettings,
	F: FnOnce() -> Result<PendingSettings<S>, reinhardt_conf::settings::builder::BuildError>,
{
	execute_cargo_check(cargo, stdout, stderr).await?;
	let pending = match provider() {
		Ok(pending) => pending,
		Err(_) => {
			let mut run = VerificationRun {
				findings: Vec::new(),
				check_errors: vec![VerificationCheckError::Resolution {
					kind: ContractResolutionErrorKind::SettingsSource,
					safe_target: None,
				}],
			};
			run.sort_canonical();
			render_verification(&run, stdout)?;
			return Err(CommandError::ExecutionError(
				"contract verification found issues".to_owned(),
			));
		}
	};
	execute_contract_checks(&pending, stdout).await
}

async fn execute_cargo_check(
	cargo: &CargoCheckContext,
	stdout: &mut dyn Write,
	stderr: &mut dyn Write,
) -> CommandResult<()> {
	let plan = plan_cargo_check(cargo)?;
	let mut command = tokio::process::Command::new(&plan.program);
	command
		.args(&plan.args)
		.current_dir(&plan.working_directory)
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	for (key, value) in &plan.environment {
		command.env(key, value);
	}
	let output = command
		.output()
		.await
		.map_err(|_| CommandError::ExecutionError("cargo check could not be started".to_owned()))?;
	stdout.write_all(&output.stdout)?;
	stderr.write_all(&output.stderr)?;
	if !output.status.success() {
		return Err(CommandError::ExecutionError(
			"cargo check failed; contract verification was not run".to_owned(),
		));
	}
	Ok(())
}

async fn execute_contract_checks<S: ComposedSettings>(
	pending: &PendingSettings<S>,
	stdout: &mut dyn Write,
) -> CommandResult<()> {
	let mut run = VerificationRun::default();
	let aggregate = match crate::resolve_contract_state(pending, None).await {
		Ok(aggregate) => aggregate,
		Err(error) => {
			run.check_errors.push(VerificationCheckError::Resolution {
				kind: error.kind,
				safe_target: error.safe_target,
			});
			run.sort_canonical();
			render_verification(&run, stdout)?;
			return Err(CommandError::ExecutionError(
				"contract verification found issues".to_owned(),
			));
		}
	};
	append_schema_checks(&mut run, &aggregate.schema);
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

/// Render sorted verification output without exposing runtime setting values.
pub fn render_verification(run: &VerificationRun, stdout: &mut dyn Write) -> CommandResult<()> {
	for error in &run.check_errors {
		writeln!(stdout, "error: {}", render_check_error(error))?;
	}
	for finding in &run.findings {
		writeln!(stdout, "finding: {}", render_finding(finding))?;
	}
	Ok(())
}

fn render_check_error(error: &VerificationCheckError) -> String {
	match error {
		VerificationCheckError::Schema(SchemaCheckError::OpaqueMigrationState) => {
			"schema check unavailable (opaque migration state)".to_owned()
		}
		VerificationCheckError::Schema(SchemaCheckError::Autodetector { .. }) => {
			"schema check unavailable (autodetector)".to_owned()
		}
		VerificationCheckError::Resolution { kind, safe_target } => {
			let target = match safe_target {
				Some(SafeContractTarget::SettingsSection(section)) => {
					format!("settings section {section}")
				}
				Some(SafeContractTarget::Migrations) => "migrations".to_owned(),
				None => match kind {
					crate::ContractResolutionErrorKind::CargoContext => "cargo context".to_owned(),
					crate::ContractResolutionErrorKind::SettingsSource => {
						"settings source".to_owned()
					}
					crate::ContractResolutionErrorKind::SettingsSection => {
						"settings section".to_owned()
					}
					crate::ContractResolutionErrorKind::SettingsSchema => {
						"settings schema".to_owned()
					}
					crate::ContractResolutionErrorKind::MigrationCatalog => {
						"migration catalog".to_owned()
					}
					crate::ContractResolutionErrorKind::ModelRegistry => {
						"model registry".to_owned()
					}
					crate::ContractResolutionErrorKind::RouteTopology => {
						"route topology".to_owned()
					}
				},
			};
			format!("contract state resolution unavailable ({target})")
		}
	}
}

fn render_finding(finding: &VerificationFinding) -> String {
	match finding {
		VerificationFinding::Schema(SchemaFinding::MissingMigration {
			app_label,
			name_fragment,
			description,
		}) => format!("schema.missing_migration {app_label}:{name_fragment} ({description})"),
		VerificationFinding::Schema(SchemaFinding::UnappliedMigration { migration }) => {
			format!(
				"schema.unapplied_migration {}:{}",
				migration.app_label, migration.name
			)
		}
		VerificationFinding::Authorization(value) => format!(
			"authorization.missing_declaration {} {} ({}/{})",
			value.method, value.path, value.module_path, value.function_name
		),
		VerificationFinding::Settings(value) => {
			format!(
				"{} at {} expected={} actual={:?} ordinal={}",
				value.kind.code(),
				redacted_settings_path(&value.path),
				value.expected,
				value.actual,
				value.ordinal
			)
		}
	}
}
