use contract_verify_consumer::ProjectSettings;
use reinhardt::commands::{
	CargoCheckContext, execute_from_command_line_with_pending_settings_and_cargo_context,
};
use reinhardt::commands::command_error_exit_code;
use reinhardt::conf::settings::builder::SettingsBuilder;

fn settings() -> Result<
	reinhardt::conf::settings::PendingSettings<ProjectSettings>,
	reinhardt::conf::settings::builder::BuildError,
> {
	SettingsBuilder::new()
		.add_source(
			reinhardt::conf::settings::sources::DefaultSource::new()
				.with_value("core", serde_json::json!({
					"base_dir": env!("CARGO_MANIFEST_DIR"),
					"secret_key": "contract-verify-secret",
					"installed_apps": []
				}))
				.with_value("contacts", serde_json::json!({}))
				.with_value("migrations", serde_json::json!({}))
				.with_value("verification", serde_json::json!(__SETTINGS__)),
		)
		.add_source(reinhardt::conf::settings::sources::TomlFileSource::new(
			std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("settings/base.toml"),
		))
		.build_pending_composed::<ProjectSettings>()
}

#[tokio::main]
async fn main() {
	if let Some(toolchain) = option_env!("REINHARDT_RUSTUP_TOOLCHAIN") {
		// Keep nested Cargo on the same toolchain as the consumer launcher.
		unsafe { std::env::set_var("RUSTUP_TOOLCHAIN", toolchain) };
	}
	let context = CargoCheckContext::from_launcher(
		std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
		Some(env!("CARGO_PKG_NAME").to_owned()),
		Some("manage".to_owned()),
	);
	if let Err(error) = execute_from_command_line_with_pending_settings_and_cargo_context(
		settings,
		context,
	)
	.await
	{
		let exit_code = command_error_exit_code(error.as_ref());
		eprintln!("{error}");
		std::process::exit(exit_code);
	}
}
