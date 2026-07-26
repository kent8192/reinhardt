use std::sync::Arc;

use async_trait::async_trait;

use super::evaluator::{EvaluationFailure, EvaluationOutput, StartupInterrupt};
use crate::{CommandError, CommandResult};

#[derive(Clone)]
pub(crate) struct EvaluationInterrupt {
	interrupt: Arc<dyn Fn() -> Result<(), EvaluationFailure> + Send + Sync>,
}

impl EvaluationInterrupt {
	pub(crate) fn new<F>(interrupt: F) -> Self
	where
		F: Fn() -> Result<(), EvaluationFailure> + Send + Sync + 'static,
	{
		Self {
			interrupt: Arc::new(interrupt),
		}
	}

	pub(crate) fn interrupt(&self) -> Result<(), EvaluationFailure> {
		(self.interrupt)()
	}
}

#[async_trait]
pub(crate) trait EvaluatorClient: Send {
	async fn evaluate(&mut self, source: &str) -> Result<EvaluationOutput, EvaluationFailure>;
	fn interrupt(&self) -> EvaluationInterrupt;
}

pub(crate) trait EvaluatorFactory {
	fn start(&mut self) -> CommandResult<Box<dyn EvaluatorClient>>;

	fn startup_interrupt(&self) -> Option<StartupInterrupt> {
		None
	}

	fn take_warnings(&mut self) -> Vec<String> {
		Vec::new()
	}

	fn take_startup_output(&mut self) -> Vec<EvaluationOutput> {
		Vec::new()
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InputEvent {
	Source(String),
	Warning(String),
	Interrupted,
	Eof,
	EofWithPending,
}

pub(crate) trait ShellInput {
	fn read(&mut self) -> CommandResult<InputEvent>;
}

pub(crate) trait ShellOutput {
	fn stdout(&mut self, value: &str) -> CommandResult<()>;
	fn stderr(&mut self, value: &str) -> CommandResult<()>;
	fn value(&mut self, value: &str) -> CommandResult<()>;
	fn warning(&mut self, value: &str) -> CommandResult<()>;
}

#[async_trait]
pub(crate) trait InterruptSignal: Send {
	async fn wait(&mut self) -> std::io::Result<()>;
}

pub(crate) struct CtrlCSignal;

#[async_trait]
impl InterruptSignal for CtrlCSignal {
	async fn wait(&mut self) -> std::io::Result<()> {
		tokio::signal::ctrl_c().await
	}
}

pub(crate) struct ShellSession<F, W> {
	factory: Option<F>,
	evaluator: Box<dyn EvaluatorClient>,
	output: W,
	signal: Box<dyn InterruptSignal>,
}

impl<F, W> ShellSession<F, W>
where
	F: EvaluatorFactory,
	W: ShellOutput,
{
	pub(crate) fn new(mut factory: F, mut output: W) -> CommandResult<Self> {
		let evaluator = match factory.start() {
			Ok(evaluator) => evaluator,
			Err(error) => {
				forward_startup(&mut output, &mut factory)?;
				return Err(error);
			}
		};
		forward_startup(&mut output, &mut factory)?;
		Ok(Self {
			factory: Some(factory),
			evaluator,
			output,
			signal: Box::new(CtrlCSignal),
		})
	}

	#[cfg(test)]
	pub(crate) fn with_signal<S>(mut factory: F, mut output: W, signal: S) -> CommandResult<Self>
	where
		S: InterruptSignal + 'static,
	{
		let evaluator = match factory.start() {
			Ok(evaluator) => evaluator,
			Err(error) => {
				forward_startup(&mut output, &mut factory)?;
				return Err(error);
			}
		};
		forward_startup(&mut output, &mut factory)?;
		Ok(Self::from_client_with_signal(
			factory, evaluator, output, signal,
		))
	}

	#[cfg(test)]
	pub(crate) fn from_client_with_signal<S>(
		factory: F,
		evaluator: Box<dyn EvaluatorClient>,
		output: W,
		signal: S,
	) -> Self
	where
		S: InterruptSignal + 'static,
	{
		Self {
			factory: Some(factory),
			evaluator,
			output,
			signal: Box::new(signal),
		}
	}

	#[cfg(test)]
	pub(crate) fn output(&self) -> &W {
		&self.output
	}

	pub(crate) async fn execute_once(&mut self, source: &str) -> CommandResult<()> {
		match self.evaluate(source).await {
			Ok(output) => self.forward(output),
			Err(failure) => {
				self.forward_failure_output(&failure)?;
				Err(command_error(failure))
			}
		}
	}

	pub(crate) async fn run_interactive<I>(&mut self, input: &mut I) -> CommandResult<()>
	where
		F: Send + 'static,
		I: ShellInput,
	{
		loop {
			match input.read()? {
				InputEvent::Source(source) => {
					let trimmed = source.trim();
					if trimmed == "exit" || trimmed == "quit" {
						return Ok(());
					}
					if trimmed.is_empty() {
						continue;
					}
					match self.evaluate(&source).await {
						Ok(output) => self.forward(output)?,
						Err(failure) if failure_is_fatal(&failure) => {
							self.forward_failure_output(&failure)?;
							self.output.warning(failure_message(&failure))?;
							self.recover().await?;
						}
						Err(failure) => {
							self.forward_failure_output(&failure)?;
							self.output.warning(failure_message(&failure))?;
						}
					}
				}
				InputEvent::Interrupted => {
					self.output.warning("Current input was discarded.")?;
				}
				InputEvent::Warning(message) => self.output.warning(&message)?,
				InputEvent::Eof => return Ok(()),
				InputEvent::EofWithPending => {
					self.output
						.warning("Pending input was discarded at end of file.")?;
					return Ok(());
				}
			}
		}
	}

	async fn evaluate(&mut self, source: &str) -> Result<EvaluationOutput, EvaluationFailure> {
		let interrupt = self.evaluator.interrupt();
		let response = self.evaluator.evaluate(source);
		let signal = self.signal.wait();
		tokio::pin!(response);
		tokio::pin!(signal);
		tokio::select! {
		biased;
		result = &mut response => result,
			signal = signal => {
				interrupt.interrupt()?;
				match signal {
					Ok(()) => {
						let output = match response.as_mut().await {
							Ok(output) => output,
							Err(failure) => failure.output().cloned().unwrap_or_default(),
						};
						if output.stdout.is_empty() && output.stderr.is_empty() {
							Err(EvaluationFailure::Interrupted)
						} else {
							Err(EvaluationFailure::Output {
								failure: Box::new(EvaluationFailure::Interrupted),
								output,
							})
						}
					}
					Err(error) => Err(EvaluationFailure::ProcessExited(format!(
						"failed to listen for evaluation interruption: {error}"
					))),
				}
			}
		}
	}

	fn forward(&mut self, output: EvaluationOutput) -> CommandResult<()> {
		if !output.stdout.is_empty() {
			self.output.stdout(&output.stdout)?;
		}
		if !output.stderr.is_empty() {
			self.output.stderr(&output.stderr)?;
		}
		if let Some(value) = output.value {
			self.output.value(&value)?;
		}
		Ok(())
	}

	fn forward_failure_output(&mut self, failure: &EvaluationFailure) -> CommandResult<()> {
		if let Some(output) = failure.output() {
			self.forward(output.clone())?;
		}
		Ok(())
	}

	async fn recover(&mut self) -> CommandResult<()>
	where
		F: Send + 'static,
	{
		let startup_interrupt = self
			.factory
			.as_ref()
			.and_then(EvaluatorFactory::startup_interrupt);
		let factory = self.factory.take().expect("shell factory is available");
		let mut startup = tokio::task::spawn_blocking(move || {
			let mut factory = factory;
			let replacement = factory.start();
			let warnings = factory.take_warnings();
			let startup_output = factory.take_startup_output();
			(factory, replacement, warnings, startup_output)
		});
		let signal = tokio::signal::ctrl_c();
		tokio::pin!(signal);
		tokio::select! {
			result = &mut startup => {
				let (factory, replacement, warnings, startup_output) = result
					.map_err(|error| CommandError::ExecutionError(error.to_string()))?;
				self.factory = Some(factory);
				for warning in warnings {
					self.output.warning(&warning)?;
				}
				for output in startup_output {
					self.forward(output)?;
				}
				self.evaluator = replacement?;
				self.output.warning("Shell state was reset and the project prelude was reloaded.")
			}
			result = &mut signal => {
				result.map_err(|error| CommandError::ExecutionError(error.to_string()))?;
				if let Some(interrupt) = startup_interrupt {
					let _ = interrupt.interrupt();
				}
				drop(startup);
				Err(CommandError::ExecutionError("Shell recovery was interrupted.".to_string()))
			}
		}
	}
}

fn forward_startup<F, W>(output: &mut W, factory: &mut F) -> CommandResult<()>
where
	F: EvaluatorFactory,
	W: ShellOutput,
{
	for warning in factory.take_warnings() {
		output.warning(&warning)?;
	}
	for startup_output in factory.take_startup_output() {
		if !startup_output.stdout.is_empty() {
			output.stdout(&startup_output.stdout)?;
		}
		if !startup_output.stderr.is_empty() {
			output.stderr(&startup_output.stderr)?;
		}
		if let Some(value) = startup_output.value {
			output.value(&value)?;
		}
	}
	Ok(())
}

fn failure_is_fatal(failure: &EvaluationFailure) -> bool {
	match failure {
		EvaluationFailure::Panic(_)
		| EvaluationFailure::ProcessExited(_)
		| EvaluationFailure::ContextReset(_)
		| EvaluationFailure::Interrupted => true,
		EvaluationFailure::Output { failure, .. } => failure_is_fatal(failure),
		EvaluationFailure::Compilation(_) | EvaluationFailure::Runtime(_) => false,
	}
}

fn failure_message(failure: &EvaluationFailure) -> &str {
	match failure {
		EvaluationFailure::Compilation(message)
		| EvaluationFailure::Runtime(message)
		| EvaluationFailure::Panic(message)
		| EvaluationFailure::ProcessExited(message)
		| EvaluationFailure::ContextReset(message) => message,
		EvaluationFailure::Output { failure, .. } => failure_message(failure),
		EvaluationFailure::Interrupted => "Evaluation was interrupted.",
	}
}

fn command_error(failure: EvaluationFailure) -> CommandError {
	CommandError::ExecutionError(failure_message(&failure).to_string())
}

#[cfg(test)]
mod tests {
	use std::collections::VecDeque;
	use std::future::{Pending, pending};
	use std::sync::{Arc, Mutex};

	use async_trait::async_trait;

	use super::{
		EvaluationInterrupt, EvaluatorClient, EvaluatorFactory, InputEvent, InterruptSignal,
		ShellInput, ShellOutput, ShellSession,
	};
	use crate::shell::evaluator::{EvaluationFailure, EvaluationOutput};
	use crate::{CommandError, CommandResult};

	#[derive(Default)]
	struct FakeState {
		starts: usize,
		sources: Vec<String>,
		interrupts: usize,
	}

	struct FakeFactory {
		state: Arc<Mutex<FakeState>>,
		starts: VecDeque<CommandResult<VecDeque<Result<EvaluationOutput, EvaluationFailure>>>>,
	}

	struct WarningFactory {
		state: Arc<Mutex<FakeState>>,
		warnings: Vec<String>,
	}

	struct FailedStartupFactory {
		output: Vec<EvaluationOutput>,
	}

	impl EvaluatorFactory for FailedStartupFactory {
		fn start(&mut self) -> CommandResult<Box<dyn EvaluatorClient>> {
			Err(CommandError::ExecutionError("bootstrap failed".to_string()))
		}

		fn take_startup_output(&mut self) -> Vec<EvaluationOutput> {
			std::mem::take(&mut self.output)
		}
	}

	impl EvaluatorFactory for WarningFactory {
		fn start(&mut self) -> CommandResult<Box<dyn EvaluatorClient>> {
			Ok(Box::new(FakeClient {
				state: Arc::clone(&self.state),
				outcomes: VecDeque::new(),
			}))
		}

		fn take_warnings(&mut self) -> Vec<String> {
			std::mem::take(&mut self.warnings)
		}
	}

	impl FakeFactory {
		fn new(
			starts: impl IntoIterator<
				Item = CommandResult<VecDeque<Result<EvaluationOutput, EvaluationFailure>>>,
			>,
		) -> (Self, Arc<Mutex<FakeState>>) {
			let state = Arc::new(Mutex::new(FakeState::default()));
			(
				Self {
					state: Arc::clone(&state),
					starts: starts.into_iter().collect(),
				},
				state,
			)
		}
	}

	impl EvaluatorFactory for FakeFactory {
		fn start(&mut self) -> CommandResult<Box<dyn EvaluatorClient>> {
			self.state.lock().expect("fake state lock").starts += 1;
			let outcomes = self.starts.pop_front().ok_or_else(|| {
				CommandError::ExecutionError("unexpected evaluator start".to_string())
			})??;
			Ok(Box::new(FakeClient {
				state: Arc::clone(&self.state),
				outcomes,
			}))
		}
	}

	struct FakeClient {
		state: Arc<Mutex<FakeState>>,
		outcomes: VecDeque<Result<EvaluationOutput, EvaluationFailure>>,
	}

	#[async_trait]
	impl EvaluatorClient for FakeClient {
		async fn evaluate(&mut self, source: &str) -> Result<EvaluationOutput, EvaluationFailure> {
			self.state
				.lock()
				.expect("fake state lock")
				.sources
				.push(source.to_string());
			self.outcomes
				.pop_front()
				.expect("fake evaluator should have an outcome")
		}

		fn interrupt(&self) -> EvaluationInterrupt {
			let state = Arc::clone(&self.state);
			EvaluationInterrupt::new(move || {
				state.lock().expect("fake state lock").interrupts += 1;
				Ok(())
			})
		}
	}

	#[derive(Default)]
	struct FakeOutput {
		stdout: Vec<String>,
		stderr: Vec<String>,
		values: Vec<String>,
		warnings: Vec<String>,
	}

	impl ShellOutput for FakeOutput {
		fn stdout(&mut self, value: &str) -> CommandResult<()> {
			self.stdout.push(value.to_string());
			Ok(())
		}

		fn stderr(&mut self, value: &str) -> CommandResult<()> {
			self.stderr.push(value.to_string());
			Ok(())
		}

		fn value(&mut self, value: &str) -> CommandResult<()> {
			self.values.push(value.to_string());
			Ok(())
		}

		fn warning(&mut self, value: &str) -> CommandResult<()> {
			self.warnings.push(value.to_string());
			Ok(())
		}
	}

	struct SharedOutput(Arc<Mutex<FakeOutput>>);

	impl ShellOutput for SharedOutput {
		fn stdout(&mut self, value: &str) -> CommandResult<()> {
			self.0.lock().expect("shared output lock").stdout(value)
		}

		fn stderr(&mut self, value: &str) -> CommandResult<()> {
			self.0.lock().expect("shared output lock").stderr(value)
		}

		fn value(&mut self, value: &str) -> CommandResult<()> {
			self.0.lock().expect("shared output lock").value(value)
		}

		fn warning(&mut self, value: &str) -> CommandResult<()> {
			self.0.lock().expect("shared output lock").warning(value)
		}
	}

	struct FakeInput {
		events: VecDeque<CommandResult<InputEvent>>,
	}

	impl ShellInput for FakeInput {
		fn read(&mut self) -> CommandResult<InputEvent> {
			self.events.pop_front().unwrap_or(Ok(InputEvent::Eof))
		}
	}

	struct NeverInterrupt;

	#[async_trait]
	impl InterruptSignal for NeverInterrupt {
		async fn wait(&mut self) -> std::io::Result<()> {
			let pending: Pending<std::io::Result<()>> = pending();
			pending.await
		}
	}

	struct InterruptNow;

	#[async_trait]
	impl InterruptSignal for InterruptNow {
		async fn wait(&mut self) -> std::io::Result<()> {
			Ok(())
		}
	}

	struct InterruptListenerError;

	#[async_trait]
	impl InterruptSignal for InterruptListenerError {
		async fn wait(&mut self) -> std::io::Result<()> {
			Err(std::io::Error::other("signal listener failed"))
		}
	}

	fn output(stdout: &str, stderr: &str, value: Option<&str>) -> EvaluationOutput {
		EvaluationOutput {
			stdout: stdout.to_string(),
			stderr: stderr.to_string(),
			value: value.map(str::to_string),
		}
	}

	fn outcomes(
		values: impl IntoIterator<Item = Result<EvaluationOutput, EvaluationFailure>>,
	) -> CommandResult<VecDeque<Result<EvaluationOutput, EvaluationFailure>>> {
		Ok(values.into_iter().collect())
	}

	#[tokio::test]
	async fn execute_once_uses_one_evaluation_and_forwards_visible_output() {
		let (factory, state) =
			FakeFactory::new([outcomes([Ok(output("hello\n", "warning\n", Some("42")))])]);
		let mut session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");

		session
			.execute_once("credential = \"do-not-echo\"")
			.await
			.expect("one-shot evaluation should succeed");

		let state = state.lock().expect("fake state lock");
		assert_eq!(state.starts, 1);
		assert_eq!(state.sources, ["credential = \"do-not-echo\""]);
		assert_eq!(session.output().stdout, ["hello\n"]);
		assert_eq!(session.output().stderr, ["warning\n"]);
		assert_eq!(session.output().values, ["42"]);
		assert_eq!(session.output().warnings, Vec::<String>::new());
	}

	#[test]
	fn startup_warnings_are_forwarded_before_the_first_input() {
		let factory = WarningFactory {
			state: Arc::new(Mutex::new(FakeState::default())),
			warnings: vec!["InventoryItem requires a qualified import.".to_string()],
		};

		let session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");

		assert_eq!(
			session.output().warnings,
			["InventoryItem requires a qualified import."]
		);
	}

	#[test]
	fn failed_startup_forwards_captured_output_before_returning_its_error() {
		let shared = Arc::new(Mutex::new(FakeOutput::default()));
		let output = SharedOutput(Arc::clone(&shared));
		let factory = FailedStartupFactory {
			output: vec![output(
				"prelude output\n",
				"prelude diagnostic\n",
				Some("42"),
			)],
		};

		let error = match ShellSession::with_signal(factory, output, NeverInterrupt) {
			Ok(_) => panic!("failed bootstrap should return its error"),
			Err(error) => error,
		};

		assert_eq!(error.to_string(), "Execution error: bootstrap failed");
		let output = shared.lock().expect("shared output lock");
		assert_eq!(output.stdout, ["prelude output\n"]);
		assert_eq!(output.stderr, ["prelude diagnostic\n"]);
		assert_eq!(output.values, ["42"]);
	}

	#[tokio::test]
	async fn execute_once_returns_nonfatal_error_without_replacing_context_or_echoing_source() {
		let secret_source = "let token = \"credential-sentinel\";";
		let (factory, state) = FakeFactory::new([outcomes([Err(EvaluationFailure::Compilation(
			"expected expression".to_string(),
		))])]);
		let mut session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");

		let error = session
			.execute_once(secret_source)
			.await
			.expect_err("compilation should fail command mode");

		let rendered = error.to_string();
		assert!(rendered.contains("expected expression"));
		assert!(!rendered.contains(secret_source));
		assert!(!rendered.contains("credential-sentinel"));
		assert_eq!(state.lock().expect("fake state lock").starts, 1);
	}

	#[tokio::test]
	async fn execute_once_forwards_output_captured_before_a_failure() {
		let failure = EvaluationFailure::Output {
			failure: Box::new(EvaluationFailure::Runtime("query failed".to_string())),
			output: output("before failure\n", "diagnostic\n", None),
		};
		let (factory, _) = FakeFactory::new([outcomes([Err(failure)])]);
		let mut session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");

		let error = session
			.execute_once("println!(\"before failure\"); query()?;")
			.await
			.expect_err("evaluation should fail after forwarding output");

		assert_eq!(error.to_string(), "Execution error: query failed");
		assert_eq!(session.output().stdout, ["before failure\n"]);
		assert_eq!(session.output().stderr, ["diagnostic\n"]);
	}

	#[tokio::test]
	async fn interactive_nonfatal_failure_preserves_context_and_fatal_failure_replaces_it() {
		let first = outcomes([
			Ok(output("", "", Some("1"))),
			Err(EvaluationFailure::Runtime("query failed".to_string())),
			Err(EvaluationFailure::Panic("child panicked".to_string())),
		]);
		let second = outcomes([Ok(output("", "", Some("2")))]);
		let (factory, state) = FakeFactory::new([first, second]);
		let mut session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");
		let mut input = FakeInput {
			events: [
				Ok(InputEvent::Source("let retained = 1;".to_string())),
				Ok(InputEvent::Source("bad_query()?".to_string())),
				Ok(InputEvent::Source("panic!()".to_string())),
				Ok(InputEvent::Source("1 + 1".to_string())),
				Ok(InputEvent::Eof),
			]
			.into_iter()
			.collect(),
		};

		session
			.run_interactive(&mut input)
			.await
			.expect("interactive recovery should succeed");

		let state = state.lock().expect("fake state lock");
		assert_eq!(state.starts, 2);
		assert_eq!(
			state.sources,
			["let retained = 1;", "bad_query()?", "panic!()", "1 + 1"]
		);
		assert_eq!(session.output().values, ["1", "2"]);
		assert_eq!(
			session.output().warnings,
			[
				"query failed",
				"child panicked",
				"Shell state was reset and the project prelude was reloaded."
			]
		);
	}

	#[tokio::test]
	async fn interactive_context_reset_failure_replaces_the_evaluator() {
		let first = outcomes([Err(EvaluationFailure::ContextReset(
			"evaluation changed stored variable types: value".to_string(),
		))]);
		let second = outcomes([Ok(output("", "", Some("2")))]);
		let (factory, state) = FakeFactory::new([first, second]);
		let mut session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");
		let mut input = FakeInput {
			events: [
				Ok(InputEvent::Source("let value = 1;".to_string())),
				Ok(InputEvent::Source("1 + 1".to_string())),
				Ok(InputEvent::Eof),
			]
			.into_iter()
			.collect(),
		};

		session
			.run_interactive(&mut input)
			.await
			.expect("context reset should recover the session");

		let state = state.lock().expect("fake state lock");
		assert_eq!(state.starts, 2);
		assert_eq!(state.sources, ["let value = 1;", "1 + 1"]);
		assert_eq!(session.output().values, ["2"]);
		assert_eq!(
			session.output().warnings,
			[
				"evaluation changed stored variable types: value",
				"Shell state was reset and the project prelude was reloaded."
			]
		);
	}

	#[tokio::test]
	async fn failed_fatal_recovery_terminates_interactive_session() {
		let (factory, state) = FakeFactory::new([
			outcomes([Err(EvaluationFailure::ProcessExited(
				"child exited".to_string(),
			))]),
			Err(CommandError::ExecutionError(
				"replacement bootstrap failed".to_string(),
			)),
		]);
		let mut session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");
		let mut input = FakeInput {
			events: [Ok(InputEvent::Source("std::process::exit(1)".to_string()))]
				.into_iter()
				.collect(),
		};

		let error = session
			.run_interactive(&mut input)
			.await
			.expect_err("failed replacement should end the session");

		assert_eq!(
			error.to_string(),
			"Execution error: replacement bootstrap failed"
		);
		assert_eq!(state.lock().expect("fake state lock").starts, 2);
	}

	struct PendingClient {
		state: Arc<Mutex<FakeState>>,
	}

	#[async_trait]
	impl EvaluatorClient for PendingClient {
		async fn evaluate(&mut self, _source: &str) -> Result<EvaluationOutput, EvaluationFailure> {
			pending().await
		}

		fn interrupt(&self) -> EvaluationInterrupt {
			let state = Arc::clone(&self.state);
			EvaluationInterrupt::new(move || {
				state.lock().expect("fake state lock").interrupts += 1;
				Ok(())
			})
		}
	}

	struct InterruptFactory {
		state: Arc<Mutex<FakeState>>,
		replacement: Option<Box<dyn EvaluatorClient>>,
	}

	impl EvaluatorFactory for InterruptFactory {
		fn start(&mut self) -> CommandResult<Box<dyn EvaluatorClient>> {
			let mut state = self.state.lock().expect("fake state lock");
			state.starts += 1;
			drop(state);
			if self.replacement.is_some() {
				return Ok(Box::new(PendingClient {
					state: Arc::clone(&self.state),
				}));
			}
			self.replacement
				.take()
				.ok_or_else(|| CommandError::ExecutionError("missing replacement".to_string()))
		}
	}

	#[tokio::test]
	async fn evaluation_interrupt_kills_running_context_and_replaces_it() {
		let state = Arc::new(Mutex::new(FakeState::default()));
		let factory = InterruptFactory {
			state: Arc::clone(&state),
			replacement: Some(Box::new(PendingClient {
				state: Arc::clone(&state),
			})),
		};
		let mut session = ShellSession::from_client_with_signal(
			factory,
			Box::new(PendingClient {
				state: Arc::clone(&state),
			}),
			FakeOutput::default(),
			InterruptNow,
		);
		let mut input = FakeInput {
			events: [
				Ok(InputEvent::Source(
					"tokio::time::sleep(std::time::Duration::MAX).await".to_string(),
				)),
				Ok(InputEvent::Eof),
			]
			.into_iter()
			.collect(),
		};

		session
			.run_interactive(&mut input)
			.await
			.expect("interrupt recovery should succeed");

		let state = state.lock().expect("fake state lock");
		assert_eq!(state.interrupts, 1);
		assert_eq!(state.starts, 1);
		assert_eq!(
			session.output().warnings,
			[
				"Evaluation was interrupted.",
				"Shell state was reset and the project prelude was reloaded."
			]
		);
	}

	#[tokio::test]
	async fn input_interrupt_clears_editing_state_and_eof_exits_successfully() {
		let (factory, state) = FakeFactory::new([outcomes([])]);
		let mut session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");
		let mut input = FakeInput {
			events: [Ok(InputEvent::Interrupted), Ok(InputEvent::Eof)]
				.into_iter()
				.collect(),
		};

		session
			.run_interactive(&mut input)
			.await
			.expect("EOF should exit successfully");

		assert_eq!(
			state.lock().expect("fake state lock").sources,
			Vec::<String>::new()
		);
		assert_eq!(session.output().warnings, ["Current input was discarded."]);
	}

	#[tokio::test]
	async fn pending_eof_warns_once_and_exits_successfully() {
		let (factory, _) = FakeFactory::new([outcomes([])]);
		let mut session = ShellSession::with_signal(factory, FakeOutput::default(), NeverInterrupt)
			.expect("session should start");
		let mut input = FakeInput {
			events: [Ok(InputEvent::EofWithPending)].into_iter().collect(),
		};

		session
			.run_interactive(&mut input)
			.await
			.expect("pending EOF should exit successfully");

		assert_eq!(
			session.output().warnings,
			["Pending input was discarded at end of file."]
		);
	}

	#[tokio::test]
	async fn signal_listener_failure_interrupts_running_evaluation_before_recovery() {
		let state = Arc::new(Mutex::new(FakeState::default()));
		let factory = InterruptFactory {
			state: Arc::clone(&state),
			replacement: Some(Box::new(PendingClient {
				state: Arc::clone(&state),
			})),
		};
		let mut session = ShellSession::from_client_with_signal(
			factory,
			Box::new(PendingClient {
				state: Arc::clone(&state),
			}),
			FakeOutput::default(),
			InterruptListenerError,
		);
		let mut input = FakeInput {
			events: [
				Ok(InputEvent::Source("long_running()".to_string())),
				Ok(InputEvent::Eof),
			]
			.into_iter()
			.collect(),
		};

		session
			.run_interactive(&mut input)
			.await
			.expect("fatal signal failure should recover before EOF");

		let state = state.lock().expect("fake state lock");
		assert_eq!(state.interrupts, 1);
		assert_eq!(state.starts, 1);
		assert_eq!(
			session.output().warnings,
			[
				"failed to listen for evaluation interruption: signal listener failed",
				"Shell state was reset and the project prelude was reloaded."
			]
		);
	}
}
