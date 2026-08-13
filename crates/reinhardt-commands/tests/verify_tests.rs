//! Deterministic Cargo replay planning tests.

#![cfg(feature = "contract")]

use reinhardt_commands::{
	CargoCheckContext, CargoCheckPlan, CargoConfigReplay, CargoProfile, CargoReplayUnsupported,
	plan_cargo_check,
};
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
		},
	}
}

#[test]
fn plan_normalizes_features_and_replays_profile_and_context() {
	let plan = plan_cargo_check(&context()).expect("context should be replayable");
	assert_eq!(
		plan,
		CargoCheckPlan {
			program: "cargo".to_owned(),
			args: vec![
				"check",
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
			.collect(),
			environment: vec![(
				"CARGO_ENCODED_RUSTFLAGS".to_owned(),
				"-C\u{1f}debuginfo=1".to_owned(),
			)],
		}
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
