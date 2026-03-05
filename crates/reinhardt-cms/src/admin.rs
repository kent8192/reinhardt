//! Admin UI integration
//!
//! Integration with reinhardt-pages for CMS admin interface.

use crate::error::{CmsError, CmsResult};
use crate::pages::{Page, PageId};
use std::collections::HashMap;

/// Admin page registry
pub struct AdminPageRegistry {
	pages: HashMap<String, Box<dyn PageTypeDescriptor>>,
}

impl AdminPageRegistry {
	/// Create a new admin page registry
	pub fn new() -> Self {
		Self {
			pages: HashMap::new(),
		}
	}

	/// Register a page type
	pub fn register<T: PageTypeDescriptor + 'static>(&mut self, page_type: T) {
		self.pages
			.insert(page_type.type_name().to_string(), Box::new(page_type));
	}

	/// Get a page type descriptor
	pub fn get(&self, type_name: &str) -> Option<&dyn PageTypeDescriptor> {
		self.pages.get(type_name).map(|b| b.as_ref())
	}
}

impl Default for AdminPageRegistry {
	fn default() -> Self {
		Self::new()
	}
}

/// Descriptor for a page type in the admin
pub trait PageTypeDescriptor: Send + Sync {
	/// Get the type name
	fn type_name(&self) -> &str;

	/// Get the human-readable label
	fn label(&self) -> &str;

	/// Get the icon class/name
	fn icon(&self) -> &str;

	/// Can this page type be created as a child of the given parent?
	fn can_create_at(&self, parent: Option<&dyn Page>) -> bool;
}

/// Page editor interface
pub struct PageEditor {
	/// Cache for page data during editing
	page_cache: std::collections::HashMap<PageId, serde_json::Value>,
}

impl PageEditor {
	/// Create a new page editor
	pub fn new() -> Self {
		Self {
			page_cache: std::collections::HashMap::new(),
		}
	}

	/// Render the edit form for a page
	pub async fn render_edit_form(&self, page_id: PageId) -> CmsResult<String> {
		use reinhardt_core::types::page::{IntoPage, PageElement};

		// Get page data from cache or use empty object
		let page_data = self
			.page_cache
			.get(&page_id)
			.cloned()
			.unwrap_or_else(|| serde_json::json!({}));

		let title_value = page_data
			.get("title")
			.and_then(|v| v.as_str())
			.unwrap_or("");
		let slug_value = page_data.get("slug").and_then(|v| v.as_str()).unwrap_or("");
		let content_value = page_data
			.get("content")
			.and_then(|v| v.as_str())
			.unwrap_or("");

		let form = PageElement::new("form")
			.attr("id", "page-edit-form")
			.attr("data-page-id", page_id.to_string())
			.child(
				PageElement::new("div")
					.attr("class", "form-group")
					.child(
						PageElement::new("label")
							.attr("for", "title")
							.child("Title"),
					)
					.child(
						PageElement::new("input")
							.attr("type", "text")
							.attr("id", "title")
							.attr("name", "title")
							.attr("value", title_value.to_string())
							.attr("class", "form-control")
							.bool_attr("required", true),
					),
			)
			.child(
				PageElement::new("div")
					.attr("class", "form-group")
					.child(PageElement::new("label").attr("for", "slug").child("Slug"))
					.child(
						PageElement::new("input")
							.attr("type", "text")
							.attr("id", "slug")
							.attr("name", "slug")
							.attr("value", slug_value.to_string())
							.attr("class", "form-control")
							.bool_attr("required", true),
					),
			)
			.child(
				PageElement::new("div")
					.attr("class", "form-group")
					.child(
						PageElement::new("label")
							.attr("for", "content")
							.child("Content"),
					)
					.child(
						PageElement::new("textarea")
							.attr("id", "content")
							.attr("name", "content")
							.attr("class", "form-control")
							.attr("rows", "10")
							.child(content_value.to_string()),
					),
			)
			.child(
				PageElement::new("div")
					.attr("class", "form-actions")
					.child(
						PageElement::new("button")
							.attr("type", "submit")
							.attr("class", "btn btn-primary")
							.child("Save"),
					)
					.child(
						PageElement::new("button")
							.attr("type", "button")
							.attr("class", "btn btn-secondary")
							.attr("onclick", "history.back()")
							.child("Cancel"),
					),
			)
			.into_page();

		Ok(form.render_to_string())
	}

	/// Save page changes
	pub async fn save_page(&mut self, page_id: PageId, data: serde_json::Value) -> CmsResult<()> {
		// Validate required fields
		if !data.is_object() {
			return Err(CmsError::Generic("Page data must be an object".to_string()));
		}

		let obj = data.as_object().unwrap();

		// Check for required fields
		if !obj.contains_key("title") {
			return Err(CmsError::Generic(
				"Missing required field: title".to_string(),
			));
		}

		if !obj.contains_key("slug") {
			return Err(CmsError::Generic(
				"Missing required field: slug".to_string(),
			));
		}

		// Store in cache
		self.page_cache.insert(page_id, data);

		Ok(())
	}
}

impl Default for PageEditor {
	fn default() -> Self {
		Self::new()
	}
}
