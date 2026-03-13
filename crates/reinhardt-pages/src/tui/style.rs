//! CSS-like style to ratatui Style conversion.

use ratatui::style::{Color, Modifier, Style};

/// Converts CSS-like style properties to ratatui `Style`.
///
/// Provides a mapping from common CSS color names and style properties
/// to their ratatui equivalents.
pub struct StyleConverter;

impl StyleConverter {
	/// Converts a CSS color string to a ratatui `Color`.
	///
	/// Supports:
	/// - Named colors: `red`, `blue`, `green`, `yellow`, `white`, `black`, etc.
	/// - Hex colors: `#RGB`, `#RRGGBB`
	///
	/// Returns `None` for unrecognized color values.
	pub fn parse_color(value: &str) -> Option<Color> {
		let trimmed = value.trim().to_lowercase();
		match trimmed.as_str() {
			"black" => Some(Color::Black),
			"red" => Some(Color::Red),
			"green" => Some(Color::Green),
			"yellow" => Some(Color::Yellow),
			"blue" => Some(Color::Blue),
			"magenta" | "purple" => Some(Color::Magenta),
			"cyan" => Some(Color::Cyan),
			"white" => Some(Color::White),
			"gray" | "grey" => Some(Color::Gray),
			"darkgray" | "darkgrey" => Some(Color::DarkGray),
			"lightred" => Some(Color::LightRed),
			"lightgreen" => Some(Color::LightGreen),
			"lightyellow" => Some(Color::LightYellow),
			"lightblue" => Some(Color::LightBlue),
			"lightmagenta" => Some(Color::LightMagenta),
			"lightcyan" => Some(Color::LightCyan),
			_ if trimmed.starts_with('#') => Self::parse_hex_color(&trimmed),
			_ => None,
		}
	}

	/// Parses a hex color string into a ratatui `Color`.
	fn parse_hex_color(hex: &str) -> Option<Color> {
		let hex = hex.trim_start_matches('#');
		match hex.len() {
			3 => {
				// #RGB -> #RRGGBB
				let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
				let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
				let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
				Some(Color::Rgb(r, g, b))
			}
			6 => {
				let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
				let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
				let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
				Some(Color::Rgb(r, g, b))
			}
			_ => None,
		}
	}

	/// Builds a ratatui `Style` from a set of CSS-like property key-value pairs.
	///
	/// Supported properties:
	/// - `color` / `foreground`: Text foreground color
	/// - `background` / `background-color`: Background color
	/// - `font-weight: bold`: Bold text modifier
	/// - `font-style: italic`: Italic text modifier
	/// - `text-decoration: underline`: Underline modifier
	/// - `text-decoration: line-through`: Crossed-out modifier
	pub fn from_css_properties(properties: &[(&str, &str)]) -> Style {
		let mut style = Style::default();

		for (key, value) in properties {
			match *key {
				"color" | "foreground" => {
					if let Some(color) = Self::parse_color(value) {
						style = style.fg(color);
					}
				}
				"background" | "background-color" => {
					if let Some(color) = Self::parse_color(value) {
						style = style.bg(color);
					}
				}
				"font-weight" => {
					if value.eq_ignore_ascii_case("bold") {
						style = style.add_modifier(Modifier::BOLD);
					}
				}
				"font-style" => {
					if value.eq_ignore_ascii_case("italic") {
						style = style.add_modifier(Modifier::ITALIC);
					}
				}
				"text-decoration" => {
					let val_lower = value.to_lowercase();
					if val_lower.contains("underline") {
						style = style.add_modifier(Modifier::UNDERLINED);
					}
					if val_lower.contains("line-through") {
						style = style.add_modifier(Modifier::CROSSED_OUT);
					}
				}
				_ => {
					// Unsupported CSS property, silently ignored
				}
			}
		}

		style
	}

	/// Extracts CSS-like style from an inline `style` attribute value.
	///
	/// Parses simple `property: value;` pairs from the attribute string.
	pub fn from_inline_style(style_attr: &str) -> Style {
		let properties: Vec<(&str, &str)> = style_attr
			.split(';')
			.filter_map(|pair| {
				let mut parts = pair.splitn(2, ':');
				let key = parts.next()?.trim();
				let value = parts.next()?.trim();
				if key.is_empty() || value.is_empty() {
					None
				} else {
					Some((key, value))
				}
			})
			.collect();

		Self::from_css_properties(&properties)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("red", Some(Color::Red))]
	#[case("blue", Some(Color::Blue))]
	#[case("green", Some(Color::Green))]
	#[case("white", Some(Color::White))]
	#[case("black", Some(Color::Black))]
	#[case("yellow", Some(Color::Yellow))]
	#[case("cyan", Some(Color::Cyan))]
	#[case("magenta", Some(Color::Magenta))]
	#[case("purple", Some(Color::Magenta))]
	#[case("gray", Some(Color::Gray))]
	#[case("grey", Some(Color::Gray))]
	fn test_parse_named_colors(#[case] input: &str, #[case] expected: Option<Color>) {
		// Arrange & Act
		let result = StyleConverter::parse_color(input);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_parse_hex_color_6_digit() {
		// Arrange
		let input = "#ff0000";

		// Act
		let result = StyleConverter::parse_color(input);

		// Assert
		assert_eq!(result, Some(Color::Rgb(255, 0, 0)));
	}

	#[rstest]
	fn test_parse_hex_color_3_digit() {
		// Arrange
		let input = "#f00";

		// Act
		let result = StyleConverter::parse_color(input);

		// Assert
		assert_eq!(result, Some(Color::Rgb(255, 0, 0)));
	}

	#[rstest]
	fn test_parse_unknown_color_returns_none() {
		// Arrange
		let input = "not-a-color";

		// Act
		let result = StyleConverter::parse_color(input);

		// Assert
		assert_eq!(result, None);
	}

	#[rstest]
	fn test_from_css_properties_foreground() {
		// Arrange
		let props = [("color", "red")];

		// Act
		let style = StyleConverter::from_css_properties(&props);

		// Assert
		assert_eq!(style.fg, Some(Color::Red));
	}

	#[rstest]
	fn test_from_css_properties_background() {
		// Arrange
		let props = [("background-color", "blue")];

		// Act
		let style = StyleConverter::from_css_properties(&props);

		// Assert
		assert_eq!(style.bg, Some(Color::Blue));
	}

	#[rstest]
	fn test_from_css_properties_bold() {
		// Arrange
		let props = [("font-weight", "bold")];

		// Act
		let style = StyleConverter::from_css_properties(&props);

		// Assert
		assert!(style.add_modifier.contains(Modifier::BOLD));
	}

	#[rstest]
	fn test_from_css_properties_italic() {
		// Arrange
		let props = [("font-style", "italic")];

		// Act
		let style = StyleConverter::from_css_properties(&props);

		// Assert
		assert!(style.add_modifier.contains(Modifier::ITALIC));
	}

	#[rstest]
	fn test_from_css_properties_underline() {
		// Arrange
		let props = [("text-decoration", "underline")];

		// Act
		let style = StyleConverter::from_css_properties(&props);

		// Assert
		assert!(style.add_modifier.contains(Modifier::UNDERLINED));
	}

	#[rstest]
	fn test_from_inline_style_multiple_properties() {
		// Arrange
		let style_attr = "color: red; font-weight: bold; background-color: black";

		// Act
		let style = StyleConverter::from_inline_style(style_attr);

		// Assert
		assert_eq!(style.fg, Some(Color::Red));
		assert_eq!(style.bg, Some(Color::Black));
		assert!(style.add_modifier.contains(Modifier::BOLD));
	}

	#[rstest]
	fn test_from_inline_style_empty() {
		// Arrange
		let style_attr = "";

		// Act
		let style = StyleConverter::from_inline_style(style_attr);

		// Assert
		assert_eq!(style, Style::default());
	}

	#[rstest]
	fn test_from_inline_style_trailing_semicolon() {
		// Arrange
		let style_attr = "color: green;";

		// Act
		let style = StyleConverter::from_inline_style(style_attr);

		// Assert
		assert_eq!(style.fg, Some(Color::Green));
	}
}
