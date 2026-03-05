//! Unit tests for app discovery functionality

use reinhardt_apps::{
	AppConfig, LocaleProvider, MediaProvider, StaticFilesProvider, get_app_commands,
	get_app_locales, get_app_media, get_app_static_files,
};
use rstest::rstest;

/// Test StaticFilesProvider default implementation
///
/// Verifies that the default implementation returns None when the static directory doesn't exist
#[rstest]
fn test_static_files_provider_default() {
	let config = AppConfig::new("test_app", "myproject.test_app");

	// Default implementation returns None if no path is set
	let static_dir = config.static_dir();
	assert!(static_dir.is_none());
}

/// Test StaticFilesProvider with non-existent path
///
/// Verifies that the provider returns None when the path doesn't have a static directory
#[rstest]
fn test_static_files_provider_nonexistent_path() {
	// Arrange - set path directly to a non-existent relative path
	let mut config = AppConfig::new("test_app", "myproject.test_app");
	config.path = Some("nonexistent/path/to/app".to_string());

	// Act - should return None because nonexistent/path/to/app/static doesn't exist
	let static_dir = config.static_dir();

	// Assert
	assert!(static_dir.is_none());
}

/// Test LocaleProvider default implementation
///
/// Verifies that the default implementation returns None when the locale directory doesn't exist
#[rstest]
fn test_locale_provider_default() {
	let config = AppConfig::new("test_app", "myproject.test_app");

	// Default implementation returns None if no path is set
	let locale_dir = config.locale_dir();
	assert!(locale_dir.is_none());
}

/// Test MediaProvider default implementation
///
/// Verifies that the default implementation returns None when the media directory doesn't exist
#[rstest]
fn test_media_provider_default() {
	let config = AppConfig::new("test_app", "myproject.test_app");

	// Default implementation returns None if no path is set
	let media_dir = config.media_dir();
	assert!(media_dir.is_none());
}

/// Test get_app_static_files function
///
/// Verifies that the inventory iterator can be called without errors
/// Note: The actual content depends on what's registered in the binary
#[rstest]
fn test_get_app_static_files() {
	let configs = get_app_static_files();

	// Should be callable and return a Vec
	// The length depends on what's registered via inventory in the test binary
	// Just verify it's a valid Vec (no panic)
	let _count = configs.len();
}

/// Test get_app_locales function
///
/// Verifies that the inventory iterator can be called without errors
#[rstest]
fn test_get_app_locales() {
	let configs = get_app_locales();

	// Should be callable and return a Vec (no panic)
	let _count = configs.len();
}

/// Test get_app_commands function
///
/// Verifies that the inventory iterator can be called without errors
#[rstest]
fn test_get_app_commands() {
	let configs = get_app_commands();

	// Should be callable and return a Vec (no panic)
	let _count = configs.len();
}

/// Test get_app_media function
///
/// Verifies that the inventory iterator can be called without errors
#[rstest]
fn test_get_app_media() {
	let configs = get_app_media();

	// Should be callable and return a Vec (no panic)
	let _count = configs.len();
}

/// Test StaticFilesProvider with existing directory
///
/// Creates a temporary directory structure and verifies the provider finds it
#[rstest]
fn test_static_files_provider_with_existing_dir() {
	use std::fs;

	// Arrange - create temporary directory structure
	let temp_dir = std::env::temp_dir().join("reinhardt_test_app_discovery");
	let static_dir = temp_dir.join("static");
	fs::create_dir_all(&static_dir).unwrap();

	// Set the path field directly (absolute paths are used for filesystem lookup in tests)
	let mut config = AppConfig::new("test_app", "myproject.test_app");
	config.path = Some(temp_dir.to_str().unwrap().to_string());

	// Act
	let result = config.static_dir();

	// Assert
	assert!(result.is_some());
	assert_eq!(result.unwrap(), static_dir);

	// Cleanup
	fs::remove_dir_all(&temp_dir).ok();
}

/// Test LocaleProvider with existing directory
///
/// Creates a temporary directory structure and verifies the provider finds it
#[rstest]
fn test_locale_provider_with_existing_dir() {
	use std::fs;

	// Arrange - create temporary directory structure
	let temp_dir = std::env::temp_dir().join("reinhardt_test_app_locale");
	let locale_dir = temp_dir.join("locale");
	fs::create_dir_all(&locale_dir).unwrap();

	// Set the path field directly (absolute paths are used for filesystem lookup in tests)
	let mut config = AppConfig::new("test_app", "myproject.test_app");
	config.path = Some(temp_dir.to_str().unwrap().to_string());

	// Act
	let result = config.locale_dir();

	// Assert
	assert!(result.is_some());
	assert_eq!(result.unwrap(), locale_dir);

	// Cleanup
	fs::remove_dir_all(&temp_dir).ok();
}

/// Test MediaProvider with existing directory
///
/// Creates a temporary directory structure and verifies the provider finds it
#[rstest]
fn test_media_provider_with_existing_dir() {
	use std::fs;

	// Arrange - create temporary directory structure
	let temp_dir = std::env::temp_dir().join("reinhardt_test_app_media");
	let media_dir = temp_dir.join("media");
	fs::create_dir_all(&media_dir).unwrap();

	// Set the path field directly (absolute paths are used for filesystem lookup in tests)
	let mut config = AppConfig::new("test_app", "myproject.test_app");
	config.path = Some(temp_dir.to_str().unwrap().to_string());

	// Act
	let result = config.media_dir();

	// Assert
	assert!(result.is_some());
	assert_eq!(result.unwrap(), media_dir);

	// Cleanup
	fs::remove_dir_all(&temp_dir).ok();
}
