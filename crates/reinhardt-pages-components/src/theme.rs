//! Theme system for customizing component appearance

/// Theme configuration
///
/// Defines colors, typography, and styling for all components.
#[derive(Debug, Clone)]
pub struct Theme {
	// Colors
	/// Primary color
	pub primary: String,
	/// Secondary color
	pub secondary: String,
	/// Success color
	pub success: String,
	/// Danger color
	pub danger: String,
	/// Warning color
	pub warning: String,
	/// Info color
	pub info: String,
	/// Light color
	pub light: String,
	/// Dark color
	pub dark: String,

	// Typography
	/// Font family
	pub font_family: String,
	/// Base font size
	pub font_size_base: String,
	/// Base line height
	pub line_height_base: String,

	// Effects
	/// Border radius
	pub border_radius: String,
	/// Box shadow
	pub box_shadow: String,
}

impl Theme {
	/// Create default Bootstrap 5.3 theme
	pub fn default_theme() -> Self {
		Self {
			primary: "#0d6efd".into(),
			secondary: "#6c757d".into(),
			success: "#198754".into(),
			danger: "#dc3545".into(),
			warning: "#ffc107".into(),
			info: "#0dcaf0".into(),
			light: "#f8f9fa".into(),
			dark: "#212529".into(),

			font_family: "system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif".into(),
			font_size_base: "1rem".into(),
			line_height_base: "1.5".into(),

			border_radius: "0.375rem".into(),
			box_shadow: "0 0.5rem 1rem rgba(0, 0, 0, 0.15)".into(),
		}
	}

	/// Convert theme to CSS variables
	pub fn to_css_variables(&self) -> String {
		format!(
			r#":root {{
  --bs-primary: {};
  --bs-secondary: {};
  --bs-success: {};
  --bs-danger: {};
  --bs-warning: {};
  --bs-info: {};
  --bs-light: {};
  --bs-dark: {};

  --bs-font-family: {};
  --bs-font-size-base: {};
  --bs-line-height-base: {};

  --bs-border-radius: {};
  --bs-box-shadow: {};
}}"#,
			self.primary,
			self.secondary,
			self.success,
			self.danger,
			self.warning,
			self.info,
			self.light,
			self.dark,
			self.font_family,
			self.font_size_base,
			self.line_height_base,
			self.border_radius,
			self.box_shadow,
		)
	}

	/// Builder method for primary color
	pub fn primary(mut self, color: impl Into<String>) -> Self {
		self.primary = color.into();
		self
	}

	/// Builder method for secondary color
	pub fn secondary(mut self, color: impl Into<String>) -> Self {
		self.secondary = color.into();
		self
	}

	/// Builder method for border radius
	pub fn border_radius(mut self, radius: impl Into<String>) -> Self {
		self.border_radius = radius.into();
		self
	}
}

impl Default for Theme {
	fn default() -> Self {
		Self::default_theme()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_theme() {
		let theme = Theme::default_theme();
		assert_eq!(theme.primary, "#0d6efd");
		assert_eq!(
			theme.font_family,
			"system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif"
		);
	}

	#[test]
	fn test_to_css_variables() {
		let theme = Theme::default_theme();
		let css = theme.to_css_variables();
		assert!(css.contains("--bs-primary: #0d6efd"));
		assert!(css.contains("--bs-success: #198754"));
	}

	#[test]
	fn test_builder_methods() {
		let theme = Theme::default_theme()
			.primary("#007bff")
			.border_radius("0.5rem");

		assert_eq!(theme.primary, "#007bff");
		assert_eq!(theme.border_radius, "0.5rem");
	}
}
