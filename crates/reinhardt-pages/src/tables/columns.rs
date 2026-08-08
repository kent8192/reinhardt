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
	use rstest::rstest;
	use std::collections::HashMap;

	#[rstest]
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
	#[rstest]
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

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_render_tests {
	use super::*;
	use crate::tables::column::Column as ColumnTrait;
	use rstest::*;
	use std::collections::HashMap;
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[rstest]
	#[wasm_bindgen_test]
	fn specialized_columns_render_configured_values() {
		// Arrange
		let choice = ChoiceColumn::new("status", "Status")
			.choices(HashMap::from([("draft".to_string(), "Draft".to_string())]));
		let boolean = BooleanColumn::with_icons("verified", "Verified", "YES", "NO");
		let datetime = DateTimeColumn::new("created_at", "Created").format("%Y/%m/%d %H:%M");
		let link = LinkColumn::new("id", "Profile", "/profiles/{id}");
		let link_with_text = LinkColumn::with_text("slug", "Article", "/articles/{slug}", "Read");

		// Act
		let choice_value = "draft".to_string();
		let choice_element = choice.render(&choice_value);
		let boolean_value = true;
		let boolean_element = boolean.render(&boolean_value);
		let date_value = "2026-08-08 14:30:00".to_string();
		let date_element = datetime.render(&date_value);
		let link_value = "42".to_string();
		let link_element = link.render(&link_value);
		let slug_value = "coverage".to_string();
		let link_with_text_element = link_with_text.render(&slug_value);

		// Assert
		assert_eq!(
			choice_element.as_web_sys().text_content(),
			Some("Draft".to_string())
		);
		assert_eq!(
			boolean_element.as_web_sys().text_content(),
			Some("YES".to_string())
		);
		assert_eq!(date_element.as_web_sys().text_content(), Some(date_value));
		assert_eq!(
			link_element.as_web_sys().outer_html(),
			"<td><a href=\"/profiles/42\">42</a></td>",
		);
		assert_eq!(
			link_with_text_element.as_web_sys().outer_html(),
			"<td><a href=\"/articles/coverage\">Read</a></td>",
		);

		#[cfg(feature = "chrono")]
		{
			let datetime = DateTimeColumn::new("created_at", "Created").format("%Y/%m/%d %H:%M");
			let value =
				chrono::NaiveDateTime::parse_from_str("2026-08-08 14:30:00", "%Y-%m-%d %H:%M:%S")
					.unwrap();
			let element = datetime.render(&value);
			assert_eq!(
				element.as_web_sys().text_content(),
				Some("2026/08/08 14:30".to_string())
			);
		}
	}
}
