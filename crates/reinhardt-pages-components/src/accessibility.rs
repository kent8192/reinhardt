//! Accessibility (A11y) attributes and utilities

use std::collections::HashMap;

/// ARIA live region politeness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaLive {
	/// No announcement
	Off,
	/// Polite announcement (wait for idle)
	Polite,
	/// Assertive announcement (immediate)
	Assertive,
}

impl AriaLive {
	/// Convert to string value
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Off => "off",
			Self::Polite => "polite",
			Self::Assertive => "assertive",
		}
	}
}

/// ARIA attributes for accessible components
#[derive(Debug, Clone, Default)]
pub struct AriaAttributes {
	/// aria-label
	pub label: Option<String>,
	/// aria-labelledby
	pub labelledby: Option<String>,
	/// aria-describedby
	pub describedby: Option<String>,
	/// role
	pub role: Option<String>,
	/// aria-live
	pub live: Option<AriaLive>,
	/// aria-expanded
	pub expanded: Option<bool>,
	/// aria-selected
	pub selected: Option<bool>,
	/// aria-hidden
	pub hidden: Option<bool>,
}

impl AriaAttributes {
	/// Create new empty ARIA attributes
	pub fn new() -> Self {
		Self::default()
	}

	/// Set aria-label
	pub fn label(mut self, label: impl Into<String>) -> Self {
		self.label = Some(label.into());
		self
	}

	/// Set role
	pub fn role(mut self, role: impl Into<String>) -> Self {
		self.role = Some(role.into());
		self
	}

	/// Set aria-live
	pub fn live(mut self, live: AriaLive) -> Self {
		self.live = Some(live);
		self
	}

	/// Set aria-expanded
	pub fn expanded(mut self, expanded: bool) -> Self {
		self.expanded = Some(expanded);
		self
	}

	/// Convert to HTML attributes map
	pub fn to_html_attributes(&self) -> HashMap<String, String> {
		let mut attrs = HashMap::new();

		if let Some(label) = &self.label {
			attrs.insert("aria-label".into(), label.clone());
		}
		if let Some(labelledby) = &self.labelledby {
			attrs.insert("aria-labelledby".into(), labelledby.clone());
		}
		if let Some(describedby) = &self.describedby {
			attrs.insert("aria-describedby".into(), describedby.clone());
		}
		if let Some(role) = &self.role {
			attrs.insert("role".into(), role.clone());
		}
		if let Some(live) = &self.live {
			attrs.insert("aria-live".into(), live.as_str().into());
		}
		if let Some(expanded) = &self.expanded {
			attrs.insert("aria-expanded".into(), expanded.to_string());
		}
		if let Some(selected) = &self.selected {
			attrs.insert("aria-selected".into(), selected.to_string());
		}
		if let Some(hidden) = &self.hidden {
			attrs.insert("aria-hidden".into(), hidden.to_string());
		}

		attrs
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_aria_live_as_str() {
		assert_eq!(AriaLive::Off.as_str(), "off");
		assert_eq!(AriaLive::Polite.as_str(), "polite");
		assert_eq!(AriaLive::Assertive.as_str(), "assertive");
	}

	#[test]
	fn test_aria_attributes_builder() {
		let attrs = AriaAttributes::new()
			.label("Submit button")
			.role("button")
			.expanded(true);

		assert_eq!(attrs.label, Some("Submit button".into()));
		assert_eq!(attrs.role, Some("button".into()));
		assert_eq!(attrs.expanded, Some(true));
	}

	#[test]
	fn test_to_html_attributes() {
		let attrs = AriaAttributes::new()
			.label("Submit")
			.role("button")
			.expanded(false);

		let html_attrs = attrs.to_html_attributes();
		assert_eq!(html_attrs.get("aria-label"), Some(&"Submit".to_string()));
		assert_eq!(html_attrs.get("role"), Some(&"button".to_string()));
		assert_eq!(html_attrs.get("aria-expanded"), Some(&"false".to_string()));
	}
}
