use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use evcxr::{CommandContext, Error, EvalContext, EvalContextOutputs};
use tokio::sync::oneshot;
use toml_edit::InlineTable;

use super::config::ValidatedShellConfig;
use super::imports::ImportPlan;
use super::session::{EvaluationInterrupt, EvaluatorClient, EvaluatorFactory};
use crate::{CommandError, CommandResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EvaluationFailure {
	Compilation(String),
	Runtime(String),
	Panic(String),
	ProcessExited(String),
	ContextReset(String),
	Output {
		failure: Box<EvaluationFailure>,
		output: EvaluationOutput,
	},
	// Task 5 constructs this variant when its cancellation branch wins.
	#[allow(dead_code)]
	Interrupted,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EvaluationOutput {
	pub(crate) stdout: String,
	pub(crate) stderr: String,
	pub(crate) value: Option<String>,
}

impl EvaluationFailure {
	fn with_output(self, output: EvaluationOutput) -> Self {
		if output.stdout.is_empty() && output.stderr.is_empty() {
			self
		} else {
			Self::Output {
				failure: Box::new(self),
				output,
			}
		}
	}

	pub(crate) fn output(&self) -> Option<&EvaluationOutput> {
		match self {
			Self::Output { output, .. } => Some(output),
			_ => None,
		}
	}
}

pub(crate) trait BlockingShellEvaluator: Send {
	fn evaluate(&mut self, source: &str) -> Result<EvaluationOutput, EvaluationFailure>;
	fn interrupt_handle(&self) -> EvaluationInterrupt;
}

#[derive(Clone, Default)]
pub(crate) struct StartupInterrupt {
	requested: Arc<AtomicBool>,
	handle: Arc<Mutex<Option<EvaluationInterrupt>>>,
}

impl StartupInterrupt {
	pub(crate) fn interrupt(&self) -> Result<(), EvaluationFailure> {
		self.requested.store(true, Ordering::Release);
		let handle = self
			.handle
			.lock()
			.map_err(|_| {
				EvaluationFailure::ProcessExited("startup interrupt lock is poisoned".to_string())
			})?
			.clone();
		if let Some(handle) = handle {
			handle.interrupt()?;
		}
		Ok(())
	}

	fn register(&self, handle: EvaluationInterrupt) -> Result<(), EvaluationFailure> {
		let mut registered = self.handle.lock().map_err(|_| {
			EvaluationFailure::ProcessExited("startup interrupt lock is poisoned".to_string())
		})?;
		*registered = Some(handle.clone());
		if self.requested.load(Ordering::Acquire) {
			drop(registered);
			handle.interrupt()?;
		}
		Ok(())
	}

	fn clear(&self) {
		if let Ok(mut handle) = self.handle.lock() {
			*handle = None;
		}
	}
}

// Task 5 constructs this adapter inside its blocking evaluator worker.
#[allow(dead_code)]
pub(crate) struct EvcxrEvaluator {
	// Drops before output drainers so Windows closes the Job Object and all
	// inherited output-pipe handles before reader threads are joined.
	process_tree_guard: EvaluatorProcessTreeGuard,
	context: CommandContext,
	output_drainers: OutputDrainers,
	process_handle: Arc<Mutex<Child>>,
	owns_process_group: bool,
	evaluator_id: u64,
	evaluation_sequence: u64,
}

impl EvcxrEvaluator {
	// Task 5 calls this constructor after validating the shell configuration.
	#[allow(dead_code)]
	pub(crate) fn bootstrap(
		config: &ValidatedShellConfig,
		startup_interrupt: &StartupInterrupt,
	) -> Result<(Self, Vec<String>), EvaluationFailure> {
		let mut command = Command::new(
			std::env::current_exe()
				.map_err(|error| EvaluationFailure::ProcessExited(error.to_string()))?,
		);
		configure_evaluator_build_dir(&mut command);
		configure_evaluator_process_group(&mut command)?;
		let (eval, outputs) =
			EvalContext::with_subprocess_command(command).map_err(classify_startup_error)?;
		let process_handle = eval.process_handle();
		let owns_process_group = cfg!(unix);
		startup_interrupt.register(EvaluationInterrupt::new(move || {
			let mut process = process_handle.lock().map_err(|_| {
				EvaluationFailure::ProcessExited("evaluator process lock is poisoned".to_string())
			})?;
			terminate_evaluator_process(&mut process, owns_process_group)
		}))?;
		Self::bootstrap_with_context_and_process_group(config, eval, outputs, cfg!(unix))
	}

	fn bootstrap_with_context(
		config: &ValidatedShellConfig,
		eval: EvalContext,
		outputs: EvalContextOutputs,
	) -> Result<(Self, Vec<String>), EvaluationFailure> {
		Self::bootstrap_with_context_and_process_group(config, eval, outputs, false)
	}

	fn bootstrap_with_context_and_process_group(
		config: &ValidatedShellConfig,
		mut eval: EvalContext,
		outputs: EvalContextOutputs,
		owns_process_group: bool,
	) -> Result<(Self, Vec<String>), EvaluationFailure> {
		let mut process_guard =
			EvaluatorProcessGuard::new(eval.process_handle(), owns_process_group)?;
		let mut state = eval.state();
		let dependency = path_dependency(config)?;
		let cargo_dependency_name = config
			.crate_name()
			.strip_prefix("r#")
			.unwrap_or(config.crate_name());
		state
			.add_dep(cargo_dependency_name, &dependency)
			.map_err(classify_startup_error)?;
		state
			.add_dep(
				"tokio",
				r#"{ version = "1", features = ["rt", "rt-multi-thread", "time"] }"#,
			)
			.map_err(classify_startup_error)?;
		state.set_preserve_vars_on_panic(false);
		eval.eval_with_state("", state)
			.map_err(classify_startup_error)?;

		let prelude = bootstrap_prelude(config);
		let (aliases, project_prelude) = prelude
			.split_first()
			.expect("shell bootstrap always provides framework aliases");
		let import_plan = ImportPlan::from_registry(config.installed_app_labels());
		let mut warnings = import_plan.warnings().to_vec();
		let context = CommandContext::with_eval_context(eval);
		let process_handle = context.process_handle();
		let output_drainers = OutputDrainers::new(outputs);
		let process_tree_guard = process_guard.take_process_tree_guard();
		let mut evaluator = Self {
			process_tree_guard,
			context,
			output_drainers,
			process_handle,
			owns_process_group,
			evaluator_id: NEXT_EVALUATOR_ID.fetch_add(1, Ordering::Relaxed),
			evaluation_sequence: 0,
		};
		process_guard.disarm();
		let output = evaluator
			.evaluate(aliases)
			.map_err(|error| startup_prelude_error(error, &warnings))?;
		append_startup_output(&mut warnings, output);
		for import in import_plan.imports() {
			match evaluator.evaluate(import) {
				Ok(output) => append_startup_output(&mut warnings, output),
				Err(EvaluationFailure::Compilation(error)) => warnings.push(format!(
					"Model import is unavailable to the evaluator and was skipped: {import}: {error}"
				)),
				Err(error) => return Err(startup_prelude_error(error, &warnings)),
			}
		}
		for statement in project_prelude {
			let output = evaluator
				.evaluate(&statement)
				.map_err(|error| startup_prelude_error(error, &warnings))?;
			append_startup_output(&mut warnings, output);
		}
		let mut startup_output = warnings
			.iter()
			.filter(|entry| entry.starts_with('\u{1}'))
			.cloned()
			.collect::<Vec<_>>();
		warnings.retain(|entry| !entry.starts_with('\u{1}'));
		warnings.sort();
		warnings.append(&mut startup_output);
		Ok((evaluator, warnings))
	}
}

struct EvaluatorProcessGuard {
	process_handle: Arc<Mutex<Child>>,
	process_tree_guard: Option<EvaluatorProcessTreeGuard>,
	owns_process_group: bool,
	armed: bool,
}

impl EvaluatorProcessGuard {
	fn new(
		process_handle: Arc<Mutex<Child>>,
		owns_process_group: bool,
	) -> Result<Self, EvaluationFailure> {
		let process_tree_guard = {
			let process = process_handle.lock().map_err(|_| {
				EvaluationFailure::ProcessExited("evaluator process lock is poisoned".to_string())
			})?;
			EvaluatorProcessTreeGuard::new(&process)?
		};
		Ok(Self {
			process_handle,
			process_tree_guard: Some(process_tree_guard),
			owns_process_group,
			armed: true,
		})
	}

	fn disarm(&mut self) {
		self.armed = false;
	}

	fn take_process_tree_guard(&mut self) -> EvaluatorProcessTreeGuard {
		self.process_tree_guard
			.take()
			.expect("process tree guard is available until evaluator construction completes")
	}
}

#[cfg(windows)]
struct EvaluatorProcessTreeGuard {
	job: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(not(windows))]
struct EvaluatorProcessTreeGuard;

impl EvaluatorProcessTreeGuard {
	#[cfg(windows)]
	fn new(process: &Child) -> Result<Self, EvaluationFailure> {
		use std::os::windows::io::AsRawHandle;
		use windows_sys::Win32::Foundation::CloseHandle;
		use windows_sys::Win32::System::JobObjects::{
			AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
			JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
			SetInformationJobObject,
		};

		// A Job Object owns every evaluator descendant, including children that
		// inherit stdout/stderr, and therefore prevents output-drainer deadlocks.
		let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
		if job.is_null() {
			return Err(EvaluationFailure::ProcessExited(
				std::io::Error::last_os_error().to_string(),
			));
		}
		let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
		limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
		let configured = unsafe {
			SetInformationJobObject(
				job,
				JobObjectExtendedLimitInformation,
				&mut limits as *mut _ as *mut _,
				std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
			)
		};
		let assigned = configured != 0
			&& unsafe { AssignProcessToJobObject(job, process.as_raw_handle()) } != 0;
		if !assigned {
			unsafe { CloseHandle(job) };
			return Err(EvaluationFailure::ProcessExited(
				std::io::Error::last_os_error().to_string(),
			));
		}
		Ok(Self { job })
	}

	#[cfg(not(windows))]
	fn new(_process: &Child) -> Result<Self, EvaluationFailure> {
		Ok(Self)
	}
}

#[cfg(windows)]
impl Drop for EvaluatorProcessTreeGuard {
	fn drop(&mut self) {
		if !self.job.is_null() {
			unsafe { windows_sys::Win32::Foundation::CloseHandle(self.job) };
			self.job = std::ptr::null_mut();
		}
	}
}

impl Drop for EvaluatorProcessGuard {
	fn drop(&mut self) {
		if self.armed
			&& let Ok(mut process) = self.process_handle.lock()
		{
			let _ = terminate_evaluator_process(&mut process, self.owns_process_group);
			let _ = process.wait();
		}
	}
}

impl Drop for EvcxrEvaluator {
	fn drop(&mut self) {
		if let Ok(mut process) = self.process_handle.lock() {
			let _ = terminate_evaluator_process(&mut process, self.owns_process_group);
			let _ = process.wait();
		}
	}
}

impl BlockingShellEvaluator for EvcxrEvaluator {
	fn evaluate(&mut self, source: &str) -> Result<EvaluationOutput, EvaluationFailure> {
		self.evaluation_sequence += 1;
		let sentinel = format!(
			"__reinhardt_shell_evaluation_{}_{}",
			self.evaluator_id, self.evaluation_sequence
		);
		let marker = format!(
			"__REINHARDT_SHELL_OUTPUT_BOUNDARY_{}_{}_{:032x}__",
			self.evaluator_id,
			self.evaluation_sequence,
			rand::random::<u128>(),
		);
		let (pending_stdout, pending_stderr) = self.output_drainers.begin(&marker);
		let source = source_with_commit_sentinel(source, &sentinel);
		let result = self.context.execute(&source);
		let committed = result.is_ok()
			&& self
				.context
				.variables_and_types()
				.any(|(name, _)| name == sentinel);
		let boundary_result = if matches!(result, Err(Error::SubprocessTerminated(_))) {
			None
		} else {
			let sentinel_cleanup = if committed {
				format!("::std::mem::drop({sentinel});\n")
			} else {
				String::new()
			};
			Some(self.context.execute(&format!(
				"{{\n{sentinel_cleanup}\
				 ::std::println!(\"{marker}\");\n\
				 ::std::eprintln!(\"{marker}\");\n}}"
			)))
		};
		let (stdout, stderr) = match boundary_result {
			Some(Ok(_)) => self.output_drainers.finish(&marker)?,
			Some(Err(error)) => {
				let (stdout, stderr) = self.output_drainers.finish_after_disconnect();
				return Err(classify_boundary_error(error, &stderr).with_output(
					EvaluationOutput {
						stdout: format!("{pending_stdout}{stdout}"),
						stderr: format!("{pending_stderr}{stderr}"),
						value: None,
					},
				));
			}
			None => self.output_drainers.finish_after_disconnect(),
		};
		let output = EvaluationOutput {
			stdout: format!("{pending_stdout}{stdout}"),
			stderr: format!("{pending_stderr}{stderr}"),
			value: None,
		};

		match result {
			Ok(_) if !committed => Err(EvaluationFailure::Runtime(
				"evaluation failed before committing state".to_string(),
			)
			.with_output(output)),
			Ok(outputs) => Ok(EvaluationOutput {
				stdout: output.stdout,
				stderr: output.stderr,
				value: outputs.get("text/plain").map(str::to_owned),
			}),
			Err(Error::CompilationErrors(errors)) => Err(EvaluationFailure::Compilation(
				errors
					.iter()
					.map(|error| error.message())
					.collect::<Vec<_>>()
					.join("\n"),
			)
			.with_output(output)),
			Err(Error::SubprocessTerminated(message)) => {
				if is_panic_output(&output.stderr) {
					Err(EvaluationFailure::Panic(message).with_output(output))
				} else {
					Err(EvaluationFailure::ProcessExited(message).with_output(output))
				}
			}
			Err(Error::TypeRedefinedVariablesLost(variables)) => {
				Err(EvaluationFailure::ContextReset(format!(
					"evaluation changed stored variable types: {}",
					variables.join(", ")
				))
				.with_output(output))
			}
			Err(Error::Message(message)) => {
				Err(EvaluationFailure::Runtime(message).with_output(output))
			}
		}
	}

	fn interrupt_handle(&self) -> EvaluationInterrupt {
		let process_handle = Arc::clone(&self.process_handle);
		let owns_process_group = self.owns_process_group;
		EvaluationInterrupt::new(move || {
			let mut process = process_handle.lock().map_err(|_| {
				EvaluationFailure::ProcessExited("evaluator process lock is poisoned".to_string())
			})?;
			terminate_evaluator_process(&mut process, owns_process_group)
		})
	}
}

enum EvaluatorRequest {
	Evaluate {
		source: String,
		response: oneshot::Sender<Result<EvaluationOutput, EvaluationFailure>>,
	},
	Close,
}

pub(crate) struct EvaluatorWorker {
	requests: Option<mpsc::Sender<EvaluatorRequest>>,
	interrupt: EvaluationInterrupt,
	worker: Option<JoinHandle<()>>,
}

pub(crate) struct EvcxrEvaluatorFactory {
	config: ValidatedShellConfig,
	warnings: Vec<String>,
	startup_output: Vec<EvaluationOutput>,
	startup_interrupt: StartupInterrupt,
}

impl EvcxrEvaluatorFactory {
	pub(crate) fn new(config: ValidatedShellConfig) -> Self {
		Self {
			config,
			warnings: Vec::new(),
			startup_output: Vec::new(),
			startup_interrupt: StartupInterrupt::default(),
		}
	}

	pub(crate) fn startup_interrupt(&self) -> StartupInterrupt {
		self.startup_interrupt.clone()
	}
}

impl EvaluatorFactory for EvcxrEvaluatorFactory {
	fn start(&mut self) -> CommandResult<Box<dyn EvaluatorClient>> {
		let config = self.config.clone();
		let startup_interrupt = self.startup_interrupt.clone();
		let started = EvaluatorWorker::start_with(move || {
			let (evaluator, warnings) = EvcxrEvaluator::bootstrap(&config, &startup_interrupt)?;
			Ok((Box::new(evaluator), warnings))
		});
		let (evaluator, warnings) = match started {
			Ok(started) => started,
			Err(EvaluationFailure::Output { failure, output }) => {
				self.startup_output.push(output);
				return Err(evaluation_command_error(*failure));
			}
			Err(failure) => return Err(evaluation_command_error(failure)),
		};
		self.startup_interrupt.clear();
		let (warnings, startup_output) = split_startup_output(warnings);
		self.warnings = warnings;
		self.startup_output = startup_output;
		Ok(Box::new(evaluator))
	}

	fn startup_interrupt(&self) -> Option<StartupInterrupt> {
		Some(self.startup_interrupt())
	}

	fn take_warnings(&mut self) -> Vec<String> {
		std::mem::take(&mut self.warnings)
	}

	fn take_startup_output(&mut self) -> Vec<EvaluationOutput> {
		std::mem::take(&mut self.startup_output)
	}
}

impl EvaluatorWorker {
	pub(crate) fn start_with<F>(factory: F) -> Result<(Self, Vec<String>), EvaluationFailure>
	where
		F: FnOnce() -> Result<(Box<dyn BlockingShellEvaluator>, Vec<String>), EvaluationFailure>
			+ Send
			+ 'static,
	{
		let (requests, receiver) = mpsc::channel();
		let (startup, started) = mpsc::sync_channel(1);
		let worker = thread::spawn(move || {
			let (mut evaluator, warnings) = match factory() {
				Ok(started) => started,
				Err(error) => {
					let _ = startup.send(Err(error));
					return;
				}
			};
			let interrupt = evaluator.interrupt_handle();
			if startup.send(Ok((interrupt, warnings))).is_err() {
				return;
			}
			while let Ok(request) = receiver.recv() {
				match request {
					EvaluatorRequest::Evaluate { source, response } => {
						let result = evaluator.evaluate(&source);
						let _ = response.send(result);
					}
					EvaluatorRequest::Close => break,
				}
			}
		});
		match started.recv() {
			Ok(Ok((interrupt, warnings))) => Ok((
				Self {
					requests: Some(requests),
					interrupt,
					worker: Some(worker),
				},
				warnings,
			)),
			Ok(Err(error)) => {
				let _ = worker.join();
				Err(error)
			}
			Err(_) => {
				let _ = worker.join();
				Err(EvaluationFailure::ProcessExited(
					"evaluator worker exited during startup".to_string(),
				))
			}
		}
	}

	#[cfg(test)]
	pub(crate) fn spawn(evaluator: Box<dyn BlockingShellEvaluator>) -> Self {
		Self::start_with(move || Ok((evaluator, Vec::new())))
			.expect("test evaluator worker should start")
			.0
	}
}

#[async_trait::async_trait]
impl EvaluatorClient for EvaluatorWorker {
	async fn evaluate(&mut self, source: &str) -> Result<EvaluationOutput, EvaluationFailure> {
		let (response, result) = oneshot::channel();
		self.requests
			.as_ref()
			.ok_or_else(|| {
				EvaluationFailure::ProcessExited("evaluator worker is unavailable".to_string())
			})?
			.send(EvaluatorRequest::Evaluate {
				source: source.to_string(),
				response,
			})
			.map_err(|_| {
				EvaluationFailure::ProcessExited("evaluator worker has exited".to_string())
			})?;
		result.await.map_err(|_| {
			EvaluationFailure::ProcessExited(
				"evaluator worker dropped its response channel".to_string(),
			)
		})?
	}

	fn interrupt(&self) -> EvaluationInterrupt {
		self.interrupt.clone()
	}
}

impl Drop for EvaluatorWorker {
	fn drop(&mut self) {
		if let Some(requests) = self.requests.take() {
			let _ = requests.send(EvaluatorRequest::Close);
			let _ = self.interrupt.interrupt();
			drop(requests);
		}
		if let Some(worker) = self.worker.take() {
			let _ = worker.join();
		}
	}
}

fn evaluation_command_error(failure: EvaluationFailure) -> CommandError {
	let message = match failure {
		EvaluationFailure::Compilation(message)
		| EvaluationFailure::Runtime(message)
		| EvaluationFailure::Panic(message)
		| EvaluationFailure::ProcessExited(message)
		| EvaluationFailure::ContextReset(message) => message,
		EvaluationFailure::Output { failure, .. } => return evaluation_command_error(*failure),
		EvaluationFailure::Interrupted => "Evaluation was interrupted.".to_string(),
	};
	CommandError::ExecutionError(message)
}

static NEXT_EVALUATOR_ID: AtomicU64 = AtomicU64::new(1);
const OUTPUT_DISCONNECT_TIMEOUT: Duration = Duration::from_secs(2);

fn path_dependency(config: &ValidatedShellConfig) -> Result<String, EvaluationFailure> {
	let path = config.manifest_dir().to_str().ok_or_else(|| {
		EvaluationFailure::Runtime(
			"project manifest directory cannot be represented in Cargo TOML".to_string(),
		)
	})?;
	let mut dependency = InlineTable::new();
	dependency.insert("package", config.package_name().into());
	dependency.insert("path", path.into());
	let mut features = toml_edit::Array::new();
	for feature in config.dependency_features() {
		features.push(feature);
	}
	dependency.insert("features", features.into());
	if !config.default_features() {
		dependency.insert("default-features", false.into());
	}
	Ok(dependency.to_string())
}

fn bootstrap_prelude(config: &ValidatedShellConfig) -> Vec<String> {
	let crate_name = config.crate_name();
	let project_prelude = config.project_prelude();
	let settings_factory = config.settings_factory_path();

	vec![
		format!(
			"use {crate_name}::config::shell::framework;\n\
			 use {crate_name}::config::shell::framework::prelude::*;\n\
			 use {crate_name} as project_crate;"
		),
		format!(
			"let __reinhardt_typed_settings: \
			 project_crate::config::shell::ShellSettings = {settings_factory}();\n\
			 let __reinhardt_shell: \
			 project_crate::config::shell::ProjectShellEnvironment = \
			 project_crate::config::shell::ProjectShellEnvironment::bootstrap(\
			 __reinhardt_typed_settings).await?;\n\
			 let settings: project_crate::config::shell::ShellSettings = \
			 __reinhardt_shell.settings().clone();\n\
			 let db: project_crate::config::shell::ShellDatabase = \
			 __reinhardt_shell.database();\n\
			 let di: project_crate::config::shell::ShellDi = \
			 __reinhardt_shell.di();\n\
			 {project_prelude}"
		),
	]
}

#[cfg(unix)]
fn configure_evaluator_process_group(command: &mut Command) -> Result<(), EvaluationFailure> {
	use std::os::unix::process::CommandExt;

	// SAFETY: setpgid is async-signal-safe and the closure only creates a new process group
	// for the evaluator child before it executes the management binary.
	unsafe {
		command.pre_exec(|| {
			if nix::libc::setpgid(0, 0) == -1 {
				Err(std::io::Error::last_os_error())
			} else {
				Ok(())
			}
		});
	}
	Ok(())
}

#[cfg(not(unix))]
fn configure_evaluator_process_group(_command: &mut Command) -> Result<(), EvaluationFailure> {
	Ok(())
}

fn configure_evaluator_build_dir(command: &mut Command) {
	configure_evaluator_build_dir_from(command, std::env::var_os("CARGO_BUILD_BUILD_DIR"));
}

fn configure_evaluator_build_dir_from(
	command: &mut Command,
	build_dir: Option<std::ffi::OsString>,
) {
	if build_dir.is_none() {
		// Workaround for evcxr/evcxr#487 (tracked in reinhardt-web#5817).
		// Remove this workaround when evcxr supports Cargo's separate build.build-dir.
		//
		// Ideal implementation (without workaround):
		//   let (context, outputs) = evcxr::EvalContext::new()?;
		command.env("CARGO_BUILD_BUILD_DIR", "target");
	}
}

fn terminate_evaluator_process(
	process: &mut Child,
	owns_process_group: bool,
) -> Result<(), EvaluationFailure> {
	#[cfg(unix)]
	{
		use nix::sys::signal::{Signal, killpg};
		use nix::unistd::Pid;

		if owns_process_group {
			let process_group = Pid::from_raw(process.id() as i32);
			match killpg(process_group, Signal::SIGKILL) {
				Ok(()) => return Ok(()),
				Err(nix::errno::Errno::ESRCH) => return Ok(()),
				Err(_) => {}
			}
		}
	}
	#[cfg(not(unix))]
	let _ = owns_process_group;
	process
		.kill()
		.map_err(|error| EvaluationFailure::ProcessExited(error.to_string()))
}

fn source_with_commit_sentinel(source: &str, sentinel: &str) -> String {
	let mut prefix_end = 0;
	let mut found_inner_prefix = false;
	loop {
		let rest = &source[prefix_end..];
		let whitespace = rest.len() - rest.trim_start_matches([' ', '\t', '\r', '\n']).len();
		let candidate = &rest[whitespace..];
		let item_end = if candidate.starts_with("#![") {
			complete_inner_attribute(candidate)
		} else if candidate.starts_with("//!") {
			candidate
				.find('\n')
				.map(|end| end + 1)
				.or(Some(candidate.len()))
		} else if candidate.starts_with("/*!") {
			complete_inner_block_doc(candidate)
		} else if candidate.starts_with("//") || candidate.starts_with("/*") {
			ordinary_comment_before_inner_prefix(candidate)
		} else {
			None
		};
		let Some(item_end) = item_end else {
			if found_inner_prefix {
				prefix_end += whitespace;
			}
			break;
		};
		prefix_end += whitespace + item_end;
		found_inner_prefix = true;
	}
	format!(
		"{}let {sentinel}: ::std::string::String = ::std::string::String::new();\n{}",
		&source[..prefix_end],
		&source[prefix_end..]
	)
}

fn ordinary_comment_before_inner_prefix(source: &str) -> Option<usize> {
	let mut consumed = 0;
	loop {
		let remaining = &source[consumed..];
		let comment_end = if remaining.starts_with("//") {
			remaining
				.find('\n')
				.map(|end| end + 1)
				.unwrap_or(remaining.len())
		} else if remaining.starts_with("/*") {
			remaining.find("*/").map(|end| end + 2)?
		} else {
			return None;
		};
		consumed += comment_end;
		let following = &source[consumed..];
		let whitespace =
			following.len() - following.trim_start_matches([' ', '\t', '\r', '\n']).len();
		let next = &following[whitespace..];
		if next.starts_with("#![") || next.starts_with("//!") || next.starts_with("/*!") {
			return Some(consumed);
		}
		if !next.starts_with("//") && !next.starts_with("/*") {
			return None;
		}
		consumed += whitespace;
	}
}

fn append_startup_output(warnings: &mut Vec<String>, output: EvaluationOutput) {
	if !output.stdout.is_empty() {
		warnings.push(format!("\u{1}stdout:{}", output.stdout));
	}
	if !output.stderr.is_empty() {
		warnings.push(format!("\u{1}stderr:{}", output.stderr));
	}
}

fn split_startup_output(entries: Vec<String>) -> (Vec<String>, Vec<EvaluationOutput>) {
	let mut warnings = Vec::new();
	let mut output = Vec::new();
	for entry in entries {
		if let Some(stdout) = entry.strip_prefix("\u{1}stdout:") {
			output.push(EvaluationOutput {
				stdout: stdout.to_owned(),
				..Default::default()
			});
		} else if let Some(stderr) = entry.strip_prefix("\u{1}stderr:") {
			output.push(EvaluationOutput {
				stderr: stderr.to_owned(),
				..Default::default()
			});
		} else {
			warnings.push(entry);
		}
	}
	(warnings, output)
}

fn complete_inner_block_doc(source: &str) -> Option<usize> {
	let bytes = source.as_bytes();
	let mut depth = 1usize;
	let mut index = 3usize;
	while index + 1 < bytes.len() {
		match (bytes[index], bytes[index + 1]) {
			(b'/', b'*') => {
				depth += 1;
				index += 2;
			}
			(b'*', b'/') => {
				depth -= 1;
				index += 2;
				if depth == 0 {
					return Some(index);
				}
			}
			_ => index += 1,
		}
	}
	None
}

fn complete_inner_attribute(source: &str) -> Option<usize> {
	enum StringState {
		Quoted { escaped: bool },
		Raw { hashes: usize },
	}

	let bytes = source.as_bytes();
	let mut depth = 0usize;
	let mut index = 0usize;
	let mut string = None;
	while index < bytes.len() {
		if let Some(state) = &mut string {
			match state {
				StringState::Quoted { escaped } => {
					if *escaped {
						*escaped = false;
					} else if bytes[index] == b'\\' {
						*escaped = true;
					} else if bytes[index] == b'"' {
						string = None;
					}
					index += 1;
				}
				StringState::Raw { hashes } => {
					let closing = bytes[index] == b'"'
						&& bytes
							.get(index + 1..index + 1 + *hashes)
							.is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'));
					if closing {
						index += 1 + *hashes;
						string = None;
					} else {
						index += 1;
					}
				}
			}
			continue;
		}

		if matches!(bytes[index], b'r' | b'b') {
			let mut quote = index + usize::from(bytes[index] == b'b');
			if bytes.get(quote) == Some(&b'r') {
				quote += 1;
				let hashes = bytes[quote..]
					.iter()
					.take_while(|byte| **byte == b'#')
					.count();
				quote += hashes;
				if bytes.get(quote) == Some(&b'"') {
					string = Some(StringState::Raw { hashes });
					index = quote + 1;
					continue;
				}
			}
		}
		if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
			index = bytes[index..]
				.iter()
				.position(|byte| *byte == b'\n')
				.map_or(bytes.len(), |offset| index + offset + 1);
			continue;
		}
		if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
			let mut comment_depth = 1usize;
			index += 2;
			while index + 1 < bytes.len() && comment_depth > 0 {
				match (bytes[index], bytes[index + 1]) {
					(b'/', b'*') => {
						comment_depth += 1;
						index += 2;
					}
					(b'*', b'/') => {
						comment_depth -= 1;
						index += 2;
					}
					_ => index += 1,
				}
			}
			if comment_depth != 0 {
				return None;
			}
			continue;
		}
		if bytes[index] == b'\'' {
			let character_end = if bytes.get(index + 1) == Some(&b'\\') {
				bytes[index + 2..]
					.iter()
					.position(|byte| *byte == b'\'')
					.map(|offset| index + offset + 3)
			} else if bytes.get(index + 2) == Some(&b'\'') {
				Some(index + 3)
			} else {
				None
			};
			if let Some(end) = character_end {
				index = end;
				continue;
			}
		}

		match bytes[index] {
			b'"' => string = Some(StringState::Quoted { escaped: false }),
			b'[' => depth += 1,
			b']' => {
				depth = depth.checked_sub(1)?;
				if depth == 0 {
					return Some(index + 1);
				}
			}
			_ => {}
		}
		index += 1;
	}
	None
}

fn classify_startup_error(error: Error) -> EvaluationFailure {
	match error {
		Error::CompilationErrors(errors) => EvaluationFailure::Compilation(
			errors
				.iter()
				.map(|error| error.message())
				.collect::<Vec<_>>()
				.join("\n"),
		),
		Error::SubprocessTerminated(message) => EvaluationFailure::ProcessExited(message),
		Error::TypeRedefinedVariablesLost(variables) => EvaluationFailure::ContextReset(format!(
			"startup changed stored variable types: {}",
			variables.join(", ")
		)),
		Error::Message(message) => EvaluationFailure::Runtime(message),
	}
}

fn startup_prelude_error(error: EvaluationFailure, warnings: &[String]) -> EvaluationFailure {
	let warning_suffix = if warnings.is_empty() {
		String::new()
	} else {
		format!("; import warnings: {}", warnings.join("; "))
	};
	match error {
		EvaluationFailure::Compilation(message) => EvaluationFailure::Compilation(format!(
			"shell bootstrap failed{warning_suffix}: {message}"
		)),
		EvaluationFailure::Runtime(message) => {
			EvaluationFailure::Runtime(format!("shell bootstrap failed{warning_suffix}: {message}"))
		}
		EvaluationFailure::ContextReset(message) => EvaluationFailure::ContextReset(format!(
			"shell bootstrap failed{warning_suffix}: {message}"
		)),
		EvaluationFailure::Output { failure, output } => EvaluationFailure::Output {
			failure: Box::new(startup_prelude_error(*failure, warnings)),
			output,
		},
		other => other,
	}
}

fn is_panic_output(stderr: &str) -> bool {
	stderr.lines().any(|line| {
		line.starts_with("thread '") && line.contains(" panicked at ")
			|| line.contains("panic in a function that cannot unwind")
	})
}

fn nonempty_diagnostic(stderr: &str, fallback: &str) -> String {
	if stderr.trim().is_empty() {
		fallback.to_string()
	} else {
		stderr.trim_end().to_string()
	}
}

fn classify_boundary_error(error: Error, stderr: &str) -> EvaluationFailure {
	match classify_startup_error(error) {
		EvaluationFailure::ProcessExited(message) if is_panic_output(stderr) => {
			EvaluationFailure::Panic(message)
		}
		EvaluationFailure::ProcessExited(message) => EvaluationFailure::ProcessExited(message),
		other => other,
	}
}

struct OutputDrainers {
	stdout: Arc<StreamCapture>,
	stderr: Arc<StreamCapture>,
	running: Arc<AtomicBool>,
	stdout_worker: Option<JoinHandle<()>>,
	stderr_worker: Option<JoinHandle<()>>,
}

impl OutputDrainers {
	fn new(outputs: EvalContextOutputs) -> Self {
		let stdout = Arc::new(StreamCapture::default());
		let stderr = Arc::new(StreamCapture::default());
		let running = Arc::new(AtomicBool::new(true));

		let stdout_receiver = outputs.stdout;
		let stdout_buffer = Arc::clone(&stdout);
		let stdout_running = Arc::clone(&running);
		let stdout_worker = thread::spawn(move || {
			while stdout_running.load(Ordering::Acquire) {
				match stdout_receiver.recv_timeout(Duration::from_millis(50)) {
					Ok(line) => stdout_buffer.push(line),
					Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
					Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
				}
			}
			stdout_buffer.disconnect();
		});

		let stderr_receiver = outputs.stderr;
		let stderr_buffer = Arc::clone(&stderr);
		let stderr_running = Arc::clone(&running);
		let stderr_worker = thread::spawn(move || {
			while stderr_running.load(Ordering::Acquire) {
				match stderr_receiver.recv_timeout(Duration::from_millis(50)) {
					Ok(line) => stderr_buffer.push(line),
					Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
					Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
				}
			}
			stderr_buffer.disconnect();
		});

		Self {
			stdout,
			stderr,
			running,
			stdout_worker: Some(stdout_worker),
			stderr_worker: Some(stderr_worker),
		}
	}

	fn begin(&self, marker: &str) -> (String, String) {
		(self.stdout.begin(marker), self.stderr.begin(marker))
	}

	fn finish(&self, marker: &str) -> Result<(String, String), EvaluationFailure> {
		let stdout = match self.stdout.take_at_boundary(marker) {
			Ok(stdout) => stdout,
			Err(error) => {
				return Err(error.with_output(EvaluationOutput {
					stdout: self.stdout.take_pending(),
					stderr: self.stderr.take_pending(),
					value: None,
				}));
			}
		};
		let stderr = match self.stderr.take_at_boundary(marker) {
			Ok(stderr) => stderr,
			Err(error) => {
				return Err(error.with_output(EvaluationOutput {
					stdout,
					stderr: self.stderr.take_pending(),
					value: None,
				}));
			}
		};
		Ok((stdout, stderr))
	}

	fn finish_after_disconnect(&self) -> (String, String) {
		let deadline = Instant::now() + OUTPUT_DISCONNECT_TIMEOUT;
		let stdout = self.stdout.take_after_disconnect(deadline);
		let stderr = self.stderr.take_after_disconnect(deadline);
		(stdout, stderr)
	}
}

impl Drop for OutputDrainers {
	fn drop(&mut self) {
		// An evaluated program can deliberately detach from the evaluator's
		// process group while retaining an inherited capture pipe. Stop reader
		// threads independently so shutdown never waits for that foreign child.
		self.running.store(false, Ordering::Release);
		if let Some(worker) = self.stdout_worker.take() {
			let _ = worker.join();
		}
		if let Some(worker) = self.stderr_worker.take() {
			let _ = worker.join();
		}
	}
}

#[derive(Default)]
struct StreamCapture {
	state: Mutex<StreamCaptureState>,
	changed: Condvar,
}

#[derive(Default)]
struct StreamCaptureState {
	buffer: String,
	pending_marker: Option<String>,
	marker_observed: bool,
	disconnected: bool,
}

impl StreamCapture {
	fn begin(&self, marker: &str) -> String {
		let mut state = self
			.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		let pending = std::mem::take(&mut state.buffer);
		state.pending_marker = Some(marker.to_string());
		state.marker_observed = false;
		pending
	}

	fn push(&self, line: String) {
		let mut state = self
			.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		if let Some(marker) = state.pending_marker.as_deref()
			&& let Some(marker_index) = line.find(marker)
		{
			state.buffer.push_str(&line[..marker_index]);
			state.marker_observed = true;
		} else {
			state.buffer.push_str(&line);
			state.buffer.push('\n');
		}
		self.changed.notify_all();
	}

	fn disconnect(&self) {
		let mut state = self
			.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		state.disconnected = true;
		self.changed.notify_all();
	}

	fn take_pending(&self) -> String {
		let mut state = self
			.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		state.pending_marker = None;
		std::mem::take(&mut state.buffer)
	}

	fn take_at_boundary(&self, marker: &str) -> Result<String, EvaluationFailure> {
		let mut state = self
			.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		let deadline = Instant::now() + OUTPUT_DISCONNECT_TIMEOUT;
		while !state.marker_observed && !state.disconnected {
			let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
				state.pending_marker = None;
				return Err(EvaluationFailure::ProcessExited(format!(
					"timed out waiting for evaluator output boundary `{marker}`"
				)));
			};
			let (next_state, timeout) = self
				.changed
				.wait_timeout(state, remaining)
				.unwrap_or_else(std::sync::PoisonError::into_inner);
			state = next_state;
			if timeout.timed_out() && !state.marker_observed {
				state.pending_marker = None;
				return Err(EvaluationFailure::ProcessExited(format!(
					"timed out waiting for evaluator output boundary `{marker}`"
				)));
			}
		}
		if !state.marker_observed {
			return Err(EvaluationFailure::ProcessExited(format!(
				"evaluator output boundary `{marker}` was not observed"
			)));
		}
		state.pending_marker = None;
		Ok(std::mem::take(&mut state.buffer))
	}

	fn take_after_disconnect(&self, deadline: Instant) -> String {
		let mut state = self
			.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		while !state.disconnected {
			let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
				break;
			};
			let (next_state, timeout) = self
				.changed
				.wait_timeout(state, remaining)
				.unwrap_or_else(std::sync::PoisonError::into_inner);
			state = next_state;
			if timeout.timed_out() {
				break;
			}
		}
		state.pending_marker = None;
		std::mem::take(&mut state.buffer)
	}
}

#[cfg(test)]
mod tests {
	use std::future::Future;
	use std::io::Read;
	use std::path::{Path, PathBuf};
	use std::process::{Child, Command, Output};
	use std::sync::{Arc, Condvar, Mutex};
	use std::task::Poll;
	use std::thread::{self, JoinHandle};
	use std::time::{Duration, Instant};

	use evcxr::EvalContext;
	use reinhardt_db::orm::registry::{ModelInfo, global_model_registry};
	use serial_test::serial;
	use tempfile::{Builder, TempDir};

	use super::{
		BlockingShellEvaluator, EvaluationFailure, EvaluationOutput, EvaluatorWorker,
		EvcxrEvaluator, StartupInterrupt, StreamCapture, bootstrap_prelude,
		configure_evaluator_build_dir_from, evaluation_command_error, path_dependency,
		source_with_commit_sentinel,
	};
	use crate::ShellConfig;
	use crate::shell::session::{EvaluationInterrupt, EvaluatorClient};

	#[derive(Default)]
	struct InterruptProbeState {
		started: bool,
		interrupted: bool,
		interrupt_count: usize,
		dropped: bool,
	}

	struct InterruptibleEvaluator {
		state: Arc<(Mutex<InterruptProbeState>, Condvar)>,
	}

	impl BlockingShellEvaluator for InterruptibleEvaluator {
		fn evaluate(&mut self, _source: &str) -> Result<EvaluationOutput, EvaluationFailure> {
			let (state, changed) = &*self.state;
			let mut state = state.lock().expect("interrupt probe state lock");
			state.started = true;
			changed.notify_all();
			while !state.interrupted {
				state = changed.wait(state).expect("interrupt probe state wait");
			}
			Err(EvaluationFailure::ProcessExited(
				"probe evaluator interrupted".to_string(),
			))
		}

		fn interrupt_handle(&self) -> EvaluationInterrupt {
			let state = Arc::clone(&self.state);
			EvaluationInterrupt::new(move || {
				let (state, changed) = &*state;
				let mut state = state.lock().expect("interrupt probe state lock");
				state.interrupted = true;
				state.interrupt_count += 1;
				changed.notify_all();
				Ok(())
			})
		}
	}

	impl Drop for InterruptibleEvaluator {
		fn drop(&mut self) {
			self.state
				.0
				.lock()
				.expect("interrupt probe state lock")
				.dropped = true;
		}
	}

	#[test]
	fn commit_sentinel_follows_multiline_inner_attributes_and_block_docs() {
		let source = "#![\nallow(unused)\n]\n/*! shell documentation */\nlet value = 1;";
		let rendered = source_with_commit_sentinel(source, "__commit");

		assert_eq!(
			rendered,
			"#![\nallow(unused)\n]\n/*! shell documentation */\nlet __commit: ::std::string::String = ::std::string::String::new();\nlet value = 1;"
		);
	}

	#[test]
	fn commit_sentinel_follows_nested_inner_block_docs() {
		let source = "/*! outer /* nested */ still outer */\nlet value = 1;";
		let rendered = source_with_commit_sentinel(source, "__commit");

		assert_eq!(
			rendered,
			"/*! outer /* nested */ still outer */\nlet __commit: ::std::string::String = ::std::string::String::new();\nlet value = 1;"
		);
	}

	#[test]
	fn commit_sentinel_keeps_outer_and_ordinary_comments_with_the_submitted_item() {
		let source = "/* note */let value = 1;\n/// documentation\nstruct Value;";
		let rendered = source_with_commit_sentinel(source, "__commit");

		assert_eq!(
			rendered,
			"let __commit: ::std::string::String = ::std::string::String::new();\n/* note */let value = 1;\n/// documentation\nstruct Value;"
		);
	}

	#[test]
	fn commit_sentinel_ignores_brackets_in_raw_inner_attribute_literals() {
		let source = "#![doc = r##\"a ] bracket\"##]\nlet value = 1;";
		let rendered = source_with_commit_sentinel(source, "__commit");

		assert_eq!(
			rendered,
			"#![doc = r##\"a ] bracket\"##]\nlet __commit: ::std::string::String = ::std::string::String::new();\nlet value = 1;"
		);
	}

	#[test]
	fn startup_interrupt_terminates_a_process_registered_after_the_signal() {
		let startup_interrupt = StartupInterrupt::default();
		let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));

		startup_interrupt
			.interrupt()
			.expect("interrupt before process startup should be recorded");
		let interrupt_calls = Arc::clone(&calls);
		startup_interrupt
			.register(EvaluationInterrupt::new(move || {
				interrupt_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
				Ok(())
			}))
			.expect("registered process should observe the pending interrupt");

		assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
	}

	#[test]
	fn panic_output_is_not_repeated_in_the_command_error() {
		let error = evaluation_command_error(
			EvaluationFailure::Panic("panic payload".to_string()).with_output(EvaluationOutput {
				stdout: String::new(),
				stderr: "panic payload\n".to_string(),
				value: None,
			}),
		);

		assert_eq!(error.to_string().matches("panic payload").count(), 1);
	}

	#[test]
	fn evaluator_child_uses_the_build_dir_workaround_without_mutating_process_environment() {
		let mut command = Command::new("shell-evaluator-child");
		configure_evaluator_build_dir_from(&mut command, None);

		let configured = command
			.get_envs()
			.find(|(key, _)| *key == "CARGO_BUILD_BUILD_DIR")
			.and_then(|(_, value)| value)
			.expect("child command should receive a build directory");
		assert_eq!(configured, "target");
	}

	#[test]
	fn project_path_dependency_enables_commands_shell_feature() {
		let directory = Builder::new()
			.prefix("shell-dependency")
			.tempdir()
			.expect("temporary project directory should be created");
		let manifest_dir = directory.path().join("project \"quoted\"");
		std::fs::create_dir_all(&manifest_dir).expect("project directory should be created");
		std::fs::write(
			manifest_dir.join("Cargo.toml"),
			"[package]\nname = \"shell-project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
		)
		.expect("project manifest should be written");
		let config = ShellConfig::new(
			"shell-project",
			"shell_project",
			&manifest_dir,
			"shell_project::config::settings",
			[] as [&str; 0],
		)
		.with_dependency_features(["custom-feature"])
		.without_default_features()
		.validate()
		.expect("shell configuration should validate");
		let prelude = bootstrap_prelude(&config);
		assert!(prelude[0].contains("use shell_project::config::shell::framework::prelude::*;"));
		let dependency = path_dependency(&config).expect("path dependency should render");
		let manifest = format!("[dependencies]\nshell_project = {dependency}\n");
		let document = manifest
			.parse::<toml_edit::DocumentMut>()
			.expect("rendered dependency should be valid Cargo TOML");
		let dependency = document["dependencies"]["shell_project"]
			.as_inline_table()
			.expect("project dependency should be an inline table");

		assert_eq!(
			dependency.get("package").and_then(toml_edit::Value::as_str),
			Some("shell-project")
		);
		assert_eq!(
			dependency.get("path").and_then(toml_edit::Value::as_str),
			manifest_dir.to_str()
		);
		assert_eq!(
			dependency
				.get("features")
				.and_then(toml_edit::Value::as_array)
				.and_then(|features| features.get(0))
				.and_then(toml_edit::Value::as_str),
			Some("commands-shell")
		);
		assert_eq!(
			dependency
				.get("features")
				.and_then(toml_edit::Value::as_array)
				.and_then(|features| features.get(1))
				.and_then(toml_edit::Value::as_str),
			Some("custom-feature")
		);
		assert_eq!(
			dependency
				.get("default-features")
				.and_then(toml_edit::Value::as_bool),
			Some(false)
		);
	}

	#[test]
	fn stream_capture_preserves_partial_output_before_boundary() {
		let capture = StreamCapture::default();
		let marker = "__REINHARDT_SHELL_OUTPUT_BOUNDARY_1_1__";
		assert_eq!(capture.begin(marker), "");
		capture.push(format!("partial output{marker}"));

		assert_eq!(
			capture
				.take_at_boundary(marker)
				.expect("partial-output boundary should be recognized"),
			"partial output"
		);
	}

	#[test]
	fn stream_capture_returns_output_arriving_after_the_previous_boundary() {
		let capture = StreamCapture::default();
		let first_marker = "__REINHARDT_SHELL_OUTPUT_BOUNDARY_1_1__";
		let second_marker = "__REINHARDT_SHELL_OUTPUT_BOUNDARY_1_2__";
		assert_eq!(capture.begin(first_marker), "");
		capture.push(first_marker.to_string());
		assert_eq!(
			capture
				.take_at_boundary(first_marker)
				.expect("first boundary should be recognized"),
			""
		);
		capture.push("detached output".to_string());

		assert_eq!(capture.begin(second_marker), "detached output\n");
		capture.push(second_marker.to_string());
		assert_eq!(
			capture
				.take_at_boundary(second_marker)
				.expect("second boundary should be recognized"),
			""
		);
	}

	#[test]
	fn stream_capture_waits_for_a_delayed_boundary_while_connected() {
		let capture = Arc::new(StreamCapture::default());
		let marker = "__REINHARDT_SHELL_OUTPUT_BOUNDARY_1_1__";
		assert_eq!(capture.begin(marker), "");
		let producer = {
			let capture = Arc::clone(&capture);
			let marker = marker.to_string();
			thread::spawn(move || {
				thread::sleep(Duration::from_millis(20));
				capture.push(marker);
			})
		};

		assert_eq!(
			capture
				.take_at_boundary(marker)
				.expect("delayed boundary should be recognized"),
			""
		);
		producer
			.join()
			.expect("delayed boundary producer should join");
	}

	#[tokio::test]
	async fn worker_interrupts_a_running_evaluation_and_joins_on_drop() {
		let state = Arc::new((Mutex::new(InterruptProbeState::default()), Condvar::new()));
		let mut worker = EvaluatorWorker::spawn(Box::new(InterruptibleEvaluator {
			state: Arc::clone(&state),
		}));
		let interrupt = worker.interrupt();
		let trigger_state = Arc::clone(&state);
		let trigger = thread::spawn(move || {
			let (state, changed) = &*trigger_state;
			let mut state = state.lock().expect("interrupt probe state lock");
			while !state.started {
				state = changed.wait(state).expect("interrupt probe state wait");
			}
			drop(state);
			interrupt
				.interrupt()
				.expect("running evaluation should be interruptible");
		});

		let result = worker.evaluate("long_running()").await;
		trigger.join().expect("interrupt trigger should join");
		assert_eq!(
			result,
			Err(EvaluationFailure::ProcessExited(
				"probe evaluator interrupted".to_string()
			))
		);
		drop(worker);

		let state = state.0.lock().expect("interrupt probe state lock");
		assert_eq!(state.interrupt_count, 2);
		assert!(state.dropped);
	}

	#[test]
	fn worker_factory_constructs_evaluator_on_the_owned_thread() {
		let caller_thread = thread::current().id();
		let (created, observed) = std::sync::mpsc::channel();
		let state = Arc::new((Mutex::new(InterruptProbeState::default()), Condvar::new()));

		let (worker, warnings) = EvaluatorWorker::start_with({
			let state = Arc::clone(&state);
			move || {
				let _ = created.send(thread::current().id());
				Ok((
					Box::new(InterruptibleEvaluator { state }) as Box<dyn BlockingShellEvaluator>,
					vec!["worker warning".to_string()],
				))
			}
		})
		.expect("worker factory should start");

		assert_ne!(
			observed.recv().expect("creator thread should be observed"),
			caller_thread
		);
		assert_eq!(warnings, ["worker warning"]);
		drop(worker);
	}

	#[tokio::test]
	async fn dropping_worker_with_a_queued_evaluation_interrupts_and_joins() {
		let state = Arc::new((Mutex::new(InterruptProbeState::default()), Condvar::new()));
		let mut worker = EvaluatorWorker::spawn(Box::new(InterruptibleEvaluator {
			state: Arc::clone(&state),
		}));
		let mut evaluation = Box::pin(worker.evaluate("long_running()"));
		let request_was_queued =
			std::future::poll_fn(|context| match evaluation.as_mut().poll(context) {
				Poll::Pending => Poll::Ready(true),
				Poll::Ready(_) => Poll::Ready(false),
			})
			.await;
		assert!(request_was_queued);
		drop(evaluation);

		let (finished, completion) = std::sync::mpsc::channel();
		thread::spawn(move || {
			drop(worker);
			let _ = finished.send(());
		});
		completion
			.recv_timeout(Duration::from_secs(2))
			.expect("worker drop should interrupt queued work and join promptly");

		let state = state.0.lock().expect("interrupt probe state lock");
		assert_eq!(state.interrupt_count, 1);
		assert!(state.dropped);
	}

	struct ShellFixture {
		_directory: TempDir,
		manifest_dir: PathBuf,
		runtime_path: PathBuf,
	}

	impl ShellFixture {
		fn create() -> Self {
			let directory = Builder::new()
				.prefix("shell-fixture")
				.tempdir()
				.expect("temporary fixture directory should be created");
			let manifest_dir = directory.path().join("shell-project");
			std::fs::create_dir_all(manifest_dir.join("src"))
				.expect("fixture source directory should be created");
			let manifest = "[package]\n\
				 name = \"shell-project\"\n\
				 version = \"0.1.0\"\n\
				 edition = \"2024\"\n\
				 \n\
				 [features]\n\
				 default = []\n\
				 commands-shell = []\n";
			std::fs::write(manifest_dir.join("Cargo.toml"), manifest)
				.expect("fixture manifest should be written");
			std::fs::write(
				manifest_dir.join("src/lib.rs"),
				r#"
pub extern crate self as reinhardt;

pub mod commands {
	pub struct ShellEnvironment<S> {
		settings: S,
	}

	impl<S> ShellEnvironment<S> {
		pub async fn bootstrap(settings: S) -> std::io::Result<Self> {
			Ok(Self { settings })
		}

		pub fn settings(&self) -> &S {
			&self.settings
		}

		pub fn database(&self) -> String {
			"database-ready".to_string()
		}

		pub fn di(&self) -> String {
			"di-ready".to_string()
		}
	}
}

pub mod config {
	#[cfg(feature = "commands-shell")]
	pub mod shell {
		pub use reinhardt as framework;

		pub type ShellSettings = super::settings::Settings;
		pub type ShellDatabase = String;
		pub type ShellDi = String;
		pub type ProjectShellEnvironment =
			framework::commands::ShellEnvironment<ShellSettings>;
	}

	pub mod settings {
		pub type Settings = String;

		pub fn get_settings() -> Settings {
			"settings-default".to_string()
		}

		pub fn get_shell_settings() -> Settings {
			"settings-configured".to_string()
		}
	}
}

pub mod models {
	pub struct InventoryItem;
}
"#,
			)
			.expect("fixture library should be written");
			let runtime_path = build_test_runtime(directory.path());

			Self {
				_directory: directory,
				manifest_dir,
				runtime_path,
			}
		}

		fn config(&self) -> ShellConfig {
			ShellConfig::new(
				"shell-project",
				"shell_project",
				&self.manifest_dir,
				"shell_project::config::settings::get_shell_settings",
				["inventory"],
			)
			.with_prelude(
				"let project_prelude_loaded = \
				 std::any::TypeId::of::<InventoryItem>() == \
				 std::any::TypeId::of::<shell_project::models::InventoryItem>();\n\
				 let counter = 40;\n\
				 let retained = 41;",
			)
		}
	}

	fn build_test_runtime(directory: &Path) -> PathBuf {
		let dependencies = std::env::current_exe()
			.expect("current test executable should resolve")
			.parent()
			.expect("test executable should have a dependency directory")
			.to_path_buf();
		let mut evcxr_libraries = std::fs::read_dir(&dependencies)
			.expect("test dependency directory should be readable")
			.filter_map(Result::ok)
			.map(|entry| entry.path())
			.filter(|path| {
				path.file_name()
					.and_then(|name| name.to_str())
					.is_some_and(|name| name.starts_with("libevcxr-") && name.ends_with(".rlib"))
			})
			.collect::<Vec<_>>();
		evcxr_libraries.sort();
		let evcxr_library = evcxr_libraries
			.last()
			.expect("compiled evcxr rlib should exist");
		let runtime_source = directory.join("shell_evcxr_runtime.rs");
		let runtime_path = directory.join("shell_evcxr_runtime");
		std::fs::write(&runtime_source, "fn main() { evcxr::runtime_hook(); }\n")
			.expect("test runtime source should be written");
		let output = Command::new("rustc")
			.arg("--edition=2024")
			.arg(&runtime_source)
			.arg("-L")
			.arg(format!("dependency={}", dependencies.display()))
			.arg("--extern")
			.arg(format!("evcxr={}", evcxr_library.display()))
			.arg("-o")
			.arg(&runtime_path)
			.output()
			.expect("test runtime should compile");
		assert!(
			output.status.success(),
			"test runtime compilation failed: {}",
			String::from_utf8_lossy(&output.stderr)
		);
		runtime_path
	}

	struct ModelRegistryGuard {
		previous: Vec<ModelInfo>,
	}

	impl ModelRegistryGuard {
		fn with_inventory_item() -> Self {
			let previous = global_model_registry().all();
			global_model_registry().clear();
			global_model_registry().register(ModelInfo {
				app_label: "inventory".to_string(),
				model_name: "InventoryItem".to_string(),
				type_path: "shell_project::models::InventoryItem".to_string(),
				table_name: "inventory_item".to_string(),
			});
			Self { previous }
		}
	}

	impl Drop for ModelRegistryGuard {
		fn drop(&mut self) {
			global_model_registry().clear();
			for model in self.previous.drain(..) {
				global_model_registry().register(model);
			}
		}
	}

	struct EnvironmentVariableGuard {
		name: &'static str,
		previous: Option<std::ffi::OsString>,
	}

	impl EnvironmentVariableGuard {
		fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
			let previous = std::env::var_os(name);
			// SAFETY: Every real evaluator test is serialized under `shell_evcxr`.
			unsafe {
				std::env::set_var(name, value);
			}
			Self { name, previous }
		}
	}

	impl Drop for EnvironmentVariableGuard {
		fn drop(&mut self) {
			// SAFETY: Every real evaluator test is serialized under `shell_evcxr`.
			unsafe {
				match self.previous.take() {
					Some(value) => std::env::set_var(self.name, value),
					None => std::env::remove_var(self.name),
				}
			}
		}
	}

	fn new_evaluator(fixture: &ShellFixture) -> EvcxrEvaluator {
		let validated = fixture
			.config()
			.validate()
			.expect("fixture shell configuration should validate");
		let evcxr_tmpdir = fixture._directory.path().join("evcxr");
		std::fs::create_dir_all(&evcxr_tmpdir)
			.expect("evcxr temporary directory should be created");
		let _tmpdir = EnvironmentVariableGuard::set("EVCXR_TMPDIR", &evcxr_tmpdir);
		let context = EvalContext::with_subprocess_command(Command::new(&fixture.runtime_path));
		let (eval, outputs) = context.unwrap_or_else(|error| {
			let artifacts = walkdir::WalkDir::new(&evcxr_tmpdir)
				.into_iter()
				.filter_map(Result::ok)
				.map(|entry| entry.path().display().to_string())
				.collect::<Vec<_>>()
				.join("\n");
			panic!(
				"real evcxr context should start with the test runtime: {error}\n\
				 generated artifacts:\n{artifacts}"
			);
		});
		let (evaluator, warnings) =
			EvcxrEvaluator::bootstrap_with_context(&validated, eval, outputs)
				.expect("real evcxr evaluator should bootstrap the project");
		assert_eq!(warnings, Vec::<String>::new());
		evaluator
	}

	const REAL_EVALUATOR_PROBE: &str = "shell::evaluator::tests::real_evaluator_subprocess_probe";
	const PROCESS_EXIT_PROBE: &str = "shell::evaluator::tests::process_exit_subprocess_probe";
	const LARGE_OUTPUT_PROBE: &str = "shell::evaluator::tests::large_output_subprocess_probe";

	struct ProbeChild {
		child: Child,
		stdout_reader: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
		stderr_reader: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
		reaped: bool,
	}

	impl ProbeChild {
		fn spawn(command: &mut Command) -> Self {
			let mut child = command.spawn().expect("real evaluator probe should start");
			let mut stdout = child
				.stdout
				.take()
				.expect("probe stdout should be configured as piped");
			let stdout_reader = thread::spawn(move || {
				let mut output = Vec::new();
				stdout.read_to_end(&mut output)?;
				Ok(output)
			});
			let mut stderr = child
				.stderr
				.take()
				.expect("probe stderr should be configured as piped");
			let stderr_reader = thread::spawn(move || {
				let mut output = Vec::new();
				stderr.read_to_end(&mut output)?;
				Ok(output)
			});
			Self {
				child,
				stdout_reader: Some(stdout_reader),
				stderr_reader: Some(stderr_reader),
				reaped: false,
			}
		}

		fn wait_with_output(mut self, probe: &str, timeout: Duration) -> Result<Output, String> {
			let deadline = Instant::now() + timeout;
			let mut lifecycle_error = None;
			loop {
				match self.child.try_wait() {
					Ok(Some(_)) => break,
					Ok(None) if Instant::now() < deadline => {
						thread::sleep(Duration::from_millis(50));
					}
					Ok(None) => {
						let kill_result = self.child.kill();
						lifecycle_error = Some(format!(
							"real evaluator probe `{probe}` timed out after {timeout:?}; \
							 kill result: {kill_result:?}"
						));
						break;
					}
					Err(error) => {
						let kill_result = self.child.kill();
						lifecycle_error = Some(format!(
							"real evaluator probe `{probe}` status failed: {error}; \
							 kill result: {kill_result:?}"
						));
						break;
					}
				}
			}

			let status = self.child.wait();
			self.reaped = status.is_ok();
			let stdout = join_probe_reader(self.stdout_reader.take(), "stdout");
			let stderr = join_probe_reader(self.stderr_reader.take(), "stderr");
			let stdout = stdout.map_err(|error| format_probe_failure(error, &[], &[]))?;
			let stderr = stderr.map_err(|error| format_probe_failure(error, &stdout, &[]))?;
			if let Some(error) = lifecycle_error {
				return Err(format_probe_failure(error, &stdout, &stderr));
			}
			let status = status
				.map_err(|error| format_probe_failure(error.to_string(), &stdout, &stderr))?;
			Ok(Output {
				status,
				stdout,
				stderr,
			})
		}
	}

	impl Drop for ProbeChild {
		fn drop(&mut self) {
			if !self.reaped {
				if self.child.try_wait().ok().flatten().is_none() {
					let _ = self.child.kill();
				}
				let _ = self.child.wait();
			}
			if let Some(reader) = self.stdout_reader.take() {
				let _ = reader.join();
			}
			if let Some(reader) = self.stderr_reader.take() {
				let _ = reader.join();
			}
		}
	}

	fn join_probe_reader(
		reader: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
		stream: &str,
	) -> Result<Vec<u8>, String> {
		reader
			.ok_or_else(|| format!("probe {stream} reader was already consumed"))?
			.join()
			.map_err(|_| format!("probe {stream} reader panicked"))?
			.map_err(|error| format!("probe {stream} read failed: {error}"))
	}

	fn format_probe_failure(
		message: impl std::fmt::Display,
		stdout: &[u8],
		stderr: &[u8],
	) -> String {
		format!(
			"{message}\nstdout:\n{}\nstderr:\n{}",
			String::from_utf8_lossy(stdout),
			String::from_utf8_lossy(stderr)
		)
	}

	fn run_probe(probe: &str, timeout: Duration) {
		let mut command =
			Command::new(std::env::current_exe().expect("current test executable should resolve"));
		command
			.arg("--ignored")
			.arg("--exact")
			.arg(probe)
			.arg("--no-capture")
			.env("CARGO_BUILD_BUILD_DIR", "target")
			.env("CARGO_BUILD_JOBS", "1")
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped());
		let output = ProbeChild::spawn(&mut command)
			.wait_with_output(probe, timeout)
			.unwrap_or_else(|error| panic!("{error}"));

		assert!(
			output.status.success(),
			"real evaluator probe failed:\nstdout:\n{}\nstderr:\n{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
	}

	#[test]
	#[serial(shell_evcxr)]
	fn real_evaluator_probes_run_sequentially_with_timeouts() {
		run_probe(REAL_EVALUATOR_PROBE, Duration::from_secs(240));
		run_probe(PROCESS_EXIT_PROBE, Duration::from_secs(240));
	}

	#[test]
	fn probe_runner_drains_output_larger_than_pipe_capacity() {
		run_probe(LARGE_OUTPUT_PROBE, Duration::from_secs(3));
	}

	#[ignore = "subprocess-only pipe-capacity probe"]
	#[test]
	fn large_output_subprocess_probe() {
		use std::io::Write;

		let output = vec![b'x'; 8 * 1024 * 1024];
		std::io::stdout()
			.write_all(&output)
			.expect("large stdout payload should be written");
		std::io::stderr()
			.write_all(&output)
			.expect("large stderr payload should be written");
	}

	#[ignore = "subprocess-only real evaluator probe"]
	#[test]
	fn real_evaluator_subprocess_probe() {
		crate::shell_runtime_hook();
		assert_eq!(
			std::env::var("CARGO_BUILD_BUILD_DIR").as_deref(),
			Ok("target")
		);
		let fixture = ShellFixture::create();
		let _registry = ModelRegistryGuard::with_inventory_item();
		let mut evaluator = new_evaluator(&fixture);

		let bindings = evaluator
			.evaluate(
				"settings.as_str() == \"settings-configured\" \
				 && db == \"database-ready\" \
				 && di == \"di-ready\" \
				 && project_prelude_loaded",
			)
			.expect("bootstrap bindings should evaluate");
		assert_eq!(bindings.value.as_deref(), Some("true"));

		let asynchronous = evaluator
			.evaluate(
				"tokio::time::sleep(std::time::Duration::from_millis(1)).await;\n\
				 counter + 2",
			)
			.expect("top-level await should evaluate");
		assert_eq!(asynchronous.value.as_deref(), Some("42"));

		let compilation = evaluator
			.evaluate("missing_name + 1")
			.expect_err("unknown binding should fail compilation");
		assert!(matches!(compilation, EvaluationFailure::Compilation(_)));
		let retained = evaluator
			.evaluate("retained + 1")
			.expect("prior binding should remain after compilation failure");
		assert_eq!(retained.value.as_deref(), Some("42"));

		let runtime_failure = evaluator
			.evaluate(r#"Err::<(), _>(std::io::Error::other("runtime-probe"))?"#)
			.expect_err("top-level question mark should report a runtime failure");
		match runtime_failure {
			EvaluationFailure::Runtime(message) => {
				assert!(message.contains("runtime-probe"));
			}
			other => panic!("expected runtime failure, got {other:?}"),
		}
		let retained = evaluator
			.evaluate("retained + 1")
			.expect("prior binding should remain after runtime failure");
		assert_eq!(retained.value.as_deref(), Some("42"));

		let output = evaluator
			.evaluate(
				r#"println!("stdout-probe");
				   eprintln!("stderr-probe");
				   42"#,
			)
			.expect("output-producing source should evaluate");
		assert_eq!(output.stdout, "stdout-probe\n");
		assert!(output.stderr.contains("stderr-probe"));
		assert!(!output.stdout.contains("__REINHARDT_SHELL_OUTPUT_BOUNDARY_"));
		assert!(!output.stderr.contains("__REINHARDT_SHELL_OUTPUT_BOUNDARY_"));
		assert_eq!(output.value.as_deref(), Some("42"));
		let quiet = evaluator
			.evaluate("retained + 1")
			.expect("boundary markers should preserve later values");
		assert_eq!(quiet.stdout, "");
		assert_eq!(quiet.stderr, "");
		assert_eq!(quiet.value.as_deref(), Some("42"));
		let leaked_sentinels = evaluator
			.context
			.variables_and_types()
			.filter(|(name, _)| name.starts_with("__reinhardt_shell_evaluation_"))
			.map(|(name, _)| name.to_string())
			.collect::<Vec<_>>();
		assert_eq!(leaked_sentinels, Vec::<String>::new());

		let source = r#"let credential = "not-for-diagnostics"; credential.no_such_method()"#;
		let diagnostic = evaluator
			.evaluate(source)
			.expect_err("invalid method should fail compilation");
		let EvaluationFailure::Compilation(diagnostic) = diagnostic else {
			panic!("expected compilation failure");
		};
		assert!(!diagnostic.contains(source));
		assert!(!diagnostic.contains("not-for-diagnostics"));

		let panic = evaluator
			.evaluate(r#"panic!("panic-classification-probe")"#)
			.expect_err("panic should make the evaluator unusable");
		let EvaluationFailure::Output { failure, output } = panic else {
			panic!("expected panic output, got {panic:?}");
		};
		let EvaluationFailure::Panic(message) = *failure else {
			panic!("expected panic classification");
		};
		assert!(message.contains("panic-classification-probe"));
		assert!(output.stderr.contains("panic-classification-probe"));
	}

	#[ignore = "subprocess-only process exit probe"]
	#[tokio::test]
	async fn process_exit_subprocess_probe() {
		crate::shell_runtime_hook();
		assert_eq!(
			std::env::var("CARGO_BUILD_BUILD_DIR").as_deref(),
			Ok("target")
		);
		let fixture = ShellFixture::create();
		let _registry = ModelRegistryGuard::with_inventory_item();
		let started_path = fixture._directory.path().join("interrupt-started");
		let source = format!(
			"std::fs::write({:?}, \"started\").unwrap();\n\
			 tokio::time::sleep(std::time::Duration::from_secs(30)).await;",
			started_path
		);
		let mut interrupted_worker = EvaluatorWorker::spawn(Box::new(new_evaluator(&fixture)));
		let interrupt = interrupted_worker.interrupt();
		let interrupt_started_path = started_path.clone();
		let interrupter = thread::spawn(move || {
			let deadline = Instant::now() + Duration::from_secs(30);
			while !interrupt_started_path.is_file() {
				assert!(
					Instant::now() < deadline,
					"real evaluator did not start the interrupt probe"
				);
				thread::sleep(Duration::from_millis(20));
			}
			interrupt
				.interrupt()
				.expect("running real evaluator should be interrupted");
		});
		let interrupted = interrupted_worker
			.evaluate(&source)
			.await
			.expect_err("interrupted real evaluator should terminate");
		interrupter
			.join()
			.expect("real evaluator interrupter should join");
		assert!(matches!(
			interrupted,
			EvaluationFailure::ProcessExited(_) | EvaluationFailure::Panic(_)
		));
		drop(interrupted_worker);

		let validated = fixture
			.config()
			.validate()
			.expect("real factory configuration should validate");
		let evcxr_tmpdir = fixture._directory.path().join("factory-evcxr");
		std::fs::create_dir_all(&evcxr_tmpdir).expect("factory evcxr directory should be created");
		let _tmpdir = EnvironmentVariableGuard::set("EVCXR_TMPDIR", &evcxr_tmpdir);
		let runtime_path = fixture.runtime_path.clone();
		let (mut replacement, warnings) = EvaluatorWorker::start_with(move || {
			let context = EvalContext::with_subprocess_command(Command::new(runtime_path));
			let (eval, outputs) = context.map_err(super::classify_startup_error)?;
			let (evaluator, warnings) =
				EvcxrEvaluator::bootstrap_with_context(&validated, eval, outputs)?;
			Ok((
				Box::new(evaluator) as Box<dyn BlockingShellEvaluator>,
				warnings,
			))
		})
		.expect("real evaluator should bootstrap inside its owned worker");
		assert_eq!(warnings, Vec::<String>::new());
		let retained = replacement
			.evaluate("retained + 1")
			.await
			.expect("replacement should replay the project prelude");
		assert_eq!(retained.value.as_deref(), Some("42"));
		drop(replacement);

		let mut process_exit_evaluator = EvaluatorWorker::spawn(Box::new(new_evaluator(&fixture)));
		let process_exit = process_exit_evaluator
			.evaluate("std::process::exit(7)")
			.await
			.expect_err("process exit should make the evaluator unusable");
		assert!(matches!(process_exit, EvaluationFailure::ProcessExited(_)));
	}
}
