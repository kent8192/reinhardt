#![cfg(not(target_arch = "wasm32"))]
//! Use case tests for static asset URL resolution
//!
//! These tests verify real-world scenarios and integration patterns.

#[cfg(native)]
mod use_case_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;
	use std::collections::HashMap;

	/// Test development environment with no manifest
	#[rstest]
	fn test_use_case_development_environment() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let css_url = resolve_static("css/style.css");
		let js_url = resolve_static("js/app.js");
		let img_url = resolve_static("images/logo.png");

		assert_eq!(css_url, "/static/css/style.css");
		assert_eq!(js_url, "/static/js/app.js");
		assert_eq!(img_url, "/static/images/logo.png");
	}

	/// Test production environment with manifest for cache busting
	#[rstest]
	fn test_use_case_production_environment() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.a1b2c3d4.css".to_string(),
		);
		manifest.insert("js/app.js".to_string(), "js/app.e5f6g7h8.js".to_string());
		manifest.insert(
			"images/logo.png".to_string(),
			"images/logo.i9j0k1l2.png".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let css_url = resolve_static("css/style.css");
		let js_url = resolve_static("js/app.js");
		let img_url = resolve_static("images/logo.png");

		assert_eq!(css_url, "/static/css/style.a1b2c3d4.css");
		assert_eq!(js_url, "/static/js/app.e5f6g7h8.js");
		assert_eq!(img_url, "/static/images/logo.i9j0k1l2.png");
	}

	/// Test CDN deployment use case
	#[rstest]
	fn test_use_case_cdn_deployment() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.prod.css".to_string(),
		);
		manifest.insert("js/app.js".to_string(), "js/app.prod.js".to_string());

		let config = TemplateStaticConfig::new("https://cdn.example.com/static/".to_string())
			.with_manifest(manifest);
		init_static_resolver(config);

		let css_url = resolve_static("css/style.css");
		let js_url = resolve_static("js/app.js");

		assert!(css_url.starts_with("https://cdn.example.com"));
		assert_eq!(css_url, "https://cdn.example.com/static/css/style.prod.css");
		assert_eq!(js_url, "https://cdn.example.com/static/js/app.prod.js");
	}

	/// Test theme switching use case
	#[rstest]
	fn test_use_case_theme_switching() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Dynamic theme selection
		for theme in ["light", "dark", "auto"].iter() {
			let css_path = format!("css/themes/{}/style.css", theme);
			let result = resolve_static(&css_path);

			assert!(result.contains(theme));
			assert!(result.ends_with("style.css"));
		}
	}

	/// Test multi-language/i18n asset loading
	#[rstest]
	fn test_use_case_i18n_assets() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		for lang in ["en", "ja", "es", "fr"].iter() {
			let js_path = format!("js/i18n/{}/messages.js", lang);
			let result = resolve_static(&js_path);

			assert!(result.contains(lang));
			assert!(result.contains("messages.js"));
		}
	}

	/// Test versioned API assets use case
	#[rstest]
	fn test_use_case_versioned_assets() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let paths = vec![
			"js/api/v1/client.js",
			"js/api/v2/client.js",
			"js/api/v3/client.js",
		];

		for path in paths {
			let result = resolve_static(path);
			assert!(result.contains(path));
		}
	}

	/// Test responsive image use case
	#[rstest]
	fn test_use_case_responsive_images() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let sizes = vec!["sm", "md", "lg", "xl"];
		for size in sizes {
			let path = format!("images/hero-{}.jpg", size);
			let result = resolve_static(&path);
			assert!(result.contains(size));
		}
	}

	/// Test build-time vendor bundling use case
	#[rstest]
	fn test_use_case_vendor_bundling() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"js/vendor.js".to_string(),
			"js/vendor.abc123.js".to_string(),
		);
		manifest.insert(
			"css/vendor.css".to_string(),
			"css/vendor.def456.css".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let vendor_js = resolve_static("js/vendor.js");
		let vendor_css = resolve_static("css/vendor.css");

		assert!(vendor_js.contains("abc123"));
		assert!(vendor_css.contains("def456"));
	}

	/// Test analytics/tracking script loading
	#[rstest]
	fn test_use_case_tracking_scripts() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let tracking_js = resolve_static("js/tracking/analytics.js");

		assert_eq!(tracking_js, "/static/js/tracking/analytics.js");
	}

	/// Test favicon and app icons use case
	#[rstest]
	fn test_use_case_app_icons() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let icons = vec![
			"favicon.ico",
			"apple-touch-icon.png",
			"manifest.json",
			"robots.txt",
		];

		for icon in icons {
			let result = resolve_static(icon);
			assert!(result.contains(icon));
		}
	}
}

#[cfg(wasm)]
mod wasm_use_case_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_use_case_cdn() {
		init_static_resolver("https://cdn.example.com/static/".to_string());
		let result = resolve_static("css/style.css");
		assert_eq!(result, "https://cdn.example.com/static/css/style.css");
	}

	#[wasm_bindgen_test]
	fn test_wasm_use_case_theme() {
		init_static_resolver("/static/".to_string());
		let light = resolve_static("css/themes/light/style.css");
		let dark = resolve_static("css/themes/dark/style.css");
		assert!(light.contains("light"));
		assert!(dark.contains("dark"));
	}
}
