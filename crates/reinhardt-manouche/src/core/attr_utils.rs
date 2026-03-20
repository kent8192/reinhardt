//! Shared utilities for HTML attribute name conversion.

/// SVG attributes that require camelCase output.
const SVG_CAMEL_CASE_ATTRS: &[(&str, &str)] = &[
	("view_box", "viewBox"),
	("preserve_aspect_ratio", "preserveAspectRatio"),
	("clip_path_units", "clipPathUnits"),
	("gradient_transform", "gradientTransform"),
	("gradient_units", "gradientUnits"),
	("length_adjust", "lengthAdjust"),
	("marker_height", "markerHeight"),
	("marker_units", "markerUnits"),
	("marker_width", "markerWidth"),
	("mask_content_units", "maskContentUnits"),
	("mask_units", "maskUnits"),
	("path_length", "pathLength"),
	("pattern_content_units", "patternContentUnits"),
	("pattern_transform", "patternTransform"),
	("pattern_units", "patternUnits"),
	("ref_x", "refX"),
	("ref_y", "refY"),
	("spread_method", "spreadMethod"),
	("text_length", "textLength"),
];

/// Standard HTML boolean attributes that can be written in standalone form
/// (e.g., `required` instead of `required: true`).
pub const BOOLEAN_ATTRS: &[&str] = &[
	"allowfullscreen",
	"async",
	"autofocus",
	"autoplay",
	"checked",
	"controls",
	"default",
	"defer",
	"disabled",
	"formnovalidate",
	"hidden",
	"inert",
	"ismap",
	"itemscope",
	"loop",
	"multiple",
	"muted",
	"nomodule",
	"novalidate",
	"open",
	"playsinline",
	"readonly",
	"required",
	"reversed",
	"selected",
];

/// Converts a Rust identifier to an HTML attribute name.
///
/// Strips `r#` prefix, maps SVG camelCase attributes, and converts `_` to `-`.
pub(crate) fn ident_to_html_attr_name(name: &str) -> String {
	let name = name.strip_prefix("r#").unwrap_or(name);
	// Check SVG camelCase mapping first
	if let Some(&(_, camel)) = SVG_CAMEL_CASE_ATTRS
		.iter()
		.find(|&&(snake, _)| snake == name)
	{
		return camel.to_string();
	}
	name.replace('_', "-")
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("class", "class")]
	#[case("data_testid", "data-testid")]
	#[case("aria_label", "aria-label")]
	#[case("r#type", "type")]
	#[case("r#for", "for")]
	#[case("type", "type")]
	fn test_ident_to_html_attr_name(#[case] input: &str, #[case] expected: &str) {
		// Act
		let result = ident_to_html_attr_name(input);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case("view_box", "viewBox")]
	#[case("preserve_aspect_ratio", "preserveAspectRatio")]
	#[case("ref_x", "refX")]
	#[case("ref_y", "refY")]
	#[case("gradient_transform", "gradientTransform")]
	#[case("text_length", "textLength")]
	fn test_svg_camel_case_mapping(#[case] input: &str, #[case] expected: &str) {
		// Act
		let result = ident_to_html_attr_name(input);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_non_svg_attr_still_uses_hyphen() {
		// Act
		let result = ident_to_html_attr_name("data_custom_attr");

		// Assert
		assert_eq!(result, "data-custom-attr");
	}
}
