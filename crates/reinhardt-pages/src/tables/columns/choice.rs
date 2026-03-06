//! Choice column type implementation

use crate::builder::html::td;
use crate::dom::Element;
use crate::tables::column::Column as ColumnTrait;
use std::any::Any;
use std::collections::HashMap;

/// Column for choice fields
///
/// This column type renders choice values with human-readable labels.
pub struct ChoiceColumn {
	name: String,
	label: String,
	choices: HashMap<String, String>,
	orderable: bool,
	visible: bool,
}

impl ChoiceColumn {
	/// Creates a new choice column
	pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			label: label.into(),
			choices: HashMap::new(),
			orderable: true,
			visible: true,
		}
	}

	/// Sets the choices mapping from values to display labels
	pub fn choices(mut self, choices: HashMap<String, String>) -> Self {
		self.choices = choices;
		self
	}

	/// Sets whether this column is orderable
	pub fn orderable(mut self, orderable: bool) -> Self {
		self.orderable = orderable;
		self
	}

	/// Sets whether this column is visible
	pub fn visible(mut self, visible: bool) -> Self {
		self.visible = visible;
		self
	}
}

impl ColumnTrait for ChoiceColumn {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> &str {
		&self.label
	}

	fn render(&self, value: &dyn Any) -> Element {
		if let Some(val) = value.downcast_ref::<String>() {
			let display = self
				.choices
				.get(val)
				.map_or(val.as_str(), |label| label.as_str());
			td().text(display).build()
		} else {
			td().text("-").build()
		}
	}

	fn is_orderable(&self) -> bool {
		self.orderable
	}

	fn is_visible(&self) -> bool {
		self.visible
	}
}
