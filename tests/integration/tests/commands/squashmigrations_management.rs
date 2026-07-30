//! End-to-end coverage for the `squashmigrations` management command.

use clap::Parser;
use reinhardt_commands::{Cli, CommandError, run_command};
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, FilesystemRepository, FilesystemSource, Migration,
	MigrationRenderOptions, MigrationSource, Operation,
};
use serial_test::serial;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

const NONINTERACTIVE_CHILD_ENV: &str = "REINHARDT_SQUASHMIGRATIONS_NONINTERACTIVE_CHILD";
const NONINTERACTIVE_TEST_NAME: &str =
	"squashmigrations_management::noninteractive_input_without_no_input_does_not_write";

struct ProjectDirGuard {
	original_dir: PathBuf,
}

impl ProjectDirGuard {
	fn enter(project_dir: &Path) -> Self {
		let original_dir = std::env::current_dir().expect("current directory should be readable");
		std::env::set_current_dir(project_dir)
			.expect("temporary project directory should be enterable");
		Self { original_dir }
	}
}

impl Drop for ProjectDirGuard {
	fn drop(&mut self) {
		std::env::set_current_dir(&self.original_dir)
			.expect("original current directory should be restored");
	}
}

fn create_table(name: &str) -> Operation {
	Operation::CreateTable {
		name: name.to_string(),
		columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	}
}

fn write_migration(repository: &FilesystemRepository, migration: &Migration) {
	let source = repository
		.render(
			migration,
			MigrationRenderOptions {
				include_header: true,
			},
		)
		.expect("migration fixture should render");
	repository
		.create_new_source(&migration.app_label, &migration.name, &source)
		.expect("migration fixture should be created");
}

fn create_linear_project() -> TempDir {
	let project = TempDir::new().expect("temporary project should be created");
	let repository = FilesystemRepository::new(project.path().join("migrations"));

	let auth = Migration::new("0001_initial", "auth");

	let mut initial = Migration::new("0001_initial", "polls");
	initial.dependencies = vec![("auth".to_string(), "0001_initial".to_string())];
	initial.operations = vec![
		create_table("entries"),
		create_table("temporary_entries"),
		Operation::AddColumn {
			table: "entries".to_string(),
			column: ColumnDefinition::new("temporary_note", FieldType::Text),
			mysql_options: None,
		},
	];

	let mut remove_temporary_table = Migration::new("0002_remove_temporary_table", "polls");
	remove_temporary_table.dependencies = vec![("polls".to_string(), "0001_initial".to_string())];
	remove_temporary_table.operations = vec![Operation::DropTable {
		name: "temporary_entries".to_string(),
	}];

	let mut audit_barrier = Migration::new("0003_audit_barrier", "polls");
	audit_barrier.dependencies = vec![(
		"polls".to_string(),
		"0002_remove_temporary_table".to_string(),
	)];
	audit_barrier.operations = vec![Operation::RunSQL {
		sql: "UPDATE entries SET temporary_note = NULL".to_string(),
		reverse_sql: None,
	}];

	let mut remove_temporary_column = Migration::new("0004_remove_temporary_column", "polls");
	remove_temporary_column.dependencies =
		vec![("polls".to_string(), "0003_audit_barrier".to_string())];
	remove_temporary_column.operations = vec![Operation::DropColumn {
		table: "entries".to_string(),
		column: "temporary_note".to_string(),
		old_definition: None,
	}];

	for migration in [
		&auth,
		&initial,
		&remove_temporary_table,
		&audit_barrier,
		&remove_temporary_column,
	] {
		write_migration(&repository, migration);
	}
	project
}

async fn loaded_migration(project: &Path, name: &str) -> Migration {
	FilesystemSource::new(project.join("migrations"))
		.all_migrations()
		.await
		.expect("generated migration tree should reload strictly")
		.into_iter()
		.find(|migration| migration.app_label == "polls" && migration.name == name)
		.expect("generated squashed migration should be present")
}

#[tokio::test]
#[serial(command_current_dir)]
async fn cli_dispatch_generates_parseable_squash_with_exact_semantics() {
	// Arrange
	let project = create_linear_project();
	let _cwd = ProjectDirGuard::enter(project.path());
	let cli = Cli::try_parse_from([
		"manage",
		"squashmigrations",
		"polls",
		"0001_initial",
		"0004_remove_temporary_column",
		"--no-input",
		"--squashed-name",
		"release",
	])
	.expect("Django-compatible three-positional syntax should parse");

	// Act
	run_command(cli.command, cli.verbosity)
		.await
		.expect("squashmigrations command should succeed");

	// Assert
	let generated_path = project.path().join("migrations/polls/0001_release.rs");
	let generated_source =
		fs::read_to_string(&generated_path).expect("generated source should be readable");
	syn::parse_file(&generated_source).expect("generated source should be valid Rust syntax");
	let generated = loaded_migration(project.path(), "0001_release").await;
	assert_eq!(
		generated.dependencies,
		vec![("auth".to_string(), "0001_initial".to_string())]
	);
	assert_eq!(
		generated.replaces,
		vec![
			("polls".to_string(), "0001_initial".to_string()),
			(
				"polls".to_string(),
				"0002_remove_temporary_table".to_string()
			),
			("polls".to_string(), "0003_audit_barrier".to_string()),
			(
				"polls".to_string(),
				"0004_remove_temporary_column".to_string()
			),
		]
	);
	assert_eq!(
		generated.operations,
		vec![
			create_table("entries"),
			Operation::AddColumn {
				table: "entries".to_string(),
				column: ColumnDefinition::new("temporary_note", FieldType::Text),
				mysql_options: None,
			},
			Operation::RunSQL {
				sql: "UPDATE entries SET temporary_note = NULL".to_string(),
				reverse_sql: None,
			},
			Operation::DropColumn {
				table: "entries".to_string(),
				column: "temporary_note".to_string(),
				old_definition: None,
			},
		]
	);
}

#[tokio::test]
#[serial(command_current_dir)]
async fn no_optimize_and_no_header_preserve_every_operation_without_a_prompt() {
	// Arrange
	let project = create_linear_project();
	let _cwd = ProjectDirGuard::enter(project.path());
	let cli = Cli::try_parse_from([
		"manage",
		"squashmigrations",
		"polls",
		"0004",
		"--noinput",
		"--no-optimize",
		"--no-header",
		"--squashed-name",
		"unoptimized",
	])
	.expect("Django-compatible two-positional syntax and aliases should parse");

	// Act
	run_command(cli.command, cli.verbosity)
		.await
		.expect("noninteractive squashmigrations command should succeed");

	// Assert
	let generated_path = project.path().join("migrations/polls/0001_unoptimized.rs");
	let generated_source =
		fs::read_to_string(&generated_path).expect("generated source should be readable");
	syn::parse_file(&generated_source).expect("generated source should be valid Rust syntax");
	assert!(!generated_source.starts_with("// Generated by Reinhardt migrations."));
	let generated = loaded_migration(project.path(), "0001_unoptimized").await;
	assert_eq!(
		generated.operations,
		vec![
			create_table("entries"),
			create_table("temporary_entries"),
			Operation::AddColumn {
				table: "entries".to_string(),
				column: ColumnDefinition::new("temporary_note", FieldType::Text),
				mysql_options: None,
			},
			Operation::DropTable {
				name: "temporary_entries".to_string(),
			},
			Operation::RunSQL {
				sql: "UPDATE entries SET temporary_note = NULL".to_string(),
				reverse_sql: None,
			},
			Operation::DropColumn {
				table: "entries".to_string(),
				column: "temporary_note".to_string(),
				old_definition: None,
			},
		]
	);
}

#[tokio::test]
#[serial(command_current_dir)]
async fn ambiguous_ancestry_fails_before_creating_a_squashed_migration() {
	// Arrange
	let project = TempDir::new().expect("temporary project should be created");
	let repository = FilesystemRepository::new(project.path().join("migrations"));
	let initial = Migration::new("0001_initial", "polls");
	let mut left = Migration::new("0002_left", "polls");
	left.dependencies = vec![("polls".to_string(), "0001_initial".to_string())];
	let mut right = Migration::new("0002_right", "polls");
	right.dependencies = vec![("polls".to_string(), "0001_initial".to_string())];
	let mut merge = Migration::new("0003_merge", "polls");
	merge.dependencies = vec![
		("polls".to_string(), "0002_left".to_string()),
		("polls".to_string(), "0002_right".to_string()),
	];
	for migration in [&initial, &left, &right, &merge] {
		write_migration(&repository, migration);
	}
	let _cwd = ProjectDirGuard::enter(project.path());
	let cli = Cli::try_parse_from([
		"manage",
		"squashmigrations",
		"polls",
		"0001_initial",
		"0003_merge",
		"--no-input",
	])
	.expect("squashmigrations arguments should parse");

	// Act
	let error = run_command(cli.command, cli.verbosity)
		.await
		.expect_err("ambiguous migration ancestry should be rejected");

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid arguments: Invalid migration: Ambiguous migration ancestry for \
		 polls.0003_merge; parents: 0002_left, 0002_right"
	);
	assert_eq!(
		fs::read_dir(project.path().join("migrations/polls"))
			.expect("migration directory should be readable")
			.count(),
		4
	);
}

#[tokio::test]
#[serial(command_current_dir)]
async fn existing_destination_is_preserved_when_create_new_refuses_overwrite() {
	// Arrange
	let project = create_linear_project();
	let repository = FilesystemRepository::new(project.path().join("migrations"));
	let existing = Migration::new("0001_release", "polls");
	write_migration(&repository, &existing);
	let existing_path = project.path().join("migrations/polls/0001_release.rs");
	let existing_source =
		fs::read_to_string(&existing_path).expect("existing source should be readable");
	let _cwd = ProjectDirGuard::enter(project.path());
	let cli = Cli::try_parse_from([
		"manage",
		"squashmigrations",
		"polls",
		"0001_initial",
		"0004_remove_temporary_column",
		"--no-input",
		"--squashed-name",
		"release",
	])
	.expect("squashmigrations arguments should parse");

	// Act
	let error = run_command(cli.command, cli.verbosity)
		.await
		.expect_err("create-new semantics should reject an existing destination");

	// Assert
	let command_error = error
		.downcast_ref::<CommandError>()
		.expect("dispatch should preserve the typed command error");
	match command_error {
		CommandError::IoError(error) => assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists),
		other => panic!("expected an already-exists IO error, got {other:?}"),
	}
	assert_eq!(
		fs::read_to_string(existing_path).expect("existing source should remain readable"),
		existing_source
	);
}

#[tokio::test]
#[serial(command_current_dir)]
async fn noninteractive_input_without_no_input_does_not_write() {
	if std::env::var_os(NONINTERACTIVE_CHILD_ENV).is_some() {
		let cli = Cli::try_parse_from([
			"manage",
			"squashmigrations",
			"polls",
			"0004_remove_temporary_column",
			"--squashed-name",
			"unattended",
		])
		.expect("squashmigrations arguments should parse");
		let error = run_command(cli.command, cli.verbosity)
			.await
			.expect_err("null stdin should require --no-input");
		eprint!("{error}");

		// This process is an isolated one-shot test helper with no owned cleanup.
		std::process::exit(2);
	}

	// Arrange
	let project = create_linear_project();

	// Act
	let output =
		Command::new(std::env::current_exe().expect("test executable should be available"))
			.args(["--exact", NONINTERACTIVE_TEST_NAME, "--nocapture"])
			.env(NONINTERACTIVE_CHILD_ENV, "1")
			.current_dir(project.path())
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.output()
			.expect("noninteractive child should execute");

	// Assert
	assert_eq!(
		output.status.code(),
		Some(2),
		"child stdout: {}\nchild stderr: {}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(
		String::from_utf8(output.stderr).expect("child stderr should be UTF-8"),
		"Invalid arguments: squashmigrations requires terminal input; use --no-input in \
		 non-interactive environments"
	);
	assert!(
		!project
			.path()
			.join("migrations/polls/0001_unattended.rs")
			.exists()
	);
}
