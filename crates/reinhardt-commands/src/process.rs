use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessStdio {
	Capture,
	Inherit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessRequest {
	pub(crate) program: OsString,
	pub(crate) args: Vec<OsString>,
	pub(crate) current_dir: Option<PathBuf>,
	pub(crate) env: Vec<(OsString, OsString)>,
	pub(crate) stdio: ProcessStdio,
}

impl ProcessRequest {
	pub(crate) fn new(program: impl Into<OsString>) -> Self {
		Self {
			program: program.into(),
			args: Vec::new(),
			current_dir: None,
			env: Vec::new(),
			stdio: ProcessStdio::Capture,
		}
	}

	pub(crate) fn arg(mut self, arg: impl Into<OsString>) -> Self {
		self.args.push(arg.into());
		self
	}

	pub(crate) fn args<I, S>(mut self, args: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<OsString>,
	{
		self.args.extend(args.into_iter().map(Into::into));
		self
	}

	pub(crate) fn current_dir(mut self, current_dir: impl Into<PathBuf>) -> Self {
		self.current_dir = Some(current_dir.into());
		self
	}

	pub(crate) fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
		self.env.push((key.into(), value.into()));
		self
	}

	pub(crate) fn inherit_stdio(mut self) -> Self {
		self.stdio = ProcessStdio::Inherit;
		self
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessOutcome {
	pub(crate) success: bool,
	pub(crate) status: String,
	pub(crate) stdout: Vec<u8>,
	pub(crate) stderr: Vec<u8>,
}

pub(crate) trait ProcessRunner: Send + Sync {
	fn run(&self, request: &ProcessRequest) -> std::io::Result<ProcessOutcome>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
	fn run(&self, request: &ProcessRequest) -> std::io::Result<ProcessOutcome> {
		let mut command = Command::new(&request.program);
		command.args(&request.args);
		for (key, value) in &request.env {
			command.env(key, value);
		}
		if let Some(current_dir) = &request.current_dir {
			command.current_dir(current_dir);
		}

		match request.stdio {
			ProcessStdio::Capture => {
				let output = command.output()?;
				Ok(ProcessOutcome {
					success: output.status.success(),
					status: output.status.to_string(),
					stdout: output.stdout,
					stderr: output.stderr,
				})
			}
			ProcessStdio::Inherit => {
				command
					.stdin(Stdio::inherit())
					.stdout(Stdio::inherit())
					.stderr(Stdio::inherit());
				let status = command.status()?;
				Ok(ProcessOutcome {
					success: status.success(),
					status: status.to_string(),
					stdout: Vec::new(),
					stderr: Vec::new(),
				})
			}
		}
	}
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct FakeProcessRunner {
	outcomes: std::sync::Arc<
		std::sync::Mutex<std::collections::VecDeque<std::io::Result<ProcessOutcome>>>,
	>,
	requests: std::sync::Arc<std::sync::Mutex<Vec<ProcessRequest>>>,
}

#[cfg(test)]
impl FakeProcessRunner {
	pub(crate) fn new(outcomes: impl IntoIterator<Item = std::io::Result<ProcessOutcome>>) -> Self {
		Self {
			outcomes: std::sync::Arc::new(std::sync::Mutex::new(outcomes.into_iter().collect())),
			requests: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
		}
	}

	pub(crate) fn requests(&self) -> Vec<ProcessRequest> {
		self.requests
			.lock()
			.expect("fake process runner request lock is poisoned")
			.clone()
	}
}

#[cfg(test)]
impl ProcessRunner for FakeProcessRunner {
	fn run(&self, request: &ProcessRequest) -> std::io::Result<ProcessOutcome> {
		self.requests
			.lock()
			.expect("fake process runner request lock is poisoned")
			.push(request.clone());
		self.outcomes
			.lock()
			.expect("fake process runner outcome lock is poisoned")
			.pop_front()
			.unwrap_or_else(|| {
				Err(std::io::Error::new(
					std::io::ErrorKind::UnexpectedEof,
					"fake process runner has no scripted outcome",
				))
			})
	}
}

#[cfg(test)]
impl ProcessOutcome {
	pub(crate) fn success(stdout: Vec<u8>) -> Self {
		Self {
			success: true,
			status: "exit status: 0".to_owned(),
			stdout,
			stderr: Vec::new(),
		}
	}

	pub(crate) fn failure(status: impl Into<String>, stderr: Vec<u8>) -> Self {
		Self {
			success: false,
			status: status.into(),
			stdout: Vec::new(),
			stderr,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::ffi::OsString;
	use std::path::PathBuf;

	use super::*;

	#[test]
	fn request_builder_preserves_program_args_directory_env_and_stdio_mode() {
		let request = ProcessRequest::new("cargo")
			.args(["build", "--lib"])
			.current_dir("/tmp/project")
			.env("REINHARDT_ENV", "test")
			.inherit_stdio();

		assert_eq!(request.program, OsString::from("cargo"));
		assert_eq!(
			request.args,
			vec![OsString::from("build"), OsString::from("--lib")]
		);
		assert_eq!(request.current_dir, Some(PathBuf::from("/tmp/project")));
		assert_eq!(
			request.env,
			vec![(OsString::from("REINHARDT_ENV"), OsString::from("test"))]
		);
		assert_eq!(request.stdio, ProcessStdio::Inherit);
	}

	#[test]
	fn fake_runner_returns_scripted_outcome_and_records_request() {
		let runner = FakeProcessRunner::new([Ok(ProcessOutcome::success(b"ok".to_vec()))]);
		let request = ProcessRequest::new("rustfmt").arg("--version");

		let outcome = runner.run(&request).expect("scripted process succeeds");

		assert!(outcome.success);
		assert_eq!(outcome.stdout, b"ok");
		assert_eq!(runner.requests(), vec![request]);
	}

	#[test]
	#[cfg(unix)]
	fn system_runner_captures_process_output_and_status() {
		let runner = SystemProcessRunner;
		let request = ProcessRequest::new("sh").args(["-c", "printf process-output"]);

		let outcome = runner.run(&request).expect("shell command succeeds");

		assert!(outcome.success);
		assert_eq!(outcome.status, "exit status: 0");
		assert_eq!(outcome.stdout, b"process-output");
		assert!(outcome.stderr.is_empty());
	}

	#[test]
	#[cfg(unix)]
	fn system_runner_inherits_stdio_and_leaves_output_buffers_empty() {
		let runner = SystemProcessRunner;
		let request = ProcessRequest::new("sh")
			.args(["-c", "exit 0"])
			.inherit_stdio();

		let outcome = runner.run(&request).expect("shell command succeeds");

		assert!(outcome.success);
		assert_eq!(outcome.status, "exit status: 0");
		assert!(outcome.stdout.is_empty());
		assert!(outcome.stderr.is_empty());
	}
}
