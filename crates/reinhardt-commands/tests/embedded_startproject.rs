//! Integration test proving `startproject` produces a usable project tree
//! from embedded templates alone — no CARGO_MANIFEST_DIR dependency.

use reinhardt_commands::start_commands::StartProjectCommand;
use reinhardt_commands::{BaseCommand, CommandContext};
use rstest::*;
use serial_test::serial;
use std::collections::HashMap;
use tempfile::TempDir;

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_restful_from_embedded_only() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["sample_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("restful".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	// Act
	let res = StartProjectCommand.execute(&ctx).await;

	// Assert
	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject succeeds from embedded templates");
	let generated = tmp.path().join("sample_proj");
	assert!(
		generated.join("Cargo.toml").exists(),
		"Cargo.toml must be generated"
	);
	assert!(
		generated.join("src").is_dir(),
		"src/ directory must be generated"
	);
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_pages_from_embedded_only() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["sample_pages_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	// Act
	let res = StartProjectCommand.execute(&ctx).await;

	// Assert
	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject --with-pages succeeds from embedded templates");
	let generated = tmp.path().join("sample_pages_proj");
	assert!(
		generated.join("Cargo.toml").exists(),
		"Cargo.toml must be generated"
	);
	assert!(
		generated.join("src").is_dir(),
		"src/ directory must be generated"
	);
}
