//! Core component trait and common types

use std::collections::HashMap;

/// Component trait - base interface for all UI components
///
/// All UI components must implement this trait to be renderable.
pub trait Component: Send + Sync {
	/// Returns the component's name (for debugging)
	fn name(&self) -> &'static str;

	/// Renders the component to HTML string
	fn render(&self) -> String;

	/// Returns CSS classes for the component
	fn classes(&self) -> Vec<String> {
		vec![]
	}

	/// Returns HTML attributes for the component
	fn attributes(&self) -> HashMap<String, String> {
		HashMap::new()
	}

	/// Returns child components (if any)
	fn children(&self) -> Vec<Box<dyn Component>> {
		vec![]
	}

	/// Renders all children to HTML
	fn render_children(&self) -> String {
		self.children()
			.iter()
			.map(|c| c.render())
			.collect::<Vec<_>>()
			.join("")
	}
}

/// Color variant for components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
	/// Primary color (default blue)
	Primary,
	/// Secondary color (default gray)
	Secondary,
	/// Success color (default green)
	Success,
	/// Danger color (default red)
	Danger,
	/// Warning color (default yellow)
	Warning,
	/// Info color (default cyan)
	Info,
	/// Light color (default light gray)
	Light,
	/// Dark color (default dark gray)
	Dark,
}

impl Variant {
	/// Convert variant to CSS class string
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Primary => "primary",
			Self::Secondary => "secondary",
			Self::Success => "success",
			Self::Danger => "danger",
			Self::Warning => "warning",
			Self::Info => "info",
			Self::Light => "light",
			Self::Dark => "dark",
		}
	}
}

/// Size variant for components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
	/// Extra small
	Xs,
	/// Small
	Sm,
	/// Medium (default)
	Md,
	/// Large
	Lg,
	/// Extra large
	Xl,
}

impl Size {
	/// Convert size to CSS class string
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Xs => "xs",
			Self::Sm => "sm",
			Self::Md => "md",
			Self::Lg => "lg",
			Self::Xl => "xl",
		}
	}
}

/// HTTP method for forms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
	/// GET method
	Get,
	/// POST method
	Post,
}

impl HttpMethod {
	/// Convert HTTP method to lowercase string
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Get => "get",
			Self::Post => "post",
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_variant_as_str() {
		assert_eq!(Variant::Primary.as_str(), "primary");
		assert_eq!(Variant::Secondary.as_str(), "secondary");
		assert_eq!(Variant::Success.as_str(), "success");
		assert_eq!(Variant::Danger.as_str(), "danger");
		assert_eq!(Variant::Warning.as_str(), "warning");
		assert_eq!(Variant::Info.as_str(), "info");
		assert_eq!(Variant::Light.as_str(), "light");
		assert_eq!(Variant::Dark.as_str(), "dark");
	}

	#[test]
	fn test_size_as_str() {
		assert_eq!(Size::Xs.as_str(), "xs");
		assert_eq!(Size::Sm.as_str(), "sm");
		assert_eq!(Size::Md.as_str(), "md");
		assert_eq!(Size::Lg.as_str(), "lg");
		assert_eq!(Size::Xl.as_str(), "xl");
	}

	#[test]
	fn test_http_method_as_str() {
		assert_eq!(HttpMethod::Get.as_str(), "get");
		assert_eq!(HttpMethod::Post.as_str(), "post");
	}
}
