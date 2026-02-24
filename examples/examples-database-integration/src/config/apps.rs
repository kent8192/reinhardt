//! Application configuration for database-integration example
//!
//! This module defines the installed applications.
//! Framework features (auth, sessions, admin, etc.) are enabled via Cargo feature flags.

use reinhardt::installed_apps;

// Define user-defined installed applications.
// Framework features are enabled via Cargo feature flags, not through installed_apps!.
installed_apps! {
	todos: "todos",
}

/// Get the list of installed applications
pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
