//! Project-specific management shell configuration.

use crate::config::apps::InstalledApp;
use crate::config::settings::ProjectSettings;
use reinhardt::commands::ShellConfig;

pub use reinhardt as framework;

pub type ShellSettings = ProjectSettings;
pub type ProjectShellEnvironment = framework::commands::ShellEnvironment<ShellSettings>;
pub type ShellDatabase = framework::db::orm::DatabaseConnection;
pub type ShellDi = std::sync::Arc<framework::di::InjectionContext>;

pub fn get_shell_config() -> ShellConfig {
	ShellConfig::new(
		env!("CARGO_PKG_NAME"),
		"examples_tutorial_rest",
		env!("CARGO_MANIFEST_DIR"),
		"examples_tutorial_rest::config::settings::get_settings",
		InstalledApp::all_labels().iter().copied(),
	)
}
