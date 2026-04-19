//! Tests for the installed_apps! macro
//!
//! Uses a single `installed_apps!` invocation since the macro emits a
//! `#[macro_export]` helper (`__reinhardt_for_each_app`) that must be
//! unique at crate scope.

use reinhardt_macros::installed_apps;

installed_apps! {
	auth: "myproject.auth",
	sessions: "myproject.sessions",
	contenttypes: "myproject.contenttypes",
	myapp: "apps.myapp",
	another: "custom.another",
}

#[test]
fn test_installed_apps_basic() {
	let apps = InstalledApp::all_apps();
	assert_eq!(apps.len(), 5);
	assert!(apps.contains(&"myproject.auth".to_string()));
	assert!(apps.contains(&"myproject.contenttypes".to_string()));
}

#[test]
fn test_installed_apps_enum() {
	assert_eq!(InstalledApp::auth.path(), "myproject.auth");
	assert_eq!(InstalledApp::sessions.path(), "myproject.sessions");
}

#[test]
fn test_installed_apps_display() {
	let app = InstalledApp::auth;
	assert_eq!(format!("{}", app), "myproject.auth");
}

#[test]
fn test_installed_apps_from_str() {
	use std::str::FromStr;

	let auth = InstalledApp::from_str("myproject.auth");
	assert!(auth.is_ok());
	assert_eq!(auth.unwrap(), InstalledApp::auth);

	let invalid = InstalledApp::from_str("invalid.app");
	assert!(invalid.is_err());
}

#[test]
fn test_installed_apps_with_user_apps() {
	let apps = InstalledApp::all_apps();
	assert!(apps.contains(&"apps.myapp".to_string()));
	assert!(apps.contains(&"custom.another".to_string()));
}

#[test]
fn test_installed_apps_equality() {
	let app1 = InstalledApp::auth;
	let app2 = InstalledApp::auth;
	let app3 = InstalledApp::sessions;

	assert_eq!(app1, app2);
	assert_ne!(app1, app3);
}

#[test]
fn test_installed_apps_debug() {
	let app = InstalledApp::auth;
	let debug_str = format!("{:?}", app);
	assert!(debug_str.contains("auth"));
}

#[test]
fn installed_app_implements_app_label_trait() {
	// Arrange
	use reinhardt_apps::apps::AppLabel;
	let app = InstalledApp::auth;

	// Act
	let label: &'static str = <InstalledApp as AppLabel>::path(&app);

	// Assert
	assert_eq!(label, "myproject.auth");
}
