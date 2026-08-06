//! Plugin Management Commands
//!
//! Commands for managing Reinhardt plugins (Dentdelion).
//!
//! ## Available Commands
//!
//! - `plugin list` - List installed plugins
//! - `plugin info <name>` - Show plugin details
//! - `plugin install <name>` - Install a plugin from crates.io
//! - `plugin remove <name>` - Remove a plugin
//! - `plugin enable <name>` - Enable a plugin
//! - `plugin disable <name>` - Disable a plugin
//! - `plugin search <query>` - Search for plugins on crates.io
//! - `plugin update [name|--all]` - Update plugin(s) to latest version

use crate::{
	BaseCommand, CommandArgument, CommandContext, CommandError, CommandOption, CommandResult,
};
use async_trait::async_trait;
use reinhardt_dentdelion::crates_io::CratesIoClient;
use reinhardt_dentdelion::installer::PluginInstaller;
use reinhardt_dentdelion::manifest::{InstalledPlugin, MANIFEST_FILENAME, ProjectManifest};
use std::path::PathBuf;

/// Get the project root from context or use current directory.
fn get_project_root(ctx: &CommandContext) -> PathBuf {
	ctx.option("project-root")
		.map(PathBuf::from)
		.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn filtered_plugins(
	manifest: &ProjectManifest,
	show_enabled: bool,
	show_disabled: bool,
) -> Vec<&InstalledPlugin> {
	manifest
		.plugins
		.iter()
		.filter(|plugin| {
			if show_enabled && !show_disabled {
				plugin.enabled
			} else if show_disabled && !show_enabled {
				!plugin.enabled
			} else {
				true
			}
		})
		.collect()
}

fn local_plugin<'a>(
	manifest: &'a ProjectManifest,
	name: &str,
) -> CommandResult<&'a InstalledPlugin> {
	manifest
		.plugins
		.iter()
		.find(|plugin| plugin.name == name)
		.ok_or_else(|| {
			CommandError::ExecutionError(format!("Plugin '{name}' not found in manifest"))
		})
}

// =============================================================================
// Plugin List Command
// =============================================================================

/// List installed plugins.
///
/// Shows all plugins defined in dentdelion.toml with their status.
pub struct PluginListCommand;

#[async_trait]
impl BaseCommand for PluginListCommand {
	fn name(&self) -> &str {
		"plugin list"
	}

	fn description(&self) -> &str {
		"List installed Reinhardt plugins"
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(Some('v'), "verbose", "Show detailed information"),
			CommandOption::flag(None, "enabled", "Show only enabled plugins"),
			CommandOption::flag(None, "disabled", "Show only disabled plugins"),
			CommandOption::option(None, "project-root", "Project root directory"),
		]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let project_root = get_project_root(ctx);
		let manifest_path = project_root.join(MANIFEST_FILENAME);

		if !manifest_path.exists() {
			ctx.info("No dentdelion.toml found. No plugins installed.");
			return Ok(());
		}

		let manifest = ProjectManifest::load(&manifest_path)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to load manifest: {e}")))?;

		let show_enabled = ctx.has_option("enabled");
		let show_disabled = ctx.has_option("disabled");
		let verbose = ctx.has_option("verbose");

		let plugins = filtered_plugins(&manifest, show_enabled, show_disabled);

		if plugins.is_empty() {
			if show_enabled {
				ctx.info("No enabled plugins found.");
			} else if show_disabled {
				ctx.info("No disabled plugins found.");
			} else {
				ctx.info("No plugins installed.");
			}
			return Ok(());
		}

		ctx.info("Installed plugins:");
		for plugin in plugins {
			let status = if plugin.enabled {
				"enabled"
			} else {
				"disabled"
			};
			let status_mark = if plugin.enabled {
				"\u{2713}"
			} else {
				"\u{2717}"
			};

			if verbose {
				ctx.info(&format!(
					"  {} {} {} ({}) [{}]",
					status_mark, plugin.name, plugin.version, plugin.plugin_type, status
				));
				if let Some(source) = &plugin.source {
					ctx.verbose(&format!("      Source: {source}"));
				}
			} else {
				ctx.info(&format!(
					"  {} {} {} ({})",
					status_mark, plugin.name, plugin.version, status
				));
			}
		}

		Ok(())
	}
}

// =============================================================================
// Plugin Info Command
// =============================================================================

/// Show plugin information.
///
/// Displays detailed information about a specific plugin.
pub struct PluginInfoCommand;

#[async_trait]
impl BaseCommand for PluginInfoCommand {
	fn name(&self) -> &str {
		"plugin info"
	}

	fn description(&self) -> &str {
		"Show detailed information about a plugin"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::required(
			"name",
			"Plugin name (e.g., auth-delion)",
		)]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(None, "remote", "Fetch info from crates.io instead of local"),
			CommandOption::option(None, "project-root", "Project root directory"),
		]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let name = ctx
			.arg(0)
			.ok_or_else(|| CommandError::InvalidArguments("Plugin name is required".to_string()))?;

		if ctx.has_option("remote") {
			// Fetch from crates.io
			let client = CratesIoClient::new().map_err(|e| {
				CommandError::ExecutionError(format!("Failed to connect to crates.io: {e}"))
			})?;

			let info = client.get_crate_info(name).await.map_err(|e| {
				CommandError::ExecutionError(format!("Failed to fetch plugin info: {e}"))
			})?;

			ctx.info(&format!("{} v{}", info.name, info.version));
			if let Some(desc) = &info.description {
				ctx.info(&format!("  Description: {desc}"));
			}
			if let Some(repo) = &info.repository {
				ctx.info(&format!("  Repository: {repo}"));
			}
			if let Some(docs) = &info.documentation {
				ctx.info(&format!("  Documentation: {docs}"));
			}
			ctx.info(&format!("  Downloads: {}", info.downloads));

			if !info.versions.is_empty() {
				ctx.info("  Available versions:");
				for v in info.versions.iter().take(5) {
					let yanked = if v.yanked { " (yanked)" } else { "" };
					ctx.info(&format!("    - {}{yanked}", v.version));
				}
				if info.versions.len() > 5 {
					ctx.info(&format!("    ... and {} more", info.versions.len() - 5));
				}
			}
		} else {
			// Show local info
			let project_root = get_project_root(ctx);
			let manifest_path = project_root.join(MANIFEST_FILENAME);

			if !manifest_path.exists() {
				return Err(CommandError::ExecutionError(
					"No dentdelion.toml found. Run 'reinhardt-admin plugin install' first."
						.to_string(),
				));
			}

			let manifest = ProjectManifest::load(&manifest_path).map_err(|e| {
				CommandError::ExecutionError(format!("Failed to load manifest: {e}"))
			})?;

			let plugin = local_plugin(&manifest, name)?;

			ctx.info(&format!("{} v{}", plugin.name, plugin.version));
			ctx.info(&format!("  Type: {}", plugin.plugin_type));
			ctx.info(&format!(
				"  Status: {}",
				if plugin.enabled {
					"enabled"
				} else {
					"disabled"
				}
			));
			if let Some(source) = &plugin.source {
				ctx.info(&format!("  Source: {source}"));
			}

			// Check for config
			if let Some(config) = manifest.plugin_config.get(name) {
				ctx.info("  Configuration:");
				for (key, value) in config {
					ctx.info(&format!("    {key}: {value}"));
				}
			}
		}

		Ok(())
	}
}

// =============================================================================
// Plugin Install Command
// =============================================================================

/// Install a plugin.
///
/// Installs a plugin from crates.io.
pub struct PluginInstallCommand;

#[async_trait]
impl BaseCommand for PluginInstallCommand {
	fn name(&self) -> &str {
		"plugin install"
	}

	fn description(&self) -> &str {
		"Install a Reinhardt plugin from crates.io"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::required(
			"name",
			"Plugin name (e.g., auth-delion)",
		)]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::option(None, "version", "Specific version to install"),
			CommandOption::flag(Some('y'), "yes", "Skip confirmation prompt"),
			CommandOption::option(None, "project-root", "Project root directory"),
		]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let name = ctx
			.arg(0)
			.ok_or_else(|| CommandError::InvalidArguments("Plugin name is required".to_string()))?;

		// Suggest proper plugin name if needed
		let plugin_name = if !name.ends_with("-delion") {
			let suggested = format!("{name}-delion");
			ctx.warning(&format!(
				"Plugin name should end with '-delion'. Using '{suggested}' instead."
			));
			suggested
		} else {
			name.to_string()
		};

		let project_root = get_project_root(ctx);
		let installer = PluginInstaller::new(&project_root);

		// Initialize manifest if needed
		if !installer.has_manifest() {
			ctx.info("Initializing dentdelion.toml...");
			installer.init_manifest().map_err(|e| {
				CommandError::ExecutionError(format!("Failed to init manifest: {e}"))
			})?;
		}

		// Get version from crates.io
		ctx.info(&format!("Fetching {plugin_name} from crates.io..."));
		let client = CratesIoClient::new().map_err(|e| {
			CommandError::ExecutionError(format!("Failed to connect to crates.io: {e}"))
		})?;

		let version = if let Some(v) = ctx.option("version") {
			v.to_string()
		} else {
			client.get_latest_version(&plugin_name).await.map_err(|e| {
				CommandError::ExecutionError(format!("Failed to fetch version: {e}"))
			})?
		};

		let info = client.get_crate_info(&plugin_name).await.map_err(|e| {
			CommandError::ExecutionError(format!("Failed to fetch plugin info: {e}"))
		})?;

		// Show info and confirm
		ctx.info(&format!("  Version: {version}"));
		if let Some(desc) = &info.description {
			ctx.info(&format!("  Description: {desc}"));
		}

		if !ctx.has_option("yes") {
			ctx.info("\nThis will:");
			ctx.info(&format!(
				"  - Add {plugin_name} = \"{version}\" to Cargo.toml"
			));
			ctx.info(&format!("  - Add plugin entry to {MANIFEST_FILENAME}"));

			let confirmed = ctx
				.confirm("Continue?", true)
				.map_err(|e| CommandError::ExecutionError(format!("Prompt failed: {e}")))?;

			if !confirmed {
				ctx.info("Installation cancelled.");
				return Ok(());
			}
		}

		// Install
		installer
			.install_static(&plugin_name, &version, None)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to install plugin: {e}")))?;

		ctx.success(&format!(
			"Added {plugin_name} = \"{version}\" to Cargo.toml"
		));
		ctx.success(&format!("Updated {MANIFEST_FILENAME}"));
		ctx.info("\nRun `cargo build` to complete installation.");

		Ok(())
	}
}

// =============================================================================
// Plugin Remove Command
// =============================================================================

/// Remove a plugin.
///
/// Removes a plugin from both Cargo.toml and dentdelion.toml.
pub struct PluginRemoveCommand;

#[async_trait]
impl BaseCommand for PluginRemoveCommand {
	fn name(&self) -> &str {
		"plugin remove"
	}

	fn description(&self) -> &str {
		"Remove a Reinhardt plugin"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::required("name", "Plugin name to remove")]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(None, "purge", "Also remove plugin configuration"),
			CommandOption::flag(Some('y'), "yes", "Skip confirmation prompt"),
			CommandOption::option(None, "project-root", "Project root directory"),
		]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let name = ctx
			.arg(0)
			.ok_or_else(|| CommandError::InvalidArguments("Plugin name is required".to_string()))?;

		let project_root = get_project_root(ctx);
		let installer = PluginInstaller::new(&project_root);
		let purge = ctx.has_option("purge");

		if !ctx.has_option("yes") {
			let prompt_message = if purge {
				format!("This will remove {name} and its configuration. Continue?")
			} else {
				format!("This will remove {name} (configuration will be kept). Continue?")
			};

			let default_confirm = !purge; // Default to false when purging
			let confirmed = ctx
				.confirm(&prompt_message, default_confirm)
				.map_err(|e| CommandError::ExecutionError(format!("Prompt failed: {e}")))?;

			if !confirmed {
				ctx.info("Removal cancelled.");
				return Ok(());
			}
		}

		installer
			.uninstall(name, purge)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to remove plugin: {e}")))?;

		ctx.success(&format!("Removed {name} from Cargo.toml"));
		ctx.success(&format!("Updated {MANIFEST_FILENAME}"));
		if purge {
			ctx.info("Plugin configuration was also removed.");
		}
		ctx.info("\nRun `cargo build` to apply changes.");

		Ok(())
	}
}

// =============================================================================
// Plugin Enable Command
// =============================================================================

/// Enable a plugin.
///
/// Sets a plugin's enabled status to true in dentdelion.toml.
pub struct PluginEnableCommand;

#[async_trait]
impl BaseCommand for PluginEnableCommand {
	fn name(&self) -> &str {
		"plugin enable"
	}

	fn description(&self) -> &str {
		"Enable a disabled plugin"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::required("name", "Plugin name to enable")]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![CommandOption::option(
			None,
			"project-root",
			"Project root directory",
		)]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let name = ctx
			.arg(0)
			.ok_or_else(|| CommandError::InvalidArguments("Plugin name is required".to_string()))?;

		let project_root = get_project_root(ctx);
		let installer = PluginInstaller::new(&project_root);

		installer
			.enable_plugin(name)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to enable plugin: {e}")))?;

		ctx.success(&format!("Enabled plugin '{name}'"));

		Ok(())
	}
}

// =============================================================================
// Plugin Disable Command
// =============================================================================

/// Disable a plugin.
///
/// Sets a plugin's enabled status to false in dentdelion.toml.
pub struct PluginDisableCommand;

#[async_trait]
impl BaseCommand for PluginDisableCommand {
	fn name(&self) -> &str {
		"plugin disable"
	}

	fn description(&self) -> &str {
		"Disable an enabled plugin"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::required("name", "Plugin name to disable")]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![CommandOption::option(
			None,
			"project-root",
			"Project root directory",
		)]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let name = ctx
			.arg(0)
			.ok_or_else(|| CommandError::InvalidArguments("Plugin name is required".to_string()))?;

		let project_root = get_project_root(ctx);
		let installer = PluginInstaller::new(&project_root);

		installer
			.disable_plugin(name)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to disable plugin: {e}")))?;

		ctx.success(&format!("Disabled plugin '{name}'"));

		Ok(())
	}
}

// =============================================================================
// Plugin Search Command
// =============================================================================

/// Search for plugins on crates.io.
///
/// Searches for plugins matching the query that follow the xxx-delion naming convention.
pub struct PluginSearchCommand;

#[async_trait]
impl BaseCommand for PluginSearchCommand {
	fn name(&self) -> &str {
		"plugin search"
	}

	fn description(&self) -> &str {
		"Search for Reinhardt plugins on crates.io"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::required("query", "Search query")]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![CommandOption::option(None, "limit", "Maximum number of results").with_default("10")]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let query = ctx.arg(0).ok_or_else(|| {
			CommandError::InvalidArguments("Search query is required".to_string())
		})?;

		let limit: u64 = ctx
			.option("limit")
			.and_then(|s| s.parse().ok())
			.unwrap_or(10);

		ctx.info(&format!(
			"Searching crates.io for '*-delion' plugins matching '{query}'..."
		));

		let client = CratesIoClient::new().map_err(|e| {
			CommandError::ExecutionError(format!("Failed to connect to crates.io: {e}"))
		})?;

		let plugins = client
			.search_plugins(query, limit)
			.await
			.map_err(|e| CommandError::ExecutionError(format!("Search failed: {e}")))?;

		if plugins.is_empty() {
			ctx.info("No plugins found matching your query.");
			return Ok(());
		}

		ctx.info(&format!("\nFound {} plugin(s):\n", plugins.len()));

		for plugin in plugins {
			let desc = plugin.description.as_deref().unwrap_or("No description");
			ctx.info(&format!("  {} v{}", plugin.name, plugin.version));
			ctx.info(&format!("    {desc}"));
			ctx.info(&format!("    Downloads: {}", plugin.downloads));
			ctx.info("");
		}

		Ok(())
	}
}

// =============================================================================
// Plugin Update Command
// =============================================================================

/// Update plugin(s) to latest version.
///
/// Updates one or all plugins to their latest versions from crates.io.
pub struct PluginUpdateCommand;

#[async_trait]
impl BaseCommand for PluginUpdateCommand {
	fn name(&self) -> &str {
		"plugin update"
	}

	fn description(&self) -> &str {
		"Update plugin(s) to latest version"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::optional(
			"name",
			"Plugin name (omit for --all)",
		)]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(None, "all", "Update all plugins"),
			CommandOption::flag(Some('y'), "yes", "Skip confirmation prompt"),
			CommandOption::option(None, "project-root", "Project root directory"),
		]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let project_root = get_project_root(ctx);
		let manifest_path = project_root.join(MANIFEST_FILENAME);

		if !manifest_path.exists() {
			return Err(CommandError::ExecutionError(
				"No dentdelion.toml found. No plugins to update.".to_string(),
			));
		}

		let manifest = ProjectManifest::load(&manifest_path)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to load manifest: {e}")))?;

		let installer = PluginInstaller::new(&project_root);
		let client = CratesIoClient::new().map_err(|e| {
			CommandError::ExecutionError(format!("Failed to connect to crates.io: {e}"))
		})?;

		let plugins_to_update: Vec<_> = if ctx.has_option("all") {
			manifest.plugins.iter().map(|p| p.name.clone()).collect()
		} else if let Some(name) = ctx.arg(0) {
			vec![name.to_string()]
		} else {
			return Err(CommandError::InvalidArguments(
				"Please specify a plugin name or use --all".to_string(),
			));
		};

		if plugins_to_update.is_empty() {
			ctx.info("No plugins to update.");
			return Ok(());
		}

		let mut updated = 0;
		let mut skipped = 0;

		for name in &plugins_to_update {
			let current_plugin = manifest.plugins.iter().find(|p| &p.name == name);
			let current_version = match current_plugin {
				Some(p) => &p.version,
				None => {
					ctx.warning(&format!("Plugin '{name}' not found in manifest, skipping."));
					skipped += 1;
					continue;
				}
			};

			let latest_version = match client.get_latest_version(name).await {
				Ok(v) => v,
				Err(e) => {
					ctx.warning(&format!("Failed to fetch latest version for '{name}': {e}"));
					skipped += 1;
					continue;
				}
			};

			if current_version == &latest_version {
				ctx.verbose(&format!(
					"{name} is already at latest version ({latest_version})"
				));
				skipped += 1;
				continue;
			}

			ctx.info(&format!(
				"Updating {name}: {current_version} -> {latest_version}"
			));

			if let Err(e) = installer.update_version(name, &latest_version) {
				ctx.warning(&format!("Failed to update '{name}': {e}"));
				skipped += 1;
				continue;
			}

			updated += 1;
		}

		ctx.info("");
		if updated > 0 {
			ctx.success(&format!("Updated {updated} plugin(s)"));
			ctx.info("Run `cargo build` to apply changes.");
		}
		if skipped > 0 {
			ctx.info(&format!(
				"{skipped} plugin(s) skipped (already up-to-date or error)"
			));
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	/// Create a test project with a Cargo.toml file.
	fn create_test_project(dir: &std::path::Path) {
		let content = r#"[package]
name = "test-project"
version = "0.1.0-alpha.1"

[dependencies]
serde = "1.0"
"#;
		std::fs::write(dir.join("Cargo.toml"), content).unwrap();
	}

	/// Create a test dentdelion.toml manifest with a plugin.
	fn create_test_manifest(dir: &std::path::Path) {
		let content = r#"[dentdelion]
format_version = "1.0"

[[plugins]]
name = "test-delion"
type = "static"
version = "1.0.0"
enabled = true
"#;
		std::fs::write(dir.join("dentdelion.toml"), content).unwrap();
	}

	/// Create a test manifest with a disabled plugin.
	fn create_test_manifest_with_disabled(dir: &std::path::Path) {
		let content = r#"[dentdelion]
format_version = "1.0"

[[plugins]]
name = "test-delion"
type = "static"
version = "1.0.0"
enabled = false
"#;
		std::fs::write(dir.join("dentdelion.toml"), content).unwrap();
	}

	fn create_test_manifest_with_enabled_and_disabled(dir: &std::path::Path) {
		let content = r#"[dentdelion]
format_version = "1.0"

[[plugins]]
name = "enabled-delion"
type = "static"
version = "1.0.0"
enabled = true

[[plugins]]
name = "disabled-delion"
type = "static"
version = "2.0.0"
enabled = false
"#;
		std::fs::write(dir.join("dentdelion.toml"), content).expect("manifest fixture is written");
	}

	#[test]
	fn filtered_plugins_respects_enabled_and_disabled_flags() {
		// Arrange
		let temp_dir = TempDir::new().expect("temporary project is created");
		create_test_manifest_with_enabled_and_disabled(temp_dir.path());
		let manifest = ProjectManifest::load(temp_dir.path().join(MANIFEST_FILENAME))
			.expect("manifest fixture is parsed");

		// Act
		let enabled = filtered_plugins(&manifest, true, false);
		let disabled = filtered_plugins(&manifest, false, true);
		let all = filtered_plugins(&manifest, true, true);

		// Assert
		assert_eq!(
			enabled
				.iter()
				.map(|plugin| plugin.name.as_str())
				.collect::<Vec<_>>(),
			vec!["enabled-delion"],
		);
		assert_eq!(
			disabled
				.iter()
				.map(|plugin| plugin.name.as_str())
				.collect::<Vec<_>>(),
			vec!["disabled-delion"],
		);
		assert_eq!(
			all.iter()
				.map(|plugin| plugin.name.as_str())
				.collect::<Vec<_>>(),
			vec!["enabled-delion", "disabled-delion"],
		);
	}

	#[test]
	fn local_plugin_lookup_returns_exact_match_or_actionable_error() {
		// Arrange
		let temp_dir = TempDir::new().expect("temporary project is created");
		create_test_manifest(temp_dir.path());
		let manifest = ProjectManifest::load(temp_dir.path().join(MANIFEST_FILENAME))
			.expect("manifest fixture is parsed");

		// Act
		let plugin = local_plugin(&manifest, "test-delion").expect("local plugin is found");
		let error = local_plugin(&manifest, "missing-delion")
			.expect_err("unknown local plugin is rejected");

		// Assert
		assert_eq!(plugin.name, "test-delion");
		assert_eq!(plugin.version, "1.0.0");
		assert!(matches!(
			error,
			CommandError::ExecutionError(message)
				if message == "Plugin 'missing-delion' not found in manifest"
		));
	}

	// ==========================================================================
	// Metadata Tests (existing)
	// ==========================================================================

	#[test]
	fn test_plugin_list_command_metadata() {
		let cmd = PluginListCommand;
		assert_eq!(cmd.name(), "plugin list");
		assert!(!cmd.requires_system_checks());
	}

	#[test]
	fn test_plugin_install_command_metadata() {
		let cmd = PluginInstallCommand;
		assert_eq!(cmd.name(), "plugin install");
		assert_eq!(cmd.arguments().len(), 1);
		assert_eq!(cmd.arguments()[0].name, "name");
	}

	#[test]
	fn test_plugin_search_command_metadata() {
		let cmd = PluginSearchCommand;
		assert_eq!(cmd.name(), "plugin search");
		assert!(!cmd.requires_system_checks());
	}

	// ==========================================================================
	// All Commands Metadata Tests
	// ==========================================================================

	#[test]
	fn test_plugin_info_command_metadata() {
		let cmd = PluginInfoCommand;
		assert_eq!(cmd.name(), "plugin info");
		assert_eq!(cmd.arguments().len(), 1);
		assert_eq!(cmd.arguments()[0].name, "name");
		assert!(!cmd.requires_system_checks());
	}

	#[test]
	fn test_plugin_remove_command_metadata() {
		let cmd = PluginRemoveCommand;
		assert_eq!(cmd.name(), "plugin remove");
		assert_eq!(cmd.arguments().len(), 1);
		assert_eq!(cmd.arguments()[0].name, "name");
		assert!(!cmd.requires_system_checks());
	}

	#[test]
	fn test_plugin_enable_command_metadata() {
		let cmd = PluginEnableCommand;
		assert_eq!(cmd.name(), "plugin enable");
		assert_eq!(cmd.arguments().len(), 1);
		assert_eq!(cmd.arguments()[0].name, "name");
		assert!(!cmd.requires_system_checks());
	}

	#[test]
	fn test_plugin_disable_command_metadata() {
		let cmd = PluginDisableCommand;
		assert_eq!(cmd.name(), "plugin disable");
		assert_eq!(cmd.arguments().len(), 1);
		assert_eq!(cmd.arguments()[0].name, "name");
		assert!(!cmd.requires_system_checks());
	}

	#[test]
	fn test_plugin_update_command_metadata() {
		let cmd = PluginUpdateCommand;
		assert_eq!(cmd.name(), "plugin update");
		assert_eq!(cmd.arguments().len(), 1);
		assert_eq!(cmd.arguments()[0].name, "name");
		assert!(!cmd.requires_system_checks());
	}

	// ==========================================================================
	// PluginListCommand Execute Tests
	// ==========================================================================

	#[tokio::test]
	async fn test_plugin_list_no_manifest() {
		let temp_dir = TempDir::new().unwrap();
		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);

		let cmd = PluginListCommand;
		// Should succeed even without manifest (just shows "no plugins" message)
		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_plugin_list_reports_malformed_manifest_with_load_context() {
		// Arrange
		let temp_dir = TempDir::new().expect("temporary project is created");
		std::fs::write(temp_dir.path().join(MANIFEST_FILENAME), "[dentdelion\n")
			.expect("malformed manifest fixture is written");
		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().display().to_string(),
		);
		let command = PluginListCommand;

		// Act
		let error = command
			.execute(&ctx)
			.await
			.expect_err("malformed manifests must not be treated as empty");

		// Assert
		let CommandError::ExecutionError(message) = error else {
			panic!("malformed manifest returned the wrong error variant");
		};
		assert_eq!(
			message,
			"Failed to load manifest: failed to parse manifest: TOML parse error at line 1, column 12\n  |\n1 | [dentdelion\n  |            ^\nunclosed table, expected `]`\n"
		);
	}

	#[tokio::test]
	async fn test_plugin_list_with_plugins() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);

		let cmd = PluginListCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_plugin_list_with_verbose() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.set_option("verbose".to_string(), "true".to_string());

		let cmd = PluginListCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok());
	}

	// ==========================================================================
	// PluginInfoCommand Execute Tests
	// ==========================================================================

	#[tokio::test]
	async fn test_plugin_info_local_plugin() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.add_arg("test-delion".to_string());

		let cmd = PluginInfoCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_plugin_info_not_found() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.add_arg("nonexistent-delion".to_string());

		let cmd = PluginInfoCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_plugin_info_no_argument() {
		let ctx = CommandContext::default();

		let cmd = PluginInfoCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_err());
	}

	// ==========================================================================
	// PluginEnableCommand Execute Tests
	// ==========================================================================

	#[tokio::test]
	async fn test_plugin_enable_success() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest_with_disabled(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.add_arg("test-delion".to_string());

		let cmd = PluginEnableCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_plugin_enable_not_found() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.add_arg("nonexistent-delion".to_string());

		let cmd = PluginEnableCommand;
		let error = cmd
			.execute(&ctx)
			.await
			.expect_err("unknown plugins cannot be enabled");
		assert!(matches!(
			error,
			CommandError::ExecutionError(message)
				if message == "Failed to enable plugin: plugin not found: nonexistent-delion"
		));
	}

	#[tokio::test]
	async fn test_plugin_enable_no_argument() {
		let ctx = CommandContext::default();

		let cmd = PluginEnableCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_err());
	}

	// ==========================================================================
	// PluginDisableCommand Execute Tests
	// ==========================================================================

	#[tokio::test]
	async fn test_plugin_disable_success() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.add_arg("test-delion".to_string());

		let cmd = PluginDisableCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_plugin_disable_not_found() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.add_arg("nonexistent-delion".to_string());

		let cmd = PluginDisableCommand;
		let error = cmd
			.execute(&ctx)
			.await
			.expect_err("unknown plugins cannot be disabled");
		assert!(matches!(
			error,
			CommandError::ExecutionError(message)
				if message == "Failed to disable plugin: plugin not found: nonexistent-delion"
		));
	}

	#[tokio::test]
	async fn test_plugin_disable_no_argument() {
		let ctx = CommandContext::default();

		let cmd = PluginDisableCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_err());
	}

	// ==========================================================================
	// PluginRemoveCommand Execute Tests
	// ==========================================================================

	#[tokio::test]
	async fn test_plugin_remove_success() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		// Add the plugin to Cargo.toml as well
		let cargo_content = r#"[package]
name = "test-project"
version = "0.1.0-alpha.1"

[dependencies]
serde = "1.0"
test-delion = "1.0.0"
"#;
		std::fs::write(temp_dir.path().join("Cargo.toml"), cargo_content).unwrap();

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.set_option("yes".to_string(), "true".to_string());
		ctx.add_arg("test-delion".to_string());

		let cmd = PluginRemoveCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_plugin_remove_not_found() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.set_option("yes".to_string(), "true".to_string());
		ctx.add_arg("nonexistent-delion".to_string());

		let cmd = PluginRemoveCommand;
		let error = cmd
			.execute(&ctx)
			.await
			.expect_err("unknown plugins cannot be removed");
		assert!(matches!(
			error,
			CommandError::ExecutionError(message)
				if message == "Failed to remove plugin: plugin not found: nonexistent-delion"
		));
	}

	#[tokio::test]
	async fn test_plugin_remove_no_argument() {
		let ctx = CommandContext::default();

		let cmd = PluginRemoveCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_err());
	}

	// ==========================================================================
	// PluginUpdateCommand Execute Tests
	// ==========================================================================

	#[tokio::test]
	async fn test_plugin_update_no_manifest() {
		let temp_dir = TempDir::new().unwrap();

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		ctx.add_arg("test-delion".to_string());

		let cmd = PluginUpdateCommand;
		let result = cmd.execute(&ctx).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_plugin_update_no_args_no_all_flag() {
		let temp_dir = TempDir::new().unwrap();
		create_test_project(temp_dir.path());
		create_test_manifest(temp_dir.path());

		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);
		// no args, no --all flag

		let cmd = PluginUpdateCommand;
		let error = cmd
			.execute(&ctx)
			.await
			.expect_err("update needs a name or --all");
		assert!(matches!(
			error,
			CommandError::InvalidArguments(message)
				if message == "Please specify a plugin name or use --all"
		));
	}

	// ==========================================================================
	// get_project_root Tests
	// ==========================================================================

	#[test]
	fn test_get_project_root_from_option() {
		let temp_dir = TempDir::new().unwrap();
		let mut ctx = CommandContext::default();
		ctx.set_option(
			"project-root".to_string(),
			temp_dir.path().to_str().unwrap().to_string(),
		);

		let root = get_project_root(&ctx);
		assert_eq!(root, temp_dir.path());
	}

	#[test]
	fn test_get_project_root_defaults_to_current_dir() {
		let ctx = CommandContext::default();

		let root = get_project_root(&ctx);
		assert_eq!(root, std::env::current_dir().unwrap());
	}
}
