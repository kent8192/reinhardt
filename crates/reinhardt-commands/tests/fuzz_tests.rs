//! Fuzz tests using proptest
//!
//! Tests that verify the robustness of command components
//! against random and edge-case inputs.

use proptest::prelude::*;
use reinhardt_commands::{
	CommandContext, CommandRegistry, TemplateContext, generate_secret_key, to_camel_case,
};

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]

	// =============================================================================
	// CommandContext Fuzz Tests
	// =============================================================================

	/// Fuzz CommandContext with random argument strings
	///
	/// **Category**: Fuzz
	/// **Verifies**: CommandContext handles arbitrary string arguments
	#[test]
	fn fuzz_context_args(args in prop::collection::vec(any::<String>(), 0..50)) {
		let ctx = CommandContext::new(args.clone());

		// Should not panic
		prop_assert_eq!(ctx.args.len(), args.len());

		// All args should be accessible
		for (i, arg) in args.iter().enumerate() {
			prop_assert_eq!(ctx.arg(i), Some(arg));
		}

		// Out of bounds should return None
		prop_assert_eq!(ctx.arg(args.len()), None);
	}

	/// Fuzz CommandContext with random option keys and values
	///
	/// **Category**: Fuzz
	/// **Verifies**: CommandContext handles arbitrary option data
	#[test]
	fn fuzz_context_options(
		options in prop::collection::hash_map("[a-z_-]{1,20}", any::<String>(), 0..20)
	) {
		let mut ctx = CommandContext::new(vec![]);

		// Set options
		for (k, v) in &options {
			ctx.set_option(k.clone(), v.clone());
		}

		// Should not panic
		// Verify all set options are accessible
		for (k, v) in &options {
			prop_assert!(ctx.has_option(k));
			prop_assert_eq!(ctx.option(k), Some(v));
		}
	}

	/// Fuzz CommandContext with random verbosity levels
	///
	/// **Category**: Fuzz
	/// **Verifies**: All verbosity values work correctly
	#[test]
	fn fuzz_context_verbosity(level in any::<u8>()) {
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_verbosity(level);

		prop_assert_eq!(ctx.verbosity(), level);
	}

	/// Fuzz CommandContext add_arg with random strings
	///
	/// **Category**: Fuzz
	/// **Verifies**: add_arg handles arbitrary strings
	#[test]
	fn fuzz_context_add_arg(initial_args in prop::collection::vec(any::<String>(), 0..10),
							new_args in prop::collection::vec(any::<String>(), 0..20)) {
		let mut ctx = CommandContext::new(initial_args.clone());

		let expected_len = initial_args.len() + new_args.len();

		for arg in &new_args {
			ctx.add_arg(arg.clone());
		}

		prop_assert_eq!(ctx.args.len(), expected_len);
	}

	// =============================================================================
	// TemplateContext Fuzz Tests
	// =============================================================================

	/// Fuzz TemplateContext with random string key-value pairs
	///
	/// **Category**: Fuzz
	/// **Verifies**: TemplateContext handles arbitrary string data
	#[test]
	fn fuzz_template_context_strings(
		pairs in prop::collection::vec(("[a-zA-Z_][a-zA-Z0-9_]*", any::<String>()), 0..20)
	) {
		let mut ctx = TemplateContext::new();

		for (key, value) in &pairs {
			let _ = ctx.insert(key.clone(), value.clone());
		}

		// Convert to tera context should not panic
		let tera_ctx: tera::Context = ctx.into();

		// Rendering should work
		let mut tera = tera::Tera::default();
		let result = tera.render_str("test", &tera_ctx);
		prop_assert!(result.is_ok());
	}

	/// Fuzz TemplateContext with random numeric values
	///
	/// **Category**: Fuzz
	/// **Verifies**: TemplateContext handles numeric values
	#[test]
	fn fuzz_template_context_numbers(
		int_pairs in prop::collection::vec(("[a-zA-Z_][a-zA-Z0-9_]*", any::<i64>()), 0..10),
		float_pairs in prop::collection::vec(("[a-zA-Z_][a-zA-Z0-9_]*", any::<f64>().prop_filter("finite", |f| f.is_finite())), 0..10)
	) {
		let mut ctx = TemplateContext::new();

		for (key, value) in &int_pairs {
			let _ = ctx.insert(key.clone(), *value);
		}
		for (key, value) in &float_pairs {
			let _ = ctx.insert(key.clone(), *value);
		}

		// Should not panic
		let _tera_ctx: tera::Context = ctx.into();
	}

	/// Fuzz TemplateContext with random boolean values
	///
	/// **Category**: Fuzz
	/// **Verifies**: TemplateContext handles boolean values
	#[test]
	fn fuzz_template_context_booleans(
		pairs in prop::collection::vec(("[a-zA-Z_][a-zA-Z0-9_]*", any::<bool>()), 0..20)
	) {
		let mut ctx = TemplateContext::new();

		for (key, value) in &pairs {
			let _ = ctx.insert(key.clone(), *value);
		}

		let _tera_ctx: tera::Context = ctx.into();
	}

	// =============================================================================
	// Utility Function Fuzz Tests
	// =============================================================================

	/// Fuzz generate_secret_key
	///
	/// **Category**: Fuzz
	/// **Verifies**: Secret key generation is always valid
	#[test]
	fn fuzz_generate_secret_key(_dummy in 0..100u32) {
		let key = generate_secret_key();

		// Always 50 characters
		prop_assert_eq!(key.len(), 50);

		// All characters are valid
		let valid_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+";
		for c in key.chars() {
			prop_assert!(valid_chars.contains(c), "Invalid character: {}", c);
		}
	}

	/// Fuzz to_camel_case with random inputs
	///
	/// **Category**: Fuzz
	/// **Verifies**: to_camel_case handles arbitrary strings
	#[test]
	fn fuzz_to_camel_case(input in ".*") {
		// Should not panic
		let result = to_camel_case(&input);

		// Result should be a valid string
		prop_assert!(result.len() <= input.len() * 2); // Rough upper bound
	}

	/// Fuzz to_camel_case with snake_case-like inputs
	///
	/// **Category**: Fuzz
	/// **Verifies**: to_camel_case handles snake_case patterns
	#[test]
	fn fuzz_to_camel_case_snake(
		parts in prop::collection::vec("[a-z]{1,10}", 1..5)
	) {
		let input = parts.join("_");
		let result = to_camel_case(&input);

		// Result should start with uppercase if input is not empty
		if !input.is_empty() && input.chars().next().unwrap().is_alphabetic() {
			let first_char = result.chars().next().unwrap();
			prop_assert!(first_char.is_uppercase() || !first_char.is_alphabetic());
		}
	}

	// =============================================================================
	// CommandRegistry Fuzz Tests
	// =============================================================================

	/// Fuzz CommandRegistry with random command names
	///
	/// **Category**: Fuzz
	/// **Verifies**: CommandRegistry handles various command names
	#[test]
	fn fuzz_registry_names(names in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_-]*", 0..20)) {
		use async_trait::async_trait;
		use reinhardt_commands::{BaseCommand, CommandResult};

		struct DummyCommand(String);

		#[async_trait]
		impl BaseCommand for DummyCommand {
			fn name(&self) -> &str { &self.0 }
			async fn execute(&self, _: &CommandContext) -> CommandResult<()> { Ok(()) }
		}

		let mut registry = CommandRegistry::new();

		for name in &names {
			registry.register(Box::new(DummyCommand(name.clone())));
		}

		// All unique names should be retrievable
		for name in names.iter().collect::<std::collections::HashSet<_>>() {
			prop_assert!(registry.get(name).is_some(), "Name '{}' should be retrievable", name);
		}
	}

	/// Fuzz CommandRegistry::list with many commands
	///
	/// **Category**: Fuzz
	/// **Verifies**: Registry handles many commands
	#[test]
	fn fuzz_registry_list(count in 0usize..100) {
		use async_trait::async_trait;
		use reinhardt_commands::{BaseCommand, CommandResult};

		struct NumberedCommand(usize);

		#[async_trait]
		impl BaseCommand for NumberedCommand {
			fn name(&self) -> &str {
				// Intentionally return static str based on number
				// This is a workaround for the test - in real code, name would be stored
				Box::leak(format!("cmd{}", self.0).into_boxed_str())
			}
			async fn execute(&self, _: &CommandContext) -> CommandResult<()> { Ok(()) }
		}

		let mut registry = CommandRegistry::new();

		for i in 0..count {
			registry.register(Box::new(NumberedCommand(i)));
		}

		let list = registry.list();
		prop_assert_eq!(list.len(), count);
	}
}

// =============================================================================
// Non-proptest Fuzz-like Tests
// =============================================================================

/// Test CommandContext with extreme Unicode
///
/// **Category**: Fuzz
/// **Verifies**: Extreme Unicode is handled
#[test]
fn test_context_extreme_unicode() {
	// RTL text, combining characters, ZWJ sequences
	let extreme_strings = vec![
		"مرحبا".to_string(),                     // Arabic
		"שָׁלוֹם".to_string(),                      // Hebrew with vowels
		"👨‍👩‍👧‍👦".to_string(),                        // Family emoji (ZWJ)
		"e\u{0301}".to_string(),                 // e + combining acute
		"\u{200B}invisible\u{200B}".to_string(), // Zero-width spaces
		"🏳️‍🌈".to_string(),                        // Rainbow flag (ZWJ)
	];

	let ctx = CommandContext::new(extreme_strings.clone());

	for (i, s) in extreme_strings.iter().enumerate() {
		assert_eq!(ctx.arg(i), Some(s), "Unicode arg {} should match", i);
	}
}

/// Test TemplateContext with edge-case keys
///
/// **Category**: Fuzz
/// **Verifies**: Edge-case keys are handled
#[test]
fn test_template_context_edge_keys() {
	let mut ctx = TemplateContext::new();

	// Valid Tera keys
	let _ = ctx.insert("a", "single char");
	let _ = ctx.insert("_underscore", "leading underscore");
	let _ = ctx.insert("CamelCase", "camel case");
	let _ = ctx.insert("UPPERCASE", "all caps");
	let _ = ctx.insert("with123numbers", "with numbers");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	// Each should be accessible
	assert!(tera.render_str("{{ a }}", &tera_ctx).is_ok());
	assert!(tera.render_str("{{ _underscore }}", &tera_ctx).is_ok());
	assert!(tera.render_str("{{ CamelCase }}", &tera_ctx).is_ok());
}
