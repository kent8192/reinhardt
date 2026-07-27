#![cfg(any(
	target_os = "macos",
	all(target_os = "linux", not(target_env = "uclibc"))
))]

//! Desktop Unix end-to-end coverage for the ORM-aware Rust management shell.

use std::fs;
use std::io::{self, Read};
use std::ops::{Deref, DerefMut};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nix::errno::Errno;
#[cfg(target_os = "macos")]
use nix::libc;
use nix::sys::signal::{Signal, kill, killpg};
#[cfg(all(target_os = "linux", not(target_env = "uclibc")))]
use nix::sys::wait::{Id, waitid};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{Pid, getpgid, getpgrp};
use rexpect::session::PtySession;
use tempfile::TempDir;

// The evaluator builds its dependencies outside the fixture target directory.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(600);
const FIXTURE_BUILD_TIMEOUT: Duration = Duration::from_secs(600);
const PTY_TIMEOUT: Duration = Duration::from_secs(60);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const READER_TIMEOUT: Duration = Duration::from_secs(2);
const SHELL_RESET_WARNING: &str = "Shell state was reset and the project prelude was reloaded.";
#[cfg(target_os = "macos")]
static DARWIN_RUNNING_EPERM_OBSERVED: AtomicBool = AtomicBool::new(false);

struct BackgroundReader {
	result: Receiver<io::Result<Vec<u8>>>,
	handle: Option<JoinHandle<()>>,
}

impl BackgroundReader {
	fn spawn<R>(mut reader: R, name: &'static str) -> io::Result<Self>
	where
		R: Read + Send + 'static,
	{
		let (result_tx, result) = mpsc::sync_channel(1);
		let handle = std::thread::Builder::new()
			.name(name.to_string())
			.spawn(move || {
				let mut output = Vec::new();
				let result = reader.read_to_end(&mut output).map(|_| output);
				let _ = result_tx.send(result);
			})?;
		Ok(Self {
			result,
			handle: Some(handle),
		})
	}

	fn finish(&mut self, timeout: Duration) -> io::Result<Vec<u8>> {
		match self.result.recv_timeout(timeout) {
			Ok(result) => {
				self.join_finished()?;
				result
			}
			Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
				io::ErrorKind::TimedOut,
				"process output reader did not finish before its deadline",
			)),
			Err(mpsc::RecvTimeoutError::Disconnected) => {
				self.join_finished()?;
				Err(io::Error::other(
					"process output reader exited without reporting a result",
				))
			}
		}
	}

	fn join_finished(&mut self) -> io::Result<()> {
		if let Some(handle) = self.handle.take() {
			handle
				.join()
				.map_err(|_| io::Error::other("process output reader panicked"))?;
		}
		Ok(())
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LeaderState {
	Running,
	Anchored,
	Reaped,
}

struct SupervisedGroup {
	process_group: Pid,
	leader_state: LeaderState,
	stdout_reader: Option<BackgroundReader>,
	stderr_reader: Option<BackgroundReader>,
	cleaned: bool,
}

impl SupervisedGroup {
	fn spawn(mut command: Command) -> io::Result<Self> {
		command
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped());
		command.process_group(0);
		let mut child = command.spawn()?;
		let process_group = Pid::from_raw(
			child
				.id()
				.try_into()
				.map_err(|_| io::Error::other("child PID exceeded i32"))?,
		);
		let mut supervisor = Self {
			process_group,
			leader_state: LeaderState::Running,
			stdout_reader: None,
			stderr_reader: None,
			cleaned: false,
		};
		let stdout = child
			.stdout
			.take()
			.ok_or_else(|| io::Error::other("piped stdout was not available"))?;
		let stderr = child
			.stderr
			.take()
			.ok_or_else(|| io::Error::other("piped stderr was not available"))?;
		supervisor.stdout_reader = Some(BackgroundReader::spawn(
			stdout,
			"shell-e2e-supervised-stdout",
		)?);
		supervisor.stderr_reader = Some(BackgroundReader::spawn(
			stderr,
			"shell-e2e-supervised-stderr",
		)?);
		drop(child);

		Ok(supervisor)
	}

	fn wait_with_output(mut self, timeout: Duration) -> io::Result<Output> {
		let deadline = Instant::now() + timeout;
		if !wait_for_leader_exit_anchored(self.process_group, deadline)? {
			return Err(io::Error::new(
				io::ErrorKind::TimedOut,
				format!("process group exceeded its {timeout:?} deadline"),
			));
		}
		self.leader_state = LeaderState::Anchored;
		let status = self.finish_anchored_subtree(CLEANUP_TIMEOUT)?;
		let stdout = finish_reader(&mut self.stdout_reader, READER_TIMEOUT)?;
		let stderr = finish_reader(&mut self.stderr_reader, READER_TIMEOUT)?;
		self.cleaned = true;
		Ok(Output {
			status,
			stdout,
			stderr,
		})
	}

	fn finish_anchored_subtree(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
		debug_assert_eq!(self.leader_state, LeaderState::Anchored);
		let deadline = Instant::now() + timeout;
		send_anchored_group_sigkill(self.process_group, deadline)?;
		std::thread::sleep(Duration::from_millis(10));
		send_anchored_group_sigkill(self.process_group, deadline)?;
		let status = reap_anchored_leader(self.process_group, deadline)?.ok_or_else(|| {
			io::Error::new(
				io::ErrorKind::TimedOut,
				"anchored process-group leader was not reaped before its deadline",
			)
		})?;
		self.leader_state = LeaderState::Reaped;
		if !wait_for_process_group_exit(self.process_group, deadline)? {
			return Err(io::Error::new(
				io::ErrorKind::TimedOut,
				"process group did not disappear after its leader was reaped",
			));
		}
		Ok(status)
	}

	fn force_cleanup(&mut self, timeout: Duration) -> io::Result<bool> {
		let deadline = Instant::now() + timeout;
		match self.leader_state {
			LeaderState::Running => {
				if !anchor_running_subtree(self.process_group, deadline)? {
					return Ok(false);
				}
				self.leader_state = LeaderState::Anchored;
			}
			LeaderState::Anchored => {
				signal_anchored_subtree(self.process_group, deadline)?;
			}
			LeaderState::Reaped => {
				return wait_for_process_group_exit(self.process_group, deadline);
			}
		}
		send_anchored_group_sigkill(self.process_group, deadline)?;
		if reap_anchored_leader(self.process_group, deadline)?.is_none() {
			return Ok(false);
		}
		self.leader_state = LeaderState::Reaped;
		wait_for_process_group_exit(self.process_group, deadline)
	}
}

impl Drop for SupervisedGroup {
	fn drop(&mut self) {
		if self.cleaned {
			return;
		}
		let group_gone = match self.force_cleanup(CLEANUP_TIMEOUT) {
			Ok(group_gone) => group_gone,
			Err(error) => {
				eprintln!("shell E2E process-group cleanup failed: {error}");
				false
			}
		};
		if group_gone {
			if let Err(error) = finish_reader(&mut self.stdout_reader, READER_TIMEOUT)
				&& error.kind() != io::ErrorKind::TimedOut
			{
				eprintln!("shell E2E stdout reader cleanup failed: {error}");
			}
			if let Err(error) = finish_reader(&mut self.stderr_reader, READER_TIMEOUT)
				&& error.kind() != io::ErrorKind::TimedOut
			{
				eprintln!("shell E2E stderr reader cleanup failed: {error}");
			}
		}
	}
}

fn finish_reader(reader: &mut Option<BackgroundReader>, timeout: Duration) -> io::Result<Vec<u8>> {
	match reader.take() {
		Some(mut reader) => reader.finish(timeout),
		None => Ok(Vec::new()),
	}
}

fn supervised_output(command: Command, timeout: Duration) -> io::Result<Output> {
	SupervisedGroup::spawn(command)?.wait_with_output(timeout)
}

struct SupervisedPty {
	session: Option<PtySession>,
	child_pid: Pid,
	process_group: Option<Pid>,
	leader_state: LeaderState,
	cleaned: bool,
}

impl SupervisedPty {
	fn spawn(command: Command) -> Result<Self, String> {
		let mut session =
			rexpect::session::spawn_command(command, Some(PTY_TIMEOUT.as_millis() as u64))
				.map_err(|error| format!("interactive shell should start: {error}"))?;
		let child_pid = session.process.child_pid;
		session.process.set_kill_timeout(Some(250));
		let mut supervisor = Self {
			session: Some(session),
			child_pid,
			process_group: None,
			leader_state: LeaderState::Running,
			cleaned: false,
		};
		supervisor.arm_process_group()?;
		Ok(supervisor)
	}

	fn arm_process_group(&mut self) -> Result<(), String> {
		let parent_group = getpgrp();
		let deadline = Instant::now() + Duration::from_secs(5);
		loop {
			match retry_eintr(deadline, || getpgid(Some(self.child_pid))) {
				Ok(child_group) if child_group == self.child_pid && child_group != parent_group => {
					self.process_group = Some(child_group);
					return Ok(());
				}
				Ok(_) => {}
				Err(error) => {
					return Err(format!("PTY child process group should resolve: {error}"));
				}
			}
			if Instant::now() >= deadline {
				return Err(format!(
					"refusing unsafe PTY supervision: child={}, parent_group={parent_group}",
					self.child_pid
				));
			}
			std::thread::sleep(Duration::from_millis(5));
		}
	}

	fn assert_success(mut self) {
		let deadline = Instant::now() + CLEANUP_TIMEOUT;
		assert!(
			wait_for_leader_exit_anchored(self.child_pid, deadline)
				.expect("PTY leader observation should succeed"),
			"PTY leader should exit before its deadline"
		);
		self.leader_state = LeaderState::Anchored;
		let status = self
			.finish_anchored_subtree(CLEANUP_TIMEOUT)
			.expect("PTY anchored subtree cleanup should succeed");
		assert_pty_success(status);
		self.drop_inner_without_panic();
		self.cleaned = true;
	}

	fn finish_anchored_subtree(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
		debug_assert_eq!(self.leader_state, LeaderState::Anchored);
		let deadline = Instant::now() + timeout;
		send_anchored_group_sigkill(self.child_pid, deadline)?;
		std::thread::sleep(Duration::from_millis(10));
		send_anchored_group_sigkill(self.child_pid, deadline)?;
		let status = reap_anchored_leader(self.child_pid, deadline)?.ok_or_else(|| {
			io::Error::new(
				io::ErrorKind::TimedOut,
				"anchored PTY leader was not reaped before its deadline",
			)
		})?;
		self.leader_state = LeaderState::Reaped;
		if !wait_for_process_group_exit(self.child_pid, deadline)? {
			return Err(io::Error::new(
				io::ErrorKind::TimedOut,
				"PTY process group did not disappear after leader reap",
			));
		}
		Ok(status)
	}

	fn force_cleanup(&mut self, timeout: Duration) -> io::Result<bool> {
		let deadline = Instant::now() + timeout;
		match self.leader_state {
			LeaderState::Running => {
				if !anchor_running_subtree(self.child_pid, deadline)? {
					return Ok(false);
				}
				self.leader_state = LeaderState::Anchored;
			}
			LeaderState::Anchored => {
				signal_anchored_subtree(self.child_pid, deadline)?;
			}
			LeaderState::Reaped => {
				return wait_for_process_group_exit(self.child_pid, deadline);
			}
		}
		send_anchored_group_sigkill(self.child_pid, deadline)?;
		if reap_anchored_leader(self.child_pid, deadline)?.is_none() {
			return Ok(false);
		}
		self.leader_state = LeaderState::Reaped;
		wait_for_process_group_exit(self.child_pid, deadline)
	}

	fn drop_inner_without_panic(&mut self) {
		if let Some(mut session) = self.session.take() {
			session.process.set_kill_timeout(Some(250));
			let _ = catch_unwind(AssertUnwindSafe(|| drop(session)));
		}
	}
}

impl Deref for SupervisedPty {
	type Target = PtySession;

	fn deref(&self) -> &Self::Target {
		self.session
			.as_ref()
			.expect("supervisor should own its PTY session")
	}
}

impl DerefMut for SupervisedPty {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.session
			.as_mut()
			.expect("supervisor should own its PTY session")
	}
}

impl Drop for SupervisedPty {
	fn drop(&mut self) {
		if !self.cleaned {
			if let Err(error) = self.force_cleanup(CLEANUP_TIMEOUT) {
				eprintln!("shell E2E PTY cleanup failed: {error}");
			}
			self.drop_inner_without_panic();
		}
	}
}

fn retry_eintr<T>(
	deadline: Instant,
	mut operation: impl FnMut() -> nix::Result<T>,
) -> nix::Result<T> {
	loop {
		match operation() {
			Err(Errno::EINTR) if Instant::now() < deadline => {}
			Err(Errno::EINTR) => return Err(Errno::ETIMEDOUT),
			result => return result,
		}
	}
}

fn signal_running_subtree_with(
	mut signal_leader: impl FnMut() -> io::Result<()>,
	mut signal_group: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
	signal_leader()?;
	signal_group()
}

fn signal_anchored_subtree(pid: Pid, deadline: Instant) -> io::Result<()> {
	signal_running_subtree_with(
		|| send_leader_sigkill(pid, deadline),
		|| send_anchored_group_sigkill(pid, deadline),
	)
}

fn send_leader_sigkill(pid: Pid, deadline: Instant) -> io::Result<()> {
	match retry_eintr(deadline, || kill(pid, Signal::SIGKILL)) {
		Ok(()) | Err(Errno::ESRCH) => Ok(()),
		Err(error) => Err(io::Error::from(error)),
	}
}

fn send_group_sigkill(process_group: Pid, deadline: Instant) -> io::Result<()> {
	match retry_eintr(deadline, || killpg(process_group, Signal::SIGKILL)) {
		Ok(()) | Err(Errno::ESRCH) => Ok(()),
		Err(error) => Err(io::Error::from(error)),
	}
}

fn send_anchored_group_sigkill(process_group: Pid, deadline: Instant) -> io::Result<()> {
	match retry_eintr(deadline, || killpg(process_group, Signal::SIGKILL)) {
		Ok(()) | Err(Errno::ESRCH) => Ok(()),
		// Darwin reports EPERM when the anchored group contains only its zombie leader.
		#[cfg(target_os = "macos")]
		Err(Errno::EPERM) => Ok(()),
		Err(error) => Err(io::Error::from(error)),
	}
}

fn anchor_running_subtree(pid: Pid, deadline: Instant) -> io::Result<bool> {
	send_leader_sigkill(pid, deadline)?;
	match send_group_sigkill(pid, deadline) {
		Ok(()) => wait_for_leader_exit_anchored(pid, deadline),
		#[cfg(target_os = "macos")]
		Err(error) if error.raw_os_error() == Some(Errno::EPERM as i32) => {
			anchor_after_darwin_running_group_eperm(pid, deadline)
		}
		Err(error) => Err(error),
	}
}

#[cfg(target_os = "macos")]
fn anchor_after_darwin_running_group_eperm(pid: Pid, deadline: Instant) -> io::Result<bool> {
	DARWIN_RUNNING_EPERM_OBSERVED.store(true, Ordering::Relaxed);
	loop {
		if observe_leader_exit_once(pid, deadline)? {
			send_anchored_group_sigkill(pid, deadline)?;
			return Ok(true);
		}
		if Instant::now() >= deadline {
			return Ok(false);
		}
		send_leader_sigkill(pid, deadline)?;
		std::thread::sleep(Duration::from_millis(1));
	}
}

#[cfg(all(target_os = "linux", not(target_env = "uclibc")))]
fn observe_leader_exit_once(pid: Pid, deadline: Instant) -> io::Result<bool> {
	match retry_eintr(deadline, || {
		waitid(
			Id::Pid(pid),
			WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
		)
	}) {
		Ok(WaitStatus::Exited(_, _)) | Ok(WaitStatus::Signaled(_, _, _)) => Ok(true),
		Ok(WaitStatus::StillAlive) => Ok(false),
		Ok(_) => Ok(false),
		Err(error) => Err(io::Error::from(error)),
	}
}

#[cfg(target_os = "macos")]
fn observe_leader_exit_once(pid: Pid, deadline: Instant) -> io::Result<bool> {
	let mut info = {
		// SAFETY: Zeroed siginfo_t storage is valid for waitid to initialize.
		unsafe { std::mem::zeroed::<libc::siginfo_t>() }
	};
	retry_eintr(deadline, || {
		let result = {
			// SAFETY: info is valid writable storage, and scalar arguments follow waitid.
			unsafe {
				libc::waitid(
					libc::P_PID,
					pid.as_raw() as libc::id_t,
					&mut info,
					libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
				)
			}
		};
		Errno::result(result).map(drop)
	})
	.map_err(io::Error::from)?;
	let observed_pid = {
		// SAFETY: waitid succeeded, so info is initialized, including the WNOHANG case.
		unsafe { info.si_pid() }
	};
	Ok(observed_pid != 0)
}

fn wait_for_leader_exit_anchored(pid: Pid, deadline: Instant) -> io::Result<bool> {
	loop {
		if observe_leader_exit_once(pid, deadline)? {
			return Ok(true);
		}
		if Instant::now() >= deadline {
			return Ok(false);
		}
		std::thread::sleep(Duration::from_millis(10));
	}
}

fn reap_anchored_leader(pid: Pid, deadline: Instant) -> io::Result<Option<ExitStatus>> {
	loop {
		match retry_eintr(deadline, || waitpid(pid, Some(WaitPidFlag::WNOHANG))) {
			Ok(WaitStatus::Exited(_, code)) => {
				return Ok(Some(ExitStatus::from_raw(code << 8)));
			}
			Ok(WaitStatus::Signaled(_, signal, core_dumped)) => {
				let core_flag = if core_dumped { 0x80 } else { 0 };
				return Ok(Some(ExitStatus::from_raw(signal as i32 | core_flag)));
			}
			Ok(_) => {}
			Err(error) => return Err(io::Error::from(error)),
		}
		if Instant::now() >= deadline {
			return Ok(None);
		}
		std::thread::sleep(Duration::from_millis(10));
	}
}

fn wait_for_process_group_exit(process_group: Pid, deadline: Instant) -> io::Result<bool> {
	loop {
		match retry_eintr(deadline, || killpg(process_group, None)) {
			Err(Errno::ESRCH) => return Ok(true),
			Ok(()) | Err(Errno::EPERM) => {}
			Err(error) => return Err(io::Error::from(error)),
		}
		if Instant::now() >= deadline {
			return Ok(false);
		}
		std::thread::sleep(Duration::from_millis(10));
	}
}

struct ShellProject {
	_project_dir: TempDir,
	_target_dir: TempDir,
	_evcxr_dir: TempDir,
	project_root: PathBuf,
	manage_binary: PathBuf,
}

impl ShellProject {
	fn build() -> Self {
		let project_dir = TempDir::new().expect("shell fixture directory should be created");
		let target_dir = TempDir::new().expect("shell fixture target directory should be created");
		let evcxr_dir = TempDir::new().expect("shell fixture evcxr directory should be created");
		let project_root = project_dir.path().join("shell-e2e-project");
		write_project(&project_root);

		let mut lock_command = Command::new(env!("CARGO"));
		lock_command
			.args(["generate-lockfile", "--offline", "--manifest-path"])
			.arg(project_root.join("Cargo.toml"));
		let lock_output = supervised_output(lock_command, FIXTURE_BUILD_TIMEOUT)
			.expect("shell fixture lockfile should be generated");
		assert!(
			lock_output.status.success(),
			"fixture lockfile generation failed:\nstdout:\n{}\nstderr:\n{}",
			String::from_utf8_lossy(&lock_output.stdout),
			String::from_utf8_lossy(&lock_output.stderr)
		);
		let mut build_command = Command::new(env!("CARGO"));
		build_command
			.args([
				"build",
				"--locked",
				"--offline",
				"--features",
				"commands-shell",
				"--bin",
				"manage",
				"--manifest-path",
			])
			.arg(project_root.join("Cargo.toml"))
			.env("CARGO_BUILD_BUILD_DIR", "target")
			.env("CARGO_TARGET_DIR", target_dir.path());
		let output = supervised_output(build_command, FIXTURE_BUILD_TIMEOUT)
			.expect("shell fixture manage binary should build");
		assert!(
			output.status.success(),
			"fixture build failed:\nstdout:\n{}\nstderr:\n{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);

		let manage_binary = target_dir.path().join("debug").join(if cfg!(windows) {
			"manage.exe"
		} else {
			"manage"
		});
		assert!(
			manage_binary.is_file(),
			"fixture manage binary should exist at {}",
			manage_binary.display()
		);

		Self {
			_project_dir: project_dir,
			_target_dir: target_dir,
			_evcxr_dir: evcxr_dir,
			project_root,
			manage_binary,
		}
	}

	fn command(&self) -> Command {
		let mut command = Command::new(&self.manage_binary);
		command
			.current_dir(&self.project_root)
			.env("EVCXR_TMPDIR", self._evcxr_dir.path())
			.env("EVCXR_CACHE_ENABLED", "1");
		command
	}

	fn output(&self, source: &str) -> Output {
		let mut command = self.command();
		command.args(["shell", "-c", source]);
		supervised_output(command, COMMAND_TIMEOUT).expect("manage shell command should run")
	}

	fn pty_command(&self) -> Command {
		let mut command = Command::new(&self.manage_binary);
		command
			.arg("shell")
			.current_dir(&self.project_root)
			.env("EVCXR_TMPDIR", self._evcxr_dir.path())
			.env("EVCXR_CACHE_ENABLED", "1");
		command
	}
}

#[test]
fn repository_root_resolves_workspace_manifests() {
	// Arrange
	let repository_root = repository_root();

	// Act
	let shell_config = reinhardt::commands::ShellConfig::new(
		"reinhardt-web",
		"reinhardt",
		&repository_root,
		"crate::config::settings::get_settings",
		std::iter::empty::<String>(),
	);
	let workspace_manifest_exists = repository_root.join("Cargo.toml").is_file();
	let commands_manifest_exists = repository_root
		.join("crates/reinhardt-commands/Cargo.toml")
		.is_file();

	// Assert
	assert_eq!(shell_config.manifest_dir(), repository_root);
	assert_eq!(workspace_manifest_exists, true);
	assert_eq!(commands_manifest_exists, true);
}

#[test]
fn generated_manage_shell_covers_project_bindings_models_and_recovery() {
	// Arrange
	let project = ShellProject::build();

	// Act / Assert
	let bindings = project.output(
		r#"println!("settings={}", settings.core.debug);
println!("database={:?}", db.backend());
println!("di={}", di.get_singleton::<framework::db::orm::DatabaseConnection>().is_some());
println!("marker={}", project_marker);
println!("unique={}", std::any::type_name::<AlphaOnly>());
println!("qualified={}", std::any::type_name::<project_crate::apps::alpha::nested::Record>());"#,
	);
	assert_output_success(
		&bindings,
		&[
			"settings=false",
			"database=Sqlite",
			"di=true",
			"marker=shell-e2e",
			"unique=shell_e2e_project::apps::alpha::AlphaOnly",
			"qualified=shell_e2e_project::apps::alpha::nested::Record",
		],
	);
	assert_collision_warning(&String::from_utf8_lossy(&bindings.stderr));

	let secret = "credential_like_secret_9f37b214";
	let invalid_source = format!("let {secret} = ;");
	let invalid = project.output(&invalid_source);
	assert!(!invalid.status.success(), "invalid Rust must fail");
	let invalid_stdout = String::from_utf8_lossy(&invalid.stdout);
	let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
	assert!(
		invalid_stderr.contains("expected expression"),
		"invalid Rust should report a compiler diagnostic:\n{invalid_stderr}"
	);
	assert!(
		!invalid_stdout.contains(secret),
		"compiler diagnostics must not echo credential-like source in stdout:\n{invalid_stdout}"
	);
	assert!(
		!invalid_stderr.contains(secret),
		"compiler diagnostics must not echo credential-like source in stderr:\n{invalid_stderr}"
	);

	let ambiguous = project.output(r#"println!("{}", std::any::type_name::<Record>())"#);
	assert!(
		!ambiguous.status.success(),
		"a colliding short model name must not be imported"
	);
	{
		assert_interactive_recovery(&project);
		assert_interrupt_recovery(&project);
	}
}

#[test]
fn supervised_timeout_kills_the_entire_process_group() {
	// Arrange
	let sentinel_dir = TempDir::new().expect("sentinel directory should be created");
	let child_pid_path = sentinel_dir.path().join("timeout-child-pid");
	let mut command = Command::new("sh");
	command
		.arg("-c")
		.arg("sleep 120 & child=$!; printf '%s\\n' \"$child\" > \"$1\"; wait")
		.arg("shell-e2e-supervision")
		.arg(&child_pid_path);
	let supervisor = SupervisedGroup::spawn(command).expect("supervised command should start");
	let child_pid = read_sentinel_pid(&child_pid_path);

	// Act
	let error = supervisor
		.wait_with_output(Duration::from_millis(50))
		.expect_err("supervised command should time out");

	// Assert
	assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
	assert_process_exited(child_pid);
}

#[test]
fn running_subtree_termination_signals_the_leader_before_the_group() {
	// Arrange
	let calls = std::cell::RefCell::new(Vec::new());

	// Act
	signal_running_subtree_with(
		|| {
			calls.borrow_mut().push("leader");
			Ok(())
		},
		|| {
			calls.borrow_mut().push("group");
			Ok(())
		},
	)
	.expect("injected signal operations should succeed");

	// Assert
	assert_eq!(*calls.borrow(), ["leader", "group"]);
}

#[test]
fn signal_retry_retries_interrupted_operations() {
	// Arrange
	let mut attempts = 0;
	let deadline = Instant::now() + Duration::from_secs(1);

	// Act
	let result = retry_eintr(deadline, || {
		attempts += 1;
		if attempts < 3 {
			Err(Errno::EINTR)
		} else {
			Ok("delivered")
		}
	});

	// Assert
	assert_eq!(result.expect("third attempt should succeed"), "delivered");
	assert_eq!(attempts, 3);
}

#[test]
fn signal_retry_stops_at_its_deadline() {
	// Arrange
	let mut attempts = 0;

	// Act
	let result = retry_eintr(Instant::now(), || {
		attempts += 1;
		Err::<(), _>(Errno::EINTR)
	});

	// Assert
	assert_eq!(result, Err(Errno::ETIMEDOUT));
	assert_eq!(attempts, 1);
}

#[test]
fn read_sentinel_pid_waits_for_complete_publication() {
	// Arrange
	let sentinel_dir = TempDir::new().expect("sentinel directory should be created");
	let child_pid_path = sentinel_dir.path().join("delayed-child-pid");
	let producer_path = child_pid_path.clone();
	let (created_tx, created_rx) = mpsc::sync_channel(1);
	let producer = std::thread::spawn(move || {
		fs::File::create(&producer_path).expect("empty sentinel should be created");
		created_tx
			.send(())
			.expect("file creation should be reported");
		std::thread::sleep(Duration::from_millis(50));
		fs::write(&producer_path, b"42").expect("partial PID should be written");
		std::thread::sleep(Duration::from_millis(50));
		fs::write(&producer_path, b"4242\n").expect("complete PID should be published");
	});
	created_rx
		.recv()
		.expect("empty sentinel creation should be observed");

	// Act
	let child_pid = read_sentinel_pid(&child_pid_path);
	producer.join().expect("sentinel producer should finish");

	// Assert
	assert_eq!(child_pid, Pid::from_raw(4242));
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_running_group_eperm_cleanup_is_diagnostic_free() {
	const HELPER_ENV: &str = "REINHARDT_SHELL_E2E_DARWIN_EPERM_HELPER";
	if std::env::var_os(HELPER_ENV).is_some() {
		DARWIN_RUNNING_EPERM_OBSERVED.store(false, Ordering::Relaxed);
		let sentinel_dir = TempDir::new().expect("sentinel directory should be created");
		let child_pid_path = sentinel_dir.path().join("darwin-eperm-child-pid");
		let mut command = Command::new("sh");
		command
			.arg("-c")
			.arg("sleep 0.05 & child=$!; printf '%s\\n' \"$child\" > \"$1\"; exit 0")
			.arg("shell-e2e-supervision")
			.arg(&child_pid_path);
		let supervisor = SupervisedGroup::spawn(command).expect("supervised command should start");
		let child_pid = read_sentinel_pid(&child_pid_path);
		assert_process_exited(child_pid);
		assert!(
			wait_for_leader_exit_anchored(
				supervisor.process_group,
				Instant::now() + Duration::from_secs(5)
			)
			.expect("leader observation should succeed"),
			"leader should be retained as a zombie anchor"
		);
		drop(supervisor);
		assert!(
			DARWIN_RUNNING_EPERM_OBSERVED.load(Ordering::Relaxed),
			"cleanup should exercise Darwin's running-state group EPERM transition"
		);
		return;
	}

	// Arrange
	let mut command = Command::new(std::env::current_exe().expect("test binary should resolve"));
	command
		.args([
			"--exact",
			"shell_e2e::darwin_running_group_eperm_cleanup_is_diagnostic_free",
			"--nocapture",
		])
		.env(HELPER_ENV, "1");

	// Act
	let output = supervised_output(command, Duration::from_secs(15))
		.expect("Darwin cleanup helper should finish");

	// Assert
	assert!(
		output.status.success(),
		"cleanup helper should pass:\n{}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		!stderr.contains("cleanup failed"),
		"running-state EPERM cleanup should be diagnostic-free:\n{stderr}"
	);
}

#[test]
fn supervised_success_terminates_a_background_descendant_after_the_leader_exits() {
	// Arrange
	let sentinel_dir = TempDir::new().expect("sentinel directory should be created");
	let child_pid_path = sentinel_dir.path().join("success-child-pid");
	let completion_path = sentinel_dir.path().join("success-child-complete");
	let mut command = Command::new("sh");
	command
		.arg("-c")
		.arg(
			"(sleep 120; printf done > \"$1\") >/dev/null 2>&1 & \
			 child=$!; printf '%s\\n' \"$child\" > \"$2\"; exit 0",
		)
		.arg("shell-e2e-supervision")
		.arg(&completion_path)
		.arg(&child_pid_path);

	// Act
	let output = supervised_output(command, Duration::from_secs(5))
		.expect("supervised command should clean its background descendant");

	// Assert
	assert!(output.status.success(), "leader should exit successfully");
	assert!(
		!completion_path.is_file(),
		"residual background work should be terminated before success"
	);
	let child_pid = read_sentinel_pid(&child_pid_path);
	assert_process_exited(child_pid);
}

#[test]
fn supervised_drop_during_assertion_panic_kills_the_entire_process_group() {
	// Arrange
	let sentinel_dir = TempDir::new().expect("sentinel directory should be created");
	let child_pid_path = sentinel_dir.path().join("panic-child-pid");
	let mut command = Command::new("sh");
	command
		.arg("-c")
		.arg("sleep 120 & child=$!; printf '%s\\n' \"$child\" > \"$1\"; wait")
		.arg("shell-e2e-supervision")
		.arg(&child_pid_path);

	// Act
	let supervisor = SupervisedGroup::spawn(command).expect("supervised command should start");
	let child_pid = read_sentinel_pid(&child_pid_path);
	let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
		let _supervisor = supervisor;
		panic!("forced assertion panic");
	}));

	// Assert
	assert!(
		panic_result.is_err(),
		"the assertion panic should be observed"
	);
	assert_process_exited(child_pid);
}

#[test]
fn supervised_pty_arms_only_after_rexpect_creates_a_child_process_group() {
	// Arrange
	let mut command = Command::new("sh");
	command.args(["-c", "sleep 0.1"]);

	// Act
	let session = SupervisedPty::spawn(command).expect("PTY supervisor should arm safely");

	// Assert
	assert_eq!(session.process_group, Some(session.process.child_pid));
	assert_ne!(session.process_group, Some(getpgrp()));
	session.assert_success();
}

#[test]
fn supervised_pty_fast_leader_cleanup_kills_its_background_descendant() {
	// Arrange
	let sentinel_dir = TempDir::new().expect("sentinel directory should be created");
	let child_pid_path = sentinel_dir.path().join("pty-fast-leader-child-pid");
	let mut command = Command::new("sh");
	command
		.arg("-c")
		.arg(
			"sleep 120 >/dev/null 2>&1 & child=$!; \
			 printf '%s\\n' \"$child\" > \"$1\"; exit 0",
		)
		.arg("shell-e2e-supervision")
		.arg(&child_pid_path);

	// Act
	let child_pid = match SupervisedPty::spawn(command) {
		Ok(session) => {
			let child_pid = read_sentinel_pid(&child_pid_path);
			drop(session);
			child_pid
		}
		Err(_) => read_sentinel_pid(&child_pid_path),
	};

	// Assert
	assert_process_exited(child_pid);
}

fn assert_output_success(output: &Output, expected_lines: &[&str]) {
	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		output.status.success(),
		"shell source should succeed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
	);
	for expected in expected_lines {
		assert!(
			stdout.lines().any(|line| line.trim() == *expected),
			"expected `{expected}` in stdout:\n{stdout}"
		);
	}
}

fn assert_collision_warning(output: &str) {
	let first_path = "shell_e2e_project::apps::alpha::nested::Record";
	let second_path = "shell_e2e_project::apps::beta::nested::Record";
	let first = output
		.find(first_path)
		.unwrap_or_else(|| panic!("collision warning should contain {first_path}:\n{output}"));
	let second = output
		.find(second_path)
		.unwrap_or_else(|| panic!("collision warning should contain {second_path}:\n{output}"));
	assert!(
		first < second,
		"collision warning paths should be deterministic:\n{output}"
	);
}

fn assert_interactive_recovery(project: &ShellProject) {
	let mut session =
		SupervisedPty::spawn(project.pty_command()).expect("interactive shell should start");
	let first_path = "shell_e2e_project::apps::alpha::nested::Record";
	let second_path = "shell_e2e_project::apps::beta::nested::Record";
	session
		.exp_string(first_path)
		.expect("collision warning should list the first qualified path");
	session
		.exp_string(second_path)
		.expect("collision warning should list the second qualified path");
	session.exp_string(">>> ").expect("primary prompt");

	session.send_line("let answer = 42;").expect("define state");
	session.exp_string(">>> ").expect("prompt after definition");
	session.send_line("answer").expect("read state");
	session.exp_string("42").expect("state should persist");
	session.exp_string(">>> ").expect("prompt after state read");

	session
		.send_line("this is not valid Rust")
		.expect("send invalid expression");
	session
		.exp_string("expected `;`")
		.expect("invalid expression should report a diagnostic");
	session.exp_string(">>> ").expect("prompt after diagnostic");
	session.send_line("answer").expect("read retained state");
	session
		.exp_string("42")
		.expect("nonfatal error should preserve state");
	session
		.exp_string(">>> ")
		.expect("prompt after retained state");

	session
		.send_line(r#"panic!("reset")"#)
		.expect("trigger evaluator reset");
	session
		.exp_string(SHELL_RESET_WARNING)
		.expect("panic should reset the evaluator");
	session.exp_string(">>> ").expect("prompt after reset");
	session
		.send_line("answer")
		.expect("probe state discarded by reset");
	session
		.exp_string("cannot find value `answer`")
		.expect("reset must discard user state");
	session
		.exp_string(">>> ")
		.expect("prompt after missing state");
	session
		.send_line(r#"println!("{}", settings.core.debug)"#)
		.expect("probe reloaded settings");
	session.exp_string("false").expect("settings should reload");
	session.exp_string(">>> ").expect("prompt after settings");
	session
		.send_line(r#"println!("{:?}", db.backend())"#)
		.expect("probe reloaded database");
	session
		.exp_string("Sqlite")
		.expect("database should reload");
	session.exp_string(">>> ").expect("prompt after database");
	session
		.send_line(
			r#"println!("{}", di.get_singleton::<framework::db::orm::DatabaseConnection>().is_some())"#,
		)
		.expect("probe reloaded DI");
	session.exp_string("true").expect("DI should reload");
	session.exp_string(">>> ").expect("prompt after DI");

	session.send_control('d').expect("send EOF");
	session
		.exp_eof()
		.expect("interactive shell should exit at EOF");
	session.assert_success();
}

fn assert_interrupt_recovery(project: &ShellProject) {
	let mut session =
		SupervisedPty::spawn(project.pty_command()).expect("interrupt shell should start");
	session.exp_string(">>> ").expect("primary prompt");
	let started = project.project_root.join("interrupt-started");
	session
		.send_line(&format!(
			"std::fs::write({started:?}, b\"started\").unwrap(); \
			 tokio::time::sleep(std::time::Duration::from_secs(120)).await"
		))
		.expect("start long async evaluation");
	wait_for_file(&started);
	session.send_control('c').expect("interrupt evaluation");
	session
		.exp_string(SHELL_RESET_WARNING)
		.expect("interrupt should reset the evaluator");
	session
		.exp_string(">>> ")
		.expect("prompt after interrupt reset");
	session
		.send_line(r#"println!("{}", project_marker)"#)
		.expect("probe reloaded custom prelude");
	session
		.exp_string("shell-e2e")
		.expect("custom prelude should reload");
	session
		.exp_string(">>> ")
		.expect("prompt after custom prelude");
	session.send_control('d').expect("send EOF");
	session
		.exp_eof()
		.expect("interrupt shell should exit at EOF");
	session.assert_success();
}

fn wait_for_file(path: &Path) {
	wait_for_file_with_timeout(path, Duration::from_secs(60));
}

fn wait_for_file_with_timeout(path: &Path, timeout: Duration) {
	let deadline = std::time::Instant::now() + timeout;
	while !path.is_file() {
		assert!(
			std::time::Instant::now() < deadline,
			"evaluation start sentinel should appear at {}",
			path.display()
		);
		std::thread::sleep(Duration::from_millis(25));
	}
}

fn read_sentinel_pid(path: &Path) -> Pid {
	let deadline = Instant::now() + Duration::from_secs(60);
	loop {
		match fs::read_to_string(path) {
			Ok(contents) => {
				if let Some(raw_pid) = contents.strip_suffix('\n')
					&& let Ok(raw_pid) = raw_pid.parse::<i32>()
					&& raw_pid > 0
				{
					return Pid::from_raw(raw_pid);
				}
			}
			Err(error) if error.kind() == io::ErrorKind::NotFound => {}
			Err(error) => panic!("child PID sentinel should be readable: {error}"),
		}
		assert!(
			Instant::now() < deadline,
			"complete child PID sentinel should appear at {}",
			path.display()
		);
		std::thread::sleep(Duration::from_millis(10));
	}
}

fn assert_process_exited(pid: Pid) {
	let deadline = Instant::now() + Duration::from_secs(5);
	loop {
		match kill(pid, None) {
			Err(Errno::ESRCH) => return,
			Ok(()) | Err(Errno::EPERM) if Instant::now() < deadline => {
				std::thread::sleep(Duration::from_millis(25));
			}
			result => panic!("descendant process {pid} should be gone, got {result:?}"),
		}
	}
}

fn assert_pty_success(status: ExitStatus) {
	assert!(
		status.success(),
		"EOF should exit successfully, got {status:?}"
	);
}

fn repository_root() -> PathBuf {
	let integration_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let tests_dir = integration_manifest_dir
		.parent()
		.expect("integration crate should be located directly under the tests directory");
	let repository_root = tests_dir
		.parent()
		.expect("tests directory should be located directly under the repository root");
	repository_root
		.canonicalize()
		.expect("repository root should resolve to a canonical path")
}

fn write_project(project_root: &Path) {
	let repository_root = repository_root();
	fs::create_dir_all(project_root.join("src/bin")).expect("create binary directory");
	fs::create_dir_all(project_root.join("src/config")).expect("create config directory");
	fs::create_dir_all(project_root.join("src/apps/alpha"))
		.expect("create alpha application directory");
	fs::create_dir_all(project_root.join("src/apps/beta"))
		.expect("create beta application directory");
	fs::create_dir_all(project_root.join("settings")).expect("create settings directory");

	write(
		&project_root.join("Cargo.toml"),
		&format!(
			r#"[package]
name = "shell-e2e-project"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "manage"
path = "src/bin/manage.rs"

[dependencies]
reinhardt = {{ package = "reinhardt-web", path = "{}", default-features = false, features = ["minimal", "core", "conf", "database", "db-sqlite", "commands-shell", "di"] }}
ctor = "0.6"
serde = {{ version = "1", features = ["derive"] }}
tokio = {{ version = "1", features = ["full"] }}

[features]
default = []
commands-shell = ["reinhardt/commands-shell"]
"#,
			repository_root.display()
		),
	);
	write(
		&project_root.join("src/lib.rs"),
		r#"pub mod apps;
pub mod config;
"#,
	);
	write(
		&project_root.join("src/apps.rs"),
		r#"pub mod alpha;
pub mod beta;
"#,
	);
	write(
		&project_root.join("src/apps/alpha.rs"),
		r#"pub mod nested;

use reinhardt::prelude::*;

#[model(app_label = "alpha", table_name = "alpha_only")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AlphaOnly {
	#[field(primary_key = true)]
	pub id: i64,
}
"#,
	);
	write(
		&project_root.join("src/apps/alpha/nested.rs"),
		r#"use reinhardt::prelude::*;

#[model(app_label = "alpha", table_name = "alpha_record")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Record {
	#[field(primary_key = true)]
	pub id: i64,
}
"#,
	);
	write(
		&project_root.join("src/apps/beta.rs"),
		r#"pub mod nested;

use reinhardt::prelude::*;

#[model(app_label = "beta", table_name = "beta_only")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BetaOnly {
	#[field(primary_key = true)]
	pub id: i64,
}
"#,
	);
	write(
		&project_root.join("src/apps/beta/nested.rs"),
		r#"use reinhardt::prelude::*;

#[model(app_label = "beta", table_name = "beta_record")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Record {
	#[field(primary_key = true)]
	pub id: i64,
}
"#,
	);
	write(
		&project_root.join("src/config.rs"),
		r#"pub mod apps;
pub mod settings;
#[cfg(feature = "commands-shell")]
pub mod shell;
"#,
	);
	write(
		&project_root.join("src/config/apps.rs"),
		r#"use reinhardt::installed_apps;

installed_apps! {
	alpha: "alpha",
	beta: "beta",
}
"#,
	);
	write(
		&project_root.join("src/config/settings.rs"),
		r#"use reinhardt::conf::settings::builder::SettingsBuilder;
use reinhardt::conf::settings::profile::Profile;
use reinhardt::conf::settings::sources::{DefaultSource, TomlFileSource};
use reinhardt::settings;

#[settings(core: CoreSettings | contacts: ContactSettings)]
pub struct ProjectSettings;

pub fn get_settings() -> ProjectSettings {
	SettingsBuilder::new()
		.profile(Profile::parse("local"))
		.add_source(DefaultSource::new())
		.add_source(TomlFileSource::new(
			std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("settings/base.toml"),
		))
		.build_composed()
		.expect("shell fixture settings should build")
}
"#,
	);
	write(
		&project_root.join("src/config/shell.rs"),
		r##"use crate::config::apps::InstalledApp;
use crate::config::settings::ProjectSettings;
use reinhardt::commands::ShellConfig;

pub use reinhardt as framework;

pub type ShellSettings = ProjectSettings;
pub type ProjectShellEnvironment = framework::commands::ShellEnvironment<ShellSettings>;
pub type ShellDatabase = framework::db::orm::DatabaseConnection;
pub type ShellDi = std::sync::Arc<framework::di::InjectionContext>;

pub fn get_shell_config() -> ShellConfig {
	ShellConfig::new(
		env!("CARGO_PKG_NAME"),
		"shell_e2e_project",
		env!("CARGO_MANIFEST_DIR"),
		"shell_e2e_project::config::settings::get_settings",
		InstalledApp::all_labels().iter().copied(),
	)
	.with_prelude(r#"let project_marker = "shell-e2e";"#)
	.with_dependency_features(["commands-shell"])
}
"##,
	);
	write(
		&project_root.join("src/bin/manage.rs"),
		r#"use shell_e2e_project as _;

#[tokio::main]
async fn async_main() {
	let result = reinhardt::commands::execute_from_command_line_with_settings_and_shell(
		shell_e2e_project::config::settings::get_settings(),
		shell_e2e_project::config::shell::get_shell_config(),
	)
	.await;
	if let Err(error) = result {
		eprintln!("Error: {error}");
		std::process::exit(1);
	}
}

fn main() {
	reinhardt::commands::shell_runtime_hook();
	async_main();
}
"#,
	);
	write(
		&project_root.join("settings/base.toml"),
		r#"[core]
debug = false
secret_key = "shell-e2e-secret"
allowed_hosts = []
installed_apps = ["alpha", "beta"]
middleware = []
root_urlconf = ""

[core.databases.default]
engine = "sqlite"
name = ":memory:"

[contacts]
admins = []
managers = []
"#,
	);
}

fn write(path: &Path, contents: &str) {
	fs::write(path, contents).unwrap_or_else(|error| {
		panic!("failed to write {}: {error}", path.display());
	});
}
