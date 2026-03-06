//! Tests for admin UI components

use reinhardt_cms::admin::{AdminPageRegistry, PageEditor, PageTypeDescriptor};
use reinhardt_cms::error::CmsError;
use reinhardt_cms::pages::Page;
use rstest::rstest;
use serde_json::json;
use uuid::Uuid;

#[rstest]
#[tokio::test]
async fn test_render_edit_form_empty() {
	let editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	// Render form for a page without cached data
	let form_html = editor.render_edit_form(page_id).await.unwrap();

	// Verify form contains basic elements
	assert!(form_html.contains("<form"));
	assert!(form_html.contains("id=\"page-edit-form\""));
	assert!(form_html.contains("name=\"title\""));
	assert!(form_html.contains("name=\"slug\""));
	assert!(form_html.contains("name=\"content\""));
}

#[rstest]
#[tokio::test]
async fn test_render_edit_form_with_data() {
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	// Save some page data
	let page_data = json!({
		"title": "Test Page",
		"slug": "test-page",
		"content": "Test content"
	});
	editor.save_page(page_id, page_data).await.unwrap();

	// Render form
	let form_html = editor.render_edit_form(page_id).await.unwrap();

	// Verify form contains the saved data
	assert!(form_html.contains("value=\"Test Page\""));
	assert!(form_html.contains("value=\"test-page\""));
	assert!(form_html.contains("Test content"));
}

#[rstest]
#[tokio::test]
async fn test_save_page_valid() {
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	let page_data = json!({
		"title": "My Page",
		"slug": "my-page",
		"content": "Page content here"
	});

	// Save should succeed
	let result = editor.save_page(page_id, page_data.clone()).await;
	assert!(result.is_ok());

	// Verify data was saved by rendering form
	let form_html = editor.render_edit_form(page_id).await.unwrap();
	assert!(form_html.contains("value=\"My Page\""));
}

#[rstest]
#[tokio::test]
async fn test_save_page_missing_title() {
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	let page_data = json!({
		"slug": "my-page",
		"content": "Page content here"
	});

	// Save should fail due to missing title
	let result = editor.save_page(page_id, page_data).await;
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn test_save_page_missing_slug() {
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	let page_data = json!({
		"title": "My Page",
		"content": "Page content here"
	});

	// Save should fail due to missing slug
	let result = editor.save_page(page_id, page_data).await;
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn test_save_page_invalid_format() {
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	// Non-object data
	let page_data = json!(["invalid", "data"]);

	// Save should fail due to invalid format
	let result = editor.save_page(page_id, page_data).await;
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn test_multiple_pages() {
	let mut editor = PageEditor::new();

	let page1_id = Uuid::new_v4();
	let page2_id = Uuid::new_v4();

	// Save different data for two pages
	let page1_data = json!({
		"title": "Page 1",
		"slug": "page-1",
		"content": "Content 1"
	});

	let page2_data = json!({
		"title": "Page 2",
		"slug": "page-2",
		"content": "Content 2"
	});

	editor.save_page(page1_id, page1_data).await.unwrap();
	editor.save_page(page2_id, page2_data).await.unwrap();

	// Verify each page has its own data
	let form1 = editor.render_edit_form(page1_id).await.unwrap();
	let form2 = editor.render_edit_form(page2_id).await.unwrap();

	assert!(form1.contains("value=\"Page 1\""));
	assert!(form2.contains("value=\"Page 2\""));
}

// Test helper type for admin registry tests
struct TestPageType {
	type_name: String,
	label: String,
	icon: String,
}

impl PageTypeDescriptor for TestPageType {
	fn type_name(&self) -> &str {
		&self.type_name
	}

	fn label(&self) -> &str {
		&self.label
	}

	fn icon(&self) -> &str {
		&self.icon
	}

	fn can_create_at(&self, _parent: Option<&dyn Page>) -> bool {
		true
	}
}

#[rstest]
fn test_admin_registry_register_and_get() {
	// Arrange
	let mut registry = AdminPageRegistry::new();
	let page_type = TestPageType {
		type_name: "blog".to_string(),
		label: "Blog Page".to_string(),
		icon: "icon-blog".to_string(),
	};

	// Act
	registry.register(page_type);
	let retrieved = registry.get("blog");

	// Assert
	let descriptor = retrieved.unwrap();
	assert_eq!(descriptor.type_name(), "blog");
	assert_eq!(descriptor.label(), "Blog Page");
	assert_eq!(descriptor.icon(), "icon-blog");
}

#[rstest]
#[tokio::test]
async fn test_save_page_with_null_json_value() {
	// Arrange
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	// Act
	let result = editor.save_page(page_id, json!(null)).await;

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::Generic(ref msg) if msg == "Page data must be an object"));
}

#[rstest]
#[tokio::test]
async fn test_page_editor_default_trait() {
	// Arrange
	let editor = PageEditor::default();
	let page_id = Uuid::new_v4();

	// Act
	let form_html = editor.render_edit_form(page_id).await.unwrap();

	// Assert
	assert_ne!(form_html, "");
}

#[rstest]
fn test_admin_registry_default_trait() {
	// Arrange
	let registry = AdminPageRegistry::default();

	// Act
	let result = registry.get("anything");

	// Assert
	assert_eq!(result.is_none(), true);
}

#[rstest]
fn test_admin_registry_get_nonexistent_type() {
	// Arrange
	let mut registry = AdminPageRegistry::new();
	let page_type = TestPageType {
		type_name: "blog".to_string(),
		label: "Blog Page".to_string(),
		icon: "icon-blog".to_string(),
	};
	registry.register(page_type);

	// Act
	let result = registry.get("nonexistent");

	// Assert
	assert_eq!(result.is_none(), true);
}

#[rstest]
fn test_admin_registry_multiple_page_types() {
	// Arrange
	let mut registry = AdminPageRegistry::new();
	let types = vec![
		TestPageType {
			type_name: "blog".to_string(),
			label: "Blog".to_string(),
			icon: "icon-blog".to_string(),
		},
		TestPageType {
			type_name: "news".to_string(),
			label: "News".to_string(),
			icon: "icon-news".to_string(),
		},
		TestPageType {
			type_name: "event".to_string(),
			label: "Event".to_string(),
			icon: "icon-event".to_string(),
		},
	];

	// Act
	for page_type in types {
		registry.register(page_type);
	}

	// Assert
	let blog = registry.get("blog").unwrap();
	assert_eq!(blog.label(), "Blog");
	assert_eq!(blog.icon(), "icon-blog");

	let news = registry.get("news").unwrap();
	assert_eq!(news.label(), "News");
	assert_eq!(news.icon(), "icon-news");

	let event = registry.get("event").unwrap();
	assert_eq!(event.label(), "Event");
	assert_eq!(event.icon(), "icon-event");
}

#[rstest]
#[case::string_content(json!({"title": "T", "slug": "s", "body": "hello"}))]
#[case::number_content(json!({"title": "T", "slug": "s", "count": 42}))]
#[case::bool_content(json!({"title": "T", "slug": "s", "active": true}))]
#[case::nested_object(json!({"title": "T", "slug": "s", "meta": {"key": "value"}}))]
#[tokio::test]
async fn test_admin_save_various_valid_content_types(#[case] data: serde_json::Value) {
	// Arrange
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	// Act
	editor.save_page(page_id, data).await.unwrap();
	let form_html = editor.render_edit_form(page_id).await.unwrap();

	// Assert
	assert_ne!(form_html, "");
}

#[rstest]
#[case::valid_both_present(json!({"title": "T", "slug": "s"}), "")]
#[case::missing_title(json!({"slug": "s"}), "Missing required field: title")]
#[case::missing_slug(json!({"title": "T"}), "Missing required field: slug")]
#[case::missing_both(json!({}), "Missing required field: title")]
#[case::not_object(json!("string"), "Page data must be an object")]
#[tokio::test]
async fn test_save_page_validation_decision_table(
	#[case] data: serde_json::Value,
	#[case] expected_error: &str,
) {
	// Arrange
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();

	// Act
	let result = editor.save_page(page_id, data).await;

	// Assert
	if expected_error.is_empty() {
		result.unwrap();
	} else {
		let err = result.unwrap_err();
		assert!(matches!(err, CmsError::Generic(ref msg) if msg == expected_error));
	}
}
