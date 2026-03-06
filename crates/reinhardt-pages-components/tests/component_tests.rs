//! Tests for component core types and Component trait
//!
//! This module contains unit tests for:
//! - Variant enum to string conversion
//! - Size enum to string conversion
//! - HttpMethod enum to string conversion
//! - Component trait default methods
//!
//! ## Standards Compliance
//!
//! - Uses rstest framework exclusively
//! - All assertions use strict comparison
//! - No skeleton tests
//! - All comments in English

use reinhardt_pages_components::*;
use rstest::*;
use std::collections::HashMap;

// ============================================================================
// Variant Enum Tests
// ============================================================================

#[rstest]
fn test_variant_primary_as_str() {
	assert_eq!(Variant::Primary.as_str(), "primary");
}

#[rstest]
fn test_variant_secondary_as_str() {
	assert_eq!(Variant::Secondary.as_str(), "secondary");
}

#[rstest]
fn test_variant_success_as_str() {
	assert_eq!(Variant::Success.as_str(), "success");
}

#[rstest]
fn test_variant_danger_as_str() {
	assert_eq!(Variant::Danger.as_str(), "danger");
}

#[rstest]
fn test_variant_warning_as_str() {
	assert_eq!(Variant::Warning.as_str(), "warning");
}

#[rstest]
fn test_variant_info_as_str() {
	assert_eq!(Variant::Info.as_str(), "info");
}

#[rstest]
fn test_variant_light_as_str() {
	assert_eq!(Variant::Light.as_str(), "light");
}

#[rstest]
fn test_variant_dark_as_str() {
	assert_eq!(Variant::Dark.as_str(), "dark");
}

// ============================================================================
// Size Enum Tests
// ============================================================================

#[rstest]
fn test_size_xs_as_str() {
	assert_eq!(Size::Xs.as_str(), "xs");
}

#[rstest]
fn test_size_sm_as_str() {
	assert_eq!(Size::Sm.as_str(), "sm");
}

#[rstest]
fn test_size_md_as_str() {
	assert_eq!(Size::Md.as_str(), "md");
}

#[rstest]
fn test_size_lg_as_str() {
	assert_eq!(Size::Lg.as_str(), "lg");
}

#[rstest]
fn test_size_xl_as_str() {
	assert_eq!(Size::Xl.as_str(), "xl");
}

// ============================================================================
// HttpMethod Enum Tests
// ============================================================================

#[rstest]
fn test_http_method_get_as_str() {
	assert_eq!(HttpMethod::Get.as_str(), "get");
}

#[rstest]
fn test_http_method_post_as_str() {
	assert_eq!(HttpMethod::Post.as_str(), "post");
}

// ============================================================================
// Component Trait Default Methods Tests
// ============================================================================

/// Test fixture: Empty component implementation
struct EmptyComponent;

impl Component for EmptyComponent {
	fn name(&self) -> &'static str {
		"empty"
	}

	fn render(&self) -> String {
		String::new()
	}
}

#[rstest]
fn test_component_render_children_empty() {
	let component = EmptyComponent;
	assert_eq!(component.render_children(), String::new());
}

#[rstest]
fn test_component_name() {
	let component = EmptyComponent;
	assert_eq!(component.name(), "empty");
}

#[rstest]
fn test_component_classes_default() {
	let component = EmptyComponent;
	assert!(component.classes().is_empty());
}

#[rstest]
fn test_component_attributes_default() {
	let component = EmptyComponent;
	assert!(component.attributes().is_empty());
}

#[rstest]
fn test_component_children_default() {
	let component = EmptyComponent;
	assert!(component.children().is_empty());
}

// ============================================================================
// Component Trait with Custom Implementation Tests
// ============================================================================

/// Test fixture: Component with custom classes
struct ComponentWithClasses;

impl Component for ComponentWithClasses {
	fn name(&self) -> &'static str {
		"with_classes"
	}

	fn render(&self) -> String {
		format!("<div class=\"{}\"></div>", self.classes().join(" "))
	}

	fn classes(&self) -> Vec<String> {
		vec!["component".into(), "custom".into()]
	}
}

#[rstest]
fn test_component_with_custom_classes() {
	let component = ComponentWithClasses;
	let classes = component.classes();
	assert_eq!(classes.len(), 2);
	assert_eq!(classes[0], "component");
	assert_eq!(classes[1], "custom");
}

#[rstest]
fn test_component_with_custom_classes_render() {
	let component = ComponentWithClasses;
	let html = component.render();
	assert_eq!(html, r#"<div class="component custom"></div>"#);
}

/// Test fixture: Component with attributes
struct ComponentWithAttributes;

impl Component for ComponentWithAttributes {
	fn name(&self) -> &'static str {
		"with_attributes"
	}

	fn render(&self) -> String {
		let mut html = String::from("<div");
		for (key, value) in self.attributes() {
			html.push_str(&format!(" {}=\"{}\"", key, value));
		}
		html.push_str("></div>");
		html
	}

	fn attributes(&self) -> HashMap<String, String> {
		let mut attrs = HashMap::new();
		attrs.insert("id".into(), "test-id".into());
		attrs.insert("data-test".into(), "value".into());
		attrs
	}
}

#[rstest]
fn test_component_with_attributes() {
	let component = ComponentWithAttributes;
	let attrs = component.attributes();
	assert_eq!(attrs.len(), 2);
	assert_eq!(attrs.get("id"), Some(&"test-id".to_string()));
	assert_eq!(attrs.get("data-test"), Some(&"value".to_string()));
}

#[rstest]
fn test_component_with_attributes_render() {
	let component = ComponentWithAttributes;
	let html = component.render();
	assert!(html.contains("id=\"test-id\""));
	assert!(html.contains("data-test=\"value\""));
}
