//! Native database client specification construction.

use crate::database_selector::ResolvedDatabase;
use crate::{CommandError, CommandResult};
use percent_encoding::percent_decode_str;
use reinhardt_db::backends::DatabaseType;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
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

pub(crate) fn run_database_shell(
	database: &ResolvedDatabase,
	client_arguments: &[OsString],
	runner: &dyn DbClientRunner,
) -> CommandResult<()> {
	let spec = build_client_spec(database, client_arguments)?;
	let client_name = spec.executable.to_string_lossy();

	match runner.run(&spec)? {
		DbShellOutcome::Exited(0) => Ok(()),
		DbShellOutcome::Exited(status) => Err(CommandError::ExecutionError(format!(
			"Database client `{client_name}` exited with status {status}."
		))),
		DbShellOutcome::TerminatedBySignal => Err(CommandError::ExecutionError(format!(
			"Database client `{client_name}` was terminated by a signal."
		))),
	}
}

impl DbClientRunner for PortableDbClientRunner {
	fn run(&self, spec: &DbClientSpec) -> CommandResult<DbShellOutcome> {
		let executables = resolve_executable_candidates(&spec.executable)?;
		let mut last_retryable_error = None;
		for executable in executables {
			let child = Command::new(executable)
				.args(&spec.arguments)
				.envs(
					spec.secret_environment
						.iter()
						.map(|(name, value)| (name, value)),
				)
				.stdin(Stdio::inherit())
				.stdout(Stdio::inherit())
				.stderr(Stdio::inherit())
				.spawn();
			match child {
				Ok(child) => return wait_for_client(ManagedDbClient::new(child), spec),
				Err(error)
					if matches!(
						error.kind(),
						std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
					) =>
				{
					last_retryable_error = Some(error);
				}
				Err(error) => return Err(client_launch_error(spec, error)),
			}
		}

		match last_retryable_error {
			Some(error) => Err(client_launch_error(spec, error)),
			None => Err(executable_not_found(&spec.executable)),
		}
	}
}

struct ManagedDbClient {
	child: Child,
	has_exited: bool,
}

impl ManagedDbClient {
	fn new(child: Child) -> Self {
		Self {
			child,
			has_exited: false,
		}
	}

	fn wait(&mut self) -> std::io::Result<ExitStatus> {
		let status = self.child.wait()?;
		self.has_exited = true;
		Ok(status)
	}
}

impl Drop for ManagedDbClient {
	fn drop(&mut self) {
		if !self.has_exited {
			let _ = self.child.kill();
			let _ = self.child.wait();
		}
	}
}

fn wait_for_client(
	mut child: ManagedDbClient,
	spec: &DbClientSpec,
) -> CommandResult<DbShellOutcome> {
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

fn client_launch_error(spec: &DbClientSpec, error: std::io::Error) -> CommandError {
	CommandError::ExecutionError(format!(
		"Failed to launch database client executable `{}`: {error}",
		spec.executable.to_string_lossy()
	))
}

fn resolve_executable_candidates(executable: &OsStr) -> CommandResult<Vec<PathBuf>> {
	let Some(path) = env::var_os("PATH") else {
		return Err(executable_not_found(executable));
	};
	let mut candidates = Vec::new();
	for directory in env::split_paths(&path) {
		if directory.as_os_str().is_empty() {
			continue;
		}
		for extension in executable_extensions() {
			let mut filename = executable.to_os_string();
			filename.push(extension);
			let candidate = directory.join(filename);
			if is_executable_candidate(&candidate) {
				candidates.push(candidate);
			}
		}
	}

	if candidates.is_empty() {
		Err(executable_not_found(executable))
	} else {
		Ok(candidates)
	}
}

fn executable_not_found(executable: &OsStr) -> CommandError {
	CommandError::ExecutionError(format!(
		"Database client executable `{}` was not found on PATH.",
		executable.to_string_lossy()
	))
}

#[cfg(unix)]
fn is_executable_candidate(candidate: &Path) -> bool {
	if !std::fs::metadata(candidate).is_ok_and(|metadata| metadata.is_file()) {
		return false;
	}

	effective_user_can_execute(candidate)
}

#[cfg(unix)]
fn effective_user_can_execute(candidate: &Path) -> bool {
	use std::ffi::CString;
	use std::os::unix::ffi::OsStrExt;

	let Ok(candidate) = CString::new(candidate.as_os_str().as_bytes()) else {
		return false;
	};
	// SAFETY: the path is NUL-terminated by CString, and faccessat only reads it.
	unsafe {
		libc::faccessat(
			libc::AT_FDCWD,
			candidate.as_ptr(),
			libc::X_OK,
			libc::AT_EACCESS,
		) == 0
	}
}

#[cfg(not(unix))]
fn is_executable_candidate(candidate: &Path) -> bool {
	candidate.is_file()
}

#[cfg(not(windows))]
fn executable_extensions() -> Vec<OsString> {
	vec![OsString::new()]
}

#[cfg(windows)]
fn executable_extensions() -> Vec<OsString> {
	let configured = env::var_os("PATHEXT");
	windows_executable_extensions(configured.as_deref())
}

#[cfg(any(windows, test))]
fn windows_executable_extensions(configured: Option<&OsStr>) -> Vec<OsString> {
	let mut extensions = vec![OsString::new()];
	let configured = configured.unwrap_or_else(|| OsStr::new(".COM;.EXE"));
	for extension in configured.to_string_lossy().split(';') {
		let extension = extension.to_ascii_uppercase();
		let extension = OsString::from(&extension);
		if matches!(extension.to_str(), Some(".COM" | ".EXE")) && !extensions.contains(&extension) {
			extensions.push(extension);
		}
	}
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

pub(crate) struct RedactedArguments<'a>(pub(crate) &'a [OsString]);

impl fmt::Debug for RedactedArguments<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut list = f.debug_list();
		let mut redact_next = false;
		for argument in self.0 {
			let argument_text = argument.to_string_lossy();
			let is_sensitive = redact_next
				|| argument_text.starts_with("file:")
				|| argument_is_sensitive(&argument_text);
			redact_next = argument_text
				.strip_prefix("--")
				.is_some_and(|flag| !flag.contains('=') && option_name_is_sensitive(flag));
			if is_sensitive {
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

fn option_name_is_sensitive(name: &str) -> bool {
	let normalized_name = name.to_ascii_lowercase().replace('_', "-");
	normalized_name == "url"
		|| normalized_name.ends_with("-url")
		|| normalized_name.contains("password")
		|| normalized_name.contains("passwd")
		|| normalized_name.contains("secret")
		|| normalized_name.contains("token")
		|| normalized_name.contains("api-key")
		|| normalized_name.contains("credential")
}

fn argument_is_sensitive(argument: &str) -> bool {
	if argument.contains("://") || argument.contains('@') {
		return true;
	}
	argument
		.strip_prefix("--")
		.and_then(|flag| flag.split_once('='))
		.is_some_and(|(name, _)| option_name_is_sensitive(name))
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

	let mut secret_environment = decoded_password(&parsed, "PostgreSQL")?
		.map(|password| vec![(OsString::from("PGPASSWORD"), password)])
		.unwrap_or_default();
	append_postgres_query_environment(&parsed, &mut secret_environment)?;

	Ok(DbClientSpec {
		executable: OsString::from("psql"),
		arguments,
		secret_environment,
	})
}

fn append_postgres_query_environment(
	parsed: &Url,
	environment: &mut Vec<(OsString, OsString)>,
) -> CommandResult<()> {
	for (key, value) in parsed.query_pairs() {
		let name = match key.as_ref() {
			"application_name" => "PGAPPNAME",
			"channel_binding" => "PGCHANNELBINDING",
			"connect_timeout" => "PGCONNECT_TIMEOUT",
			"gssencmode" => "PGGSSENCMODE",
			"options" => "PGOPTIONS",
			"sslcert" => "PGSSLCERT",
			"sslcrl" => "PGSSLCRL",
			"sslcrldir" => "PGSSLCRLDIR",
			"sslkey" => "PGSSLKEY",
			"sslmode" => "PGSSLMODE",
			"sslpassword" => "PGSSLPASSWORD",
			"sslrootcert" => "PGSSLROOTCERT",
			"target_session_attrs" => "PGTARGETSESSIONATTRS",
			unsupported => {
				return Err(CommandError::InvalidArguments(format!(
					"The PostgreSQL database URL contains unsupported libpq parameter `{unsupported}`."
				)));
			}
		};
		environment.push((OsString::from(name), OsString::from(value.as_ref())));
	}
	Ok(())
}

fn build_mysql_spec(database_url: &str) -> CommandResult<DbClientSpec> {
	let parsed = parse_network_url(database_url, "MySQL")?;
	let mut arguments = Vec::new();
	let uses_socket = parsed.query_pairs().any(|(key, _)| key == "socket");
	if parsed.host().is_some() && !uses_socket {
		arguments.push(OsString::from("--protocol=TCP"));
	}
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
	append_mysql_query_arguments(&parsed, &mut arguments)?;
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

fn append_mysql_query_arguments(parsed: &Url, arguments: &mut Vec<OsString>) -> CommandResult<()> {
	for (key, value) in parsed.query_pairs() {
		let option = match key.as_ref() {
			"charset" => "--default-character-set",
			"connect-timeout" => "--connect-timeout",
			"ssl-ca" => "--ssl-ca",
			"ssl-capath" => "--ssl-capath",
			"ssl-cert" => "--ssl-cert",
			"ssl-cipher" => "--ssl-cipher",
			"ssl-crl" => "--ssl-crl",
			"ssl-crlpath" => "--ssl-crlpath",
			"ssl-key" => "--ssl-key",
			"ssl-mode" => "--ssl-mode",
			"socket" => "--socket",
			"tls-ciphersuites" => "--tls-ciphersuites",
			"tls-version" => "--tls-version",
			unsupported => {
				return Err(CommandError::InvalidArguments(format!(
					"The MySQL database URL contains unsupported client parameter `{unsupported}`."
				)));
			}
		};
		arguments.push(OsString::from(option));
		arguments.push(OsString::from(value.as_ref()));
	}
	Ok(())
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
			":memory:".to_string()
		} else if let Some(path) = url_without_query.strip_prefix("sqlite:////") {
			format!("/{path}")
		} else if let Some(path) = url_without_query.strip_prefix("sqlite:///") {
			format!("/{path}")
		} else if let Some(path) = url_without_query.strip_prefix("sqlite://") {
			path.to_string()
		} else if let Some(path) = url_without_query.strip_prefix("sqlite:") {
			path.to_string()
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
		None if encoded_path.starts_with('-') => OsString::from(format!("file:{encoded_path}")),
		None => decode_component(&encoded_path, "SQLite")?,
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
	use super::{
		DbClientRunner, DbShellOutcome, PortableDbClientRunner, build_client_spec,
		run_database_shell,
	};
	use crate::CommandResult;
	use crate::database_selector::{DatabaseSelector, resolve_database};
	use std::ffi::{OsStr, OsString};
	use std::fs;
	use std::io::Write;
	use std::path::Path;
	use std::process::{Command, Stdio};
	use tempfile::TempDir;

	struct FixedOutcomeRunner(DbShellOutcome);

	impl DbClientRunner for FixedOutcomeRunner {
		fn run(&self, _spec: &super::DbClientSpec) -> CommandResult<DbShellOutcome> {
			Ok(self.0)
		}
	}

	#[rstest::rstest]
	fn command_adapter_accepts_zero_exit_status() {
		let database =
			resolved_database("postgresql://operator:do-not-print-this@db.example:5432/private");

		let result = run_database_shell(
			&database,
			&[],
			&FixedOutcomeRunner(DbShellOutcome::Exited(0)),
		);

		assert!(result.is_ok());
	}

	#[rstest::rstest]
	fn command_adapter_reports_nonzero_status_without_secrets() {
		let password = "do-not-print-this";
		let raw_url = format!("postgresql://operator:{password}@db.example:5432/private");
		let database = resolved_database(&raw_url);

		let diagnostic = run_database_shell(
			&database,
			&[],
			&FixedOutcomeRunner(DbShellOutcome::Exited(23)),
		)
		.expect_err("nonzero client status should fail")
		.to_string();

		assert_eq!(
			diagnostic,
			"Execution error: Database client `psql` exited with status 23."
		);
		assert!(!diagnostic.contains(password));
		assert!(!diagnostic.contains(&raw_url));
	}

	#[rstest::rstest]
	fn command_adapter_reports_signal_termination_distinctly_without_secrets() {
		let password = "do-not-print-this";
		let raw_url = format!("mysql://operator:{password}@db.example:3306/private");
		let database = resolved_database(&raw_url);

		let diagnostic = run_database_shell(
			&database,
			&[],
			&FixedOutcomeRunner(DbShellOutcome::TerminatedBySignal),
		)
		.expect_err("signal-terminated client should fail")
		.to_string();

		assert_eq!(
			diagnostic,
			"Execution error: Database client `mysql` was terminated by a signal."
		);
		assert!(!diagnostic.contains(password));
		assert!(!diagnostic.contains(&raw_url));
	}

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

	fn copy_test_executable_as_client(directory: &Path, name: &str) {
		let filename = if cfg!(windows) {
			format!("{name}.EXE")
		} else {
			name.to_string()
		};
		fs::copy(
			std::env::current_exe().expect("resolve test executable"),
			directory.join(filename),
		)
		.expect("copy test executable as fake client");
	}

	fn run_isolated_runner_case(case: &str, path: &OsStr, environment: &[(&str, &OsStr)]) {
		let mut child = Command::new(std::env::current_exe().expect("resolve test executable"));
		child
			.args([
				"--exact",
				"dbshell::tests::portable_runner_isolated_case_child",
				"--nocapture",
			])
			.env("DBSHELL_RUNNER_CASE", case)
			.env("PATH", path)
			.env_remove("PGPASSWORD")
			.env_remove("MYSQL_PWD")
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped());
		for (name, value) in environment {
			child.env(name, value);
		}

		let output = child.output().expect("run isolated runner test child");

		assert!(output.status.success(), "{case}: {output:?}");
	}

	#[rstest::rstest]
	fn windows_executable_extensions_reject_shell_scripts_case_insensitively() {
		let extensions =
			super::windows_executable_extensions(Some(OsStr::new(".BAT;.eXe;.CMD;.COM;.EXE")));

		assert_eq!(
			extensions,
			vec![
				OsString::new(),
				OsString::from(".EXE"),
				OsString::from(".COM"),
			]
		);
	}

	#[cfg(windows)]
	#[rstest::rstest]
	fn executable_resolution_uses_windows_pathext_entries() {
		let directory = TempDir::new().expect("create fake client directory");
		let batch_file = directory.path().join("psql.BAT");
		let executable = directory.path().join("psql.EXE");
		std::fs::write(batch_file, b"@echo off").expect("write fake batch file");
		std::fs::write(&executable, b"fake native executable").expect("write fake executable");
		run_isolated_runner_case(
			"windows-resolve",
			directory.path().as_os_str(),
			&[
				("PATHEXT", OsStr::new(".BAT;.EXE")),
				("DBSHELL_EXPECTED_EXECUTABLE", executable.as_os_str()),
			],
		);
	}

	#[rstest::rstest]
	fn portable_runner_isolated_case_child() {
		let Some(case) = std::env::var_os("DBSHELL_RUNNER_CASE") else {
			return;
		};
		let case = case.to_string_lossy();

		match case.as_ref() {
			"exit-zero" => {
				let database = resolved_database("sqlite:db.sqlite3");
				let spec = build_client_spec(&database, &[]).expect("build client spec");
				assert_eq!(
					PortableDbClientRunner.run(&spec).expect("run fake client"),
					DbShellOutcome::Exited(0)
				);
			}
			"missing-sqlite" => {
				let database = resolved_database("sqlite:db.sqlite3");
				let spec = build_client_spec(&database, &[]).expect("build client spec");
				let error = PortableDbClientRunner
					.run(&spec)
					.expect_err("client must not resolve");
				assert_eq!(
					error.to_string(),
					"Execution error: Database client executable `sqlite3` was not found on PATH."
				);
			}
			"exact-postgres" | "exact-mysql" | "exact-sqlite" => {
				let (url, expected) = match case.as_ref() {
					"exact-postgres" => (
						"postgresql://operator:postgres-secret@db.example:5544/reporting",
						"client=psql\nargument=--host\nargument=db.example\nargument=--port\nargument=5544\nargument=--username\nargument=operator\nargument=reporting\nargument=--expanded\nPGPASSWORD=postgres-secret\nMYSQL_PWD=unset\n",
					),
					"exact-mysql" => (
						"mysql://operator:mysql-secret@db.example:4406/reporting",
						"client=mysql\nargument=--protocol=TCP\nargument=--host\nargument=db.example\nargument=--port\nargument=4406\nargument=--user\nargument=operator\nargument=reporting\nargument=--expanded\nPGPASSWORD=unset\nMYSQL_PWD=mysql-secret\n",
					),
					"exact-sqlite" => (
						"sqlite:data/report.sqlite3",
						"client=sqlite3\nargument=data/report.sqlite3\nargument=--expanded\nPGPASSWORD=unset\nMYSQL_PWD=unset\n",
					),
					_ => unreachable!("exact runner case is exhaustive"),
				};
				let record = std::env::var_os("DBSHELL_RECORD").expect("read isolated record path");
				let database = resolved_database(url);
				let mut spec = build_client_spec(&database, &[OsString::from("--expanded")])
					.expect("build client spec");
				spec.secret_environment
					.push((OsString::from("DBSHELL_TEST_RECORD"), record.clone()));
				assert_eq!(
					PortableDbClientRunner.run(&spec).expect("run fake client"),
					DbShellOutcome::Exited(0)
				);
				assert_eq!(
					fs::read_to_string(record).expect("read client record"),
					expected
				);
			}
			"scoped-secret" => {
				let record = std::env::var_os("DBSHELL_RECORD").expect("read isolated record path");
				let database =
					resolved_database("postgresql://operator:child-secret@db.example/reporting");
				let mut spec = build_client_spec(&database, &[]).expect("build client spec");
				spec.secret_environment
					.push((OsString::from("DBSHELL_TEST_RECORD"), record.clone()));
				assert_eq!(
					PortableDbClientRunner.run(&spec).expect("run fake client"),
					DbShellOutcome::Exited(0)
				);
				assert_eq!(
					fs::read_to_string(record).expect("read recorded password"),
					"child-secret"
				);
				assert_eq!(
					std::env::var("PGPASSWORD").expect("read parent password"),
					"parent-secret"
				);
			}
			"missing-postgres" => {
				let database =
					resolved_database("postgresql://operator:do-not-print@db.example/reporting");
				let spec = build_client_spec(&database, &[]).expect("build client spec");
				let diagnostic = PortableDbClientRunner
					.run(&spec)
					.expect_err("missing executable should fail")
					.to_string();
				assert_eq!(
					diagnostic,
					"Execution error: Database client executable `psql` was not found on PATH."
				);
				assert!(!diagnostic.contains("do-not-print"));
				assert!(!diagnostic.contains("reporting"));
			}
			"exit-23" => {
				let database = resolved_database("sqlite:db.sqlite3");
				let spec = build_client_spec(&database, &[]).expect("build client spec");
				assert_eq!(
					PortableDbClientRunner.run(&spec).expect("run fake client"),
					DbShellOutcome::Exited(23)
				);
			}
			#[cfg(unix)]
			"signal" => {
				let database = resolved_database("sqlite:db.sqlite3");
				let spec = build_client_spec(&database, &[]).expect("build client spec");
				assert_eq!(
					PortableDbClientRunner.run(&spec).expect("run fake client"),
					DbShellOutcome::TerminatedBySignal
				);
			}
			#[cfg(windows)]
			"windows-resolve" => {
				let expected = std::env::var_os("DBSHELL_EXPECTED_EXECUTABLE")
					.expect("read expected executable");
				assert_eq!(
					super::resolve_executable_candidates(OsStr::new("psql"))
						.expect("resolve native executable"),
					vec![std::path::PathBuf::from(expected)]
				);
			}
			_ => panic!("unknown isolated runner case: {case}"),
		}
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_skips_non_executable_files_in_earlier_path_entries() {
		let non_executable_directory =
			TempDir::new().expect("create non-executable client directory");
		let executable_directory = TempDir::new().expect("create executable client directory");
		fs::write(
			non_executable_directory.path().join("sqlite3"),
			"#!/bin/sh\nexit 91\n",
		)
		.expect("write non-executable client");
		write_fake_client(executable_directory.path(), "sqlite3", "exit 0");
		let path =
			std::env::join_paths([non_executable_directory.path(), executable_directory.path()])
				.expect("join fake client PATH");

		run_isolated_runner_case("exit-zero", &path, &[]);
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_skips_owner_inaccessible_execute_bits() {
		use std::os::unix::fs::PermissionsExt;

		for inaccessible_mode in [0o001, 0o010] {
			let inaccessible_directory =
				TempDir::new().expect("create inaccessible client directory");
			let executable_directory = TempDir::new().expect("create executable client directory");
			let inaccessible = inaccessible_directory.path().join("sqlite3");
			fs::write(&inaccessible, "#!/bin/sh\nexit 91\n").expect("write inaccessible client");
			fs::set_permissions(&inaccessible, fs::Permissions::from_mode(inaccessible_mode))
				.expect("set inaccessible client mode");
			write_fake_client(executable_directory.path(), "sqlite3", "exit 0");
			let path =
				std::env::join_paths([inaccessible_directory.path(), executable_directory.path()])
					.expect("join fake client PATH");

			run_isolated_runner_case("exit-zero", &path, &[]);
		}
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_falls_back_after_a_retryable_spawn_error() {
		use std::os::unix::fs::PermissionsExt;

		let stale_directory = TempDir::new().expect("create stale client directory");
		let executable_directory = TempDir::new().expect("create executable client directory");
		let stale = stale_directory.path().join("sqlite3");
		fs::write(&stale, "#!/definitely/missing/dbshell-test-interpreter\n")
			.expect("write stale client");
		fs::set_permissions(&stale, fs::Permissions::from_mode(0o755))
			.expect("set stale client mode");
		write_fake_client(executable_directory.path(), "sqlite3", "exit 0");
		let path = std::env::join_paths([stale_directory.path(), executable_directory.path()])
			.expect("join fake client PATH");

		run_isolated_runner_case("exit-zero", &path, &[]);
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_reports_only_owner_inaccessible_client_as_missing() {
		use std::os::unix::fs::PermissionsExt;

		let directory = TempDir::new().expect("create inaccessible client directory");
		let inaccessible = directory.path().join("sqlite3");
		fs::write(&inaccessible, "#!/bin/sh\nexit 0\n").expect("write inaccessible client");
		fs::set_permissions(&inaccessible, fs::Permissions::from_mode(0o001))
			.expect("set inaccessible client mode");

		run_isolated_runner_case("missing-sqlite", directory.path().as_os_str(), &[]);
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_follows_symlinks_to_executable_files() {
		use std::os::unix::fs::symlink;

		let directory = TempDir::new().expect("create fake client directory");
		let target_directory = TempDir::new().expect("create fake target directory");
		write_fake_client(target_directory.path(), "sqlite-client", "exit 0");
		symlink(
			target_directory.path().join("sqlite-client"),
			directory.path().join("sqlite3"),
		)
		.expect("create executable client symlink");

		run_isolated_runner_case("exit-zero", directory.path().as_os_str(), &[]);
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_reports_non_executable_files_as_missing() {
		let directory = TempDir::new().expect("create fake client directory");
		fs::write(directory.path().join("sqlite3"), "#!/bin/sh\nexit 0\n")
			.expect("write non-executable client");

		run_isolated_runner_case("missing-sqlite", directory.path().as_os_str(), &[]);
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_path_semantics_are_isolated_in_child_processes() {
		let directory = TempDir::new().expect("create fake client directory");
		write_fake_client(directory.path(), "sqlite3", "exit 0");
		let relative_directory = directory.path().join("clients");
		fs::create_dir(&relative_directory).expect("create relative client directory");
		write_fake_client(&relative_directory, "sqlite3", "exit 0");
		let executable = std::env::current_exe().expect("resolve test executable");

		for (case, path) in [
			("unset", None),
			("empty", Some("")),
			("relative", Some("clients")),
		] {
			let mut child = Command::new(&executable);
			child
				.args([
					"--exact",
					"dbshell::tests::portable_runner_path_semantics_child",
					"--nocapture",
				])
				.env("DBSHELL_PATH_CASE", case)
				.current_dir(directory.path())
				.stdin(Stdio::null())
				.stdout(Stdio::piped())
				.stderr(Stdio::piped());
			if let Some(path) = path {
				child.env("PATH", path);
			} else {
				child.env_remove("PATH");
			}

			let output = child.output().expect("run PATH semantics child");

			assert!(output.status.success(), "{case}: {output:?}");
		}
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_path_semantics_child() {
		let Some(case) = std::env::var_os("DBSHELL_PATH_CASE") else {
			return;
		};
		let database = resolved_database("sqlite:db.sqlite3");
		let spec = build_client_spec(&database, &[]).expect("build client spec");

		if case == "relative" {
			let outcome = PortableDbClientRunner
				.run(&spec)
				.expect("run client from explicit relative PATH entry");
			assert_eq!(outcome, DbShellOutcome::Exited(0));
		} else {
			let error = PortableDbClientRunner
				.run(&spec)
				.expect_err("unset or empty PATH must not search the current directory");
			assert_eq!(
				error.to_string(),
				"Execution error: Database client executable `sqlite3` was not found on PATH."
			);
		}
	}

	#[cfg(unix)]
	#[rstest::rstest]
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

		for case in ["exact-postgres", "exact-mysql", "exact-sqlite"] {
			run_isolated_runner_case(case, &path, &[("DBSHELL_RECORD", record.as_os_str())]);
		}
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_scopes_secret_environment_to_the_child() {
		let directory = TempDir::new().expect("create fake client directory");
		let record = directory.path().join("record");
		write_fake_client(
			directory.path(),
			"psql",
			"printf '%s' \"$PGPASSWORD\" > \"$DBSHELL_TEST_RECORD\"",
		);

		run_isolated_runner_case(
			"scoped-secret",
			directory.path().as_os_str(),
			&[
				("DBSHELL_RECORD", record.as_os_str()),
				("PGPASSWORD", OsStr::new("parent-secret")),
			],
		);
	}

	#[rstest::rstest]
	fn portable_runner_reports_a_missing_executable_without_leaking_arguments() {
		let directory = TempDir::new().expect("create empty client directory");

		run_isolated_runner_case("missing-postgres", directory.path().as_os_str(), &[]);
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_preserves_nonzero_exit_status() {
		let directory = TempDir::new().expect("create fake client directory");
		write_fake_client(directory.path(), "sqlite3", "exit 23");

		run_isolated_runner_case("exit-23", directory.path().as_os_str(), &[]);
	}

	#[cfg(unix)]
	#[rstest::rstest]
	fn portable_runner_distinguishes_signal_termination_from_an_exit_code() {
		let directory = TempDir::new().expect("create fake client directory");
		write_fake_client(directory.path(), "sqlite3", "kill -TERM $$");

		run_isolated_runner_case("signal", directory.path().as_os_str(), &[]);
	}

	#[rstest::rstest]
	fn portable_runner_inherits_standard_streams() {
		// Arrange
		let directory = TempDir::new().expect("create fake client directory");
		copy_test_executable_as_client(directory.path(), "sqlite3");
		let executable = std::env::current_exe().expect("resolve test executable");
		let mut child = Command::new(executable)
			.args([
				"--exact",
				"dbshell::tests::portable_runner_inherits_standard_streams_child",
				"--nocapture",
			])
			.env("DBSHELL_STREAM_CHILD", "1")
			.env("PATH", directory.path())
			.env("PATHEXT", ".BAT;.EXE")
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

		// Act
		let output = child.wait_with_output().expect("wait for test child");

		// Assert
		assert_eq!(output.status.code(), Some(0), "{output:?}");
		let stdout = String::from_utf8(output.stdout).expect("child stdout should be UTF-8");
		let stderr = String::from_utf8(output.stderr).expect("child stderr should be UTF-8");
		assert_eq!(
			extract_forwarded_output(&stdout, "fake-stdout-begin\n", "fake-stdout-end\n"),
			"fake-stdout:terminal-input\nfake-secret:child-secret\n"
		);
		assert_eq!(
			extract_forwarded_output(&stderr, "fake-stderr-begin\n", "fake-stderr-end\n"),
			"fake-stderr:terminal-input\n"
		);
	}

	fn extract_forwarded_output<'a>(output: &'a str, begin: &str, end: &str) -> &'a str {
		assert_eq!(output.matches(begin).count(), 1, "{output}");
		assert_eq!(output.matches(end).count(), 1, "{output}");
		let (_, forwarded) = output
			.split_once(begin)
			.expect("forwarded output should have a start marker");
		let (forwarded, _) = forwarded
			.split_once(end)
			.expect("forwarded output should have an end marker");
		forwarded
	}

	#[rstest::rstest]
	fn portable_runner_inherits_standard_streams_child() {
		if std::env::var_os("DBSHELL_STREAM_CHILD").is_none() {
			return;
		}
		let spec = super::DbClientSpec {
			executable: OsString::from("sqlite3"),
			arguments: vec![
				OsString::from("--exact"),
				OsString::from("dbshell::tests::portable_runner_fake_client_child"),
				OsString::from("--nocapture"),
			],
			secret_environment: vec![
				(OsString::from("DBSHELL_FAKE_CLIENT"), OsString::from("1")),
				(
					OsString::from("DBSHELL_FAKE_SECRET"),
					OsString::from("child-secret"),
				),
			],
		};

		let outcome = PortableDbClientRunner.run(&spec).expect("run fake client");

		assert_eq!(outcome, DbShellOutcome::Exited(0));
	}

	#[rstest::rstest]
	fn portable_runner_fake_client_child() {
		if std::env::var_os("DBSHELL_FAKE_CLIENT").is_none() {
			return;
		}
		let mut input = String::new();
		std::io::stdin()
			.read_line(&mut input)
			.expect("read inherited stdin");

		println!("fake-stdout-begin");
		print!("fake-stdout:{input}");
		println!(
			"fake-secret:{}",
			std::env::var("DBSHELL_FAKE_SECRET").expect("read child-only secret")
		);
		println!("fake-stdout-end");
		eprintln!("fake-stderr-begin");
		eprint!("fake-stderr:{input}");
		eprintln!("fake-stderr-end");
	}

	#[rstest::rstest]
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
			vec![
				(OsString::from("PGPASSWORD"), OsString::from(password)),
				(OsString::from("PGSSLMODE"), OsString::from("require")),
			]
		);
	}

	#[rstest::rstest]
	fn postgres_rejects_unsupported_connection_parameters() {
		let database = resolved_database("postgresql://operator@db.example/reporting?unknown=yes");

		let error = build_client_spec(&database, &[])
			.err()
			.expect("unsupported parameter should fail");

		assert_eq!(
			error.to_string(),
			"Invalid arguments: The PostgreSQL database URL contains unsupported libpq parameter `unknown`."
		);
	}

	#[rstest::rstest]
	fn mysql_decodes_connection_fields_and_preserves_supported_url_query_parameters() {
		let password = "s ecret?";
		let database = resolved_database(
			"mysql://report%2Buser:s%20ecret%3F@db.example:4406/analytics%2Fdaily?charset=utf8mb4&ssl-mode=REQUIRED",
		);
		let passthrough = vec![OsString::from("--skip-column-names")];

		let spec = build_client_spec(&database, &passthrough).expect("build MySQL client");

		assert_eq!(spec.executable, OsString::from("mysql"));
		assert_eq!(
			spec.arguments,
			vec![
				OsString::from("--protocol=TCP"),
				OsString::from("--host"),
				OsString::from("db.example"),
				OsString::from("--port"),
				OsString::from("4406"),
				OsString::from("--user"),
				OsString::from("report+user"),
				OsString::from("--default-character-set"),
				OsString::from("utf8mb4"),
				OsString::from("--ssl-mode"),
				OsString::from("REQUIRED"),
				OsString::from("analytics/daily"),
				OsString::from("--skip-column-names"),
			]
		);
		assert_eq!(
			spec.secret_environment,
			vec![(OsString::from("MYSQL_PWD"), OsString::from(password))]
		);
	}

	#[rstest::rstest]
	fn mysql_uses_socket_query_parameter_without_forcing_tcp() {
		let database = resolved_database(
			"mysql://operator@localhost:4406/reporting?socket=%2Fvar%2Frun%2Fmysql.sock",
		);

		let spec = build_client_spec(&database, &[]).expect("build MySQL socket client");

		assert_eq!(
			spec.arguments,
			vec![
				OsString::from("--host"),
				OsString::from("localhost"),
				OsString::from("--port"),
				OsString::from("4406"),
				OsString::from("--user"),
				OsString::from("operator"),
				OsString::from("--socket"),
				OsString::from("/var/run/mysql.sock"),
				OsString::from("reporting"),
			]
		);
	}

	#[rstest::rstest]
	fn db_client_debug_redacts_sensitive_passthrough_arguments() {
		let spec = super::DbClientSpec {
			executable: OsString::from("mysql"),
			arguments: vec![
				OsString::from("--password=cli-secret"),
				OsString::from("--token"),
				OsString::from("token-secret"),
				OsString::from("--database-url"),
				OsString::from("mysql://operator:database-secret@localhost/app"),
				OsString::from("--safe-option"),
			],
			secret_environment: Vec::new(),
		};

		let debug = format!("{spec:?}");

		assert_eq!(
			debug,
			"DbClientSpec { executable: \"mysql\", arguments: [\"[REDACTED]\", \"--token\", \"[REDACTED]\", \"--database-url\", \"[REDACTED]\", \"--safe-option\"], secret_environment: [] }"
		);
		assert!(!debug.contains("cli-secret"));
		assert!(!debug.contains("token-secret"));
		assert!(!debug.contains("database-secret"));
	}

	#[rstest::rstest]
	fn mysql_rejects_unsupported_connection_parameters() {
		let database = resolved_database("mysql://operator@db.example/reporting?unknown=yes");

		let error = build_client_spec(&database, &[])
			.err()
			.expect("unsupported parameter should fail");

		assert_eq!(
			error.to_string(),
			"Invalid arguments: The MySQL database URL contains unsupported client parameter `unknown`."
		);
	}

	#[rstest::rstest]
	fn sqlite_preserves_relative_and_absolute_file_paths_without_query_parameters() {
		let relative = resolved_database("sqlite:data/report%20cache.sqlite3");
		let three_slash_absolute = resolved_database("sqlite:///tmp/report%20cache.sqlite3");
		let absolute = resolved_database("sqlite:////tmp/report%20cache.sqlite3");

		let relative_spec =
			build_client_spec(&relative, &[]).expect("build relative SQLite client");
		let three_slash_absolute_spec = build_client_spec(&three_slash_absolute, &[])
			.expect("build three-slash absolute SQLite client");
		let absolute_spec =
			build_client_spec(&absolute, &[]).expect("build absolute SQLite client");

		assert_eq!(relative_spec.executable, OsString::from("sqlite3"));
		assert_eq!(
			relative_spec.arguments,
			vec![OsString::from("data/report cache.sqlite3")]
		);
		assert!(relative_spec.secret_environment.is_empty());
		assert_eq!(
			three_slash_absolute_spec.executable,
			OsString::from("sqlite3")
		);
		assert_eq!(
			three_slash_absolute_spec.arguments,
			vec![OsString::from("/tmp/report cache.sqlite3")]
		);
		assert!(three_slash_absolute_spec.secret_environment.is_empty());
		assert_eq!(absolute_spec.executable, OsString::from("sqlite3"));
		assert_eq!(
			absolute_spec.arguments,
			vec![OsString::from("/tmp/report cache.sqlite3")]
		);
		assert!(absolute_spec.secret_environment.is_empty());
	}

	#[rstest::rstest]
	fn sqlite_disambiguates_relative_filenames_that_start_with_a_hyphen() {
		let database = resolved_database("sqlite:-archive.db");

		let spec = build_client_spec(&database, &[]).expect("build SQLite client");

		assert_eq!(spec.arguments, vec![OsString::from("file:-archive.db")]);
	}

	#[rstest::rstest]
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

	#[rstest::rstest]
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

	#[rstest::rstest]
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

	#[rstest::rstest]
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

	#[rstest::rstest]
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

	#[rstest::rstest]
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
			"DbClientSpec { executable: \"psql\", arguments: [\"--host\", \"db.example\", \"--port\", \"5432\", \"--username\", \"operator\", \"private\"], secret_environment: [(\"PGPASSWORD\", \"[REDACTED]\"), (\"PGSSLMODE\", \"[REDACTED]\")] }"
		);
	}

	#[rstest::rstest]
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
