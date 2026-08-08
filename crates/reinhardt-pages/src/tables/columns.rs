//! Column type implementations
//!
//! This module provides various column types for different data rendering:
//! - `Column<T>`: Basic column for any type
//! - `LinkColumn`: Column with hyperlink
//! - `BooleanColumn`: Column for boolean values (checkmark/X)
//! - `DateTimeColumn`: Column for date/time formatting
//! - `EmailColumn`: Column for email addresses with mailto links
//! - `ChoiceColumn`: Column for choice fields
//! - `TemplateColumn`: Column with custom template
//! - `JSONColumn`: Column for JSON data
//! - `CheckBoxColumn`: Column with checkbox
//! - `URLColumn`: Column for URLs

pub mod basic;
pub mod boolean;
pub mod checkbox;
pub mod choice;
pub mod datetime;
pub mod email;
pub mod json;
pub mod link;
pub mod template;
pub mod url;

// Re-exports
pub use basic::Column;
pub use boolean::BooleanColumn;
pub use checkbox::CheckBoxColumn;
pub use choice::ChoiceColumn;
pub use datetime::DateTimeColumn;
pub use email::EmailColumn;
pub use json::JSONColumn;
pub use link::LinkColumn;
pub use template::TemplateColumn;
pub use url::URLColumn;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tables::column::Column as ColumnTrait;
	use std::collections::HashMap;

	#[test]
	fn table_columns_preserve_metadata_and_visibility_configuration() {
		// Arrange
		let basic = Column::<u32>::new("id", "Identifier")
			.orderable(false)
			.visible(false);
		let boolean = BooleanColumn::new("active", "Active")
			.orderable(false)
			.visible(false);
		let checkbox = CheckBoxColumn::new("selected", "Selected")
			.orderable(true)
			.visible(false);
		let choice = ChoiceColumn::new("status", "Status")
			.choices(HashMap::from([("draft".to_string(), "Draft".to_string())]))
			.orderable(false)
			.visible(false);
		let datetime = DateTimeColumn::new("created_at", "Created")
			.format("%Y-%m-%d")
			.orderable(false)
			.visible(false);

		// Act
		let metadata = [
			(
				basic.name(),
				basic.label(),
				basic.is_orderable(),
				basic.is_visible(),
			),
			(
				boolean.name(),
				boolean.label(),
				boolean.is_orderable(),
				boolean.is_visible(),
			),
			(
				checkbox.name(),
				checkbox.label(),
				checkbox.is_orderable(),
				checkbox.is_visible(),
			),
			(
				choice.name(),
				choice.label(),
				choice.is_orderable(),
				choice.is_visible(),
			),
			(
				datetime.name(),
				datetime.label(),
				datetime.is_orderable(),
				datetime.is_visible(),
			),
		];

		// Assert
		assert_eq!(
			metadata,
			[
				("id", "Identifier", false, false),
				("active", "Active", false, false),
				("selected", "Selected", true, false),
				("status", "Status", false, false),
				("created_at", "Created", false, false),
			]
		);
	}
	#[test]
	fn specialized_columns_expose_expected_defaults_and_custom_metadata() {
		// Arrange
		let boolean = BooleanColumn::with_icons("verified", "Verified", "yes", "no");
		let email = EmailColumn::new("email", "Email").visible(false);
		let json = JSONColumn::new("payload", "Payload").orderable(true);
		let link = LinkColumn::new("id", "Profile", "/profiles/{id}")
			.orderable(false)
			.visible(false);
		let link_with_text = LinkColumn::with_text("slug", "Article", "/articles/{slug}", "Read");
		let template = TemplateColumn::new("summary", "Summary")
			.orderable(true)
			.visible(false);
		let url = URLColumn::new("website", "Website")
			.orderable(false)
			.visible(false);

		// Act
		let metadata = [
			(
				boolean.name(),
				boolean.label(),
				boolean.is_orderable(),
				boolean.is_visible(),
			),
			(
				email.name(),
				email.label(),
				email.is_orderable(),
				email.is_visible(),
			),
			(
				json.name(),
				json.label(),
				json.is_orderable(),
				json.is_visible(),
			),
			(
				link.name(),
				link.label(),
				link.is_orderable(),
				link.is_visible(),
			),
			(
				link_with_text.name(),
				link_with_text.label(),
				link_with_text.is_orderable(),
				link_with_text.is_visible(),
			),
			(
				template.name(),
				template.label(),
				template.is_orderable(),
				template.is_visible(),
			),
			(
				url.name(),
				url.label(),
				url.is_orderable(),
				url.is_visible(),
			),
		];

		// Assert
		assert_eq!(
			metadata,
			[
				("verified", "Verified", true, true),
				("email", "Email", true, false),
				("payload", "Payload", true, true),
				("id", "Profile", false, false),
				("slug", "Article", true, true),
				("summary", "Summary", true, false),
				("website", "Website", false, false),
			]
		);
	}
}
