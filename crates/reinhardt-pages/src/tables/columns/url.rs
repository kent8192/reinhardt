//! URL column type implementation

use crate::builder::html::{a, td};
use crate::dom::Element;
use crate::tables::column::Column as ColumnTrait;
use std::any::Any;

/// Column for URLs
///
/// This column type renders URLs as clickable links.
pub struct URLColumn {
	name: String,
	label: String,
	orderable: bool,
	visible: bool,
}

impl URLColumn {
	/// Creates a new URL column
	pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			label: label.into(),
			orderable: true,
			visible: true,
		}
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

impl ColumnTrait for URLColumn {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> &str {
		&self.label
	}

	fn render(&self, value: &dyn Any) -> Element {
		if let Some(url) = value.downcast_ref::<String>() {
			let link = a().attr("href", url).text(url).build();
			td().child(link).build()
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
