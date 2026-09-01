//! Stable support types for controlled `page!` form elements.
//!
//! The `bind:` directive accepts [`Signal`](crate::reactive::Signal) values
//! directly for text-like (`text`, `search`, `tel`, `url`, `email`, `password`,
//! and `color`), numeric (`number` and `range`), checkbox, radio, and select
//! controls. Numeric controls can additionally report rejected input through
//! [`NumberParseError`].
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

pub(crate) const SSR_OMITTED_PASSWORD_ATTRIBUTE: &str = "data-rh-password-omitted";

#[cfg(any(wasm, test, feature = "testing"))]
pub(crate) fn is_text_input_type(input_type: &str) -> bool {
	["text", "search", "tel", "url", "email", "password", "color"]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}

#[cfg(any(wasm, test, feature = "testing"))]
pub(crate) fn is_number_input_type(input_type: &str) -> bool {
	["number", "range"]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}

#[cfg(any(wasm, test, feature = "testing"))]
pub(crate) fn range_constraints_conflict(
	first: (f64, f64, Option<f64>, f64),
	second: (f64, f64, Option<f64>, f64),
) -> bool {
	let (first_min, first_max, first_step, first_base) = first;
	let (second_min, second_max, second_step, second_base) = second;
	first_max < second_min
		|| second_max < first_min
		|| incompatible_range_step_grids(first_step, first_base, second_step, second_base)
}

#[cfg(any(wasm, test, feature = "testing"))]
fn incompatible_range_step_grids(
	first_step: Option<f64>,
	first_base: f64,
	second_step: Option<f64>,
	second_base: f64,
) -> bool {
	let (Some(first_step), Some(second_step)) = (first_step, second_step) else {
		return false;
	};
	let mut larger = first_step.max(second_step);
	let mut smaller = first_step.min(second_step);
	let tolerance = larger.max(1.0) * 1e-12;
	while smaller > tolerance {
		let remainder = larger % smaller;
		larger = smaller;
		smaller = remainder.abs();
	}
	let phase = (first_base - second_base) / larger;
	let phase_tolerance = phase.abs().max(1.0) * 1e-9;
	!phase.is_finite() || (phase - phase.round()).abs() > phase_tolerance
}
