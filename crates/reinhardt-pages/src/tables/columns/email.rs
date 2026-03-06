//! Email column type implementation

use crate::builder::html::{a, td};
use crate::dom::Element;
use crate::tables::column::Column as ColumnTrait;
use std::any::Any;

/// Column for email addresses
///
/// This column type renders email addresses as mailto: links.
pub struct EmailColumn {
	name: String,
	label: String,
	orderable: bool,
	visible: bool,
}

impl EmailColumn {
	/// Creates a new email column
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

impl ColumnTrait for EmailColumn {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> &str {
		&self.label
	}

	fn render(&self, value: &dyn Any) -> Element {
		if let Some(email) = value.downcast_ref::<String>() {
			let href = format!("mailto:{}", email);
			let link = a().attr("href", &href).text(email).build();
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
