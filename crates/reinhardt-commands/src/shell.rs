//! Project-facing configuration and runtime support for the Rust management shell.

mod config;
#[cfg(feature = "shell")]
mod environment;
#[cfg(feature = "shell")]
mod evaluator;
#[cfg(feature = "shell")]
mod imports;
#[cfg(feature = "shell")]
mod session;
#[cfg(feature = "shell")]
mod terminal;

pub use config::ShellConfig;
#[cfg(feature = "shell")]
pub use environment::ShellEnvironment;

#[cfg(feature = "shell")]
use evaluator::EvcxrEvaluatorFactory;
#[cfg(feature = "shell")]
use session::{EvaluatorFactory, ShellInput, ShellOutput, ShellSession};
#[cfg(feature = "shell")]
use terminal::TerminalInput;

#[cfg(feature = "shell")]
struct ConsoleOutput;

#[cfg(feature = "shell")]
impl ShellOutput for ConsoleOutput {
	fn stdout(&mut self, value: &str) -> crate::CommandResult<()> {
		write_stream(std::io::stdout(), value)
	}

	fn stderr(&mut self, value: &str) -> crate::CommandResult<()> {
		write_stream(std::io::stderr(), value)
	}

	fn value(&mut self, value: &str) -> crate::CommandResult<()> {
		write_stream(std::io::stdout(), &format!("{value}\n"))
	}

	fn warning(&mut self, value: &str) -> crate::CommandResult<()> {
		write_stream(std::io::stderr(), &format!("{value}\n"))
	}
}

#[cfg(feature = "shell")]
fn write_stream(mut stream: impl std::io::Write, value: &str) -> crate::CommandResult<()> {
	stream.write_all(value.as_bytes())?;
	stream.flush()?;
	Ok(())
}

#[cfg(feature = "shell")]
pub(crate) async fn run(config: &ShellConfig, command: Option<String>) -> crate::CommandResult<()> {
	let validated = config.validate()?;
	let project_identifier = validated.package_name().to_string();
	let factory = EvcxrEvaluatorFactory::new(validated);
	let output = ConsoleOutput;
	match command {
		Some(source) => {
			start_session(factory, output)
				.await?
				.execute_once(&source)
				.await
		}
		None => {
			let mut input = TerminalInput::new(&project_identifier)?;
			let mut session = start_session(factory, output).await?;
			session.run_interactive(&mut input).await
		}
	}
}

async fn start_session(
	factory: EvcxrEvaluatorFactory,
	output: ConsoleOutput,
) -> crate::CommandResult<ShellSession<EvcxrEvaluatorFactory, ConsoleOutput>> {
	let mut startup = tokio::task::spawn_blocking(move || ShellSession::new(factory, output));
	tokio::select! {
		result = &mut startup => result.map_err(|error| crate::CommandError::ExecutionError(error.to_string()))?,
		result = tokio::signal::ctrl_c() => {
			result.map_err(|error| crate::CommandError::ExecutionError(error.to_string()))?;
			if let Ok(Ok(session)) = startup.await {
				drop(session);
			}
			Err(crate::CommandError::ExecutionError("Shell startup was interrupted.".to_string()))
		}
	}
}

#[cfg(feature = "shell")]
async fn run_session<F, W, I>(
	command: Option<String>,
	factory: F,
	output: W,
	input: &mut I,
) -> crate::CommandResult<()>
where
	F: EvaluatorFactory,
	W: ShellOutput,
	I: ShellInput,
{
	let mut session = ShellSession::new(factory, output)?;
	match command {
		Some(source) => session.execute_once(&source).await,
		None => session.run_interactive(input).await,
	}
}

#[cfg(all(test, feature = "shell"))]
async fn run_with_components<F, W, I>(
	config: &ShellConfig,
	command: Option<String>,
	factory: F,
	output: W,
	input: &mut I,
) -> crate::CommandResult<()>
where
	F: EvaluatorFactory,
	W: ShellOutput,
	I: ShellInput,
{
	config.validate()?;
	run_session(command, factory, output, input).await
}

/// Installs the evcxr runtime entry point when shell support is enabled.
#[cfg(feature = "shell")]
pub fn shell_runtime_hook() {
	evcxr::runtime_hook();
}

/// Performs no runtime setup when shell support is disabled.
#[cfg(not(feature = "shell"))]
pub fn shell_runtime_hook() {}

#[cfg(all(test, feature = "shell"))]
mod tests {
	use std::collections::VecDeque;
	use std::sync::{Arc, Mutex};

	use async_trait::async_trait;
	use clap::Parser;
	use tempfile::tempdir;

	use super::config::ShellConfig;
	use super::evaluator::{EvaluationFailure, EvaluationOutput};
	use super::session::{
		EvaluationInterrupt, EvaluatorClient, EvaluatorFactory, InputEvent, ShellInput, ShellOutput,
	};
	use crate::{CommandError, CommandResult};

	#[test]
	fn raw_clap_parser_requires_driver_normalization_for_numeric_verbosity() {
		assert!(crate::cli::Cli::try_parse_from(["manage", "-v", "shell"]).is_ok());
		assert!(crate::cli::Cli::try_parse_from(["manage", "-vv", "shell"]).is_ok());
		assert!(crate::cli::Cli::try_parse_from(["manage", "--verbosity", "shell"]).is_ok());
		assert!(crate::cli::Cli::try_parse_from(["manage", "--verbose", "shell"]).is_err());
		assert!(crate::cli::Cli::try_parse_from(["manage", "--verbosity=2", "shell"]).is_err());
	}

	#[derive(Default)]
	struct DriverState {
		sources: Vec<String>,
		input_reads: usize,
	}

	struct FakeEvaluator {
		state: Arc<Mutex<DriverState>>,
		outcomes: VecDeque<Result<EvaluationOutput, EvaluationFailure>>,
	}

	#[async_trait]
	impl EvaluatorClient for FakeEvaluator {
		async fn evaluate(&mut self, source: &str) -> Result<EvaluationOutput, EvaluationFailure> {
			self.state
				.lock()
				.expect("driver state lock")
				.sources
				.push(source.to_string());
			self.outcomes.pop_front().expect("fake evaluator outcome")
		}

		fn interrupt(&self) -> EvaluationInterrupt {
			EvaluationInterrupt::new(|| Ok(()))
		}
	}

	struct FakeFactory {
		state: Arc<Mutex<DriverState>>,
		outcomes: Option<VecDeque<Result<EvaluationOutput, EvaluationFailure>>>,
	}

	impl EvaluatorFactory for FakeFactory {
		fn start(&mut self) -> CommandResult<Box<dyn EvaluatorClient>> {
			Ok(Box::new(FakeEvaluator {
				state: self.state.clone(),
				outcomes: self.outcomes.take().expect("factory starts once"),
			}))
		}
	}

	#[derive(Default)]
	struct FakeOutput;

	impl ShellOutput for FakeOutput {
		fn stdout(&mut self, _value: &str) -> CommandResult<()> {
			Ok(())
		}

		fn stderr(&mut self, _value: &str) -> CommandResult<()> {
			Ok(())
		}

		fn value(&mut self, _value: &str) -> CommandResult<()> {
			Ok(())
		}

		fn warning(&mut self, _value: &str) -> CommandResult<()> {
			Ok(())
		}
	}

	struct FakeInput {
		state: Arc<Mutex<DriverState>>,
		events: VecDeque<InputEvent>,
	}

	impl ShellInput for FakeInput {
		fn read(&mut self) -> CommandResult<InputEvent> {
			self.state.lock().expect("driver state lock").input_reads += 1;
			self.events
				.pop_front()
				.ok_or_else(|| CommandError::ExecutionError("fake input exhausted".to_string()))
		}
	}

	fn shell_config() -> (tempfile::TempDir, ShellConfig) {
		let directory = tempdir().expect("temporary shell project");
		std::fs::write(
			directory.path().join("Cargo.toml"),
			"[package]\nname = \"shell-probe\"\nversion = \"0.1.0\"\n",
		)
		.expect("write shell manifest");
		let config = ShellConfig::new(
			"shell-probe",
			"shell_probe",
			directory.path(),
			"shell_probe::config::get_settings",
			["probe"],
		);
		(directory, config)
	}

	fn fake_components(
		outcomes: impl IntoIterator<Item = Result<EvaluationOutput, EvaluationFailure>>,
		events: impl IntoIterator<Item = InputEvent>,
	) -> (FakeFactory, FakeOutput, FakeInput, Arc<Mutex<DriverState>>) {
		let state = Arc::new(Mutex::new(DriverState::default()));
		(
			FakeFactory {
				state: state.clone(),
				outcomes: Some(outcomes.into_iter().collect()),
			},
			FakeOutput,
			FakeInput {
				state: state.clone(),
				events: events.into_iter().collect(),
			},
			state,
		)
	}

	#[tokio::test]
	async fn one_shot_source_is_evaluated_without_reading_interactive_input() {
		let (_directory, config) = shell_config();
		let (factory, output, mut input, state) = fake_components(
			[Ok(EvaluationOutput {
				stdout: String::new(),
				stderr: String::new(),
				value: Some("42".to_string()),
			})],
			[],
		);

		super::run_with_components(
			&config,
			Some("let answer = 42;".to_string()),
			factory,
			output,
			&mut input,
		)
		.await
		.expect("one-shot shell should succeed");

		let state = state.lock().expect("driver state lock");
		assert_eq!(state.sources, ["let answer = 42;"]);
		assert_eq!(state.input_reads, 0);
	}

	#[tokio::test]
	async fn absent_command_runs_interactive_input_until_eof() {
		let (_directory, config) = shell_config();
		let (factory, output, mut input, state) = fake_components(
			[Ok(EvaluationOutput {
				stdout: String::new(),
				stderr: String::new(),
				value: Some("42".to_string()),
			})],
			[InputEvent::Source("40 + 2".to_string()), InputEvent::Eof],
		);

		super::run_with_components(&config, None, factory, output, &mut input)
			.await
			.expect("interactive shell should succeed");

		let state = state.lock().expect("driver state lock");
		assert_eq!(state.sources, ["40 + 2"]);
		assert_eq!(state.input_reads, 2);
	}

	#[tokio::test]
	async fn invalid_one_shot_source_returns_an_error() {
		let (_directory, config) = shell_config();
		let (factory, output, mut input, _state) = fake_components(
			[Err(EvaluationFailure::Compilation(
				"expected expression".to_string(),
			))],
			[],
		);

		let error = super::run_with_components(
			&config,
			Some("let =".to_string()),
			factory,
			output,
			&mut input,
		)
		.await
		.expect_err("invalid one-shot Rust must fail");

		assert_eq!(error.to_string(), "Execution error: expected expression");
	}
}
