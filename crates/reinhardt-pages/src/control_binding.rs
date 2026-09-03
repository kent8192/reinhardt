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

use reinhardt_core::types::page::ControlKind;
pub use reinhardt_core::types::page::{
	ControlBindingError, NumberParseError, NumberParseErrorKind, NumberValue,
};

pub(crate) const SSR_OMITTED_PASSWORD_ATTRIBUTE: &str = "data-rh-password-omitted";

#[cfg(any(wasm, all(native, feature = "testing")))]
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

#[cfg(all(native, feature = "testing"))]
pub(crate) fn is_effective_text_input_type(input_type: Option<&str>) -> bool {
	let Some(input_type) = input_type else {
		return true;
	};
	is_text_input_type(input_type)
		|| ![
			"button",
			"checkbox",
			"date",
			"datetime-local",
			"file",
			"hidden",
			"image",
			"month",
			"number",
			"radio",
			"range",
			"reset",
			"submit",
			"time",
			"week",
		]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}

#[cfg(any(wasm, all(native, feature = "testing")))]
pub(crate) fn is_number_input_type(input_type: &str) -> bool {
	["number", "range"]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}

pub(crate) fn controlled_attribute_update_is_supported(
	tag: &str,
	kind: ControlKind,
	name: &str,
	value: Option<&str>,
) -> bool {
	reinhardt_core::types::page::control_binding::controlled_attribute_update_is_supported(
		tag, kind, name, value,
	)
}

#[cfg(any(wasm, test, feature = "testing"))]
pub(crate) fn parse_html_number(value: &str) -> Option<f64> {
	let bytes = value.as_bytes();
	let mut index = usize::from(bytes.first() == Some(&b'-'));
	let integer_start = index;
	while bytes.get(index).is_some_and(u8::is_ascii_digit) {
		index += 1;
	}
	let has_integer = index > integer_start;
	let mut has_fraction = false;
	if bytes.get(index) == Some(&b'.') {
		index += 1;
		let fraction_start = index;
		while bytes.get(index).is_some_and(u8::is_ascii_digit) {
			index += 1;
		}
		has_fraction = index > fraction_start;
		if !has_fraction {
			return None;
		}
	}
	if !has_integer && !has_fraction {
		return None;
	}
	if bytes
		.get(index)
		.is_some_and(|byte| matches!(byte, b'e' | b'E'))
	{
		index += 1;
		if bytes
			.get(index)
			.is_some_and(|byte| matches!(byte, b'+' | b'-'))
		{
			index += 1;
		}
		let exponent_start = index;
		while bytes.get(index).is_some_and(u8::is_ascii_digit) {
			index += 1;
		}
		if index == exponent_start {
			return None;
		}
	}
	(index == bytes.len())
		.then(|| value.parse::<f64>().ok())
		.flatten()
		.filter(|value| value.is_finite())
}

#[cfg(any(wasm, test, feature = "testing"))]
pub(crate) fn range_constraints_conflict(
	first: (f64, f64, Option<f64>, f64),
	second: (f64, f64, Option<f64>, f64),
) -> bool {
	let (first_min, first_max, first_step, first_base) = first;
	let (second_min, second_max, second_step, second_base) = second;
	let overlap_min = first_min.max(second_min);
	let overlap_max = first_max.min(second_max);
	overlap_min > overlap_max
		|| !range_step_grids_have_common_value(
			overlap_min,
			overlap_max,
			first_step,
			first_base,
			second_step,
			second_base,
		)
}

#[cfg(any(wasm, test, feature = "testing"))]
fn range_step_grids_have_common_value(
	overlap_min: f64,
	overlap_max: f64,
	first_step: Option<f64>,
	first_base: f64,
	second_step: Option<f64>,
	second_base: f64,
) -> bool {
	let (first_step, second_step) = match (first_step, second_step) {
		(None, None) => return true,
		(Some(step), None) => {
			return range_step_grid_has_value(overlap_min, overlap_max, step, first_base);
		}
		(None, Some(step)) => {
			return range_step_grid_has_value(overlap_min, overlap_max, step, second_base);
		}
		(Some(first_step), Some(second_step)) => (first_step, second_step),
	};
	let tolerance = first_step.max(second_step) * 1e-12;
	let mut previous_remainder = first_step;
	let mut remainder = second_step;
	let mut previous_first_coefficient = 1.0;
	let mut first_coefficient = 0.0;
	while remainder.abs() > tolerance {
		let quotient = (previous_remainder / remainder).floor();
		(previous_remainder, remainder) = (remainder, previous_remainder - quotient * remainder);
		(previous_first_coefficient, first_coefficient) = (
			first_coefficient,
			previous_first_coefficient - quotient * first_coefficient,
		);
	}
	let greatest_common_step = previous_remainder.abs();
	if !greatest_common_step.is_finite() || greatest_common_step <= 0.0 {
		return false;
	}
	let phase = (second_base - first_base) / greatest_common_step;
	let phase_tolerance = phase.abs().max(1.0) * 1e-9;
	if !phase.is_finite() || (phase - phase.round()).abs() > phase_tolerance {
		return false;
	}
	let origin = first_base + first_step * previous_first_coefficient * phase.round();
	let period = first_step / greatest_common_step * second_step;
	if !origin.is_finite() || !period.is_finite() || period <= 0.0 {
		return false;
	}
	range_step_grid_has_value(overlap_min, overlap_max, period, origin)
}

#[cfg(any(wasm, test, feature = "testing"))]
fn range_step_grid_has_value(overlap_min: f64, overlap_max: f64, step: f64, base: f64) -> bool {
	if !step.is_finite() || step <= 0.0 || !base.is_finite() {
		return false;
	}
	let step_index = ((overlap_min - base) / step).ceil();
	let candidate = base + step_index * step;
	let bound_tolerance = overlap_min.abs().max(overlap_max.abs()).max(1.0) * 1e-9;
	candidate.is_finite()
		&& candidate >= overlap_min - bound_tolerance
		&& candidate <= overlap_max + bound_tolerance
}

#[cfg(test)]
mod tests {
	use super::{parse_html_number, range_constraints_conflict};

	#[test]
	fn html_number_parser_matches_the_range_constraint_grammar() {
		// Arrange
		let cases = [
			("10", Some(10.0)),
			("-.5", Some(-0.5)),
			("1.25e+2", Some(125.0)),
			("+10", None),
			("1.", None),
			(" 1", None),
			("1e", None),
		];

		// Act
		let parsed = cases.map(|(raw, _)| parse_html_number(raw));

		// Assert
		assert_eq!(
			parsed,
			cases.map(|(_, expected)| expected),
			"native range constraints must use the HTML valid-floating-point grammar"
		);
	}

	#[test]
	fn range_step_grid_compatibility_requires_a_common_value_inside_the_overlap() {
		// Arrange
		let outside_only = ((0.0, 5.0, Some(4.0), 0.0), (2.0, 5.0, Some(6.0), 2.0));
		let inside = ((0.0, 8.0, Some(4.0), 0.0), (2.0, 8.0, Some(6.0), 2.0));

		// Act
		let outside_only_conflicts = range_constraints_conflict(outside_only.0, outside_only.1);
		let inside_conflicts = range_constraints_conflict(inside.0, inside.1);

		// Assert
		assert_eq!((outside_only_conflicts, inside_conflicts), (true, false));
	}

	#[test]
	fn continuous_range_requires_the_stepped_peer_to_enter_the_overlap() {
		// Arrange
		let outside_only = ((0.5, 0.6, None, 0.5), (0.0, 0.6, Some(1.0), 0.0));
		let inside = ((0.5, 1.1, None, 0.5), (0.0, 1.1, Some(1.0), 0.0));

		// Act
		let conflicts = [
			range_constraints_conflict(outside_only.0, outside_only.1),
			range_constraints_conflict(outside_only.1, outside_only.0),
			range_constraints_conflict(inside.0, inside.1),
			range_constraints_conflict(inside.1, inside.0),
		];

		// Assert
		assert_eq!(conflicts, [true, true, false, false]);
	}
}
