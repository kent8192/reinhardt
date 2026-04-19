#![cfg(not(target_arch = "wasm32"))]
//! Macro compilation tests for page! and head! with static assets
//!
//! These tests verify that macros compile correctly when used with resolve_static.
//! Note: Full trybuild testing would require test fixture files.
//! This file contains basic integration tests for macro usage.

#[cfg(native)]
mod macro_compile_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;

	/// Test that resolve_static works in a pattern similar to macro usage
	#[rstest]
	fn test_macro_pattern_static_url_in_attributes() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Simulate macro attribute usage
		let src = resolve_static("images/logo.png");
		let href = resolve_static("css/style.css");
		let script_src = resolve_static("js/app.js");

		assert_eq!(src, "/static/images/logo.png");
		assert_eq!(href, "/static/css/style.css");
		assert_eq!(script_src, "/static/js/app.js");
	}

	/// Test resolve_static in building HTML attribute values
	#[rstest]
	fn test_macro_pattern_html_attributes() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Simulate building HTML attributes
		let img_src = format!(r#"<img src="{}" />"#, resolve_static("icon.png"));
		let link_href = format!(
			r#"<link rel="stylesheet" href="{}" />"#,
			resolve_static("style.css")
		);
		let script_src = format!(r#"<script src="{}"></script>"#, resolve_static("app.js"));

		assert!(img_src.contains("/static/icon.png"));
		assert!(link_href.contains("/static/style.css"));
		assert!(script_src.contains("/static/app.js"));
	}

	/// Test head! macro pattern with multiple static assets
	#[rstest]
	fn test_macro_pattern_head_multiple_assets() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Simulate head! macro building
		let mut head_content = String::new();
		head_content.push_str(&format!(
			r#"<link rel="stylesheet" href="{}" />"#,
			resolve_static("css/reset.css")
		));
		head_content.push_str(&format!(
			r#"<link rel="stylesheet" href="{}" />"#,
			resolve_static("css/style.css")
		));
		head_content.push_str(&format!(
			r#"<script src="{}" defer></script>"#,
			resolve_static("js/app.js")
		));

		assert!(head_content.contains("/static/css/reset.css"));
		assert!(head_content.contains("/static/css/style.css"));
		assert!(head_content.contains("/static/js/app.js"));
		assert_eq!(head_content.matches("/static/").count(), 3);
	}

	/// Test page! macro pattern with static image src
	#[rstest]
	fn test_macro_pattern_page_image() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Simulate page! macro with image
		let image_url = resolve_static("images/logo.png");
		let page_html = format!(r#"<div><img src="{}" alt="Logo" /></div>"#, image_url);

		assert!(page_html.contains("logo.png"));
		assert!(page_html.contains("/static/"));
	}

	/// Test page! macro pattern with dynamic paths
	#[rstest]
	fn test_macro_pattern_page_dynamic_paths() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Simulate dynamic path in page! macro
		let user_id = 42;
		let avatar_path = format!("images/avatars/user_{}.png", user_id);
		let avatar_url = resolve_static(&avatar_path);

		assert_eq!(avatar_url, "/static/images/avatars/user_42.png");
	}

	/// Test resolve_static in component rendering pattern
	#[rstest]
	fn test_macro_pattern_component_rendering() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Simulate component rendering with static assets
		let component_html = format!(
			r#"<button><img src="{}" class="icon" /></button>"#,
			resolve_static("icons/menu.svg")
		);

		assert!(component_html.contains("/static/icons/menu.svg"));
	}

	/// Test with CDN URL in macro pattern
	#[rstest]
	fn test_macro_pattern_cdn_url() {
		let config = TemplateStaticConfig::new("https://cdn.example.com/static/".to_string());
		init_static_resolver(config);

		let css_url = resolve_static("css/style.css");
		let html = format!(r#"<link rel="stylesheet" href="{}" />"#, css_url);

		assert!(html.contains("https://cdn.example.com/static/css/style.css"));
	}

	/// Test resolve_static in loop pattern
	#[rstest]
	fn test_macro_pattern_looping_assets() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Simulate looping through multiple assets
		let sizes = vec!["sm", "md", "lg"];
		let mut html = String::new();

		for size in sizes {
			let url = resolve_static(&format!("images/icon-{}.svg", size));
			html.push_str(&format!(r#"<img src="{}" />"#, url));
		}

		assert!(html.contains("/static/images/icon-sm.svg"));
		assert!(html.contains("/static/images/icon-md.svg"));
		assert!(html.contains("/static/images/icon-lg.svg"));
	}

	/// Test manifest with macro pattern
	#[rstest]
	fn test_macro_pattern_manifest_hashing() {
		use std::collections::HashMap;

		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.a1b2c3.css".to_string(),
		);
		manifest.insert("js/app.js".to_string(), "js/app.d4e5f6.js".to_string());

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let css_url = resolve_static("css/style.css");
		let js_url = resolve_static("js/app.js");

		// Build HTML with hashed assets
		let html = format!(
			r#"<link rel="stylesheet" href="{}" /><script src="{}"></script>"#,
			css_url, js_url
		);

		assert!(html.contains("css/style.a1b2c3.css"));
		assert!(html.contains("js/app.d4e5f6.js"));
	}

	/// Test inline style background-image pattern
	#[rstest]
	fn test_macro_pattern_inline_styles() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let bg_url = resolve_static("images/hero-bg.jpg");
		let style = format!(r#"style="background-image: url('{}')"#, bg_url);

		assert!(style.contains("/static/images/hero-bg.jpg"));
	}
}

#[cfg(wasm)]
mod wasm_macro_compile_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_macro_pattern() {
		init_static_resolver("/static/".to_string());

		let src = resolve_static("images/logo.png");
		let html = format!(r#"<img src="{}" />"#, src);

		assert!(html.contains("/static/images/logo.png"));
	}
}
