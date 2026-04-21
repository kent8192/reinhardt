#![cfg(not(target_arch = "wasm32"))]
//! Compile-time tests for page! and form! macros using trybuild
//!
//! This test suite validates that:
//! - Valid page! macro usage compiles successfully (tests/ui/page/pass/*.rs)
//! - Invalid page! macro usage fails to compile (tests/ui/page/fail/*.rs)
//! - Valid form! macro usage compiles successfully (tests/ui/form/pass/*.rs)
//! - Invalid form! macro usage fails to compile (tests/ui/form/fail/*.rs)

#[test]
fn test_page_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/page/pass/*.rs");
}

#[test]
fn test_page_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/page/fail/*.rs");
}

#[test]
fn test_form_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/form/pass/*.rs");
}

#[test]
fn test_form_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/form/fail/*.rs");
}

// server_fn macro tests
#[test]
fn test_server_fn_macro_ui() {
	let t = trybuild::TestCases::new();
	// Codec tests
	t.pass("tests/ui/server_fn/codec_json.rs");
	t.pass("tests/ui/server_fn/codec_url.rs");
	// Fixes #3666: verify server_fn compiles without msw feature (no check-cfg errors)
	t.pass("tests/ui/server_fn/no_msw_feature.rs");
	// Issue #3858: verify FromRequest extractor params work in #[server_fn]
	t.pass("tests/ui/server_fn/with_extractors.rs");
}
