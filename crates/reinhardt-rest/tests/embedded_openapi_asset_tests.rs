//! Content verification tests for embedded OpenAPI assets.
//!
//! Verifies that embedded Swagger/ReDoc PNG favicons have valid PNG headers
//! and HTML templates contain expected markup. Covers Issue #3123.

use rstest::*;

const SWAGGER_FAVICON: &[u8] = include_bytes!("../assets/swagger.png");
const REDOC_FAVICON: &[u8] = include_bytes!("../assets/redoc.png");
const SWAGGER_UI_TEMPLATE: &str = include_str!("../src/openapi/templates/swagger_ui.tpl");
const REDOC_UI_TEMPLATE: &str = include_str!("../src/openapi/templates/redoc_ui.tpl");

/// PNG magic bytes: 0x89 P N G
const PNG_MAGIC: [u8; 4] = [0x89, 0x50, 0x4E, 0x47];

#[rstest]
fn swagger_png_is_not_empty() {
	// Assert
	assert!(
		!SWAGGER_FAVICON.is_empty(),
		"swagger.png should not be empty"
	);
}

#[rstest]
fn swagger_png_has_valid_header() {
	// Assert - PNG files must start with the PNG magic bytes
	assert!(
		SWAGGER_FAVICON.starts_with(&PNG_MAGIC),
		"swagger.png should start with PNG magic bytes"
	);
}

#[rstest]
fn redoc_png_is_not_empty() {
	// Assert
	assert!(!REDOC_FAVICON.is_empty(), "redoc.png should not be empty");
}

#[rstest]
fn redoc_png_has_valid_header() {
	// Assert
	assert!(
		REDOC_FAVICON.starts_with(&PNG_MAGIC),
		"redoc.png should start with PNG magic bytes"
	);
}

#[rstest]
fn swagger_template_contains_html_markup() {
	// Assert
	assert!(
		!SWAGGER_UI_TEMPLATE.is_empty(),
		"swagger_ui.tpl should not be empty"
	);
	assert!(
		SWAGGER_UI_TEMPLATE.contains("<html") || SWAGGER_UI_TEMPLATE.contains("<!DOCTYPE"),
		"swagger_ui.tpl should contain HTML markup"
	);
}

#[rstest]
fn redoc_template_contains_html_markup() {
	// Assert
	assert!(
		!REDOC_UI_TEMPLATE.is_empty(),
		"redoc_ui.tpl should not be empty"
	);
	assert!(
		REDOC_UI_TEMPLATE.contains("<html") || REDOC_UI_TEMPLATE.contains("<!DOCTYPE"),
		"redoc_ui.tpl should contain HTML markup"
	);
}
