//! Tests for the installed_apps! macro

use reinhardt_macros::installed_apps;

#[test]
fn test_installed_apps_basic() {
	installed_apps! {
		auth: "myproject.auth",
		contenttypes: "myproject.contenttypes",
	}

	let apps = InstalledApp::all_apps();
	assert_eq!(apps.len(), 2);
	assert!(apps.contains(&"myproject.auth".to_string()));
	assert!(apps.contains(&"myproject.contenttypes".to_string()));
}

#[test]
fn test_installed_apps_enum() {
	installed_apps! {
		auth: "myproject.auth",
		sessions: "myproject.sessions",
	}

	assert_eq!(InstalledApp::auth.path(), "myproject.auth");
	assert_eq!(InstalledApp::sessions.path(), "myproject.sessions");
}

#[test]
fn test_installed_apps_display() {
	installed_apps! {
		auth: "myproject.auth",
	}

	let app = InstalledApp::auth;
	assert_eq!(format!("{}", app), "myproject.auth");
}

#[test]
fn test_installed_apps_from_str() {
	installed_apps! {
		auth: "myproject.auth",
		sessions: "myproject.sessions",
	}

	use std::str::FromStr;

	let auth = InstalledApp::from_str("myproject.auth");
	assert!(auth.is_ok());
	assert_eq!(auth.unwrap(), InstalledApp::auth);

	let invalid = InstalledApp::from_str("invalid.app");
	assert!(invalid.is_err());
}

#[test]
fn test_installed_apps_with_user_apps() {
	installed_apps! {
		auth: "myproject.auth",
		myapp: "apps.myapp",
		another: "custom.another",
	}

	let apps = InstalledApp::all_apps();
	assert_eq!(apps.len(), 3);
	assert!(apps.contains(&"apps.myapp".to_string()));
	assert!(apps.contains(&"custom.another".to_string()));
}

#[test]
fn test_installed_apps_single_app() {
	installed_apps! {
		auth: "myproject.auth",
	}

	let apps = InstalledApp::all_apps();
	assert_eq!(apps.len(), 1);
}

#[test]
fn test_installed_apps_equality() {
	installed_apps! {
		auth: "myproject.auth",
		sessions: "myproject.sessions",
	}

	let app1 = InstalledApp::auth;
	let app2 = InstalledApp::auth;
	let app3 = InstalledApp::sessions;

	assert_eq!(app1, app2);
	assert_ne!(app1, app3);
}

#[test]
fn test_installed_apps_debug() {
	installed_apps! {
		auth: "myproject.auth",
	}

	let app = InstalledApp::auth;
	let debug_str = format!("{:?}", app);
	assert!(debug_str.contains("auth"));
}
