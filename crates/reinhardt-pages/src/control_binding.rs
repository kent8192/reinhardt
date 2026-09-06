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

pub(crate) use reinhardt_core::types::page::control_binding::SSR_OMITTED_PASSWORD_ATTRIBUTE;

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
		|| !range_step_grids_allow_reconciliation(
			overlap_min,
			overlap_max,
			first_step,
			first_base,
			second_step,
			second_base,
		)
}

#[cfg(any(wasm, test, feature = "testing"))]
fn range_step_grids_allow_reconciliation(
	overlap_min: f64,
	overlap_max: f64,
	first_step: Option<f64>,
	first_base: f64,
	second_step: Option<f64>,
	second_base: f64,
) -> bool {
	let (first_step, second_step) = match (first_step, second_step) {
		(None, None) => return true,
		(Some(_), None) | (None, Some(_)) => return false,
		(Some(first_step), Some(second_step)) => (first_step, second_step),
	};
	// ponytail: only identical step grids reconcile; differing grids stay local.
	// Broader support requires a shared normalization algorithm that cannot cycle.
	if first_step != second_step || !first_step.is_finite() || first_step <= 0.0 {
		return false;
	}
	let phase = (second_base - first_base) / first_step;
	if !phase.is_finite() || (phase - phase.round()).abs() > range_step_index_tolerance(phase) {
		return false;
	}
	range_step_grid_has_value(overlap_min, overlap_max, first_step, first_base)
}

#[cfg(any(wasm, test, feature = "testing"))]
pub(crate) fn range_step_index_tolerance(index: f64) -> f64 {
	// Cap roundoff in step units so large indices cannot hide a different grid.
	(4.0 * f64::EPSILON * index.abs().max(1.0)).min(1e-7)
}

#[cfg(any(wasm, test, feature = "testing"))]
fn range_step_grid_has_value(overlap_min: f64, overlap_max: f64, step: f64, base: f64) -> bool {
	if !step.is_finite() || step <= 0.0 || !base.is_finite() {
		return false;
	}
	let first_index = (overlap_min - base) / step;
	let last_index = (overlap_max - base) / step;
	first_index.is_finite()
		&& last_index.is_finite()
		&& (first_index - range_step_index_tolerance(first_index)).ceil()
			<= (last_index + range_step_index_tolerance(last_index)).floor()
}

#[cfg(test)]
mod tests {
	use super::{parse_html_number, range_constraints_conflict};
	use rstest::rstest;

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
	fn range_step_grid_compatibility_requires_the_same_grid_inside_the_overlap() {
		// Arrange
		let outside_only = ((0.0, 5.0, Some(4.0), 0.0), (2.0, 5.0, Some(6.0), 2.0));
		let inside = ((0.0, 8.0, Some(4.0), 0.0), (4.0, 8.0, Some(4.0), 4.0));

		// Act
		let outside_only_conflicts = range_constraints_conflict(outside_only.0, outside_only.1);
		let inside_conflicts = range_constraints_conflict(inside.0, inside.1);

		// Assert
		assert_eq!((outside_only_conflicts, inside_conflicts), (true, false));
	}

	#[rstest]
	#[case::decimal_bases((0.3, 1.0), 0.1, (0.0, 0.3), false)]
	#[case::decimal_endpoint((0.3, 0.3), 0.1, (0.0, 0.3), false)]
	#[case::rounded_endpoint((0.30000000000000004, 0.30000000000000004), 0.1, (0.0, 0.3), false)]
	#[case::different_decimal_grids((0.0, 1.0), 0.1, (0.0, 0.05), true)]
	#[case::off_grid_decimal_endpoint((0.35, 0.35), 0.1, (0.0, 0.3), true)]
	#[case::large_off_grid_endpoint((1e12 + 0.5, 1e12 + 0.5), 1.0, (0.0, 0.0), true)]
	#[case::large_aligned_phase((1e12, 1e12 + 10.0), 1.0, (0.0, 1e12), false)]
	#[case::large_half_step_phase((1e12, 1e12 + 10.0), 1.0, (0.0, 1e12 + 0.5), true)]
	#[case::large_near_half_step_phase((1e12, 1e12 + 10.0), 1.0, (0.0, 1e12 + 0.4999), true)]
	#[case::large_near_integer_phase((1e12, 1e12 + 10.0), 1.0, (0.0, 1e12 + 0.01), true)]
	fn range_grid_compatibility_tolerates_only_floating_point_roundoff(
		#[case] bounds: (f64, f64),
		#[case] step: f64,
		#[case] bases: (f64, f64),
		#[case] expected: bool,
	) {
		// Arrange
		let first = (bounds.0, bounds.1, Some(step), bases.0);
		let second = (bounds.0, bounds.1, Some(step), bases.1);

		// Act
		let conflicts = [
			range_constraints_conflict(first, second),
			range_constraints_conflict(second, first),
		];

		// Assert
		assert_eq!(conflicts, [expected; 2]);
	}

	#[test]
	fn continuous_and_stepped_ranges_keep_their_normalized_values_local() {
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
		assert_eq!(conflicts, [true; 4]);
		assert_eq!(
			range_constraints_conflict((0.5, 1.1, None, 0.5), (0.0, 1.1, None, 0.0)),
			false
		);
	}
}
