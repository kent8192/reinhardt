//! Owned event-target snapshots and target capability errors.

use std::collections::BTreeMap;
use std::fmt;

#[cfg(wasm)]
use wasm_bindgen::JsCast;

use super::EventFile;

/// Failure to read a target-dependent event capability.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventTargetError {
	/// The browser or native fixture supplied no listener target.
	MissingCurrentTarget {
		/// Exact event name.
		event: &'static str,
	},
	/// The listener element does not support the requested capability.
	UnsupportedElement {
		/// Exact event name.
		event: &'static str,
		/// Actual normalized target tag.
		actual_tag: String,
		/// Supported normalized target tags.
		expected: &'static [&'static str],
	},
	/// The target tag supports related controls but not this property.
	UnsupportedProperty {
		/// Exact event name.
		event: &'static str,
		/// Requested property.
		property: &'static str,
		/// Actual normalized target tag.
		actual_tag: String,
	},
}

impl fmt::Display for EventTargetError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::MissingCurrentTarget { event } => {
				write!(formatter, "`{event}` event has no current target")
			}
			Self::UnsupportedElement {
				event,
				actual_tag,
				expected,
			} => write!(
				formatter,
				"`{event}` event target `{actual_tag}` is unsupported; expected {}",
				expected.join(", ")
			),
			Self::UnsupportedProperty {
				event,
				property,
				actual_tag,
			} => write!(
				formatter,
				"`{event}` event target `{actual_tag}` does not expose `{property}`"
			),
		}
	}
}

impl std::error::Error for EventTargetError {}

/// Cross-target owned snapshot of stable event-target state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTarget {
	tag_name: String,
	attributes: BTreeMap<String, String>,
	value: Option<String>,
	checked: Option<bool>,
	selected_values: Vec<String>,
	files: Vec<EventFile>,
	text_content: Option<String>,
	content_editable: bool,
}

impl EventTarget {
	/// Returns the normalized lowercase element tag.
	#[must_use]
	pub fn tag_name(&self) -> &str {
		&self.tag_name
	}

	/// Returns one captured element attribute.
	#[must_use]
	pub fn attribute(&self, name: &str) -> Option<&str> {
		self.attributes.get(name).map(String::as_str)
	}

	/// Returns captured descendant text.
	#[must_use]
	pub fn text_content(&self) -> Option<&str> {
		self.text_content.as_deref()
	}

	/// Returns whether the target was contenteditable.
	#[must_use]
	pub const fn is_content_editable(&self) -> bool {
		self.content_editable
	}

	pub(crate) fn value_for(&self, event: &'static str) -> Result<String, EventTargetError> {
		if !matches!(self.tag_name.as_str(), "input" | "textarea" | "select")
			&& !self.content_editable
		{
			return Err(EventTargetError::UnsupportedElement {
				event,
				actual_tag: self.tag_name.clone(),
				expected: &["input", "textarea", "select", "contenteditable"],
			});
		}
		self.value
			.clone()
			.or_else(|| {
				self.content_editable
					.then(|| self.text_content.clone())
					.flatten()
			})
			.ok_or_else(|| EventTargetError::UnsupportedProperty {
				event,
				property: "value",
				actual_tag: self.tag_name.clone(),
			})
	}

	pub(crate) fn checked_for(&self, event: &'static str) -> Result<bool, EventTargetError> {
		if self.tag_name != "input" {
			return Err(EventTargetError::UnsupportedElement {
				event,
				actual_tag: self.tag_name.clone(),
				expected: &["input[type=checkbox]", "input[type=radio]"],
			});
		}
		if !self.attribute("type").is_some_and(|kind| {
			kind.eq_ignore_ascii_case("checkbox") || kind.eq_ignore_ascii_case("radio")
		}) {
			return Err(EventTargetError::UnsupportedProperty {
				event,
				property: "checked",
				actual_tag: self.tag_name.clone(),
			});
		}
		self.checked
			.ok_or_else(|| EventTargetError::UnsupportedProperty {
				event,
				property: "checked",
				actual_tag: self.tag_name.clone(),
			})
	}

	pub(crate) fn selected_values_for(
		&self,
		event: &'static str,
	) -> Result<Vec<String>, EventTargetError> {
		if self.tag_name != "select" {
			return Err(EventTargetError::UnsupportedElement {
				event,
				actual_tag: self.tag_name.clone(),
				expected: &["select"],
			});
		}
		Ok(self.selected_values.clone())
	}

	pub(crate) fn files_for(
		&self,
		event: &'static str,
	) -> Result<Vec<EventFile>, EventTargetError> {
		if self.tag_name != "input" {
			return Err(EventTargetError::UnsupportedElement {
				event,
				actual_tag: self.tag_name.clone(),
				expected: &["input[type=file]"],
			});
		}
		if !self
			.attribute("type")
			.is_some_and(|kind| kind.eq_ignore_ascii_case("file"))
		{
			return Err(EventTargetError::UnsupportedProperty {
				event,
				property: "files",
				actual_tag: self.tag_name.clone(),
			});
		}
		Ok(self.files.clone())
	}

	#[cfg(native)]
	pub(crate) fn from_native(target: &reinhardt_core::types::page::NativeEventTarget) -> Self {
		Self {
			tag_name: target.tag_name().to_owned(),
			attributes: target.attributes().clone(),
			value: target.value().map(ToOwned::to_owned),
			checked: target.checked(),
			selected_values: target.selected_values().to_vec(),
			files: target.files().iter().map(EventFile::from).collect(),
			text_content: target.text_content().map(ToOwned::to_owned),
			content_editable: target.is_content_editable(),
		}
	}

	#[cfg(wasm)]
	pub(crate) fn from_web_target(target: web_sys::EventTarget) -> Option<Self> {
		let element = target.dyn_into::<web_sys::Element>().ok()?;
		let tag_name = element.tag_name().to_ascii_lowercase();
		let attribute_map = element.attributes();
		let mut attributes = BTreeMap::new();
		for index in 0..attribute_map.length() {
			if let Some(attribute) = attribute_map.item(index) {
				attributes.insert(attribute.name(), attribute.value());
			}
		}

		let input = element.dyn_ref::<web_sys::HtmlInputElement>();
		let textarea = element.dyn_ref::<web_sys::HtmlTextAreaElement>();
		let select = element.dyn_ref::<web_sys::HtmlSelectElement>();
		let html = element.dyn_ref::<web_sys::HtmlElement>();
		let content_editable = html.is_some_and(web_sys::HtmlElement::is_content_editable);
		let value = input
			.map(web_sys::HtmlInputElement::value)
			.or_else(|| textarea.map(web_sys::HtmlTextAreaElement::value))
			.or_else(|| select.map(web_sys::HtmlSelectElement::value))
			.or_else(|| content_editable.then(|| element.text_content()).flatten());
		let checked = input.map(web_sys::HtmlInputElement::checked);
		let mut selected_values = Vec::new();
		if let Some(select) = select {
			let options = select.options();
			for index in 0..options.length() {
				if let Some(option) = options.item(index)
					&& let Ok(option) = option.dyn_into::<web_sys::HtmlOptionElement>()
					&& option.selected()
				{
					selected_values.push(option.value());
				}
			}
		}
		let mut files = Vec::new();
		if let Some(file_list) = input.and_then(web_sys::HtmlInputElement::files) {
			for index in 0..file_list.length() {
				if let Some(file) = file_list.get(index) {
					files.push(EventFile::from(file));
				}
			}
		}

		Some(Self {
			tag_name,
			attributes,
			value,
			checked,
			selected_values,
			files,
			text_content: element.text_content(),
			content_editable,
		})
	}
}
