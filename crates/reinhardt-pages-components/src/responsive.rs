//! Responsive design utilities

/// Responsive breakpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
	/// Extra small (< 576px)
	Xs,
	/// Small (≥ 576px)
	Sm,
	/// Medium (≥ 768px)
	Md,
	/// Large (≥ 992px)
	Lg,
	/// Extra large (≥ 1200px)
	Xl,
	/// Extra extra large (≥ 1400px)
	Xxl,
}

impl Breakpoint {
	/// Get minimum width in pixels for this breakpoint
	pub fn min_width(&self) -> Option<u32> {
		match self {
			Self::Xs => None,
			Self::Sm => Some(576),
			Self::Md => Some(768),
			Self::Lg => Some(992),
			Self::Xl => Some(1200),
			Self::Xxl => Some(1400),
		}
	}

	/// Convert breakpoint to CSS class suffix
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Xs => "xs",
			Self::Sm => "sm",
			Self::Md => "md",
			Self::Lg => "lg",
			Self::Xl => "xl",
			Self::Xxl => "xxl",
		}
	}
}

/// Responsive value that can vary by breakpoint
#[derive(Debug, Clone)]
pub struct ResponsiveValue<T> {
	/// Extra small
	pub xs: Option<T>,
	/// Small
	pub sm: Option<T>,
	/// Medium
	pub md: Option<T>,
	/// Large
	pub lg: Option<T>,
	/// Extra large
	pub xl: Option<T>,
	/// Extra extra large
	pub xxl: Option<T>,
}

impl<T> ResponsiveValue<T> {
	/// Create new responsive value with default
	pub fn new(default: T) -> Self {
		Self {
			xs: Some(default),
			sm: None,
			md: None,
			lg: None,
			xl: None,
			xxl: None,
		}
	}

	/// Set small breakpoint value
	pub fn sm(mut self, value: T) -> Self {
		self.sm = Some(value);
		self
	}

	/// Set medium breakpoint value
	pub fn md(mut self, value: T) -> Self {
		self.md = Some(value);
		self
	}

	/// Set large breakpoint value
	pub fn lg(mut self, value: T) -> Self {
		self.lg = Some(value);
		self
	}

	/// Set extra large breakpoint value
	pub fn xl(mut self, value: T) -> Self {
		self.xl = Some(value);
		self
	}

	/// Set extra extra large breakpoint value
	pub fn xxl(mut self, value: T) -> Self {
		self.xxl = Some(value);
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_breakpoint_min_width() {
		assert_eq!(Breakpoint::Xs.min_width(), None);
		assert_eq!(Breakpoint::Sm.min_width(), Some(576));
		assert_eq!(Breakpoint::Md.min_width(), Some(768));
		assert_eq!(Breakpoint::Lg.min_width(), Some(992));
		assert_eq!(Breakpoint::Xl.min_width(), Some(1200));
		assert_eq!(Breakpoint::Xxl.min_width(), Some(1400));
	}

	#[test]
	fn test_breakpoint_as_str() {
		assert_eq!(Breakpoint::Xs.as_str(), "xs");
		assert_eq!(Breakpoint::Sm.as_str(), "sm");
		assert_eq!(Breakpoint::Md.as_str(), "md");
		assert_eq!(Breakpoint::Lg.as_str(), "lg");
		assert_eq!(Breakpoint::Xl.as_str(), "xl");
		assert_eq!(Breakpoint::Xxl.as_str(), "xxl");
	}

	#[test]
	fn test_responsive_value() {
		let value = ResponsiveValue::new(12).sm(6).md(4).lg(3);

		assert_eq!(value.xs, Some(12));
		assert_eq!(value.sm, Some(6));
		assert_eq!(value.md, Some(4));
		assert_eq!(value.lg, Some(3));
		assert_eq!(value.xl, None);
	}
}
