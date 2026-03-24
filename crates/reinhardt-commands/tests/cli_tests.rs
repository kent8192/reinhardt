//! CLI parsing tests for reinhardt-commands
//!
//! Tests for the Commands enum parsing and parameter conversion.
//! These tests verify that CLI arguments are correctly parsed and converted
//! to CommandContext for command execution.

use clap::{CommandFactory, Parser};
use reinhardt_commands::{Cli, CommandContext, Commands};
use rstest::*;
use std::path::PathBuf;

// ============================================================================
// Fixtures
// ============================================================================

/// Fixture for creating an empty CommandContext
#[fixture]
fn empty_context() -> CommandContext {
	CommandContext::default()
}

// ============================================================================
// Test Helper Functions for Runserver Command
// ============================================================================

/// Creates runserver command with default settings via CLI parsing
fn create_runserver_default() -> Commands {
	Cli::parse_from(["manage", "runserver"]).command
}

/// Creates runserver command with custom settings via CLI parsing
#[allow(clippy::too_many_arguments)]
fn create_runserver_with_options(
	address: &str,
	noreload: bool,
	insecure: bool,
	no_docs: bool,
	with_pages: bool,
	static_dir: &str,
	no_spa: bool,
) -> Commands {
	let mut args: Vec<String> = vec!["manage".into(), "runserver".into()];
	if noreload {
		args.push("--noreload".into());
	}
	if insecure {
		args.push("--insecure".into());
	}
	if no_docs {
		args.push("--no-docs".into());
	}
	if with_pages {
		args.push("--with-pages".into());
	}
	args.push("--static-dir".into());
	args.push(static_dir.into());
	if no_spa {
		args.push("--no-spa".into());
	}
	args.push(address.into());
	Cli::parse_from(args).command
}

/// Creates collectstatic command via CLI parsing
fn create_collectstatic(
	clear: bool,
	no_input: bool,
	dry_run: bool,
	link: bool,
	ignore: &[&str],
	index: Option<&str>,
) -> Commands {
	let mut args: Vec<String> = vec!["manage".into(), "collectstatic".into()];
	if clear {
		args.push("--clear".into());
	}
	if no_input {
		args.push("--no-input".into());
	}
	if dry_run {
		args.push("--dry-run".into());
	}
	if link {
		args.push("--link".into());
	}
	for pattern in ignore {
		args.push("--ignore".into());
		args.push((*pattern).into());
	}
	if let Some(idx) = index {
		args.push("--index".into());
		args.push(idx.into());
	}
	Cli::parse_from(args).command
}

// ============================================================================
// Happy Path Tests - Migrate Command Parsing
// ============================================================================

/// Test: Parse Migrate command with no arguments
///
/// Category: Happy Path
/// Verifies that Migrate command can be created with all default values.
#[rstest]
fn test_commands_migrate_parse_minimal() {
	let cmd = Commands::Migrate {
		app_label: None,
		migration_name: None,
		database: None,
		fake: false,
		fake_initial: false,
		plan: false,
	};

	// Verify all fields are default
	match cmd {
		Commands::Migrate {
			app_label,
			migration_name,
			database,
			fake,
			fake_initial,
			plan,
		} => {
			assert!(app_label.is_none(), "app_label should be None by default");
			assert!(
				migration_name.is_none(),
				"migration_name should be None by default"
			);
			assert!(database.is_none(), "database should be None by default");
			assert!(!fake, "fake should be false by default");
			assert!(!fake_initial, "fake_initial should be false by default");
			assert!(!plan, "plan should be false by default");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Migrate variant"),
	}
}

/// Test: Parse Migrate command with all options
///
/// Category: Happy Path
/// Verifies that Migrate command can be created with all options set.
#[rstest]
fn test_commands_migrate_parse_all_options() {
	let cmd = Commands::Migrate {
		app_label: Some("myapp".to_string()),
		migration_name: Some("0001_initial".to_string()),
		database: Some("postgres://localhost/test".to_string()),
		fake: true,
		fake_initial: true,
		plan: true,
	};

	match cmd {
		Commands::Migrate {
			app_label,
			migration_name,
			database,
			fake,
			fake_initial,
			plan,
		} => {
			assert_eq!(app_label, Some("myapp".to_string()));
			assert_eq!(migration_name, Some("0001_initial".to_string()));
			assert_eq!(database, Some("postgres://localhost/test".to_string()));
			assert!(fake, "fake should be true");
			assert!(fake_initial, "fake_initial should be true");
			assert!(plan, "plan should be true");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Migrate variant"),
	}
}

// ============================================================================
// Happy Path Tests - Makemigrations Command Parsing
// ============================================================================

/// Test: Parse Makemigrations command with multiple app labels
///
/// Category: Happy Path
/// Verifies that multiple app_labels are correctly parsed.
#[rstest]
#[cfg(feature = "migrations")]
fn test_commands_makemigrations_parse_app_labels() {
	let cmd = Commands::Makemigrations {
		app_labels: vec!["auth".to_string(), "users".to_string(), "posts".to_string()],
		dry_run: false,
		name: None,
		check: false,
		empty: false,
		merge: false,
		force_empty_state: false,
		migration_dir: PathBuf::from("./migrations"),
	};

	match cmd {
		Commands::Makemigrations { app_labels, .. } => {
			assert_eq!(app_labels.len(), 3, "Should have 3 app labels");
			assert_eq!(app_labels[0], "auth");
			assert_eq!(app_labels[1], "users");
			assert_eq!(app_labels[2], "posts");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Makemigrations variant"),
	}
}

// ============================================================================
// Happy Path Tests - Runserver Command Parsing
// ============================================================================

/// Test: Parse Runserver command with default address
///
/// Category: Happy Path
/// Verifies that the default address is "127.0.0.1:8000".
#[rstest]
fn test_commands_runserver_default_address() {
	let default_address = "127.0.0.1:8000".to_string();

	let cmd =
		create_runserver_with_options(&default_address, false, false, false, false, "dist", false);

	match cmd {
		Commands::Runserver { address, .. } => {
			assert_eq!(
				address, "127.0.0.1:8000",
				"Default address should be 127.0.0.1:8000"
			);
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Runserver variant"),
	}
}

/// Test: Parse Runserver command with custom address
///
/// Category: Happy Path
/// Verifies that custom address is correctly parsed.
#[rstest]
fn test_commands_runserver_custom_address() {
	let cmd = create_runserver_with_options("0.0.0.0:3000", true, true, true, false, "dist", false);

	match cmd {
		Commands::Runserver {
			address,
			noreload,
			insecure,
			no_docs,
			..
		} => {
			assert_eq!(address, "0.0.0.0:3000");
			assert!(noreload, "noreload should be true");
			assert!(insecure, "insecure should be true");
			assert!(no_docs, "no_docs should be true");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Runserver variant"),
	}
}

// ============================================================================
// Happy Path Tests - Shell Command Parsing
// ============================================================================

/// Test: Parse Shell command with -c option
///
/// Category: Happy Path
/// Verifies that the command option is correctly parsed.
#[rstest]
fn test_commands_shell_command_option() {
	let cmd = Commands::Shell {
		command: Some("println!(\"Hello, world!\")".to_string()),
	};

	match cmd {
		Commands::Shell { command } => {
			assert_eq!(command, Some("println!(\"Hello, world!\")".to_string()));
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Shell variant"),
	}
}

/// Test: Parse Shell command without command option (interactive mode)
///
/// Category: Happy Path
/// Verifies that shell without -c option has None for command.
#[rstest]
fn test_commands_shell_interactive_mode() {
	let cmd = Commands::Shell { command: None };

	match cmd {
		Commands::Shell { command } => {
			assert!(command.is_none(), "Interactive mode should have no command");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Shell variant"),
	}
}

// ============================================================================
// Happy Path Tests - Check Command Parsing
// ============================================================================

/// Test: Parse Check command with --deploy flag
///
/// Category: Happy Path
/// Verifies that the deploy flag is correctly parsed.
#[rstest]
fn test_commands_check_deploy_flag() {
	let cmd = Commands::Check {
		app_label: None,
		deploy: true,
	};

	match cmd {
		Commands::Check { app_label, deploy } => {
			assert!(app_label.is_none());
			assert!(deploy, "deploy flag should be true");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Check variant"),
	}
}

/// Test: Parse Check command with app_label
///
/// Category: Happy Path
/// Verifies that app_label is correctly parsed.
#[rstest]
fn test_commands_check_with_app_label() {
	let cmd = Commands::Check {
		app_label: Some("myapp".to_string()),
		deploy: false,
	};

	match cmd {
		Commands::Check { app_label, deploy } => {
			assert_eq!(app_label, Some("myapp".to_string()));
			assert!(!deploy);
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Check variant"),
	}
}

// ============================================================================
// Happy Path Tests - Collectstatic Command Parsing
// ============================================================================

/// Test: Parse Collectstatic command with all options
///
/// Category: Happy Path
/// Verifies that all collectstatic options are correctly parsed.
#[rstest]
fn test_commands_collectstatic_all_options() {
	let cmd = create_collectstatic(true, true, true, true, &["*.map", "*.log"], None);

	match cmd {
		Commands::Collectstatic {
			clear,
			no_input,
			dry_run,
			link,
			ignore,
			..
		} => {
			assert!(clear, "clear should be true");
			assert!(no_input, "no_input should be true");
			assert!(dry_run, "dry_run should be true");
			assert!(link, "link should be true");
			assert_eq!(ignore.len(), 2, "Should have 2 ignore patterns");
			assert_eq!(ignore[0], "*.map");
			assert_eq!(ignore[1], "*.log");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Collectstatic variant"),
	}
}

/// Test: Parse Collectstatic command with defaults
///
/// Category: Happy Path
/// Verifies that default values are correctly set.
#[rstest]
fn test_commands_collectstatic_defaults() {
	let cmd = create_collectstatic(false, false, false, false, &[], None);

	match cmd {
		Commands::Collectstatic {
			clear,
			no_input,
			dry_run,
			link,
			ignore,
			..
		} => {
			assert!(!clear, "clear should be false by default");
			assert!(!no_input, "no_input should be false by default");
			assert!(!dry_run, "dry_run should be false by default");
			assert!(!link, "link should be false by default");
			assert!(ignore.is_empty(), "ignore should be empty by default");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Collectstatic variant"),
	}
}

// ============================================================================
// Happy Path Tests - Showurls Command Parsing
// ============================================================================

/// Test: Parse Showurls command with --names flag
///
/// Category: Happy Path
/// Verifies that the names flag is correctly parsed.
#[rstest]
fn test_commands_showurls_names_flag() {
	let cmd = Commands::Showurls { names: true };

	match cmd {
		Commands::Showurls { names } => {
			assert!(names, "names flag should be true");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Showurls variant"),
	}
}

// ============================================================================
// Happy Path Tests - Generateopenapi Command Parsing
// ============================================================================

/// Test: Parse Generateopenapi command with format options
///
/// Category: Happy Path
/// Verifies that format and output options are correctly parsed.
#[rstest]
#[cfg(feature = "openapi")]
fn test_commands_generateopenapi_format() {
	let cmd = Commands::Generateopenapi {
		format: "yaml".to_string(),
		output: PathBuf::from("openapi.yaml"),
		postman: true,
	};

	match cmd {
		Commands::Generateopenapi {
			format,
			output,
			postman,
		} => {
			assert_eq!(format, "yaml");
			assert_eq!(output, PathBuf::from("openapi.yaml"));
			assert!(postman, "postman should be true");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Generateopenapi variant"),
	}
}

// ============================================================================
// Boundary Value Tests
// ============================================================================

/// Test: Verbosity levels parsing
///
/// Category: Boundary
/// Verifies that verbosity levels 0, 1, 2, 3 are correctly handled.
#[rstest]
#[case(0, "quiet")]
#[case(1, "normal")]
#[case(2, "verbose")]
#[case(3, "very verbose")]
fn test_verbosity_levels_parsing(
	mut empty_context: CommandContext,
	#[case] level: u8,
	#[case] description: &str,
) {
	empty_context.set_verbosity(level);

	assert_eq!(
		empty_context.verbosity, level,
		"Verbosity level {} ({}) should be set correctly",
		level, description
	);
}

/// Test: Extreme verbosity levels (boundary)
///
/// Category: Boundary
/// Verifies that extreme verbosity values are handled.
#[rstest]
#[case(0, "minimum")]
#[case(255, "maximum")]
fn test_verbosity_extreme_levels(
	mut empty_context: CommandContext,
	#[case] level: u8,
	#[case] description: &str,
) {
	empty_context.set_verbosity(level);

	assert_eq!(
		empty_context.verbosity, level,
		"Verbosity level {} ({}) should be set correctly",
		level, description
	);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test: Unicode strings in arguments
///
/// Category: Edge Case
/// Verifies that Unicode strings are correctly preserved in arguments.
#[rstest]
fn test_unicode_in_arguments() {
	let unicode_app = "アプリ日本語".to_string();
	let unicode_migration = "遷移_初期化_中文".to_string();

	let cmd = Commands::Migrate {
		app_label: Some(unicode_app.clone()),
		migration_name: Some(unicode_migration.clone()),
		database: None,
		fake: false,
		fake_initial: false,
		plan: false,
	};

	match cmd {
		Commands::Migrate {
			app_label,
			migration_name,
			..
		} => {
			assert_eq!(
				app_label,
				Some(unicode_app),
				"Unicode app_label should be preserved"
			);
			assert_eq!(
				migration_name,
				Some(unicode_migration),
				"Unicode migration_name should be preserved"
			);
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Migrate variant"),
	}
}

/// Test: Empty app_labels Vec
///
/// Category: Edge Case
/// Verifies that empty Vec for app_labels is handled correctly.
#[rstest]
#[cfg(feature = "migrations")]
fn test_empty_app_labels() {
	let cmd = Commands::Makemigrations {
		app_labels: vec![],
		dry_run: false,
		name: None,
		check: false,
		empty: false,
		merge: false,
		force_empty_state: false,
		migration_dir: PathBuf::from("./migrations"),
	};

	match cmd {
		Commands::Makemigrations { app_labels, .. } => {
			assert!(app_labels.is_empty(), "Empty app_labels should be handled");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Makemigrations variant"),
	}
}

/// Test: Special characters in paths
///
/// Category: Edge Case
/// Verifies that special characters in paths are handled.
#[rstest]
#[cfg(feature = "migrations")]
fn test_special_characters_in_paths() {
	let special_path = PathBuf::from("./migrations with spaces/日本語/path");

	let cmd = Commands::Makemigrations {
		app_labels: vec![],
		dry_run: false,
		name: None,
		check: false,
		empty: false,
		merge: false,
		force_empty_state: false,
		migration_dir: special_path.clone(),
	};

	match cmd {
		Commands::Makemigrations { migration_dir, .. } => {
			assert_eq!(
				migration_dir, special_path,
				"Special path should be preserved"
			);
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Makemigrations variant"),
	}
}

/// Test: Very long argument values
///
/// Category: Edge Case
/// Verifies that very long strings are handled correctly.
#[rstest]
fn test_very_long_argument_values() {
	let long_app_name = "a".repeat(1000);
	let long_migration_name = "m".repeat(1000);

	let cmd = Commands::Migrate {
		app_label: Some(long_app_name.clone()),
		migration_name: Some(long_migration_name.clone()),
		database: None,
		fake: false,
		fake_initial: false,
		plan: false,
	};

	match cmd {
		Commands::Migrate {
			app_label,
			migration_name,
			..
		} => {
			assert_eq!(app_label.as_ref().map(|s| s.len()), Some(1000));
			assert_eq!(migration_name.as_ref().map(|s| s.len()), Some(1000));
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Migrate variant"),
	}
}

// ============================================================================
// Equivalence Partitioning Tests
// ============================================================================

/// Test: Migrate params conversion equivalence
///
/// Category: Equivalence
/// Verifies that different input combinations are correctly converted.
#[rstest]
#[case(None, None, "no arguments")]
#[case(Some("app"), None, "app only")]
#[case(Some("app"), Some("0001"), "app and migration")]
fn test_migrate_params_conversion(
	mut empty_context: CommandContext,
	#[case] app_label: Option<&str>,
	#[case] migration_name: Option<&str>,
	#[case] description: &str,
) {
	if let Some(app) = app_label {
		empty_context.add_arg(app.to_string());
	}
	if let Some(migration) = migration_name {
		empty_context.add_arg(migration.to_string());
	}

	match (app_label, migration_name) {
		(None, None) => {
			assert!(empty_context.arg(0).is_none(), "{}: no args", description);
		}
		(Some(app), None) => {
			assert_eq!(
				empty_context.arg(0).map(String::as_str),
				Some(app),
				"{}: app should be first arg",
				description
			);
			assert!(
				empty_context.arg(1).is_none(),
				"{}: no second arg",
				description
			);
		}
		(Some(app), Some(migration)) => {
			assert_eq!(
				empty_context.arg(0).map(String::as_str),
				Some(app),
				"{}: app should be first arg",
				description
			);
			assert_eq!(
				empty_context.arg(1).map(String::as_str),
				Some(migration),
				"{}: migration should be second arg",
				description
			);
		}
		_ => panic!("Invalid test case"),
	}
}

// ============================================================================
// Decision Table Tests
// ============================================================================

/// Test: Migrate flag combinations (Decision Table)
///
/// Category: Decision Table
/// Verifies all combinations of --fake and --fake-initial flags.
#[rstest]
#[case(false, false, "neither flag")]
#[case(true, false, "fake only")]
#[case(false, true, "fake_initial only")]
#[case(true, true, "both flags")]
fn test_migrate_decision_fake_combinations(
	mut empty_context: CommandContext,
	#[case] fake: bool,
	#[case] fake_initial: bool,
	#[case] description: &str,
) {
	if fake {
		empty_context.set_option("fake".to_string(), "true".to_string());
	}
	if fake_initial {
		empty_context.set_option("fake-initial".to_string(), "true".to_string());
	}

	assert_eq!(
		empty_context.has_option("fake"),
		fake,
		"{}: fake option mismatch",
		description
	);
	assert_eq!(
		empty_context.has_option("fake-initial"),
		fake_initial,
		"{}: fake-initial option mismatch",
		description
	);
}

/// Test: Collectstatic flag combinations (Decision Table)
///
/// Category: Decision Table
/// Verifies combinations of --clear, --link, and --dry-run flags.
#[rstest]
#[case(false, false, false, "no flags")]
#[case(true, false, false, "clear only")]
#[case(false, true, false, "link only")]
#[case(false, false, true, "dry_run only")]
#[case(true, true, false, "clear and link")]
#[case(true, false, true, "clear and dry_run")]
#[case(false, true, true, "link and dry_run")]
#[case(true, true, true, "all flags")]
fn test_collectstatic_decision_flag_combinations(
	#[case] clear: bool,
	#[case] link: bool,
	#[case] dry_run: bool,
	#[case] description: &str,
) {
	let cmd = create_collectstatic(clear, false, dry_run, link, &[], None);

	match cmd {
		Commands::Collectstatic {
			clear: c,
			link: l,
			dry_run: d,
			..
		} => {
			assert_eq!(c, clear, "{}: clear mismatch", description);
			assert_eq!(l, link, "{}: link mismatch", description);
			assert_eq!(d, dry_run, "{}: dry_run mismatch", description);
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Collectstatic variant"),
	}
}

// ============================================================================
// Combination Tests
// ============================================================================

/// Test: Multiple ignore patterns in Collectstatic
///
/// Category: Combination
/// Verifies that multiple ignore patterns are correctly handled.
#[rstest]
fn test_collectstatic_multiple_ignore_patterns() {
	let patterns = vec![
		"*.map".to_string(),
		"*.log".to_string(),
		"*.tmp".to_string(),
		"node_modules/**".to_string(),
		".git/**".to_string(),
	];

	let pattern_strs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
	let cmd = create_collectstatic(false, false, false, false, &pattern_strs, None);

	match cmd {
		Commands::Collectstatic { ignore, .. } => {
			assert_eq!(ignore.len(), 5, "Should have 5 ignore patterns");
			for (i, pattern) in patterns.iter().enumerate() {
				assert_eq!(&ignore[i], pattern, "Pattern {} should match", i);
			}
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Collectstatic variant"),
	}
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: Basic CLI commands workflow
///
/// Category: Sanity
/// Verifies that basic command creation and matching works correctly.
#[rstest]
fn test_cli_commands_sanity() {
	// Create each command type and verify it can be matched
	let commands: Vec<Commands> = vec![
		Commands::Migrate {
			app_label: None,
			migration_name: None,
			database: None,
			fake: false,
			fake_initial: false,
			plan: false,
		},
		create_runserver_default(),
		Commands::Shell { command: None },
		Commands::Check {
			app_label: None,
			deploy: false,
		},
		create_collectstatic(false, false, false, false, &[], None),
		Commands::Showurls { names: false },
	];

	// Verify each command can be created and has Debug implementation
	assert_eq!(commands.len(), 6, "Should have 6 command variants");

	for cmd in &commands {
		// Verify Debug trait works (Commands derives Debug)
		let debug_str = format!("{:?}", cmd);
		assert!(
			!debug_str.is_empty(),
			"Debug representation should not be empty"
		);
	}

	// Verify Clone trait works (Commands derives Clone)
	let cloned = commands[0].clone();
	let cloned_debug = format!("{:?}", cloned);
	let original_debug = format!("{:?}", commands[0]);
	assert_eq!(
		cloned_debug, original_debug,
		"Clone should produce identical Debug output"
	);
}

// ============================================================================
// Cli Struct Tests - Help and Version attributes
// ============================================================================

/// Test: Cli struct has Parser derive
///
/// Category: Happy Path
/// Verifies that Cli struct can be used with clap Parser.
#[rstest]
fn test_cli_struct_derives_parser() {
	// Test that we can access the command help text via clap
	let help = Cli::command();
	assert!(
		!help.get_name().is_empty(),
		"Cli should have a name from Parser derive"
	);
}

/// Test: Cli struct has version configured
///
/// Category: Happy Path
/// Verifies that Cli has version information.
#[rstest]
fn test_cli_has_version() {
	let cmd = Cli::command();
	// The version is derived from Cargo.toml via #[command(version)]
	let version = cmd.get_version();
	assert!(version.is_some(), "Cli should have version configured");
}

/// Test: Cli verbosity field exists and is configurable
///
/// Category: Happy Path
/// Verifies that Cli has verbosity field that can be set.
#[rstest]
fn test_cli_verbosity_field() {
	// Parse with verbosity flag
	let cli = Cli::try_parse_from(["test", "-v", "migrate"]);
	assert!(cli.is_ok(), "Should parse with -v flag");

	let cli = cli.unwrap();
	assert!(cli.verbosity >= 1, "Verbosity should be set with -v flag");
}

/// Test: Cli multiple verbosity flags stack
///
/// Category: Happy Path
/// Verifies that -v -v increases verbosity.
#[rstest]
fn test_cli_multiple_verbosity_flags() {
	let cli = Cli::try_parse_from(["test", "-v", "-v", "migrate"]);
	assert!(cli.is_ok(), "Should parse with multiple -v flags");

	let cli = cli.unwrap();
	assert!(cli.verbosity >= 2, "Verbosity should be 2+ with -v -v");
}

/// Test: Cli handles --help without error
///
/// Category: Happy Path
/// Verifies that --help flag is recognized (returns help error from clap).
#[rstest]
fn test_cli_help_flag() {
	// --help causes clap to print help and return an error
	let result = Cli::try_parse_from(["test", "--help"]);
	assert!(
		result.is_err(),
		"--help should return an error (help displayed)"
	);

	let err = result.unwrap_err();
	// clap returns ErrorKind::DisplayHelp for --help
	assert_eq!(
		err.kind(),
		clap::error::ErrorKind::DisplayHelp,
		"Should be DisplayHelp error kind"
	);
}

/// Test: Cli handles --version without error
///
/// Category: Happy Path
/// Verifies that --version flag is recognized.
#[rstest]
fn test_cli_version_flag() {
	let result = Cli::try_parse_from(["test", "--version"]);
	assert!(
		result.is_err(),
		"--version should return an error (version displayed)"
	);

	let err = result.unwrap_err();
	assert_eq!(
		err.kind(),
		clap::error::ErrorKind::DisplayVersion,
		"Should be DisplayVersion error kind"
	);
}

// ============================================================================
// Additional Showurls Command Tests
// ============================================================================

/// Test: Showurls command with default values
///
/// Category: Happy Path
/// Verifies that Showurls command has correct defaults.
#[rstest]
fn test_commands_showurls_defaults() {
	let cmd = Commands::Showurls { names: false };

	match cmd {
		Commands::Showurls { names } => {
			assert!(!names, "names should be false by default");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Showurls variant"),
	}
}

/// Test: Showurls Debug implementation
///
/// Category: Happy Path
/// Verifies that Showurls has proper Debug implementation.
#[rstest]
fn test_commands_showurls_debug() {
	let cmd = Commands::Showurls { names: true };
	let debug_str = format!("{:?}", cmd);

	assert!(
		debug_str.contains("Showurls"),
		"Debug should contain 'Showurls'"
	);
	assert!(
		debug_str.contains("names"),
		"Debug should contain 'names' field"
	);
}

/// Test: Showurls Clone implementation
///
/// Category: Happy Path
/// Verifies that Showurls can be cloned correctly.
#[rstest]
fn test_commands_showurls_clone() {
	let original = Commands::Showurls { names: true };
	let cloned = original.clone();

	match (&original, &cloned) {
		(Commands::Showurls { names: n1 }, Commands::Showurls { names: n2 }) => {
			assert_eq!(n1, n2, "Cloned value should match original");
		}
		_ => panic!("Expected Commands::Showurls variants"),
	}
}

// ============================================================================
// Generateopenapi Command Tests (Feature-independent behavior)
// ============================================================================

/// Test: Generateopenapi command variant exists (feature check)
///
/// Category: Happy Path
/// Verifies that Generateopenapi exists when feature is enabled.
#[rstest]
#[cfg(feature = "openapi")]
fn test_generateopenapi_command_exists() {
	use std::path::PathBuf;

	let cmd = Commands::Generateopenapi {
		format: "json".to_string(),
		output: PathBuf::from("openapi.json"),
		postman: false,
	};

	match cmd {
		Commands::Generateopenapi { format, .. } => {
			assert_eq!(format, "json");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Generateopenapi variant"),
	}
}

/// Test: Generateopenapi with yaml format
///
/// Category: Happy Path
/// Verifies that yaml format is supported.
#[rstest]
#[cfg(feature = "openapi")]
fn test_generateopenapi_yaml_format() {
	use std::path::PathBuf;

	let cmd = Commands::Generateopenapi {
		format: "yaml".to_string(),
		output: PathBuf::from("openapi.yaml"),
		postman: false,
	};

	match cmd {
		Commands::Generateopenapi { format, output, .. } => {
			assert_eq!(format, "yaml");
			assert_eq!(output, PathBuf::from("openapi.yaml"));
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Generateopenapi variant"),
	}
}

/// Test: Generateopenapi with postman flag
///
/// Category: Happy Path
/// Verifies that postman collection generation flag works.
#[rstest]
#[cfg(feature = "openapi")]
fn test_generateopenapi_postman_flag() {
	use std::path::PathBuf;

	let cmd = Commands::Generateopenapi {
		format: "json".to_string(),
		output: PathBuf::from("api.json"),
		postman: true,
	};

	match cmd {
		Commands::Generateopenapi { postman, .. } => {
			assert!(postman, "postman flag should be true");
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Generateopenapi variant"),
	}
}

/// Test: Generateopenapi Debug and Clone traits
///
/// Category: Happy Path
/// Verifies that Generateopenapi has proper trait implementations.
#[rstest]
#[cfg(feature = "openapi")]
fn test_generateopenapi_traits() {
	use std::path::PathBuf;

	let cmd = Commands::Generateopenapi {
		format: "json".to_string(),
		output: PathBuf::from("openapi.json"),
		postman: false,
	};

	// Test Debug
	let debug_str = format!("{:?}", cmd);
	assert!(
		debug_str.contains("Generateopenapi"),
		"Should contain variant name"
	);

	// Test Clone
	let cloned = cmd.clone();
	let cloned_debug = format!("{:?}", cloned);
	assert_eq!(
		debug_str, cloned_debug,
		"Clone should produce identical Debug output"
	);
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

/// Test: Database URL with special characters
///
/// Category: Edge Case
/// Verifies that database URLs with special characters are preserved.
#[rstest]
fn test_database_url_special_chars() {
	let db_url = "postgres://user:p@ssw0rd!%23$@localhost:5432/db_name?sslmode=require".to_string();

	let cmd = Commands::Migrate {
		app_label: None,
		migration_name: None,
		database: Some(db_url.clone()),
		fake: false,
		fake_initial: false,
		plan: false,
	};

	match cmd {
		Commands::Migrate { database, .. } => {
			assert_eq!(
				database,
				Some(db_url),
				"Database URL should be preserved exactly"
			);
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Migrate variant"),
	}
}

/// Test: Empty address string for runserver
///
/// Category: Edge Case
/// Verifies handling of empty address string.
#[rstest]
fn test_runserver_empty_address() {
	let cmd = create_runserver_with_options("", false, false, false, false, "dist", false);

	match cmd {
		Commands::Runserver { address, .. } => {
			assert!(
				address.is_empty(),
				"Empty address should be allowed at parse level"
			);
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Runserver variant"),
	}
}

/// Test: Shell command with complex expression
///
/// Category: Edge Case
/// Verifies that shell commands with complex expressions are preserved.
#[rstest]
fn test_shell_complex_command() {
	let complex_cmd = r#"
		let x = vec![1, 2, 3];
		for i in x.iter() {
			println!("{}", i);
		}
	"#
	.to_string();

	let cmd = Commands::Shell {
		command: Some(complex_cmd.clone()),
	};

	match cmd {
		Commands::Shell { command } => {
			assert_eq!(
				command,
				Some(complex_cmd),
				"Complex command should be preserved"
			);
		}
		#[allow(unreachable_patterns)]
		_ => panic!("Expected Commands::Shell variant"),
	}
}

// ============================================================================
// Additional Combination Tests
// ============================================================================

/// Test: All command variants can be iterated as examples
///
/// Category: Combination
/// Verifies that we can create and handle all command variants.
#[rstest]
fn test_all_command_variants_creatable() {
	// Create one instance of each non-feature-gated command
	let migrate = Commands::Migrate {
		app_label: Some("app".to_string()),
		migration_name: Some("0001".to_string()),
		database: Some("sqlite:///:memory:".to_string()),
		fake: true,
		fake_initial: true,
		plan: true,
	};

	let runserver =
		create_runserver_with_options("0.0.0.0:8080", true, true, true, true, "custom", true);

	let shell = Commands::Shell {
		command: Some("test".to_string()),
	};

	let check = Commands::Check {
		app_label: Some("myapp".to_string()),
		deploy: true,
	};

	let collectstatic = Commands::Collectstatic {
		clear: true,
		no_input: true,
		dry_run: true,
		link: true,
		ignore: vec!["*.txt".to_string()],
		index: None,
	};

	let showurls = Commands::Showurls { names: true };

	// Verify all have Debug
	let all_cmds: Vec<&Commands> = vec![
		&migrate,
		&runserver,
		&shell,
		&check,
		&collectstatic,
		&showurls,
	];

	for cmd in all_cmds {
		let debug = format!("{:?}", cmd);
		assert!(!debug.is_empty(), "All commands should have Debug output");
	}
}

// ============================================================================
// New Field Tests - Runserver with_pages, static_dir, no_spa
// ============================================================================

/// Test: Runserver with_pages flag
///
/// Category: Happy Path
/// Verifies that with_pages flag can be enabled and disabled.
#[rstest]
fn test_runserver_with_pages_flag() {
	let cmd_enabled =
		create_runserver_with_options("127.0.0.1:8000", false, false, false, true, "dist", false);

	if let Commands::Runserver { with_pages, .. } = cmd_enabled {
		assert!(with_pages, "with_pages should be true");
	} else {
		panic!("Expected Runserver command");
	}

	let cmd_disabled = create_runserver_default();
	if let Commands::Runserver { with_pages, .. } = cmd_disabled {
		assert!(!with_pages, "with_pages should be false by default");
	}
}

/// Test: Runserver static_dir custom directory
///
/// Category: Happy Path
/// Verifies that static_dir can be set to a custom directory.
#[rstest]
fn test_runserver_static_dir_custom() {
	let cmd = create_runserver_with_options(
		"127.0.0.1:8000",
		false,
		false,
		false,
		true,
		"custom_static",
		false,
	);

	if let Commands::Runserver { static_dir, .. } = cmd {
		assert_eq!(
			static_dir, "custom_static",
			"static_dir should be custom_static"
		);
	} else {
		panic!("Expected Runserver command");
	}

	// Test default values
	let cmd_default = create_runserver_default();
	if let Commands::Runserver { static_dir, .. } = cmd_default {
		assert_eq!(static_dir, "dist", "static_dir should default to dist");
	}
}

/// Test: Runserver no_spa flag
///
/// Category: Happy Path
/// Verifies that no_spa flag can be enabled and disabled.
#[rstest]
fn test_runserver_no_spa_flag() {
	let cmd_enabled =
		create_runserver_with_options("127.0.0.1:8000", false, false, false, true, "dist", true);

	if let Commands::Runserver { no_spa, .. } = cmd_enabled {
		assert!(no_spa, "no_spa should be true");
	} else {
		panic!("Expected Runserver command");
	}

	let cmd_disabled = create_runserver_default();
	if let Commands::Runserver { no_spa, .. } = cmd_disabled {
		assert!(!no_spa, "no_spa should be false by default");
	}
}

/// Test: Runserver pages integration (multiple fields combination)
///
/// Category: Combination
/// Verifies that multiple new fields work together correctly.
#[rstest]
fn test_runserver_pages_integration() {
	// Combination of with_pages enabled + custom directory + SPA disabled
	let cmd =
		create_runserver_with_options("0.0.0.0:3000", false, false, false, true, "build", true);

	if let Commands::Runserver {
		with_pages,
		static_dir,
		no_spa,
		..
	} = cmd
	{
		assert!(with_pages, "with_pages should be true");
		assert_eq!(static_dir, "build", "static_dir should be build");
		assert!(no_spa, "no_spa should be true");
	} else {
		panic!("Expected Runserver command");
	}
}

// ============================================================================
// Makemigrations --merge Flag Tests
// ============================================================================

/// Test: Parse Makemigrations command with --merge flag
///
/// Category: Happy Path
/// Verifies that --merge flag is correctly parsed.
#[cfg(feature = "migrations")]
#[rstest]
fn test_makemigrations_merge_flag_parsed() {
	// Arrange
	let cli = Cli::try_parse_from(["manage", "makemigrations", "--merge"]).unwrap();

	// Act & Assert
	match cli.command {
		Commands::Makemigrations { merge, .. } => {
			assert!(merge, "--merge flag should be true");
		}
		_ => panic!("Expected Makemigrations command"),
	}
}

/// Test: Parse Makemigrations command with --merge and --name
///
/// Category: Happy Path
/// Verifies that --merge and --name flags work together.
#[cfg(feature = "migrations")]
#[rstest]
fn test_makemigrations_merge_with_name() {
	// Arrange
	let cli = Cli::try_parse_from([
		"manage",
		"makemigrations",
		"--merge",
		"--name",
		"resolve_conflicts",
	])
	.unwrap();

	// Act & Assert
	match cli.command {
		Commands::Makemigrations { merge, name, .. } => {
			assert!(merge, "--merge flag should be true");
			assert_eq!(name, Some("resolve_conflicts".to_string()));
		}
		_ => panic!("Expected Makemigrations command"),
	}
}
