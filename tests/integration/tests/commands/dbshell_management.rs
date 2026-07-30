//! Cross-platform end-to-end coverage for the native database shell command.

use reinhardt_commands::{Commands, run_command};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

const CHILD_CASE_ENV: &str = "REINHARDT_DBSHELL_CHILD_CASE";
const DATABASE_URL_ENV: &str = "REINHARDT_DBSHELL_DATABASE_URL";
const RECORD_ENV: &str = "REINHARDT_DBSHELL_RECORD";
const FAKE_EXIT_ENV: &str = "REINHARDT_DBSHELL_FAKE_EXIT";
const EXPECTED_ERROR_ENV: &str = "REINHARDT_DBSHELL_EXPECTED_ERROR";

struct FakeClients {
	_temp: TempDir,
	path: PathBuf,
}

impl FakeClients {
	fn new(client_names: &[&str]) -> Self {
		let temp = temporary_directory("reinhardt-dbshell-integration-");
		let executable = compile_fake_client(temp.path());

		for client_name in client_names {
			let target = temp.path().join(platform_executable_name(client_name));
			fs::copy(&executable, &target).expect("install fake database client");
		}

		Self {
			path: temp.path().to_path_buf(),
			_temp: temp,
		}
	}

	fn empty() -> Self {
		let temp = temporary_directory("reinhardt-dbshell-empty-path-");
		Self {
			path: temp.path().to_path_buf(),
			_temp: temp,
		}
	}
}

fn temporary_directory(prefix: &str) -> TempDir {
	let mut builder = tempfile::Builder::new();
	builder.prefix(prefix);
	#[cfg(unix)]
	let temp = builder.tempdir_in("/tmp");
	#[cfg(not(unix))]
	let temp = builder.tempdir();
	temp.expect("create temporary database client directory")
}

fn platform_executable_name(client_name: &str) -> OsString {
	if cfg!(windows) {
		OsString::from(format!("{client_name}.EXE"))
	} else {
		OsString::from(client_name)
	}
}

fn compile_fake_client(directory: &Path) -> PathBuf {
	let source = directory.join("fake_client.rs");
	let executable = directory.join(platform_executable_name("fake-client"));
	fs::write(
		&source,
		r#"
use std::fmt::Write as _;

fn main() {
    let mut record = String::new();
    for argument in std::env::args_os().skip(1) {
        writeln!(&mut record, "argument={}", argument.to_string_lossy()).unwrap();
    }
    for name in ["PGPASSWORD", "MYSQL_PWD"] {
        match std::env::var_os(name) {
            Some(value) => writeln!(&mut record, "{name}={}", value.to_string_lossy()).unwrap(),
            None => writeln!(&mut record, "{name}=unset").unwrap(),
        }
    }
    std::fs::write(std::env::var_os("REINHARDT_DBSHELL_RECORD").unwrap(), record).unwrap();
    let status = std::env::var("REINHARDT_DBSHELL_FAKE_EXIT")
        .unwrap_or_else(|_| "0".to_owned())
        .parse::<i32>()
        .unwrap();
    std::process::exit(status);
}
"#,
	)
	.expect("write fake database client source");

	let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc")))
		.arg(&source)
		.arg("--edition=2024")
		.arg("-o")
		.arg(&executable)
		.output()
		.expect("compile fake database client");
	assert!(
		output.status.success(),
		"fake database client compilation failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	executable
}

fn run_child(
	case: &str,
	database_url: &str,
	clients: &FakeClients,
	record: &Path,
	exit_status: i32,
	expected_error: Option<&str>,
) -> Output {
	let mut command =
		Command::new(std::env::current_exe().expect("resolve integration test binary"));
	command
		.args([
			"--exact",
			"dbshell_management::dbshell_management_child",
			"--nocapture",
		])
		.env(CHILD_CASE_ENV, case)
		.env(DATABASE_URL_ENV, database_url)
		.env(RECORD_ENV, record)
		.env(FAKE_EXIT_ENV, exit_status.to_string())
		.env("PATH", &clients.path)
		.env_remove("PGPASSWORD")
		.env_remove("MYSQL_PWD")
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	if let Some(expected_error) = expected_error {
		command.env(EXPECTED_ERROR_ENV, expected_error);
	} else {
		command.env_remove(EXPECTED_ERROR_ENV);
	}

	command.output().expect("run isolated dbshell test child")
}

fn assert_child_success(output: &Output, case: &str) {
	assert!(
		output.status.success(),
		"{case} child failed\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

#[test]
fn dbshell_management_records_exact_backend_arguments_and_credentials() {
	struct BackendCase {
		name: &'static str,
		client: &'static str,
		url: &'static str,
		expected: &'static str,
	}

	let cases = [
		BackendCase {
			name: "postgres",
			client: "psql",
			url: "postgresql://ops%20user:postgres-secret@db.example:5544/reporting%20data",
			expected: "argument=--host\nargument=db.example\nargument=--port\nargument=5544\nargument=--username\nargument=ops user\nargument=reporting data\nargument=--expanded\nargument=value with spaces\nPGPASSWORD=postgres-secret\nMYSQL_PWD=unset\n",
		},
		BackendCase {
			name: "mysql",
			client: "mysql",
			url: "mysql://ops%20user:mysql-secret@db.example:4406/reporting%20data",
			expected: "argument=--host\nargument=db.example\nargument=--port\nargument=4406\nargument=--user\nargument=ops user\nargument=reporting data\nargument=--expanded\nargument=value with spaces\nPGPASSWORD=unset\nMYSQL_PWD=mysql-secret\n",
		},
		BackendCase {
			name: "sqlite",
			client: "sqlite3",
			url: "sqlite:data/report.sqlite3?mode=ro",
			expected: "argument=file:data/report.sqlite3?mode=ro\nargument=--expanded\nargument=value with spaces\nPGPASSWORD=unset\nMYSQL_PWD=unset\n",
		},
	];

	for backend in cases {
		let clients = FakeClients::new(&[backend.client]);
		let record = clients.path.join(format!("{}.record", backend.name));
		let output = run_child(backend.name, backend.url, &clients, &record, 0, None);

		assert_child_success(&output, backend.name);
		assert_eq!(
			fs::read_to_string(record).expect("read fake database client record"),
			backend.expected
		);
	}
}

#[test]
fn dbshell_management_propagates_nonzero_status_without_credentials() {
	let clients = FakeClients::new(&["psql"]);
	let record = clients.path.join("nonzero.record");
	let database_url = "postgresql://operator:nonzero-secret@db.example/reporting";
	let expected_error = "Execution error: Database client `psql` exited with status 23.";

	let output = run_child(
		"nonzero",
		database_url,
		&clients,
		&record,
		23,
		Some(expected_error),
	);

	assert_child_success(&output, "nonzero");
	let diagnostics = format!(
		"{}{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(!diagnostics.contains("nonzero-secret"));
	assert!(!diagnostics.contains(database_url));
}

#[test]
fn dbshell_management_reports_missing_native_client_without_credentials() {
	let clients = FakeClients::empty();
	let record = clients.path.join("missing.record");
	let database_url = "mysql://operator:missing-secret@db.example/reporting";
	let expected_error =
		"Execution error: Database client executable `mysql` was not found on PATH.";

	let output = run_child(
		"missing",
		database_url,
		&clients,
		&record,
		0,
		Some(expected_error),
	);

	assert_child_success(&output, "missing");
	let diagnostics = format!(
		"{}{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(!diagnostics.contains("missing-secret"));
	assert!(!diagnostics.contains(database_url));
	assert!(!record.exists());
}

#[test]
fn dbshell_management_child() {
	let Some(case) = std::env::var_os(CHILD_CASE_ENV) else {
		return;
	};
	let database_url =
		std::env::var(DATABASE_URL_ENV).expect("isolated child database URL should be set");
	let command = Commands::Dbshell {
		database: "default".to_string(),
		database_url: Some(
			database_url
				.parse()
				.expect("isolated child database URL should parse"),
		),
		client_arguments: vec![
			OsString::from("--expanded"),
			OsString::from("value with spaces"),
		],
	};
	let runtime = tokio::runtime::Runtime::new().expect("create isolated child runtime");
	let result = runtime.block_on(run_command(command, 0));

	match std::env::var(EXPECTED_ERROR_ENV) {
		Ok(expected_error) => assert_eq!(
			result
				.expect_err("isolated dbshell case should fail")
				.to_string(),
			expected_error,
			"unexpected dbshell error for {}",
			case.to_string_lossy()
		),
		Err(_) => result.expect("isolated dbshell case should succeed"),
	}
}
