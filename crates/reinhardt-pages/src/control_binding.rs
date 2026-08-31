//! Stable support types for controlled `page!` form elements.
//!
//! The `bind:` directive accepts [`Signal`](crate::reactive::Signal) values
//! directly for string-valued (`text`, `search`, `tel`, `url`, `email`,
//! `password`, `color`, `date`, `datetime-local`, `month`, `week`, and `time`),
//! numeric (`number` and `range`), checkbox, radio, and select controls. Numeric
//! controls can additionally report rejected input through [`NumberParseError`].
//! Binding lowering passes these `Copy` signal handles by value, so generated
//! call sites remain clean under Clippy's `clone_on_copy` lint.
//!
//! # Target parity
//!
//! This is a P2 API: the same support types and binding contract are available
//! for browser DOM controls, server rendering, and native component tests.

pub use reinhardt_core::types::page::{
	ControlBindingError, NumberParseError, NumberParseErrorKind, NumberValue,
};

#[cfg(any(wasm, test, feature = "testing"))]
pub(crate) fn is_text_input_type(input_type: &str) -> bool {
	[
		"text",
		"search",
		"tel",
		"url",
		"email",
		"password",
		"color",
		"date",
		"datetime-local",
		"month",
		"week",
		"time",
	]
	.iter()
	.any(|known| input_type.eq_ignore_ascii_case(known))
}

#[cfg(any(wasm, test, feature = "testing"))]
pub(crate) fn is_number_input_type(input_type: &str) -> bool {
	["number", "range"]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}
