//! Command execution context

use std::collections::HashMap;
use std::sync::Arc;

/// Execution context passed to management commands.
///
/// Contains command arguments, options, verbosity level, and optional
/// application settings needed during command execution.
#[derive(Debug, Clone)]
pub struct CommandContext {
	/// Positional arguments passed to the command.
	pub args: Vec<String>,
	/// Named options and their values (each option can have multiple values).
	pub options: HashMap<String, Vec<String>>,
	/// Verbosity level (0 = quiet, higher = more output).
	pub verbosity: u8,
	/// Optional reference to application settings
	#[allow(deprecated)] // Settings is deprecated; retained for backward compatibility
	pub settings: Option<Arc<reinhardt_conf::settings::Settings>>,
}

impl CommandContext {
	/// Creates a new command context with the given positional arguments.
	pub fn new(args: Vec<String>) -> Self {
		Self {
			args,
			options: HashMap::new(),
			verbosity: 0,
			settings: None,
		}
	}

	/// Replaces the positional arguments.
	pub fn with_args(mut self, args: Vec<String>) -> Self {
		self.args = args;
		self
	}

	/// Replaces the named options map.
	pub fn with_options(mut self, options: HashMap<String, Vec<String>>) -> Self {
		self.options = options;
		self
	}

	/// Set the application settings reference
	#[allow(deprecated)] // Settings is deprecated; retained for backward compatibility
	pub fn with_settings(mut self, settings: Arc<reinhardt_conf::settings::Settings>) -> Self {
		self.settings = Some(settings);
		self
	}

	/// Returns the positional argument at the given index, if present.
	pub fn arg(&self, index: usize) -> Option<&String> {
		self.args.get(index)
	}

	/// Returns the first value of a named option, if present.
	pub fn option(&self, key: &str) -> Option<&String> {
		self.options.get(key).and_then(|v| v.first())
	}

	/// Returns all values for a named option, if present.
	pub fn option_values(&self, key: &str) -> Option<Vec<String>> {
		self.options.get(key).cloned()
	}

	/// Returns whether a named option is present.
	pub fn has_option(&self, key: &str) -> bool {
		self.options.contains_key(key)
	}

	/// Prints an informational message to stdout.
	pub fn info(&self, message: &str) {
		println!("[INFO] {}", message);
	}

	/// Prints a success message to stdout.
	pub fn success(&self, message: &str) {
		println!("[SUCCESS] {}", message);
	}

	/// Prints a warning message to stderr.
	pub fn warning(&self, message: &str) {
		eprintln!("[WARNING] {}", message);
	}

	/// Prints a verbose message to stdout.
	pub fn verbose(&self, message: &str) {
		println!("[VERBOSE] {}", message);
	}

	/// Prints an error message to stderr.
	pub fn error(&self, message: &str) {
		eprintln!("[ERROR] {}", message);
	}

	/// Sets a single-value named option.
	pub fn set_option(&mut self, key: String, value: String) {
		self.options.insert(key, vec![value]);
	}

	/// Sets a multi-value named option.
	pub fn set_option_multi(&mut self, key: String, values: Vec<String>) {
		self.options.insert(key, values);
	}

	/// Check if system checks should be skipped
	///
	/// Returns true if either `skip_checks` or `skip-checks` option is present.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::CommandContext;
	///
	/// let mut ctx = CommandContext::new(vec![]);
	/// ctx.set_option("skip-checks".to_string(), "true".to_string());
	///
	/// assert!(ctx.should_skip_checks());
	/// ```
	pub fn should_skip_checks(&self) -> bool {
		self.has_option("skip_checks") || self.has_option("skip-checks")
	}

	/// Add an argument to the context
	pub fn add_arg(&mut self, arg: String) {
		self.args.push(arg);
	}

	/// Set the verbosity level
	pub fn set_verbosity(&mut self, level: u8) {
		self.verbosity = level;
	}

	/// Get the verbosity level
	pub fn verbosity(&self) -> u8 {
		self.verbosity
	}

	/// Prompt user for confirmation
	///
	/// Returns true if confirmed, false otherwise.
	/// If testing mode or --yes flag is set, returns default_value.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::CommandContext;
	///
	/// let mut ctx = CommandContext::new(vec![]);
	/// ctx.set_option("yes".to_string(), "true".to_string());
	///
	/// // With --yes flag, returns true without prompting
	/// let confirmed = ctx.confirm("Continue?", true).unwrap();
	/// assert!(confirmed);
	/// ```
	pub fn confirm(&self, prompt: &str, default_value: bool) -> Result<bool, std::io::Error> {
		// Auto-approve in test mode
		if cfg!(test) || std::env::var("REINHARDT_TEST_MODE").is_ok() {
			return Ok(default_value);
		}

		// Auto-approve if --yes flag is present
		if self.has_option("yes") {
			return Ok(true);
		}

		// Prompt using dialoguer
		Ok(dialoguer::Confirm::new()
			.with_prompt(prompt)
			.default(default_value)
			.interact()?)
	}

	/// Prompt user for text input
	///
	/// Returns user input or default_value if --yes flag is set.
	/// In test mode, returns default_value or empty string.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::CommandContext;
	///
	/// let mut ctx = CommandContext::new(vec![]);
	/// ctx.set_option("yes".to_string(), "true".to_string());
	///
	/// // With --yes flag, returns default value without prompting
	/// let input = ctx.input("Enter name:", Some("default")).unwrap();
	/// assert_eq!(input, "default");
	/// ```
	pub fn input(
		&self,
		prompt: &str,
		default_value: Option<&str>,
	) -> Result<String, std::io::Error> {
		// Return default value in test mode
		if cfg!(test) || std::env::var("REINHARDT_TEST_MODE").is_ok() {
			return Ok(default_value.unwrap_or("").to_string());
		}

		// Return default value if --yes flag is present
		if self.has_option("yes") {
			return Ok(default_value.unwrap_or("").to_string());
		}

		let mut builder = dialoguer::Input::<String>::new().with_prompt(prompt);

		if let Some(default) = default_value {
			builder = builder.default(default.to_string());
		}

		Ok(builder.interact()?)
	}
}

impl Default for CommandContext {
	fn default() -> Self {
		Self::new(Vec::new())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_command_context_new() {
		let ctx = CommandContext::new(vec!["arg1".to_string(), "arg2".to_string()]);

		assert_eq!(ctx.args.len(), 2);
		assert_eq!(ctx.args[0], "arg1");
		assert_eq!(ctx.args[1], "arg2");
		assert!(ctx.options.is_empty());
		assert_eq!(ctx.verbosity, 0);
		assert!(ctx.settings.is_none());
	}

	#[rstest]
	fn test_command_context_default() {
		let ctx = CommandContext::default();

		assert!(ctx.args.is_empty());
		assert!(ctx.options.is_empty());
		assert_eq!(ctx.verbosity, 0);
		assert!(ctx.settings.is_none());
	}

	#[rstest]
	fn test_command_context_with_args() {
		let ctx = CommandContext::new(vec![]).with_args(vec!["new_arg".to_string()]);

		assert_eq!(ctx.args.len(), 1);
		assert_eq!(ctx.args[0], "new_arg");
	}

	#[rstest]
	fn test_command_context_with_options() {
		let mut options = HashMap::new();
		options.insert("key".to_string(), vec!["value".to_string()]);

		let ctx = CommandContext::new(vec![]).with_options(options);

		assert!(ctx.has_option("key"));
		assert_eq!(ctx.option("key"), Some(&"value".to_string()));
	}

	#[rstest]
	#[case(0, Some("first"))]
	#[case(1, Some("second"))]
	#[case(2, Some("third"))]
	#[case(3, None)]
	fn test_command_context_arg_access(#[case] index: usize, #[case] expected: Option<&str>) {
		let ctx = CommandContext::new(vec![
			"first".to_string(),
			"second".to_string(),
			"third".to_string(),
		]);

		assert_eq!(ctx.arg(index).map(|s| s.as_str()), expected);
	}

	#[rstest]
	#[case("single", Some("value"))]
	#[case("nonexistent", None)]
	fn test_command_context_option_access(#[case] key: &str, #[case] expected: Option<&str>) {
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option("single".to_string(), "value".to_string());

		assert_eq!(ctx.option(key).map(|s| s.as_str()), expected);
	}

	#[rstest]
	fn test_command_context_option_values() {
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi(
			"multi".to_string(),
			vec!["val1".to_string(), "val2".to_string(), "val3".to_string()],
		);

		let values = ctx.option_values("multi");
		assert!(values.is_some());
		let values = values.unwrap();
		assert_eq!(values.len(), 3);
		assert_eq!(values[0], "val1");
		assert_eq!(values[1], "val2");
		assert_eq!(values[2], "val3");

		// First value should be returned by option()
		assert_eq!(ctx.option("multi"), Some(&"val1".to_string()));

		// Nonexistent key returns None
		assert_eq!(ctx.option_values("nonexistent"), None);
	}

	#[rstest]
	#[case("exists", true)]
	#[case("does_not_exist", false)]
	fn test_command_context_has_option(#[case] key: &str, #[case] expected: bool) {
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option("exists".to_string(), "value".to_string());

		assert_eq!(ctx.has_option(key), expected);
	}

	#[rstest]
	fn test_command_context_set_option() {
		let mut ctx = CommandContext::new(vec![]);

		ctx.set_option("key1".to_string(), "value1".to_string());
		assert_eq!(ctx.option("key1"), Some(&"value1".to_string()));

		// Setting same key overwrites
		ctx.set_option("key1".to_string(), "value2".to_string());
		assert_eq!(ctx.option("key1"), Some(&"value2".to_string()));
	}

	#[rstest]
	fn test_command_context_set_option_multi() {
		let mut ctx = CommandContext::new(vec![]);

		ctx.set_option_multi(
			"files".to_string(),
			vec!["file1.txt".to_string(), "file2.txt".to_string()],
		);

		let files = ctx.option_values("files").unwrap();
		assert_eq!(files.len(), 2);
		assert_eq!(files[0], "file1.txt");
		assert_eq!(files[1], "file2.txt");
	}

	#[rstest]
	#[case("skip_checks", true)]
	#[case("skip-checks", true)]
	fn test_command_context_should_skip_checks_variants(
		#[case] option_key: &str,
		#[case] expected: bool,
	) {
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option(option_key.to_string(), "true".to_string());

		assert_eq!(ctx.should_skip_checks(), expected);
	}

	#[rstest]
	fn test_command_context_should_skip_checks_false() {
		let ctx = CommandContext::new(vec![]);

		assert!(!ctx.should_skip_checks());
	}

	#[rstest]
	fn test_command_context_add_arg() {
		let mut ctx = CommandContext::new(vec!["initial".to_string()]);

		ctx.add_arg("added".to_string());

		assert_eq!(ctx.args.len(), 2);
		assert_eq!(ctx.arg(0), Some(&"initial".to_string()));
		assert_eq!(ctx.arg(1), Some(&"added".to_string()));
	}

	#[rstest]
	#[case(0)]
	#[case(1)]
	#[case(3)]
	#[case(255)]
	fn test_command_context_verbosity(#[case] level: u8) {
		let mut ctx = CommandContext::new(vec![]);

		assert_eq!(ctx.verbosity(), 0);

		ctx.set_verbosity(level);
		assert_eq!(ctx.verbosity(), level);
	}

	#[rstest]
	#[case(true, true)]
	#[case(false, false)]
	fn test_command_context_confirm_in_test_mode(
		#[case] default_value: bool,
		#[case] expected: bool,
	) {
		let ctx = CommandContext::new(vec![]);

		// In test mode, returns default_value
		let result = ctx.confirm("Continue?", default_value).unwrap();
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_command_context_confirm_with_yes_flag() {
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option("yes".to_string(), "true".to_string());

		// Note: In test mode, cfg!(test) check comes first
		// so this actually returns default_value in tests
		let result = ctx.confirm("Continue?", false).unwrap();
		// In test mode, returns default_value (false)
		assert!(!result);
	}

	#[rstest]
	#[case(Some("default_name"), "default_name")]
	#[case(None, "")]
	fn test_command_context_input_in_test_mode(
		#[case] default_value: Option<&str>,
		#[case] expected: &str,
	) {
		let ctx = CommandContext::new(vec![]);

		// In test mode, returns default_value
		let result = ctx.input("Enter name:", default_value).unwrap();
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_command_context_input_with_yes_flag() {
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option("yes".to_string(), "true".to_string());

		// Note: In test mode, cfg!(test) check comes first
		let result = ctx.input("Enter:", Some("auto")).unwrap();
		assert_eq!(result, "auto");
	}

	#[rstest]
	fn test_command_context_clone() {
		let mut ctx = CommandContext::new(vec!["arg".to_string()]);
		ctx.set_option("key".to_string(), "value".to_string());
		ctx.set_verbosity(2);

		let cloned = ctx.clone();

		assert_eq!(cloned.args, ctx.args);
		assert_eq!(cloned.options, ctx.options);
		assert_eq!(cloned.verbosity, ctx.verbosity);
	}

	#[rstest]
	fn test_command_context_debug() {
		let ctx = CommandContext::new(vec!["test".to_string()]);

		let debug_str = format!("{:?}", ctx);

		assert!(debug_str.contains("CommandContext"));
		assert!(debug_str.contains("test"));
	}

	#[rstest]
	fn test_command_context_builder_chain() {
		let mut options = HashMap::new();
		options.insert("format".to_string(), vec!["json".to_string()]);

		let ctx = CommandContext::new(vec![])
			.with_args(vec!["command".to_string(), "subcommand".to_string()])
			.with_options(options);

		assert_eq!(ctx.args.len(), 2);
		assert_eq!(ctx.arg(0), Some(&"command".to_string()));
		assert_eq!(ctx.option("format"), Some(&"json".to_string()));
	}
}
