use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use rustyline::error::ReadlineError;
use rustyline::history::{DefaultHistory, History};
use rustyline::{Config, Editor};

use super::session::{InputEvent, ShellInput};
use crate::{CommandError, CommandResult};

pub(crate) const PRIMARY_PROMPT: &str = ">>> ";
pub(crate) const CONTINUATION_PROMPT: &str = "... ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BracketStatus {
	Complete,
	Incomplete,
	Invalid(String),
}

pub(crate) fn bracket_status(source: &str) -> BracketStatus {
	enum LexicalState {
		Code,
		LineComment,
		BlockComment(usize),
		String,
		RawString(usize),
	}

	let bytes = source.as_bytes();
	let mut stack = Vec::new();
	let mut state = LexicalState::Code;
	let mut index = 0;
	while index < bytes.len() {
		match state {
			LexicalState::LineComment => {
				if bytes[index] == b'\n' {
					state = LexicalState::Code;
				}
				index += 1;
				continue;
			}
			LexicalState::BlockComment(depth) => {
				if bytes[index..].starts_with(b"/*") {
					state = LexicalState::BlockComment(depth + 1);
					index += 2;
				} else if bytes[index..].starts_with(b"*/") {
					state = if depth == 1 {
						LexicalState::Code
					} else {
						LexicalState::BlockComment(depth - 1)
					};
					index += 2;
				} else {
					index += 1;
				}
				continue;
			}
			LexicalState::String => {
				match bytes[index] {
					b'\\' => index = (index + 2).min(bytes.len()),
					b'"' => {
						state = LexicalState::Code;
						index += 1;
					}
					_ => index += 1,
				}
				continue;
			}
			LexicalState::RawString(hash_count) => {
				if raw_string_closes_at(bytes, index, hash_count) {
					state = LexicalState::Code;
					index += hash_count + 1;
				} else {
					index += 1;
				}
				continue;
			}
			LexicalState::Code => {}
		}

		if bytes[index..].starts_with(b"//") {
			state = LexicalState::LineComment;
			index += 2;
			continue;
		}
		if bytes[index..].starts_with(b"/*") {
			state = LexicalState::BlockComment(1);
			index += 2;
			continue;
		}
		if let Some((hash_count, content_start)) = raw_string_start(bytes, index) {
			state = LexicalState::RawString(hash_count);
			index = content_start;
			continue;
		}
		if bytes[index] == b'"' {
			state = LexicalState::String;
			index += 1;
			continue;
		}
		if bytes[index] == b'\''
			&& let Some(after_literal) = char_literal_end(source, index)
		{
			index = after_literal;
			continue;
		}

		let character = bytes[index] as char;
		match character {
			'(' | '[' | '{' => stack.push(character),
			')' | ']' | '}' => match (stack.pop(), character) {
				(Some('('), ')') | (Some('['), ']') | (Some('{'), '}') => {}
				(Some(expected), _) => {
					return BracketStatus::Invalid(format!(
						"Mismatched brackets: {expected:?} is not properly closed"
					));
				}
				(None, closing) => {
					return BracketStatus::Invalid(format!(
						"Mismatched brackets: {closing:?} is unpaired"
					));
				}
			},
			_ => {}
		}
		index += 1;
	}

	if !matches!(state, LexicalState::Code | LexicalState::LineComment) || !stack.is_empty() {
		BracketStatus::Incomplete
	} else {
		BracketStatus::Complete
	}
}

fn raw_string_start(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
	if index > 0 && is_identifier_byte(bytes[index - 1]) {
		return None;
	}
	let r_index = match bytes.get(index..) {
		Some([b'r', ..]) => index,
		Some([b'b' | b'c', b'r', ..]) => index + 1,
		_ => return None,
	};
	let mut quote_index = r_index + 1;
	while bytes.get(quote_index) == Some(&b'#') {
		quote_index += 1;
	}
	(bytes.get(quote_index) == Some(&b'"')).then_some((quote_index - r_index - 1, quote_index + 1))
}

fn raw_string_closes_at(bytes: &[u8], index: usize, hash_count: usize) -> bool {
	bytes.get(index) == Some(&b'"')
		&& bytes
			.get(index + 1..index + 1 + hash_count)
			.is_some_and(|hashes| hashes.iter().all(|byte| *byte == b'#'))
}

fn is_identifier_byte(byte: u8) -> bool {
	byte.is_ascii_alphanumeric() || byte == b'_'
}

fn char_literal_end(source: &str, quote_index: usize) -> Option<usize> {
	let bytes = source.as_bytes();
	let mut index = quote_index + 1;
	match *bytes.get(index)? {
		b'\n' | b'\r' | b'\'' => return None,
		b'\\' => {
			index += 1;
			match *bytes.get(index)? {
				b'x' => index += 3,
				b'u' if bytes.get(index + 1) == Some(&b'{') => {
					index += 2;
					while bytes.get(index).is_some_and(|byte| *byte != b'}') {
						index += 1;
					}
					index += 1;
				}
				_ => index += 1,
			}
		}
		_ => {
			let character = source.get(index..)?.chars().next()?;
			index += character.len_utf8();
		}
	}
	(bytes.get(index) == Some(&b'\'')).then_some(index + 1)
}

pub(crate) trait LineReader {
	fn read_line(&mut self, prompt: &str) -> rustyline::Result<String>;

	fn add_history(&mut self, _source: &str) -> rustyline::Result<()> {
		Ok(())
	}

	fn save_history(&mut self, _path: &Path) -> rustyline::Result<()> {
		Ok(())
	}
}

pub(crate) struct RustylineLineReader {
	editor: Editor<(), DefaultHistory>,
}

impl LineReader for RustylineLineReader {
	fn read_line(&mut self, prompt: &str) -> rustyline::Result<String> {
		self.editor.readline(prompt)
	}

	fn add_history(&mut self, source: &str) -> rustyline::Result<()> {
		self.editor.add_history_entry(source).map(|_| ())
	}

	fn save_history(&mut self, path: &Path) -> rustyline::Result<()> {
		self.editor.save_history(path)
	}
}

pub(crate) struct TerminalInput<R = RustylineLineReader> {
	reader: R,
	history_path: Option<PathBuf>,
	warnings: VecDeque<String>,
	pending_source: String,
	completed_source: Option<String>,
}

impl TerminalInput<RustylineLineReader> {
	pub(crate) fn new(project_identifier: &str) -> CommandResult<Self> {
		let config = Config::builder().auto_add_history(false).build();
		let mut editor = Editor::with_config(config).map_err(readline_error)?;
		let mut warnings = VecDeque::new();
		let history_path =
			dirs::data_local_dir().map(|directory| history_path_in(&directory, project_identifier));
		let history_path = match history_path {
			Some(path) => {
				if let Some(parent) = path.parent()
					&& let Err(error) = std::fs::create_dir_all(parent)
				{
					warnings.push_back(format!("Could not prepare shell history: {error}"));
					None
				} else {
					if let Some(warning) = load_history_best_effort(editor.history_mut(), &path) {
						warnings.push_back(warning);
					}
					Some(path)
				}
			}
			None => {
				warnings.push_back(
					"Could not determine the platform data directory; shell history is disabled."
						.to_string(),
				);
				None
			}
		};
		Ok(Self {
			reader: RustylineLineReader { editor },
			history_path,
			warnings,
			pending_source: String::new(),
			completed_source: None,
		})
	}
}

impl<R> TerminalInput<R>
where
	R: LineReader,
{
	#[cfg(test)]
	fn with_reader(reader: R, history_path: Option<PathBuf>) -> Self {
		Self {
			reader,
			history_path,
			warnings: VecDeque::new(),
			pending_source: String::new(),
			completed_source: None,
		}
	}

	fn finish_source(&mut self) -> InputEvent {
		let source = std::mem::take(&mut self.pending_source);
		if !source.trim().is_empty() {
			if let Err(error) = self.reader.add_history(&source) {
				self.warnings
					.push_back(format!("Could not update shell history: {error}"));
			} else if let Some(path) = &self.history_path
				&& let Err(error) = self.reader.save_history(path)
			{
				self.warnings
					.push_back(format!("Could not save shell history: {error}"));
			}
		}
		if let Some(warning) = self.warnings.pop_front() {
			self.completed_source = Some(source);
			InputEvent::Warning(warning)
		} else {
			InputEvent::Source(source)
		}
	}
}

impl<R> ShellInput for TerminalInput<R>
where
	R: LineReader,
{
	fn read(&mut self) -> CommandResult<InputEvent> {
		if let Some(warning) = self.warnings.pop_front() {
			return Ok(InputEvent::Warning(warning));
		}
		if let Some(source) = self.completed_source.take() {
			return Ok(InputEvent::Source(source));
		}
		loop {
			let prompt = if self.pending_source.is_empty() {
				PRIMARY_PROMPT
			} else {
				CONTINUATION_PROMPT
			};
			match self.reader.read_line(prompt) {
				Ok(line) => {
					if !self.pending_source.is_empty() {
						self.pending_source.push('\n');
					}
					self.pending_source.push_str(&line);
					match bracket_status(&self.pending_source) {
						BracketStatus::Complete => return Ok(self.finish_source()),
						BracketStatus::Incomplete => {}
						BracketStatus::Invalid(message) => {
							self.pending_source.clear();
							return Ok(InputEvent::Warning(message));
						}
					}
				}
				Err(ReadlineError::Interrupted) => {
					self.pending_source.clear();
					return Ok(InputEvent::Interrupted);
				}
				Err(ReadlineError::Eof) if self.pending_source.is_empty() => {
					return Ok(InputEvent::Eof);
				}
				Err(ReadlineError::Eof) => {
					self.pending_source.clear();
					return Ok(InputEvent::EofWithPending);
				}
				Err(error) => return Err(readline_error(error)),
			}
		}
	}
}

fn readline_error(error: ReadlineError) -> CommandError {
	CommandError::ExecutionError(format!("shell terminal error: {error}"))
}

pub(crate) fn history_path_in(data_dir: &Path, project_identifier: &str) -> PathBuf {
	let project_identifier = project_identifier
		.chars()
		.map(|character| {
			if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
				character
			} else {
				'-'
			}
		})
		.collect::<String>();
	let project_identifier = project_identifier.trim_matches('-');
	let project_identifier = if project_identifier.is_empty() {
		"project"
	} else {
		project_identifier
	};
	data_dir
		.join("reinhardt")
		.join("shell")
		.join(format!("{project_identifier}.history"))
}

pub(crate) fn load_history_best_effort<H>(history: &mut H, path: &Path) -> Option<String>
where
	H: History,
{
	match history.load(path) {
		Ok(()) => None,
		Err(ReadlineError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => None,
		Err(error) => Some(format!("Could not load shell history: {error}")),
	}
}

#[cfg(test)]
mod tests {
	use std::collections::VecDeque;
	use std::path::Path;

	use rustyline::error::ReadlineError;
	use rustyline::history::DefaultHistory;
	use tempfile::tempdir;

	use super::{
		BracketStatus, LineReader, TerminalInput, bracket_status, history_path_in,
		load_history_best_effort,
	};
	use crate::shell::session::{InputEvent, ShellInput};

	struct FakeLineReader {
		lines: VecDeque<rustyline::Result<String>>,
		prompts: Vec<String>,
		fail_add_history: bool,
		fail_save_history: bool,
	}

	impl LineReader for FakeLineReader {
		fn read_line(&mut self, prompt: &str) -> rustyline::Result<String> {
			self.prompts.push(prompt.to_string());
			self.lines
				.pop_front()
				.expect("fake line reader should have an event")
		}

		fn add_history(&mut self, _source: &str) -> rustyline::Result<()> {
			if self.fail_add_history {
				Err(ReadlineError::Io(std::io::Error::other(
					"history update failed",
				)))
			} else {
				Ok(())
			}
		}

		fn save_history(&mut self, _path: &Path) -> rustyline::Result<()> {
			if self.fail_save_history {
				Err(ReadlineError::Io(std::io::Error::other(
					"history save failed",
				)))
			} else {
				Ok(())
			}
		}
	}

	#[test]
	fn balanced_and_unclosed_brackets_have_distinct_completion_states() {
		assert_eq!(bracket_status("let x = (1 + 2);"), BracketStatus::Complete);
		assert_eq!(bracket_status("fn sample() {"), BracketStatus::Incomplete);
		assert_eq!(
			bracket_status("let values = [1, 2"),
			BracketStatus::Incomplete
		);
		assert_eq!(bracket_status("call("), BracketStatus::Incomplete);
	}

	#[test]
	fn mismatched_closing_brackets_are_invalid() {
		assert_eq!(
			bracket_status("let value = (1 + 2];"),
			BracketStatus::Invalid("Mismatched brackets: '(' is not properly closed".to_string())
		);
		assert_eq!(
			bracket_status("}"),
			BracketStatus::Invalid("Mismatched brackets: '}' is unpaired".to_string())
		);
	}

	#[test]
	fn delimiters_inside_rust_literals_and_comments_do_not_extend_input() {
		for source in [
			r#"println!("(");"#,
			r#"let value = "escaped quote: \" and bracket (";"#,
			concat!("let value = r", "###", "\"{[( still text\"", "###", ";"),
			r#"let value = '(';"#,
			"let value = '\\'';",
			"// {",
			"/* [ */ let value = 42;",
			"/* outer { /* inner [ */ still comment } */",
		] {
			assert_eq!(
				bracket_status(source),
				BracketStatus::Complete,
				"source should be complete: {source}"
			);
		}
	}

	#[test]
	fn unterminated_rust_literals_and_block_comments_request_continuation() {
		for source in [
			r#"println!("unterminated"#,
			concat!("let value = r", "#", "\"unterminated"),
			"/* unterminated",
			"/* outer /* inner */",
		] {
			assert_eq!(
				bracket_status(source),
				BracketStatus::Incomplete,
				"source should request continuation: {source}"
			);
		}
	}

	#[test]
	fn multiline_input_uses_secondary_prompt_and_returns_one_complete_source() {
		let reader = FakeLineReader {
			lines: ["fn answer() {", "    42", "}"]
				.into_iter()
				.map(|line| Ok(line.to_string()))
				.collect(),
			prompts: Vec::new(),
			fail_add_history: false,
			fail_save_history: false,
		};
		let mut terminal = TerminalInput::with_reader(reader, None);

		let event = terminal.read().expect("multiline input should complete");

		assert_eq!(
			event,
			InputEvent::Source("fn answer() {\n    42\n}".to_string())
		);
		assert_eq!(terminal.reader.prompts, [">>> ", "... ", "... "]);
	}

	#[test]
	fn editing_interrupt_clears_pending_source_before_primary_prompt() {
		let reader = FakeLineReader {
			lines: [
				Ok("fn abandoned() {".to_string()),
				Err(ReadlineError::Interrupted),
				Ok("42".to_string()),
			]
			.into_iter()
			.collect(),
			prompts: Vec::new(),
			fail_add_history: false,
			fail_save_history: false,
		};
		let mut terminal = TerminalInput::with_reader(reader, None);

		assert_eq!(
			terminal.read().expect("interrupt should be recoverable"),
			InputEvent::Interrupted
		);
		assert_eq!(
			terminal.read().expect("next source should be complete"),
			InputEvent::Source("42".to_string())
		);
		assert_eq!(terminal.reader.prompts, [">>> ", "... ", ">>> "]);
	}

	#[test]
	fn eof_with_pending_source_reports_that_input_was_discarded() {
		let reader = FakeLineReader {
			lines: [Ok("fn abandoned() {".to_string()), Err(ReadlineError::Eof)]
				.into_iter()
				.collect(),
			prompts: Vec::new(),
			fail_add_history: false,
			fail_save_history: false,
		};
		let mut terminal = TerminalInput::with_reader(reader, None);

		assert_eq!(
			terminal.read().expect("pending EOF should be normal"),
			InputEvent::EofWithPending
		);
		assert_eq!(terminal.reader.prompts, [">>> ", "... "]);
	}

	#[test]
	fn history_path_is_project_specific_and_outside_the_project_tree() {
		let data_dir = Path::new("/user/data");

		assert_eq!(
			history_path_in(data_dir, "inventory-api"),
			Path::new("/user/data/reinhardt/shell/inventory-api.history")
		);
		assert_eq!(
			history_path_in(data_dir, "billing api"),
			Path::new("/user/data/reinhardt/shell/billing-api.history")
		);
	}

	#[test]
	fn history_load_failure_becomes_a_warning() {
		let directory = tempdir().expect("temporary history directory");
		let blocking_file = directory.path().join("not-a-directory");
		std::fs::write(&blocking_file, "occupied").expect("blocking file should be created");
		let history_path = blocking_file.join("project.history");
		let mut history = DefaultHistory::new();

		let load_warning = load_history_best_effort(&mut history, &history_path)
			.expect("load failure should become a warning");

		assert!(load_warning.starts_with("Could not load shell history:"));
	}

	#[test]
	fn history_save_failure_is_reported_before_the_completed_source() {
		let directory = tempdir().expect("temporary history directory");
		let history_path = directory.path().join("project.history");
		let reader = FakeLineReader {
			lines: [Ok("exit".to_string())].into_iter().collect(),
			prompts: Vec::new(),
			fail_add_history: false,
			fail_save_history: true,
		};
		let mut terminal = TerminalInput::with_reader(reader, Some(history_path));

		let warning = terminal
			.read()
			.expect("history save failure should remain recoverable");
		let source = terminal
			.read()
			.expect("completed source should follow its warning");

		assert_eq!(
			warning,
			InputEvent::Warning("Could not save shell history: history save failed".to_string())
		);
		assert_eq!(source, InputEvent::Source("exit".to_string()));
	}

	#[test]
	fn missing_history_is_a_normal_first_run() {
		let directory = tempdir().expect("temporary history directory");
		let mut history = DefaultHistory::new();

		assert_eq!(
			load_history_best_effort(&mut history, &directory.path().join("missing.history")),
			None
		);
	}
}
