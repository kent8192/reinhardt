use std::collections::HashMap;

use reinhardt_commands::{ShellConfig, ShellEnvironment, shell_runtime_hook};
use reinhardt_conf::settings::DatabaseConfig;
use reinhardt_conf::settings::contacts::ContactSettings;
use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::fragment::HasSettings;
use reinhardt_db::orm::{
	DatabaseConnection, DatabaseConnectionLease, get_connection, install_scoped_database,
};
use tempfile::tempdir;

fn shell_config(manifest_dir: impl Into<std::path::PathBuf>) -> ShellConfig {
	let manifest_dir = manifest_dir.into();
	write_cargo_manifest(&manifest_dir);
	ShellConfig::new(
		"inventory-site",
		"inventory_site",
		manifest_dir,
		"inventory_site::config::settings::get_settings",
		["users", "inventory"],
	)
}

fn write_cargo_manifest(manifest_dir: &std::path::Path) {
	std::fs::write(
		manifest_dir.join("Cargo.toml"),
		"[package]\nname = \"inventory-site\"\nversion = \"0.1.0\"\n",
	)
	.expect("Cargo manifest should be written");
}

#[test]
fn validation_retains_distinct_package_and_crate_names() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");

	let validated = shell_config(manifest_dir.path())
		.validate()
		.expect("valid shell configuration should validate");

	assert_eq!(validated.package_name(), "inventory-site");
	assert_eq!(validated.crate_name(), "inventory_site");
}

#[test]
fn validation_canonicalizes_the_manifest_directory() {
	let manifest_root = tempdir().expect("temporary manifest directory should be created");
	std::fs::create_dir(manifest_root.path().join("project"))
		.expect("nested manifest directory should be created");
	let manifest_dir = manifest_root.path().join("project").join("..");

	let validated = shell_config(&manifest_dir)
		.validate()
		.expect("existing manifest directory should canonicalize");

	assert_eq!(
		validated.manifest_dir(),
		manifest_root
			.path()
			.canonicalize()
			.expect("temporary manifest directory should canonicalize")
	);
}

#[test]
fn validation_rejects_relative_settings_factory_paths() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");
	let config = ShellConfig::new(
		"inventory-site",
		"inventory_site",
		manifest_dir.path(),
		"get_settings",
		["users"],
	);

	let error = config
		.validate()
		.expect_err("relative settings factory path should be rejected");

	assert_eq!(
		error.to_string(),
		"Invalid arguments: settings factory path must be an absolute Rust path"
	);
}

#[test]
fn validation_rejects_invalid_crate_identifiers() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");

	for crate_name in ["", "inventory-site", "inventory site", "inventory::site"] {
		let config = ShellConfig::new(
			"inventory-site",
			crate_name,
			manifest_dir.path(),
			"inventory_site::config::settings::get_settings",
			["users"],
		);

		let error = config
			.validate()
			.expect_err("invalid crate identifier should be rejected");

		assert_eq!(
			error.to_string(),
			format!("Invalid arguments: invalid Rust crate identifier: {crate_name:?}")
		);
	}
}

#[test]
fn validation_rejects_empty_settings_path_segments() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");
	let config = ShellConfig::new(
		"inventory-site",
		"inventory_site",
		manifest_dir.path(),
		"inventory_site::::get_settings",
		["users"],
	);

	let error = config
		.validate()
		.expect_err("empty settings path segments should be rejected");

	assert_eq!(
		error.to_string(),
		"Invalid arguments: settings factory path must contain only Rust identifiers"
	);
}

#[test]
fn validation_rejects_settings_factory_from_another_crate() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");
	write_cargo_manifest(manifest_dir.path());
	let config = ShellConfig::new(
		"inventory-site",
		"inventory_site",
		manifest_dir.path(),
		"other_crate::config::settings::get_settings",
		["users"],
	);

	let error = config
		.validate()
		.expect_err("settings factory from another crate should be rejected");

	assert_eq!(
		error.to_string(),
		"Invalid arguments: settings factory path must start with configured crate name \
`inventory_site`"
	);
}

#[test]
fn validation_rejects_a_regular_file_as_manifest_directory() {
	let manifest_file =
		tempfile::NamedTempFile::new().expect("temporary manifest file should be created");
	let canonical_file = manifest_file
		.path()
		.canonicalize()
		.expect("temporary manifest file should canonicalize");
	let config = ShellConfig::new(
		"inventory-site",
		"inventory_site",
		manifest_file.path(),
		"inventory_site::config::settings::get_settings",
		["users"],
	);

	let error = config
		.validate()
		.expect_err("a regular file should not be accepted as manifest directory");

	assert_eq!(
		error.to_string(),
		format!(
			"Invalid arguments: manifest directory is not a directory: {}",
			canonical_file.display()
		)
	);
}

#[test]
fn validation_rejects_a_directory_without_cargo_manifest() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");
	let cargo_manifest = manifest_dir
		.path()
		.canonicalize()
		.expect("temporary manifest directory should canonicalize")
		.join("Cargo.toml");
	let config = ShellConfig::new(
		"inventory-site",
		"inventory_site",
		manifest_dir.path(),
		"inventory_site::config::settings::get_settings",
		["users"],
	);

	let error = config
		.validate()
		.expect_err("directory without Cargo.toml should be rejected");

	assert_eq!(
		error.to_string(),
		format!(
			"Invalid arguments: Cargo manifest must be a regular file: {}",
			cargo_manifest.display()
		)
	);
}

#[test]
fn validation_rejects_a_non_file_cargo_manifest() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");
	let cargo_manifest = manifest_dir.path().join("Cargo.toml");
	std::fs::create_dir(&cargo_manifest)
		.expect("directory-shaped Cargo manifest should be created");
	let cargo_manifest = cargo_manifest
		.canonicalize()
		.expect("directory-shaped Cargo manifest should canonicalize");
	let config = ShellConfig::new(
		"inventory-site",
		"inventory_site",
		manifest_dir.path(),
		"inventory_site::config::settings::get_settings",
		["users"],
	);

	let error = config
		.validate()
		.expect_err("directory-shaped Cargo.toml should be rejected");

	assert_eq!(
		error.to_string(),
		format!(
			"Invalid arguments: Cargo manifest must be a regular file: {}",
			cargo_manifest.display()
		)
	);
}

#[test]
fn validation_deduplicates_installed_apps_in_declaration_order() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");
	write_cargo_manifest(manifest_dir.path());
	let config = ShellConfig::new(
		"inventory-site",
		"inventory_site",
		manifest_dir.path(),
		"inventory_site::config::settings::get_settings",
		["users", "inventory", "users", "billing", "inventory"],
	);

	let validated = config
		.validate()
		.expect("valid shell configuration should validate");

	assert_eq!(
		validated.installed_app_labels(),
		["users", "inventory", "billing"]
	);
}

#[test]
fn with_prelude_retains_project_source_for_the_final_prelude_layer() {
	let manifest_dir = tempdir().expect("temporary manifest directory should be created");
	let config = shell_config(manifest_dir.path())
		.with_prelude("use inventory_site::support::ShellHelpers;");

	let validated = config
		.validate()
		.expect("valid shell configuration should validate");

	assert_eq!(
		validated.project_prelude(),
		"use inventory_site::support::ShellHelpers;"
	);
}

#[test]
fn runtime_hook_returns_normally_outside_an_evcxr_subprocess() {
	shell_runtime_hook();
}

struct ShellTestSettings {
	name: String,
	core: CoreSettings,
	contacts: ContactSettings,
}

impl ShellTestSettings {
	fn sqlite_in_memory(name: &str) -> Self {
		let mut databases = HashMap::new();
		databases.insert("default".to_string(), DatabaseConfig::sqlite(":memory:"));
		Self {
			name: name.to_string(),
			core: CoreSettings {
				databases,
				..Default::default()
			},
			contacts: ContactSettings::default(),
		}
	}
}

impl HasSettings<CoreSettings> for ShellTestSettings {
	fn get_settings(&self) -> &CoreSettings {
		&self.core
	}
}

impl HasSettings<ContactSettings> for ShellTestSettings {
	fn get_settings(&self) -> &ContactSettings {
		&self.contacts
	}
}

struct EnvironmentVariableGuard {
	value: Option<String>,
}

impl EnvironmentVariableGuard {
	fn remove_database_url() -> Self {
		let value = std::env::var("DATABASE_URL").ok();
		// SAFETY: This test is serialized with every DATABASE_URL mutation in this target.
		unsafe {
			std::env::remove_var("DATABASE_URL");
		}
		Self { value }
	}
}

impl Drop for EnvironmentVariableGuard {
	fn drop(&mut self) {
		// SAFETY: This test is serialized with every DATABASE_URL mutation in this target.
		unsafe {
			match self.value.take() {
				Some(value) => std::env::set_var("DATABASE_URL", value),
				None => std::env::remove_var("DATABASE_URL"),
			}
		}
	}
}

#[serial_test::serial(shell_database_environment)]
#[tokio::test]
async fn environment_bootstrap_owns_settings_database_and_singleton_di() {
	let _database_url = EnvironmentVariableGuard::remove_database_url();
	let previous_database = install_scoped_database("sqlite::memory:")
		.await
		.expect("previous database should install");
	let previous_connection = previous_database.connection();

	let environment =
		ShellEnvironment::bootstrap(ShellTestSettings::sqlite_in_memory("inventory-shell"))
			.await
			.expect("shell environment should bootstrap");
	let shell_connection = environment.database();
	let di = environment.di();

	assert_eq!(environment.settings().name, "inventory-shell");
	assert_ne!(shell_connection, previous_connection);
	assert_eq!(
		shell_connection
			.execute("CREATE TABLE shell_probe (id INTEGER)", vec![])
			.await
			.expect("shell database should execute SQL"),
		0
	);
	assert_eq!(
		di.get_singleton::<DatabaseConnection>()
			.expect("database connection should be registered in DI")
			.as_ref(),
		&shell_connection
	);
	assert_eq!(
		di.get_singleton::<DatabaseConnectionLease>()
			.expect("database lease should be registered in DI")
			.handle(),
		shell_connection
	);

	drop(environment);

	assert_eq!(
		get_connection()
			.await
			.expect("previous global database should be restored"),
		previous_connection
	);
}
