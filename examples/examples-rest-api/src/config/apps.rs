//! Application configuration for example-rest-api
//!
//! This module defines the installed applications using compile-time validation.

use reinhardt::installed_apps;

// Define installed applications with compile-time validation
// Note: Framework features (auth, sessions, REST API) are enabled via Cargo.toml feature flags.
// Only register application-specific apps here.
installed_apps! {
	api: "api",
}

// Framework features are enabled in Cargo.toml:
// reinhardt = { features = ["auth", "sessions", "rest", ...] }

/// Get the list of installed applications
pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
