//! Native database client specification construction.

use crate::database_selector::ResolvedDatabase;
use crate::{CommandError, CommandResult};
use percent_encoding::percent_decode_str;
use reinhardt_db::backends::DatabaseType;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use url::{Host, Url};

pub(crate) struct DbClientSpec {
	executable: OsString,
	arguments: Vec<OsString>,
	secret_environment: Vec<(OsString, OsString)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DbShellOutcome {
	Exited(i32),
	TerminatedBySignal,
}

pub(crate) trait DbClientRunner {
	fn run(&self, spec: &DbClientSpec) -> CommandResult<DbShellOutcome>;
}

pub(crate) struct PortableDbClientRunner;

impl DbClientRunner for PortableDbClientRunner {
	fn run(&self, spec: &DbClientSpec) -> CommandResult<DbShellOutcome> {
		let executable = resolve_executable(&spec.executable)?;
		let mut child = Command::new(executable)
			.args(&spec.arguments)
			.envs(
				spec.secret_environment
					.iter()
					.map(|(name, value)| (name, value)),
			)
			.stdin(Stdio::inherit())
			.stdout(Stdio::inherit())
			.stderr(Stdio::inherit())
			.spawn()
			.map_err(|error| {
				CommandError::ExecutionError(format!(
					"Failed to launch database client executable `{}`: {error}",
					spec.executable.to_string_lossy()
				))
			})?;
		let status = child.wait().map_err(|error| {
			CommandError::ExecutionError(format!(
				"Failed to wait for database client executable `{}`: {error}",
				spec.executable.to_string_lossy()
			))
		})?;

		Ok(match status.code() {
			Some(code) => DbShellOutcome::Exited(code),
			None => DbShellOutcome::TerminatedBySignal,
		})
	}
}

fn resolve_executable(executable: &OsString) -> CommandResult<PathBuf> {
	let path = env::var_os("PATH").unwrap_or_default();
	for directory in env::split_paths(&path) {
		for extension in executable_extensions() {
			let mut filename = executable.clone();
			filename.push(extension);
			let candidate = directory.join(filename);
			if candidate.is_file() {
				return Ok(candidate);
			}
		}
	}

	Err(CommandError::ExecutionError(format!(
		"Database client executable `{}` was not found on PATH.",
		executable.to_string_lossy()
	)))
}

#[cfg(not(windows))]
fn executable_extensions() -> Vec<OsString> {
	vec![OsString::new()]
}

#[cfg(windows)]
fn executable_extensions() -> Vec<OsString> {
	let mut extensions = vec![OsString::new()];
	let configured =
		env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
	extensions.extend(
		configured
			.to_string_lossy()
			.split(';')
			.filter(|extension| !extension.is_empty())
			.map(OsString::from),
	);
	extensions
}

impl fmt::Debug for DbClientSpec {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("DbClientSpec")
			.field("executable", &self.executable)
			.field("arguments", &RedactedArguments(&self.arguments))
			.field(
				"secret_environment",
				&RedactedEnvironment(&self.secret_environment),
			)
			.finish()
	}
}

struct RedactedArguments<'a>(&'a [OsString]);

impl fmt::Debug for RedactedArguments<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut list = f.debug_list();
		for argument in self.0 {
			if argument.to_string_lossy().starts_with("file:") {
				list.entry(&RedactedValue);
			} else {
				list.entry(argument);
			}
		}
		list.finish()
	}
}

struct RedactedEnvironment<'a>(&'a [(OsString, OsString)]);

impl fmt::Debug for RedactedEnvironment<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut list = f.debug_list();
		for (name, _) in self.0 {
			list.entry(&(name, RedactedValue));
		}
		list.finish()
	}
}

struct RedactedValue;

impl fmt::Debug for RedactedValue {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("\"[REDACTED]\"")
	}
}

pub(crate) fn build_client_spec(
	database: &ResolvedDatabase,
	passthrough: &[OsString],
) -> CommandResult<DbClientSpec> {
	let mut spec = match database.backend() {
		DatabaseType::Postgres => build_postgres_spec(database.url())?,
		DatabaseType::Mysql => build_mysql_spec(database.url())?,
		DatabaseType::Sqlite => build_sqlite_spec(database.url())?,
	};
	spec.arguments.extend_from_slice(passthrough);
	Ok(spec)
}

fn build_postgres_spec(database_url: &str) -> CommandResult<DbClientSpec> {
	let parsed = parse_network_url(database_url, "PostgreSQL")?;
	let mut arguments = Vec::new();
	append_host(&mut arguments, &parsed);
	append_option(
		&mut arguments,
		"--port",
		parsed.port().map(|port| port.to_string()),
	);
	append_decoded_option(
		&mut arguments,
		"--username",
		nonempty(parsed.username()),
		"PostgreSQL",
	)?;
	append_database_name(&mut arguments, &parsed, "PostgreSQL")?;

	let secret_environment = decoded_password(&parsed, "PostgreSQL")?
		.map(|password| vec![(OsString::from("PGPASSWORD"), password)])
		.unwrap_or_default();

	Ok(DbClientSpec {
		executable: OsString::from("psql"),
		arguments,
		secret_environment,
	})
}

fn build_mysql_spec(database_url: &str) -> CommandResult<DbClientSpec> {
	let parsed = parse_network_url(database_url, "MySQL")?;
	let mut arguments = Vec::new();
	append_host(&mut arguments, &parsed);
	append_option(
		&mut arguments,
		"--port",
		parsed.port().map(|port| port.to_string()),
	);
	append_decoded_option(
		&mut arguments,
		"--user",
		nonempty(parsed.username()),
		"MySQL",
	)?;
	append_database_name(&mut arguments, &parsed, "MySQL")?;

	let secret_environment = decoded_password(&parsed, "MySQL")?
		.map(|password| vec![(OsString::from("MYSQL_PWD"), password)])
		.unwrap_or_default();

	Ok(DbClientSpec {
		executable: OsString::from("mysql"),
		arguments,
		secret_environment,
	})
}

fn build_sqlite_spec(database_url: &str) -> CommandResult<DbClientSpec> {
	let without_fragment = database_url
		.split_once('#')
		.map_or(database_url, |(url, _)| url);
	let (url_without_query, query) = without_fragment
		.split_once('?')
		.map_or((without_fragment, None), |(path, query)| {
			(path, Some(query))
		});
	let encoded_path =
		if url_without_query == "sqlite::memory:" || url_without_query == "sqlite://:memory:" {
			":memory:"
		} else if let Some(path) = url_without_query.strip_prefix("sqlite:///") {
			path
		} else if let Some(path) = url_without_query.strip_prefix("sqlite://") {
			path
		} else if let Some(path) = url_without_query.strip_prefix("sqlite:") {
			path
		} else {
			return Err(malformed_url("SQLite"));
		};
	if encoded_path.is_empty() {
		return Err(malformed_url("SQLite"));
	}
	let database_argument = match query {
		Some(query) => {
			let uri = if encoded_path.starts_with("file:") {
				format!("{encoded_path}?{query}")
			} else {
				format!("file:{encoded_path}?{query}")
			};
			OsString::from(uri)
		}
		None => decode_component(encoded_path, "SQLite")?,
	};

	Ok(DbClientSpec {
		executable: OsString::from("sqlite3"),
		arguments: vec![database_argument],
		secret_environment: Vec::new(),
	})
}

fn parse_network_url(database_url: &str, backend: &str) -> CommandResult<Url> {
	Url::parse(database_url).map_err(|_| malformed_url(backend))
}

fn append_host(arguments: &mut Vec<OsString>, parsed: &Url) {
	let Some(host) = parsed.host() else {
		return;
	};
	let host = match host {
		Host::Domain(domain) => domain.to_string(),
		Host::Ipv4(address) => address.to_string(),
		Host::Ipv6(address) => address.to_string(),
	};
	append_option(arguments, "--host", Some(host));
}

fn append_option(arguments: &mut Vec<OsString>, option: &str, value: Option<String>) {
	if let Some(value) = value {
		arguments.push(OsString::from(option));
		arguments.push(OsString::from(value));
	}
}

fn append_decoded_option(
	arguments: &mut Vec<OsString>,
	option: &str,
	value: Option<&str>,
	backend: &str,
) -> CommandResult<()> {
	if let Some(value) = value {
		arguments.push(OsString::from(option));
		arguments.push(decode_component(value, backend)?);
	}
	Ok(())
}

fn append_database_name(
	arguments: &mut Vec<OsString>,
	parsed: &Url,
	backend: &str,
) -> CommandResult<()> {
	let encoded_name = parsed.path().trim_start_matches('/');
	if !encoded_name.is_empty() {
		arguments.push(decode_component(encoded_name, backend)?);
	}
	Ok(())
}

fn decoded_password(parsed: &Url, backend: &str) -> CommandResult<Option<OsString>> {
	parsed
		.password()
		.map(|password| decode_component(password, backend))
		.transpose()
}

fn decode_component(value: &str, backend: &str) -> CommandResult<OsString> {
	percent_decode_str(value)
		.decode_utf8()
		.map(|decoded| OsString::from(decoded.as_ref()))
		.map_err(|_| malformed_url(backend))
}

fn nonempty(value: &str) -> Option<&str> {
	(!value.is_empty()).then_some(value)
}

fn malformed_url(backend: &str) -> CommandError {
	CommandError::InvalidArguments(format!("The selected {backend} database URL is malformed."))
}

#[cfg(test)]
mod tests {
	use super::{DbClientRunner, DbShellOutcome, PortableDbClientRunner, build_client_spec};
	use crate::database_selector::{DatabaseSelector, resolve_database};
	use serial_test::serial;
	use std::ffi::{OsStr, OsString};
	#[cfg(unix)]
	use std::fs;
	#[cfg(unix)]
	use std::io::Write;
	#[cfg(unix)]
	use std::path::Path;
	#[cfg(unix)]
	use std::process::{Command, Stdio};
	use tempfile::TempDir;

	fn resolved_database(url: &str) -> crate::database_selector::ResolvedDatabase {
		resolve_database(
			&DatabaseSelector {
				alias: "default".to_string(),
				url_override: Some(url.to_string()),
			},
			None,
		)
		.expect("database URL should resolve")
	}

	#[cfg(unix)]
	fn write_fake_client(directory: &Path, name: &str, body: &str) {
		use std::os::unix::fs::PermissionsExt;

		let path = directory.join(name);
		fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write fake client");
		let mut permissions = fs::metadata(&path)
			.expect("read fake client metadata")
			.permissions();
		permissions.set_mode(0o755);
		fs::set_permissions(path, permissions).expect("make fake client executable");
	}

	struct PathGuard(Option<OsString>);

	impl PathGuard {
		fn set(path: impl AsRef<OsStr>) -> Self {
			let original = std::env::var_os("PATH");
			// SAFETY: PATH-mutating tests use the shared `env` serial-test key.
			unsafe { std::env::set_var("PATH", path) };
			Self(original)
		}
	}

	impl Drop for PathGuard {
		fn drop(&mut self) {
			// SAFETY: PATH-mutating tests use the shared `env` serial-test key.
			unsafe {
				if let Some(path) = &self.0 {
					std::env::set_var("PATH", path);
				} else {
					std::env::remove_var("PATH");
				}
			}
		}
	}

	struct EnvironmentVariableGuard {
		name: &'static str,
		original: Option<OsString>,
	}

	impl EnvironmentVariableGuard {
		fn set(name: &'static str, value: impl AsRef<OsStr>) -> Self {
			let original = std::env::var_os(name);
			// SAFETY: environment-mutating tests use the shared `env` serial-test key.
			unsafe { std::env::set_var(name, value) };
			Self { name, original }
		}

		#[cfg(unix)]
		fn remove(name: &'static str) -> Self {
			let original = std::env::var_os(name);
			// SAFETY: environment-mutating tests use the shared `env` serial-test key.
			unsafe { std::env::remove_var(name) };
			Self { name, original }
		}
	}

	#[cfg(windows)]
	#[test]
	#[serial(env)]
	fn executable_resolution_uses_windows_pathext_entries() {
		let directory = TempDir::new().expect("create fake client directory");
		let executable = directory.path().join("psql.TEST");
		std::fs::write(&executable, b"fake executable").expect("write fake executable");
		let _path = PathGuard::set(directory.path());
		let _pathext = EnvironmentVariableGuard::set("PATHEXT", ".TEST;.EXE");

		let resolved =
			super::resolve_executable(&OsString::from("psql")).expect("resolve fake executable");

		assert_eq!(resolved, executable);
	}

	impl Drop for EnvironmentVariableGuard {
		fn drop(&mut self) {
			// SAFETY: environment-mutating tests use the shared `env` serial-test key.
			unsafe {
				if let Some(value) = &self.original {
					std::env::set_var(self.name, value);
				} else {
					std::env::remove_var(self.name);
				}
			}
		}
	}

	#[cfg(unix)]
	#[test]
	#[serial(env)]
	fn portable_runner_resolves_each_client_and_forwards_exact_arguments_and_environment() {
		let directory = TempDir::new().expect("create fake client directory");
		let empty_directory = TempDir::new().expect("create empty PATH directory");
		let record = directory.path().join("record");
		let script = r#"
{
	printf 'client=%s\n' "${0##*/}"
	for argument in "$@"; do
		printf 'argument=%s\n' "$argument"
	done
	printf 'PGPASSWORD=%s\n' "${PGPASSWORD-unset}"
	printf 'MYSQL_PWD=%s\n' "${MYSQL_PWD-unset}"
} > "$DBSHELL_TEST_RECORD"
"#;
		for client in ["psql", "mysql", "sqlite3"] {
			write_fake_client(directory.path(), client, script);
		}
		let path = std::env::join_paths([empty_directory.path(), directory.path()])
			.expect("join fake client PATH");
		let _path = PathGuard::set(path);
		let _postgres_password = EnvironmentVariableGuard::remove("PGPASSWORD");
		let _mysql_password = EnvironmentVariableGuard::remove("MYSQL_PWD");
		let runner = PortableDbClientRunner;

		let cases = [
			(
				"postgresql://operator:postgres-secret@db.example:5544/reporting",
				"client=psql\nargument=--host\nargument=db.example\nargument=--port\nargument=5544\nargument=--username\nargument=operator\nargument=reporting\nargument=--expanded\nPGPASSWORD=postgres-secret\nMYSQL_PWD=unset\n",
			),
			(
				"mysql://operator:mysql-secret@db.example:4406/reporting",
				"client=mysql\nargument=--host\nargument=db.example\nargument=--port\nargument=4406\nargument=--user\nargument=operator\nargument=reporting\nargument=--expanded\nPGPASSWORD=unset\nMYSQL_PWD=mysql-secret\n",
			),
			(
				"sqlite:data/report.sqlite3",
				"client=sqlite3\nargument=data/report.sqlite3\nargument=--expanded\nPGPASSWORD=unset\nMYSQL_PWD=unset\n",
			),
		];

		for (url, expected) in cases {
			let database = resolved_database(url);
			let mut spec = build_client_spec(&database, &[OsString::from("--expanded")])
				.expect("build client spec");
			spec.secret_environment.push((
				OsString::from("DBSHELL_TEST_RECORD"),
				record.as_os_str().to_owned(),
			));

			let outcome = runner.run(&spec).expect("run fake client");

			assert_eq!(outcome, DbShellOutcome::Exited(0));
			assert_eq!(fs::read_to_string(&record).expect("read record"), expected);
		}
	}

	#[cfg(unix)]
	#[test]
	#[serial(env)]
	fn portable_runner_scopes_secret_environment_to_the_child() {
		let directory = TempDir::new().expect("create fake client directory");
		let record = directory.path().join("record");
		write_fake_client(
			directory.path(),
			"psql",
			"printf '%s' \"$PGPASSWORD\" > \"$DBSHELL_TEST_RECORD\"",
		);
		let _path = PathGuard::set(directory.path());
		let _password = EnvironmentVariableGuard::set("PGPASSWORD", "parent-secret");
		let database = resolved_database("postgresql://operator:child-secret@db.example/reporting");
		let mut spec = build_client_spec(&database, &[]).expect("build client spec");
		spec.secret_environment.push((
			OsString::from("DBSHELL_TEST_RECORD"),
			record.as_os_str().to_owned(),
		));

		let outcome = PortableDbClientRunner.run(&spec).expect("run fake client");

		assert_eq!(outcome, DbShellOutcome::Exited(0));
		assert_eq!(
			fs::read_to_string(record).expect("read recorded password"),
			"child-secret"
		);
		assert_eq!(
			std::env::var("PGPASSWORD").expect("read parent password"),
			"parent-secret"
		);
	}

	#[test]
	#[serial(env)]
	fn portable_runner_reports_a_missing_executable_without_leaking_arguments() {
		let directory = TempDir::new().expect("create empty client directory");
		let _path = PathGuard::set(directory.path());
		let database = resolved_database("postgresql://operator:do-not-print@db.example/reporting");
		let spec = build_client_spec(&database, &[]).expect("build client spec");

		let error = PortableDbClientRunner
			.run(&spec)
			.expect_err("missing executable should fail");
		let diagnostic = error.to_string();

		assert_eq!(
			diagnostic,
			"Execution error: Database client executable `psql` was not found on PATH."
		);
		assert!(!diagnostic.contains("do-not-print"));
		assert!(!diagnostic.contains("reporting"));
	}

	#[cfg(unix)]
	#[test]
	#[serial(env)]
	fn portable_runner_preserves_nonzero_exit_status() {
		let directory = TempDir::new().expect("create fake client directory");
		write_fake_client(directory.path(), "sqlite3", "exit 23");
		let _path = PathGuard::set(directory.path());
		let database = resolved_database("sqlite:db.sqlite3");
		let spec = build_client_spec(&database, &[]).expect("build client spec");

		let outcome = PortableDbClientRunner.run(&spec).expect("run fake client");

		assert_eq!(outcome, DbShellOutcome::Exited(23));
	}

	#[cfg(unix)]
	#[test]
	#[serial(env)]
	fn portable_runner_distinguishes_signal_termination_from_an_exit_code() {
		let directory = TempDir::new().expect("create fake client directory");
		write_fake_client(directory.path(), "sqlite3", "kill -TERM $$");
		let _path = PathGuard::set(directory.path());
		let database = resolved_database("sqlite:db.sqlite3");
		let spec = build_client_spec(&database, &[]).expect("build client spec");

		let outcome = PortableDbClientRunner.run(&spec).expect("run fake client");

		assert_eq!(outcome, DbShellOutcome::TerminatedBySignal);
	}

	#[cfg(unix)]
	#[test]
	fn portable_runner_inherits_standard_streams() {
		let directory = TempDir::new().expect("create fake client directory");
		write_fake_client(
			directory.path(),
			"sqlite3",
			r#"
IFS= read -r input
printf 'fake-stdout:%s\n' "$input"
printf 'fake-stderr:%s\n' "$input" >&2
"#,
		);
		let executable = std::env::current_exe().expect("resolve test executable");
		let mut child = Command::new(executable)
			.args([
				"--exact",
				"dbshell::tests::portable_runner_inherits_standard_streams_child",
				"--nocapture",
			])
			.env("DBSHELL_STREAM_CHILD", "1")
			.env("PATH", directory.path())
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()
			.expect("spawn test child");
		child
			.stdin
			.take()
			.expect("open child stdin")
			.write_all(b"terminal-input\n")
			.expect("write child stdin");

		let output = child.wait_with_output().expect("wait for test child");

		assert!(output.status.success(), "{output:?}");
		assert!(
			String::from_utf8_lossy(&output.stdout).contains("fake-stdout:terminal-input"),
			"{output:?}"
		);
		assert!(
			String::from_utf8_lossy(&output.stderr).contains("fake-stderr:terminal-input"),
			"{output:?}"
		);
	}

	#[cfg(unix)]
	#[test]
	fn portable_runner_inherits_standard_streams_child() {
		if std::env::var_os("DBSHELL_STREAM_CHILD").is_none() {
			return;
		}
		let database = resolved_database("sqlite:db.sqlite3");
		let spec = build_client_spec(&database, &[]).expect("build client spec");

		let outcome = PortableDbClientRunner.run(&spec).expect("run fake client");

		assert_eq!(outcome, DbShellOutcome::Exited(0));
	}

	#[test]
	fn postgres_decodes_connection_fields_and_appends_passthrough_arguments() {
		let password = "p@ss/word";
		let database = resolved_database(
			"postgresql://user%20name:p%40ss%2Fword@[2001:db8::1]:5544/report%20data?sslmode=require",
		);
		let passthrough = vec![
			OsString::from("--single-transaction"),
			OsString::from("value with spaces"),
		];

		let spec = build_client_spec(&database, &passthrough).expect("build PostgreSQL client");

		assert_eq!(spec.executable, OsString::from("psql"));
		assert_eq!(
			spec.arguments,
			vec![
				OsString::from("--host"),
				OsString::from("2001:db8::1"),
				OsString::from("--port"),
				OsString::from("5544"),
				OsString::from("--username"),
				OsString::from("user name"),
				OsString::from("report data"),
				OsString::from("--single-transaction"),
				OsString::from("value with spaces"),
			]
		);
		assert_eq!(
			spec.secret_environment,
			vec![(OsString::from("PGPASSWORD"), OsString::from(password))]
		);
	}

	#[test]
	fn mysql_decodes_connection_fields_and_ignores_url_query_parameters() {
		let password = "s ecret?";
		let database = resolved_database(
			"mysql://report%2Buser:s%20ecret%3F@db.example:4406/analytics%2Fdaily?charset=utf8mb4",
		);
		let passthrough = vec![OsString::from("--skip-column-names")];

		let spec = build_client_spec(&database, &passthrough).expect("build MySQL client");

		assert_eq!(spec.executable, OsString::from("mysql"));
		assert_eq!(
			spec.arguments,
			vec![
				OsString::from("--host"),
				OsString::from("db.example"),
				OsString::from("--port"),
				OsString::from("4406"),
				OsString::from("--user"),
				OsString::from("report+user"),
				OsString::from("analytics/daily"),
				OsString::from("--skip-column-names"),
			]
		);
		assert_eq!(
			spec.secret_environment,
			vec![(OsString::from("MYSQL_PWD"), OsString::from(password))]
		);
	}

	#[test]
	fn sqlite_preserves_relative_and_absolute_file_paths_without_query_parameters() {
		let relative = resolved_database("sqlite:data/report%20cache.sqlite3");
		let absolute = resolved_database("sqlite:////tmp/report%20cache.sqlite3");

		let relative_spec =
			build_client_spec(&relative, &[]).expect("build relative SQLite client");
		let absolute_spec =
			build_client_spec(&absolute, &[]).expect("build absolute SQLite client");

		assert_eq!(relative_spec.executable, OsString::from("sqlite3"));
		assert_eq!(
			relative_spec.arguments,
			vec![OsString::from("data/report cache.sqlite3")]
		);
		assert!(relative_spec.secret_environment.is_empty());
		assert_eq!(absolute_spec.executable, OsString::from("sqlite3"));
		assert_eq!(
			absolute_spec.arguments,
			vec![OsString::from("/tmp/report cache.sqlite3")]
		);
		assert!(absolute_spec.secret_environment.is_empty());
	}

	#[test]
	fn sqlite_preserves_named_memory_mode_and_cache_as_a_uri_filename() {
		let database = resolved_database("sqlite:file:shared?mode=memory&cache=shared");

		let spec = build_client_spec(&database, &[]).expect("build named-memory SQLite client");

		assert_eq!(spec.executable, OsString::from("sqlite3"));
		assert_eq!(
			spec.arguments,
			vec![OsString::from("file:shared?mode=memory&cache=shared")]
		);
		assert!(spec.secret_environment.is_empty());
	}

	#[test]
	fn sqlite_preserves_absolute_read_only_mode_as_a_uri_filename() {
		let database =
			resolved_database("sqlite:////tmp/report%20cache.sqlite3?mode=ro&immutable=1");

		let spec = build_client_spec(&database, &[]).expect("build read-only SQLite client");

		assert_eq!(
			spec.arguments,
			vec![OsString::from(
				"file:/tmp/report%20cache.sqlite3?mode=ro&immutable=1"
			)]
		);
	}

	#[test]
	fn sqlite_uri_preserves_query_order_and_percent_encoding() {
		let database = resolved_database(
			"sqlite:data/report%20cache.sqlite3?cache=private&vfs=unix%2Ddotfile&mode=rw",
		);

		let spec = build_client_spec(&database, &[]).expect("build SQLite URI client");

		assert_eq!(
			spec.arguments,
			vec![OsString::from(
				"file:data/report%20cache.sqlite3?cache=private&vfs=unix%2Ddotfile&mode=rw"
			)]
		);
	}

	#[test]
	fn sqlite_uri_is_redacted_from_debug_output() {
		let raw_url = "sqlite:file:shared?mode=memory&cache=shared&token=do-not-print-this";
		let database = resolved_database(raw_url);

		let spec = build_client_spec(&database, &[]).expect("build named-memory SQLite client");
		let debug = format!("{spec:?}");

		assert!(!debug.contains(raw_url));
		assert!(!debug.contains("do-not-print-this"));
		assert_eq!(
			debug,
			"DbClientSpec { executable: \"sqlite3\", arguments: [\"[REDACTED]\"], secret_environment: [] }"
		);
	}

	#[test]
	fn sqlite_appends_passthrough_arguments_without_reinterpretation() {
		let database = resolved_database("sqlite:db.sqlite3");
		let passthrough = vec![
			OsString::from("-cmd"),
			OsString::from(".headers on"),
			OsString::from("--"),
		];

		let spec = build_client_spec(&database, &passthrough).expect("build SQLite client");

		assert_eq!(
			spec.arguments,
			vec![
				OsString::from("db.sqlite3"),
				OsString::from("-cmd"),
				OsString::from(".headers on"),
				OsString::from("--"),
			]
		);
	}

	#[test]
	fn debug_output_redacts_password_and_connection_url() {
		let password = "do-not-print-this";
		let raw_url =
			format!("postgresql://operator:{password}@db.example:5432/private?sslmode=require");
		let database = resolved_database(&raw_url);

		let spec = build_client_spec(&database, &[]).expect("build PostgreSQL client");
		let debug = format!("{spec:?}");

		assert!(!debug.contains(password));
		assert!(!debug.contains(&raw_url));
		assert_eq!(
			debug,
			"DbClientSpec { executable: \"psql\", arguments: [\"--host\", \"db.example\", \"--port\", \"5432\", \"--username\", \"operator\", \"private\"], secret_environment: [(\"PGPASSWORD\", \"[REDACTED]\")] }"
		);
	}

	#[test]
	fn malformed_url_error_omits_password_and_connection_url() {
		let password = "invalid-secret";
		let raw_url = format!("postgresql://operator:{password}@[not-an-ipv6/private");
		let database = resolved_database(&raw_url);

		let error = build_client_spec(&database, &[]).expect_err("malformed URL should fail");
		let diagnostic = error.to_string();

		assert!(!diagnostic.contains(password));
		assert!(!diagnostic.contains(&raw_url));
		assert_eq!(
			diagnostic,
			"Invalid arguments: The selected PostgreSQL database URL is malformed."
		);
	}
}
