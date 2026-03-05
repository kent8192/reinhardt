//! Color validator

use super::lazy_patterns::{
	COLOR_HEX_REGEX, COLOR_HSL_REGEX, COLOR_HSLA_REGEX, COLOR_RGB_REGEX, COLOR_RGBA_REGEX,
};
use super::{ValidationError, ValidationResult, Validator};

/// Supported color formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat {
	/// Hex color format (#RGB, #RRGGBB, #RRGGBBAA)
	Hex,
	/// RGB color format (rgb(255, 255, 255))
	RGB,
	/// RGBA color format (rgba(255, 255, 255, 1.0))
	RGBA,
	/// HSL color format (hsl(360, 100%, 50%))
	HSL,
	/// HSLA color format (hsla(360, 100%, 50%, 1.0))
	HSLA,
	/// Any supported color format
	Any,
}

/// Color validator
///
/// Validates color values in various formats including Hex, RGB, RGBA, HSL, and HSLA.
///
/// # Examples
///
/// ```
/// use reinhardt_core::validators::{ColorValidator, ColorFormat};
///
/// // Validate any color format
/// let validator = ColorValidator::new();
/// assert!(validator.validate("#FF0000").is_ok());
/// assert!(validator.validate("rgb(255, 0, 0)").is_ok());
///
/// // Validate only hex colors
/// let hex_validator = ColorValidator::hex_only();
/// assert!(hex_validator.validate("#FF0000").is_ok());
/// assert!(hex_validator.validate("rgb(255, 0, 0)").is_err());
///
/// // Allow specific formats
/// let rgb_validator = ColorValidator::new()
///     .allow_formats(vec![ColorFormat::RGB, ColorFormat::RGBA]);
/// assert!(rgb_validator.validate("rgb(255, 0, 0)").is_ok());
/// assert!(rgb_validator.validate("#FF0000").is_err());
/// ```
#[derive(Debug, Clone)]
pub struct ColorValidator {
	allowed_formats: Vec<ColorFormat>,
	message: Option<String>,
}

impl ColorValidator {
	/// Creates a new ColorValidator that accepts any color format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::ColorValidator;
	///
	/// let validator = ColorValidator::new();
	/// assert!(validator.validate("#FF0000").is_ok());
	/// assert!(validator.validate("rgb(255, 0, 0)").is_ok());
	/// assert!(validator.validate("hsl(0, 100%, 50%)").is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			allowed_formats: vec![ColorFormat::Any],
			message: None,
		}
	}

	/// Creates a ColorValidator that only accepts hex colors
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::ColorValidator;
	///
	/// let validator = ColorValidator::hex_only();
	/// assert!(validator.validate("#FF0000").is_ok());
	/// assert!(validator.validate("#F00").is_ok());
	/// assert!(validator.validate("rgb(255, 0, 0)").is_err());
	/// ```
	pub fn hex_only() -> Self {
		Self::new().allow_formats(vec![ColorFormat::Hex])
	}

	/// Creates a ColorValidator that only accepts RGB colors
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::ColorValidator;
	///
	/// let validator = ColorValidator::rgb_only();
	/// assert!(validator.validate("rgb(255, 0, 0)").is_ok());
	/// assert!(validator.validate("#FF0000").is_err());
	/// ```
	pub fn rgb_only() -> Self {
		Self::new().allow_formats(vec![ColorFormat::RGB])
	}

	/// Configures the validator to accept specific color formats
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{ColorValidator, ColorFormat};
	///
	/// let validator = ColorValidator::new()
	///     .allow_formats(vec![ColorFormat::RGB, ColorFormat::RGBA]);
	/// assert!(validator.validate("rgb(255, 0, 0)").is_ok());
	/// assert!(validator.validate("rgba(255, 0, 0, 0.5)").is_ok());
	/// assert!(validator.validate("#FF0000").is_err());
	/// ```
	pub fn allow_formats(mut self, formats: Vec<ColorFormat>) -> Self {
		self.allowed_formats = formats;
		self
	}

	/// Sets a custom error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::ColorValidator;
	///
	/// let validator = ColorValidator::new()
	///     .with_message("Invalid color value");
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}

	/// Validates a color value and returns the detected format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{ColorValidator, ColorFormat};
	///
	/// let validator = ColorValidator::new();
	/// assert_eq!(validator.validate("#FF0000").unwrap(), ColorFormat::Hex);
	/// assert_eq!(validator.validate("rgb(255, 0, 0)").unwrap(), ColorFormat::RGB);
	/// ```
	pub fn validate(&self, value: &str) -> Result<ColorFormat, ValidationError> {
		let trimmed = value.trim();

		// Check hex format
		if COLOR_HEX_REGEX.is_match(trimmed) && self.is_format_allowed(ColorFormat::Hex) {
			return Ok(ColorFormat::Hex);
		}

		// Check RGBA format (before RGB to avoid partial match)
		if COLOR_RGBA_REGEX.is_match(trimmed) && self.is_format_allowed(ColorFormat::RGBA) {
			return Ok(ColorFormat::RGBA);
		}

		// Check RGB format
		if COLOR_RGB_REGEX.is_match(trimmed) && self.is_format_allowed(ColorFormat::RGB) {
			return Ok(ColorFormat::RGB);
		}

		// Check HSLA format (before HSL to avoid partial match)
		if COLOR_HSLA_REGEX.is_match(trimmed) && self.is_format_allowed(ColorFormat::HSLA) {
			return Ok(ColorFormat::HSLA);
		}

		// Check HSL format
		if COLOR_HSL_REGEX.is_match(trimmed) && self.is_format_allowed(ColorFormat::HSL) {
			return Ok(ColorFormat::HSL);
		}

		Err(ValidationError::Custom(
			self.message
				.clone()
				.unwrap_or_else(|| format!("Invalid color: {}", value)),
		))
	}

	fn is_format_allowed(&self, format: ColorFormat) -> bool {
		self.allowed_formats.contains(&ColorFormat::Any) || self.allowed_formats.contains(&format)
	}
}

impl Default for ColorValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator<String> for ColorValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		self.validate(value.as_str()).map(|_| ())
	}
}

impl Validator<str> for ColorValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		self.validate(value).map(|_| ())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_hex_colors() {
		let validator = ColorValidator::new();

		// Valid hex colors
		assert_eq!(validator.validate("#FFF").unwrap(), ColorFormat::Hex);
		assert_eq!(validator.validate("#FFFFFF").unwrap(), ColorFormat::Hex);
		assert_eq!(validator.validate("#FFFFFFFF").unwrap(), ColorFormat::Hex);
		assert_eq!(validator.validate("#123").unwrap(), ColorFormat::Hex);
		assert_eq!(validator.validate("#123456").unwrap(), ColorFormat::Hex);
		assert_eq!(validator.validate("#12345678").unwrap(), ColorFormat::Hex);
		assert_eq!(validator.validate("#abc").unwrap(), ColorFormat::Hex);
		assert_eq!(validator.validate("#ABCDEF").unwrap(), ColorFormat::Hex);
		assert_eq!(validator.validate("#AbCdEf").unwrap(), ColorFormat::Hex);

		// Invalid hex colors
		assert!(validator.validate("#GG0000").is_err());
		assert!(validator.validate("#12").is_err());
		assert!(validator.validate("#12345").is_err());
		assert!(validator.validate("FF0000").is_err());
		assert!(validator.validate("#").is_err());
	}

	#[test]
	fn test_rgb_colors() {
		let validator = ColorValidator::new();

		// Valid RGB colors
		assert_eq!(
			validator.validate("rgb(255, 255, 255)").unwrap(),
			ColorFormat::RGB
		);
		assert_eq!(
			validator.validate("rgb(0, 0, 0)").unwrap(),
			ColorFormat::RGB
		);
		assert_eq!(
			validator.validate("rgb(128, 64, 32)").unwrap(),
			ColorFormat::RGB
		);
		assert_eq!(
			validator.validate("rgb(255,0,0)").unwrap(),
			ColorFormat::RGB
		);
		assert_eq!(
			validator.validate("rgb( 255 , 0 , 0 )").unwrap(),
			ColorFormat::RGB
		);

		// Invalid RGB colors
		assert!(validator.validate("rgb(256, 0, 0)").is_err());
		assert!(validator.validate("rgb(-1, 0, 0)").is_err());
		assert!(validator.validate("rgb(255, 0)").is_err());
		assert!(validator.validate("rgb(255, 0, 0, 0)").is_err());
		assert!(validator.validate("rgb(255.5, 0, 0)").is_err());
	}

	#[test]
	fn test_rgba_colors() {
		let validator = ColorValidator::new();

		// Valid RGBA colors
		assert_eq!(
			validator.validate("rgba(255, 255, 255, 1)").unwrap(),
			ColorFormat::RGBA
		);
		assert_eq!(
			validator.validate("rgba(0, 0, 0, 0)").unwrap(),
			ColorFormat::RGBA
		);
		assert_eq!(
			validator.validate("rgba(128, 64, 32, 0.5)").unwrap(),
			ColorFormat::RGBA
		);
		assert_eq!(
			validator.validate("rgba(255, 0, 0, 0.75)").unwrap(),
			ColorFormat::RGBA
		);
		assert_eq!(
			validator.validate("rgba(255,0,0,.5)").unwrap(),
			ColorFormat::RGBA
		);
		assert_eq!(
			validator.validate("rgba( 255 , 0 , 0 , 0.5 )").unwrap(),
			ColorFormat::RGBA
		);

		// Invalid RGBA colors
		assert!(validator.validate("rgba(256, 0, 0, 0.5)").is_err());
		assert!(validator.validate("rgba(255, 0, 0, 1.5)").is_err());
		assert!(validator.validate("rgba(255, 0, 0)").is_err());
		assert!(validator.validate("rgba(255, 0, 0, -0.5)").is_err());
	}

	#[test]
	fn test_hsl_colors() {
		let validator = ColorValidator::new();

		// Valid HSL colors
		assert_eq!(
			validator.validate("hsl(360, 100%, 50%)").unwrap(),
			ColorFormat::HSL
		);
		assert_eq!(
			validator.validate("hsl(0, 0%, 0%)").unwrap(),
			ColorFormat::HSL
		);
		assert_eq!(
			validator.validate("hsl(180, 50%, 25%)").unwrap(),
			ColorFormat::HSL
		);
		assert_eq!(
			validator.validate("hsl(120,100%,50%)").unwrap(),
			ColorFormat::HSL
		);
		assert_eq!(
			validator.validate("hsl( 240 , 75% , 50% )").unwrap(),
			ColorFormat::HSL
		);

		// Invalid HSL colors
		assert!(validator.validate("hsl(361, 100%, 50%)").is_err());
		assert!(validator.validate("hsl(360, 101%, 50%)").is_err());
		assert!(validator.validate("hsl(360, 100%, 101%)").is_err());
		assert!(validator.validate("hsl(-1, 100%, 50%)").is_err());
		assert!(validator.validate("hsl(360, 100%)").is_err());
		assert!(validator.validate("hsl(360, 100, 50%)").is_err());
	}

	#[test]
	fn test_hsla_colors() {
		let validator = ColorValidator::new();

		// Valid HSLA colors
		assert_eq!(
			validator.validate("hsla(360, 100%, 50%, 1)").unwrap(),
			ColorFormat::HSLA
		);
		assert_eq!(
			validator.validate("hsla(0, 0%, 0%, 0)").unwrap(),
			ColorFormat::HSLA
		);
		assert_eq!(
			validator.validate("hsla(180, 50%, 25%, 0.5)").unwrap(),
			ColorFormat::HSLA
		);
		assert_eq!(
			validator.validate("hsla(120,100%,50%,.75)").unwrap(),
			ColorFormat::HSLA
		);
		assert_eq!(
			validator.validate("hsla( 240 , 75% , 50% , 0.5 )").unwrap(),
			ColorFormat::HSLA
		);

		// Invalid HSLA colors
		assert!(validator.validate("hsla(361, 100%, 50%, 0.5)").is_err());
		assert!(validator.validate("hsla(360, 101%, 50%, 0.5)").is_err());
		assert!(validator.validate("hsla(360, 100%, 101%, 0.5)").is_err());
		assert!(validator.validate("hsla(360, 100%, 50%, 1.5)").is_err());
		assert!(validator.validate("hsla(360, 100%, 50%)").is_err());
		assert!(validator.validate("hsla(360, 100, 50%, 0.5)").is_err());
	}

	#[test]
	fn test_hex_only_validator() {
		let validator = ColorValidator::hex_only();

		assert!(validator.validate("#FF0000").is_ok());
		assert!(validator.validate("rgb(255, 0, 0)").is_err());
		assert!(validator.validate("hsl(0, 100%, 50%)").is_err());
	}

	#[test]
	fn test_rgb_only_validator() {
		let validator = ColorValidator::rgb_only();

		assert!(validator.validate("rgb(255, 0, 0)").is_ok());
		assert!(validator.validate("#FF0000").is_err());
		assert!(validator.validate("rgba(255, 0, 0, 0.5)").is_err());
	}

	#[test]
	fn test_allow_specific_formats() {
		let validator =
			ColorValidator::new().allow_formats(vec![ColorFormat::RGB, ColorFormat::RGBA]);

		assert!(validator.validate("rgb(255, 0, 0)").is_ok());
		assert!(validator.validate("rgba(255, 0, 0, 0.5)").is_ok());
		assert!(validator.validate("#FF0000").is_err());
		assert!(validator.validate("hsl(0, 100%, 50%)").is_err());
	}

	#[test]
	fn test_custom_message() {
		let validator = ColorValidator::hex_only().with_message("Please use hex color format");

		match validator.validate("rgb(255, 0, 0)") {
			Err(ValidationError::Custom(msg)) => {
				assert_eq!(msg, "Please use hex color format");
			}
			_ => panic!("Expected Custom error with custom message"),
		}
	}

	#[test]
	fn test_validator_trait_string() {
		let validator = ColorValidator::new();
		let color = String::from("#FF0000");

		assert!(Validator::<String>::validate(&validator, &color).is_ok());
		assert!(Validator::<String>::validate(&validator, &String::from("invalid")).is_err());
	}

	#[test]
	fn test_validator_trait_str() {
		let validator = ColorValidator::new();

		assert!(Validator::<str>::validate(&validator, "#FF0000").is_ok());
		assert!(Validator::<str>::validate(&validator, "invalid").is_err());
	}

	#[test]
	fn test_default() {
		let validator = ColorValidator::default();
		assert!(validator.validate("#FF0000").is_ok());
	}

	#[test]
	fn test_whitespace_handling() {
		let validator = ColorValidator::new();

		assert!(validator.validate("  #FF0000  ").is_ok());
		assert!(validator.validate("  rgb(255, 0, 0)  ").is_ok());
		assert!(validator.validate("  hsl(0, 100%, 50%)  ").is_ok());
	}
}
