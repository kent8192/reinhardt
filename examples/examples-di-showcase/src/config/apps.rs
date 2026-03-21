//! Application configuration for examples-di-showcase

use reinhardt::installed_apps;

installed_apps! {
	di_showcase: "di_showcase",
}

/// Get the list of installed applications
pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
