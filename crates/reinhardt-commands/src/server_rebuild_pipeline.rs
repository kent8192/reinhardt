//! Hot-reload server rebuild pipeline.
//!
//! Runs `cargo build --bin <bin>` and, on success, swaps the currently-running
//! server child process for a freshly spawned one. Emits the structured
//! `[hot-reload] ...` log lines the watcher contract requires.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use tokio::process::Child;
use tokio::time::{sleep, timeout};

use crate::process::{ProcessRequest, ProcessRunner, SystemProcessRunner};

const READINESS_TIMEOUT: Duration = Duration::from_secs(5);
const READINESS_INTERVAL: Duration = Duration::from_millis(50);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(200);

/// Outcome of a single server rebuild attempt triggered by the hot-reload loop.
#[derive(Debug)]
pub enum ServerRebuildOutcome {
	/// Build succeeded and the child was respawned.
	Ok {
		/// Wall-clock time for the entire rebuild + restart.
		duration: Duration,
	},
	/// `cargo build` exited with a non-zero status.
	BuildFailed {
		/// Wall-clock time for the failed build.
		duration: Duration,
		/// Last lines of stderr from the failed build, joined by `\n`.
		// Field is read by the watcher when it forwards diagnostics to a UI
		// channel in a later task; suppress the dead-code warning until then.
		#[allow(dead_code)]
		stderr_tail: String,
	},
	/// Building or respawning the child process failed at the OS level.
	SpawnFailed {
		/// Wall-clock time before the failure surfaced.
		duration: Duration,
		/// Description of the spawn-side failure.
		message: String,
	},
}

/// Stateless pipeline runner. Held as a unit struct so callers have a
/// consistent type-based entry point (mirrors `WasmRebuildPipeline`).
pub struct ServerRebuildPipeline;

impl ServerRebuildPipeline {
	/// Run `cargo build --bin <bin_name>` and, on success, swap the child.
	///
	/// On `BuildFailed` we deliberately leave `current_child` running so the
	/// developer keeps a working server while the source has compile errors.
	pub async fn run(
		bin_name: &str,
		current_child: &mut Child,
		respawn: impl FnOnce() -> std::io::Result<Child>,
	) -> (ServerRebuildOutcome, Option<Child>) {
		Self::run_inner_with_runner(bin_name, current_child, respawn, None, &SystemProcessRunner)
			.await
	}

	/// Run `cargo build --bin <bin_name>`, swap the child, then wait until
	/// the advertised server address accepts TCP connections.
	///
	/// If the new child starts but never becomes reachable, the failure is
	/// reported as `SpawnFailed` while the child is still returned so the
	/// watcher can keep owning and eventually replace or kill it.
	pub async fn run_with_readiness(
		bin_name: &str,
		current_child: &mut Child,
		respawn: impl FnOnce() -> std::io::Result<Child>,
		address: &str,
	) -> (ServerRebuildOutcome, Option<Child>) {
		let readiness = ServerReadinessProbe::new(address);
		Self::run_inner_with_runner(
			bin_name,
			current_child,
			respawn,
			Some(readiness),
			&SystemProcessRunner,
		)
		.await
	}

	async fn run_inner_with_runner<R: ProcessRunner + Clone + 'static>(
		bin_name: &str,
		current_child: &mut Child,
		respawn: impl FnOnce() -> std::io::Result<Child>,
		readiness: Option<ServerReadinessProbe>,
		runner: &R,
	) -> (ServerRebuildOutcome, Option<Child>) {
		let start = Instant::now();

		// Phase 1: invoke `cargo build --bin <bin_name>`.
		let runner = runner.clone();
		let request = ProcessRequest::new("cargo").args(["build", "--bin", bin_name]);
		let output_result = match tokio::task::spawn_blocking(move || runner.run(&request)).await {
			Ok(result) => result,
			Err(error) => {
				let duration = start.elapsed();
				let message = if error.is_panic() {
					"cargo build runner panicked"
				} else {
					"cargo build runner task was cancelled"
				};
				let outcome = ServerRebuildOutcome::SpawnFailed {
					duration,
					message: message.to_string(),
				};
				eprintln!("{}", Self::format_log_line(&outcome));
				eprintln!("[hot-reload] watching for next change...");
				return (outcome, None);
			}
		};

		let output = match output_result {
			Ok(o) => o,
			Err(e) => {
				let duration = start.elapsed();
				let outcome = ServerRebuildOutcome::SpawnFailed {
					duration,
					message: format!("failed to invoke cargo build: {}", e),
				};
				eprintln!("{}", Self::format_log_line(&outcome));
				eprintln!("[hot-reload] watching for next change...");
				return (outcome, None);
			}
		};

		if !output.success {
			let duration = start.elapsed();
			let stderr = String::from_utf8_lossy(&output.stderr);
			let tail = Self::tail_lines(&stderr, 20);
			let outcome = ServerRebuildOutcome::BuildFailed {
				duration,
				stderr_tail: tail.clone(),
			};
			eprintln!("{}", Self::format_log_line(&outcome));
			if !tail.is_empty() {
				// Indent the stderr tail by two spaces, matching the spec.
				for line in tail.lines() {
					eprintln!("  {}", line);
				}
			}
			eprintln!("[hot-reload] watching for next change...");
			return (outcome, None);
		}

		// Phase 2: kill the old child, await its exit, then respawn.
		if let Err(e) = current_child.kill().await {
			let duration = start.elapsed();
			let outcome = ServerRebuildOutcome::SpawnFailed {
				duration,
				message: format!("failed to kill running server: {}", e),
			};
			eprintln!("{}", Self::format_log_line(&outcome));
			eprintln!("[hot-reload] watching for next change...");
			return (outcome, None);
		}
		// We do not care about the exit status; just ensure the process is reaped.
		let _ = current_child.wait().await;

		match respawn() {
			Ok(new_child) => {
				if let Some(readiness) = readiness
					&& let Err(e) = readiness.wait_until_ready().await
				{
					let duration = start.elapsed();
					let outcome = ServerRebuildOutcome::SpawnFailed {
						duration,
						message: format!("server did not become reachable: {}", e),
					};
					eprintln!("{}", Self::format_log_line(&outcome));
					eprintln!("[hot-reload] watching for next change...");
					return (outcome, Some(new_child));
				}

				let duration = start.elapsed();
				let outcome = ServerRebuildOutcome::Ok { duration };
				eprintln!("{}", Self::format_log_line(&outcome));
				(outcome, Some(new_child))
			}
			Err(e) => {
				let duration = start.elapsed();
				let outcome = ServerRebuildOutcome::SpawnFailed {
					duration,
					message: format!("failed to respawn server: {}", e),
				};
				eprintln!("{}", Self::format_log_line(&outcome));
				eprintln!("[hot-reload] watching for next change...");
				(outcome, None)
			}
		}
	}

	/// Format the single-line summary printed to stderr by the watcher.
	pub fn format_log_line(outcome: &ServerRebuildOutcome) -> String {
		match outcome {
			ServerRebuildOutcome::Ok { duration } => format!(
				"[hot-reload] Server rebuild + restart OK (took {})",
				format_duration(*duration)
			),
			ServerRebuildOutcome::BuildFailed { duration, .. } => format!(
				"[hot-reload] Server rebuild FAILED (took {}):",
				format_duration(*duration)
			),
			ServerRebuildOutcome::SpawnFailed { duration, message } => format!(
				"[hot-reload] Server respawn FAILED (took {}): {}",
				format_duration(*duration),
				message
			),
		}
	}

	/// Return the last `n` lines of `stderr` joined by `\n`.
	///
	/// When the input has fewer than `n` lines, all lines are returned.
	pub fn tail_lines(stderr: &str, n: usize) -> String {
		if n == 0 {
			return String::new();
		}
		let lines: Vec<&str> = stderr.split('\n').collect();
		let start = lines.len().saturating_sub(n);
		lines[start..].join("\n")
	}
}

struct ServerReadinessProbe {
	address: String,
	timeout: Duration,
	interval: Duration,
	connect_timeout: Duration,
}

impl ServerReadinessProbe {
	fn new(address: &str) -> Self {
		Self::with_timing(
			address,
			READINESS_TIMEOUT,
			READINESS_INTERVAL,
			CONNECT_TIMEOUT,
		)
	}

	fn with_timing(
		address: &str,
		timeout: Duration,
		interval: Duration,
		connect_timeout: Duration,
	) -> Self {
		Self {
			address: address.to_string(),
			timeout,
			interval,
			connect_timeout,
		}
	}

	async fn wait_until_ready(&self) -> std::io::Result<()> {
		let addrs = Self::probe_addrs(&self.address)?;
		let deadline = Instant::now() + self.timeout;
		let mut last_error = String::from("no connection attempt made");

		loop {
			for addr in &addrs {
				match timeout(self.connect_timeout, tokio::net::TcpStream::connect(addr)).await {
					Ok(Ok(_stream)) => return Ok(()),
					Ok(Err(e)) => {
						last_error = format!("connect to {} failed: {}", addr, e);
					}
					Err(_) => {
						last_error = format!("connect to {} timed out", addr);
					}
				}
			}

			if Instant::now() >= deadline {
				return Err(std::io::Error::new(
					std::io::ErrorKind::TimedOut,
					format!(
						"{} did not accept connections within {}; {}",
						self.address,
						format_duration(self.timeout),
						last_error
					),
				));
			}

			sleep(self.interval).await;
		}
	}

	fn probe_addrs(address: &str) -> std::io::Result<Vec<SocketAddr>> {
		let addrs: Vec<SocketAddr> = address
			.to_socket_addrs()?
			.map(|addr| {
				if addr.ip().is_unspecified() {
					match addr {
						SocketAddr::V4(addr) => {
							SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port())
						}
						SocketAddr::V6(addr) => {
							SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), addr.port())
						}
					}
				} else {
					addr
				}
			})
			.collect();

		if addrs.is_empty() {
			return Err(std::io::Error::new(
				std::io::ErrorKind::InvalidInput,
				format!("server address {address:?} did not resolve to any socket addresses"),
			));
		}

		Ok(addrs)
	}
}

/// Format a `Duration` as `"{:.1}s"` seconds.
fn format_duration(d: Duration) -> String {
	format!("{:.1}s", d.as_secs_f32())
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::{AtomicBool, Ordering};
	use std::sync::{Arc, Mutex, mpsc};

	use crate::process::{FakeProcessRunner, ProcessOutcome, ProcessRequest, ProcessRunner};

	use super::*;

	fn spawn_long_running_test_child() -> Child {
		tokio::process::Command::new("sh")
			.args(["-c", "exec sleep 60"])
			.kill_on_drop(true)
			.spawn()
			.expect("spawn long-running test child")
	}

	fn unavailable_loopback_addr() -> String {
		let listener =
			std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
		let address = listener
			.local_addr()
			.expect("read local address")
			.to_string();
		drop(listener);
		address
	}

	#[derive(Clone)]
	struct BlockingProcessRunner {
		started: mpsc::Sender<()>,
		release: Arc<Mutex<mpsc::Receiver<()>>>,
	}

	impl ProcessRunner for BlockingProcessRunner {
		fn run(&self, _request: &ProcessRequest) -> std::io::Result<ProcessOutcome> {
			self.started.send(()).expect("signal blocking runner start");
			self.release
				.lock()
				.expect("blocking runner release lock is poisoned")
				.recv()
				.expect("release blocking runner");
			Ok(ProcessOutcome::success(Vec::new()))
		}
	}

	#[tokio::test(flavor = "current_thread")]
	async fn cargo_build_yields_to_sibling_before_blocking_runner_is_released() {
		// Arrange
		let (started_sender, started_receiver) = mpsc::channel();
		let (release_sender, release_receiver) = mpsc::channel();
		let runner = BlockingProcessRunner {
			started: started_sender,
			release: Arc::new(Mutex::new(release_receiver)),
		};
		let released = Arc::new(AtomicBool::new(false));
		let sibling_progressed = Arc::new(AtomicBool::new(false));
		let released_for_watchdog = Arc::clone(&released);
		let release_thread = std::thread::spawn(move || {
			started_receiver
				.recv_timeout(Duration::from_secs(1))
				.expect("blocking runner must start within one second");
			std::thread::sleep(Duration::from_millis(50));
			released_for_watchdog.store(true, Ordering::SeqCst);
			release_sender
				.send(())
				.expect("release blocking runner after bounded delay");
		});
		let mut child = spawn_long_running_test_child();
		let sibling_progressed_for_task = Arc::clone(&sibling_progressed);
		let released_for_task = Arc::clone(&released);

		// Act
		let ((outcome, replacement), ()) = tokio::join!(
			ServerRebuildPipeline::run_inner_with_runner(
				"manage",
				&mut child,
				|| Err(std::io::Error::other("replacement not needed")),
				None,
				&runner,
			),
			async move {
				if !released_for_task.load(Ordering::SeqCst) {
					sibling_progressed_for_task.store(true, Ordering::SeqCst);
				}
			}
		);
		release_thread.join().expect("join release watchdog");

		// Assert
		assert!(
			sibling_progressed.load(Ordering::SeqCst),
			"the sibling future must progress before the blocking runner is released"
		);
		assert!(matches!(
			outcome,
			ServerRebuildOutcome::SpawnFailed { ref message, .. }
				if message == "failed to respawn server: replacement not needed"
		));
		assert!(replacement.is_none());
		assert!(child.try_wait().expect("read child state").is_some());
	}

	#[tokio::test]
	async fn build_failure_keeps_current_child_and_returns_stderr_tail() {
		// Arrange
		let runner = FakeProcessRunner::new([Ok(ProcessOutcome::failure(
			"exit status: 101",
			(0..25)
				.map(|line| format!("error-{line}"))
				.collect::<Vec<_>>()
				.join("\n")
				.into_bytes(),
		))]);
		let mut child = spawn_long_running_test_child();

		// Act
		let (outcome, replacement) = ServerRebuildPipeline::run_inner_with_runner(
			"manage",
			&mut child,
			|| panic!("respawn must not run after build failure"),
			None,
			&runner,
		)
		.await;

		// Assert
		assert!(matches!(outcome, ServerRebuildOutcome::BuildFailed { .. }));
		assert!(replacement.is_none());
		assert!(child.try_wait().expect("read child state").is_none());
		let ServerRebuildOutcome::BuildFailed { stderr_tail, .. } = outcome else {
			unreachable!("outcome was asserted as a build failure")
		};
		assert_eq!(
			stderr_tail,
			(5..25)
				.map(|line| format!("error-{line}"))
				.collect::<Vec<_>>()
				.join("\n")
		);

		// Cleanup
		child.kill().await.expect("kill retained child");
		child.wait().await.expect("reap retained child");
	}

	#[tokio::test]
	async fn build_spawn_failure_keeps_current_child_and_skips_respawn() {
		// Arrange
		let runner = FakeProcessRunner::new([Err(std::io::Error::other("cargo unavailable"))]);
		let mut child = spawn_long_running_test_child();

		// Act
		let (outcome, replacement) = ServerRebuildPipeline::run_inner_with_runner(
			"manage",
			&mut child,
			|| panic!("respawn must not run after cargo spawn failure"),
			None,
			&runner,
		)
		.await;

		// Assert
		assert!(matches!(
			outcome,
			ServerRebuildOutcome::SpawnFailed { ref message, .. }
				if message == "failed to invoke cargo build: cargo unavailable"
		));
		assert!(replacement.is_none());
		assert!(child.try_wait().expect("read child state").is_none());

		// Cleanup
		child.kill().await.expect("kill retained child");
		child.wait().await.expect("reap retained child");
	}

	#[tokio::test]
	async fn successful_build_respawn_failure_reaps_current_child() {
		// Arrange
		let runner = FakeProcessRunner::new([Ok(ProcessOutcome::success(Vec::new()))]);
		let mut child = spawn_long_running_test_child();

		// Act
		let (outcome, replacement) = ServerRebuildPipeline::run_inner_with_runner(
			"manage",
			&mut child,
			|| Err(std::io::Error::other("new server unavailable")),
			None,
			&runner,
		)
		.await;

		// Assert
		assert!(matches!(
			outcome,
			ServerRebuildOutcome::SpawnFailed { ref message, .. }
				if message == "failed to respawn server: new server unavailable"
		));
		assert!(replacement.is_none());
		assert!(child.try_wait().expect("read child state").is_some());
	}

	#[tokio::test]
	async fn successful_build_readiness_timeout_returns_replacement_child() {
		// Arrange
		let runner = FakeProcessRunner::new([Ok(ProcessOutcome::success(Vec::new()))]);
		let mut child = spawn_long_running_test_child();
		let address = unavailable_loopback_addr();
		let readiness = ServerReadinessProbe::with_timing(
			&address,
			Duration::from_millis(20),
			Duration::from_millis(1),
			Duration::from_millis(2),
		);

		// Act
		let (outcome, replacement) = ServerRebuildPipeline::run_inner_with_runner(
			"manage",
			&mut child,
			|| Ok(spawn_long_running_test_child()),
			Some(readiness),
			&runner,
		)
		.await;

		// Assert
		assert!(matches!(
			outcome,
			ServerRebuildOutcome::SpawnFailed { ref message, .. }
				if message.starts_with("server did not become reachable:")
		));
		assert!(child.try_wait().expect("read child state").is_some());
		let mut replacement = replacement.expect("readiness failure retains replacement ownership");
		assert!(
			replacement
				.try_wait()
				.expect("read replacement state")
				.is_none()
		);

		// Cleanup
		replacement.kill().await.expect("kill replacement child");
		replacement.wait().await.expect("reap replacement child");
	}

	#[test]
	fn format_log_line_ok_includes_restart_and_duration() {
		// Arrange
		let outcome = ServerRebuildOutcome::Ok {
			duration: Duration::from_millis(2500),
		};

		// Act
		let line = ServerRebuildPipeline::format_log_line(&outcome);

		// Assert
		assert_eq!(line, "[hot-reload] Server rebuild + restart OK (took 2.5s)");
	}

	#[test]
	fn format_log_line_build_failed_starts_with_failed_prefix() {
		// Arrange
		let outcome = ServerRebuildOutcome::BuildFailed {
			duration: Duration::from_millis(800),
			stderr_tail: "error[E0308]: mismatched types".to_string(),
		};

		// Act
		let line = ServerRebuildPipeline::format_log_line(&outcome);

		// Assert
		assert_eq!(
			line, "[hot-reload] Server rebuild FAILED (took 0.8s):",
			"unexpected line: {line:?}"
		);
	}

	#[test]
	fn tail_lines_returns_last_n_lines() {
		// Arrange
		let stderr = "line1\nline2\nline3\nline4\nline5";

		// Act
		let tail = ServerRebuildPipeline::tail_lines(stderr, 3);

		// Assert
		assert_eq!(tail, "line3\nline4\nline5");
	}

	#[test]
	fn tail_lines_returns_all_when_fewer_than_n() {
		// Arrange
		let stderr = "only-line-1\nonly-line-2";

		// Act
		let tail = ServerRebuildPipeline::tail_lines(stderr, 20);

		// Assert
		assert_eq!(tail, "only-line-1\nonly-line-2");
	}

	#[test]
	fn readiness_probe_rewrites_unspecified_ipv4_to_loopback() {
		// Act
		let addrs = ServerReadinessProbe::probe_addrs("0.0.0.0:8000").unwrap();

		// Assert
		assert_eq!(addrs, vec!["127.0.0.1:8000".parse().unwrap()]);
	}

	#[test]
	fn readiness_probe_rewrites_unspecified_ipv6_to_loopback() {
		// Act
		let addrs = ServerReadinessProbe::probe_addrs("[::]:8000").unwrap();

		// Assert
		assert_eq!(addrs, vec!["[::1]:8000".parse().unwrap()]);
	}
}
