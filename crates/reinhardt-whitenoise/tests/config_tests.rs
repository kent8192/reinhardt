//! Configuration module tests

use reinhardt_whitenoise::WhiteNoiseError;
use reinhardt_whitenoise::config::WhiteNoiseConfig;
use rstest::rstest;
use std::path::PathBuf;
use tempfile::TempDir;

#[rstest]
fn test_config_default_values() {
	let config = WhiteNoiseConfig::default();
	assert_eq!(config.max_age_immutable, 31536000);
	assert_eq!(config.max_age_mutable, 60);
	assert!(config.enable_gzip);
	assert!(config.enable_brotli);
	assert_eq!(config.gzip_level, 6);
	assert_eq!(config.brotli_quality, 11);
	assert_eq!(config.min_compress_size, 1024);
	assert!(!config.allow_all_origins);
	assert!(config.manifest_path.is_none());
	assert!(!config.follow_symlinks);
}

#[rstest]
fn test_config_builder_pattern() {
	let temp_dir = TempDir::new().unwrap();
	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
		.with_compression(false, true)
		.with_max_age_immutable(3600)
		.with_max_age_mutable(120)
		.with_cors(true);

	assert_eq!(config.root, temp_dir.path());
	assert_eq!(config.static_url, "/static/");
	assert!(!config.enable_gzip);
	assert!(config.enable_brotli);
	assert_eq!(config.max_age_immutable, 3600);
	assert_eq!(config.max_age_mutable, 120);
	assert!(config.allow_all_origins);
}

#[rstest]
fn test_config_validate_success() {
	let temp_dir = TempDir::new().unwrap();
	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string());
	assert!(config.validate().is_ok());
}

#[rstest]
fn test_config_validate_nonexistent_root() {
	let config = WhiteNoiseConfig::new(PathBuf::from("/nonexistent/path"), "/static/".to_string());
	let result = config.validate();
	assert!(result.is_err());
	match result.unwrap_err() {
		WhiteNoiseError::InvalidConfig(msg) => {
			assert!(msg.contains("does not exist"));
		}
		_ => panic!("Expected InvalidConfig error"),
	}
}

#[rstest]
fn test_config_validate_file_not_directory() {
	let temp_dir = TempDir::new().unwrap();
	let file_path = temp_dir.path().join("file.txt");
	std::fs::write(&file_path, "test").unwrap();

	let config = WhiteNoiseConfig::new(file_path, "/static/".to_string());
	let result = config.validate();
	assert!(result.is_err());
	match result.unwrap_err() {
		WhiteNoiseError::InvalidConfig(msg) => {
			assert!(msg.contains("not a directory"));
		}
		_ => panic!("Expected InvalidConfig error"),
	}
}

#[rstest]
#[case(0, true)]
#[case(5, true)]
#[case(9, true)]
#[case(10, false)]
#[case(15, false)]
fn test_config_validate_gzip_level(#[case] level: u32, #[case] should_pass: bool) {
	let temp_dir = TempDir::new().unwrap();
	let mut config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string());
	config.gzip_level = level;

	let result = config.validate();
	assert_eq!(result.is_ok(), should_pass);
}

#[rstest]
#[case(0, true)]
#[case(6, true)]
#[case(11, true)]
#[case(12, false)]
#[case(15, false)]
fn test_config_validate_brotli_quality(#[case] quality: u32, #[case] should_pass: bool) {
	let temp_dir = TempDir::new().unwrap();
	let mut config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string());
	config.brotli_quality = quality;

	let result = config.validate();
	assert_eq!(result.is_ok(), should_pass);
}

#[rstest]
fn test_config_with_manifest() {
	let temp_dir = TempDir::new().unwrap();
	let manifest_path = temp_dir.path().join("manifest.json");

	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
		.with_manifest(manifest_path.clone());

	assert_eq!(config.manifest_path, Some(manifest_path));
}
