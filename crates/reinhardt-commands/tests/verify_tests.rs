//! Deterministic Cargo replay planning tests.

#![cfg(feature = "contract")]

use reinhardt_commands::{
	CargoCheckContext, CargoConfigReplay, CargoProfile, CargoReplayUnsupported,
	ContractResolutionErrorKind, SafeContractTarget, VerificationCheckError, VerificationFinding,
	VerificationRun, plan_cargo_check, render_verification,
};
use reinhardt_conf::settings::schema::{
	JsonKind, SettingsPathBuf, SettingsPathSegment, SettingsViolation, SettingsViolationKind,
};
use reinhardt_core::endpoint::EndpointSecurityViolation;
use std::path::PathBuf;

fn context() -> CargoCheckContext {
	CargoCheckContext {
		enabled_features: vec!["zeta".to_owned(), "alpha".to_owned(), "alpha".to_owned()],
		target: Some("aarch64-apple-darwin".to_owned()),
		profile: CargoProfile::Named("ci".to_owned()),
		manifest_path: PathBuf::from("/consumer/Cargo.toml"),
		package: Some("consumer".to_owned()),
		binary: Some("manage".to_owned()),
		config_replay: CargoConfigReplay::Exact {
			encoded_rustflags: Some("-C\u{1f}debuginfo=1".to_owned()),
			rustc_wrapper: Some("sccache".to_owned()),
			rustc_workspace_wrapper: Some("workspace-wrapper".to_owned()),
			rustc_linker: Some("clang".to_owned()),
		},
	}
}

#[test]
fn plan_normalizes_features_and_replays_profile_and_context() {
	let plan = plan_cargo_check(&context()).expect("context should be replayable");
	assert_eq!(plan.program, "cargo");
	assert_eq!(
		plan.args,
		vec![
			"check",
			"--quiet",
			"--no-default-features",
			"--features",
			"alpha,zeta",
			"--target",
			"aarch64-apple-darwin",
			"--profile",
			"ci",
			"--manifest-path",
			"/consumer/Cargo.toml",
			"--package",
			"consumer",
			"--bin",
			"manage",
		]
		.into_iter()
		.map(str::to_owned)
		.collect::<Vec<_>>()
	);
	assert_eq!(plan.working_directory, PathBuf::from("/consumer"));
	assert_eq!(
		plan.environment[0],
		(
			"CARGO_TARGET_DIR".to_owned(),
			"/consumer/target/reinhardt-contract-verify".to_owned(),
		)
	);
	assert_eq!(
		plan.environment[1..],
		[
			(
				"CARGO_ENCODED_RUSTFLAGS".to_owned(),
				"-C\u{1f}debuginfo=1".to_owned(),
			),
			("RUSTC_WRAPPER".to_owned(), "sccache".to_owned()),
			(
				"RUSTC_WORKSPACE_WRAPPER".to_owned(),
				"workspace-wrapper".to_owned(),
			),
			("RUSTC_LINKER".to_owned(), "clang".to_owned()),
		]
	);
}

#[test]
fn plan_release_and_dev_profiles_have_stable_flags() {
	let mut release = context();
	release.profile = CargoProfile::Release;
	release.target = None;
	release.package = None;
	release.binary = None;
	release.config_replay = CargoConfigReplay::Exact {
		encoded_rustflags: None,
		rustc_wrapper: None,
		rustc_workspace_wrapper: None,
		rustc_linker: None,
	};
	let release_plan = plan_cargo_check(&release).expect("release context should be replayable");
	assert!(release_plan.args.contains(&"--release".to_owned()));
	assert!(!release_plan.args.contains(&"--profile".to_owned()));

	let mut dev = release;
	dev.profile = CargoProfile::Dev;
	let dev_plan = plan_cargo_check(&dev).expect("dev context should be replayable");
	assert!(!dev_plan.args.contains(&"--release".to_owned()));
}

#[test]
fn unsupported_replay_has_no_process_plan() {
	let mut context = context();
	context.config_replay = CargoConfigReplay::Unsupported {
		reason: CargoReplayUnsupported::UnsupportedConfiguration,
	};
	let error = plan_cargo_check(&context).expect_err("unsupported context must fail closed");
	assert_eq!(
		error.to_string(),
		"Execution error: Cargo replay configuration is unsupported"
	);
}

#[test]
fn missing_replay_context_has_no_process_plan() {
	let mut context = context();
	context.config_replay = CargoConfigReplay::Unsupported {
		reason: CargoReplayUnsupported::MissingContext,
	};
	let error = plan_cargo_check(&context).expect_err("missing context must fail closed");
	assert_eq!(
		error.to_string(),
		"Execution error: Cargo replay configuration is unsupported"
	);
}

#[test]
fn manifest_directory_is_rejected_before_process_spawn() {
	let mut context = context();
	context.manifest_path = PathBuf::from("/consumer");
	let error = plan_cargo_check(&context).expect_err("directory must not be passed to Cargo");
	assert_eq!(
		error.to_string(),
		"Execution error: Cargo replay manifest path must name Cargo.toml"
	);
}

#[test]
fn rendering_is_redacted_and_canonical() {
	let settings = VerificationFinding::Settings(SettingsViolation {
		kind: SettingsViolationKind::TypeMismatch,
		path: SettingsPathBuf::from_segments([
			SettingsPathSegment::Key("database"),
			SettingsPathSegment::AnyKey,
		]),
		expected: "string",
		actual: Some(JsonKind::Number),
		ordinal: 3,
	});
	let authorization = VerificationFinding::Authorization(EndpointSecurityViolation {
		method: "GET".to_owned(),
		path: "/health".to_owned(),
		module_path: "consumer::routes".to_owned(),
		function_name: "health".to_owned(),
	});
	let mut first = VerificationRun {
		findings: vec![settings.clone(), authorization.clone()],
		check_errors: Vec::new(),
	};
	let mut second = VerificationRun {
		findings: vec![authorization, settings],
		check_errors: Vec::new(),
	};
	first.sort_canonical();
	second.sort_canonical();
	let mut first_output = Vec::new();
	let mut second_output = Vec::new();
	render_verification(&first, &mut first_output).expect("render first run");
	render_verification(&second, &mut second_output).expect("render second run");
	assert_eq!(first_output, second_output);
	let output = String::from_utf8(first_output).expect("UTF-8 output");
	assert_eq!(
		output,
		"finding: authorization.missing_declaration GET /health (consumer::routes/health)\n\
finding: settings.type_mismatch at database.* expected=string actual=Some(Number) ordinal=3\n"
	);
}

#[test]
fn resolution_rendering_keeps_safe_targets_distinct() {
	let mut run = VerificationRun {
		findings: Vec::new(),
		check_errors: vec![
			VerificationCheckError::Resolution {
				kind: ContractResolutionErrorKind::SettingsSection,
				safe_target: Some(SafeContractTarget::SettingsSection("migrations")),
			},
			VerificationCheckError::Resolution {
				kind: ContractResolutionErrorKind::RouteTopology,
				safe_target: None,
			},
		],
	};
	run.sort_canonical();
	let mut rendered = Vec::new();
	render_verification(&run, &mut rendered).expect("render resolution errors");
	let output = String::from_utf8(rendered).expect("UTF-8 output");
	assert_eq!(
		output,
		"error: contract state resolution unavailable (route topology)\n\
error: contract state resolution unavailable (settings section migrations)\n"
	);
}
