//! Test for the installed_apps! macro with an empty app list.
//!
//! Separated from installed_apps_tests.rs because each test binary
//! can only have one `installed_apps!` invocation (the macro emits a
//! `#[macro_export]` helper that must be unique at crate scope).

use reinhardt_macros::installed_apps;

installed_apps! {}

#[test]
fn test_installed_apps_empty() {
	// Arrange
	use std::str::FromStr;

	// Act
	let apps = InstalledApp::all_apps();
	let from_str_result = InstalledApp::from_str("anything");

	// Assert
	assert_eq!(apps, Vec::<String>::new());
	assert!(from_str_result.is_err());
	assert_eq!(from_str_result.unwrap_err(), "Unknown app: anything");
}
