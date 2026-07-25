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

#[test]
fn test_head_macro_pass() {
	trybuild::TestCases::new().pass("tests/ui/head/pass/*.rs");
}

#[test]
fn test_head_macro_fail() {
	trybuild::TestCases::new().compile_fail("tests/ui/head/fail/*.rs");
}

// server_fn macro tests
#[test]
fn test_server_fn_macro_ui() {
	let t = trybuild::TestCases::new();
	// Guard query-key code generation against breaking existing server functions.
	t.pass("tests/ui/server_fn/query_key_custom_result_alias.rs");
	// MSW mock arguments intentionally require serializable, cloneable request types.
	// This fixture isolates the non-MSW native compatibility guarantee.
	#[cfg(not(feature = "msw"))]
	t.pass("tests/ui/server_fn/query_key_non_query_args.rs");
	t.pass("tests/ui/server_fn/query_key_private_interfaces.rs");
	t.pass("tests/ui/server_fn/query_key_injected_no_msw.rs");
	#[cfg(feature = "model-server-fnset")]
	t.pass("tests/ui/server_fn/injected_database_connection_copy.rs");
	t.pass("tests/ui/server_fn/codec_json.rs");
	t.pass("tests/ui/server_fn/codec_url.rs");
	// Fixes #3666: verify server_fn compiles without msw feature (no check-cfg errors)
	#[cfg(not(feature = "msw"))]
	t.pass("tests/ui/server_fn/no_msw_feature.rs");
	// Verify injected server_fn params do not leave regular args unused in generated helpers.
	t.pass("tests/ui/server_fn/inject_query_key_no_unused.rs");
	t.pass("tests/ui/server_fn/result_alias_query_key.rs");
	t.pass("tests/ui/server_fn/response_metadata.rs");
	t.pass("tests/ui/server_fn/result_alias.rs");
	t.pass("tests/ui/server_fn/structured_error_public_api.rs");
	t.pass("tests/ui/server_fn/auto_register_default.rs");
	t.pass("tests/ui/server_fn/auto_register_opt_out.rs");
	t.pass("tests/ui/server_fn/auto_register_name_collision.rs");
	t.pass("tests/ui/server_fn/short_imported_request.rs");
	t.pass("tests/ui/server_fn/no_cfg_aliases.rs");
	// Issue #3858: verify FromRequest extractor params work in #[server_fn]
	t.pass("tests/ui/server_fn/with_extractors.rs");
}

#[test]
fn test_server_fn_inject_patterns() {
	trybuild::TestCases::new().pass("tests/ui/server_fn/with_inject.rs");
}

#[test]
fn test_server_fn_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/server_fn/fail/*.rs");
}

#[test]
fn test_server_fnset_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/server_fnset/pass/low_level.rs");
	#[cfg(feature = "model-server-fnset")]
	{
		t.pass("tests/ui/server_fnset/pass/model_*.rs");
		t.pass("tests/ui/server_fnset/pass/overrides_and_actions.rs");
	}
}

#[test]
fn test_server_fnset_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/server_fnset/fail/empty_name.rs");
	t.compile_fail("tests/ui/server_fnset/fail/invalid_item.rs");
	t.compile_fail("tests/ui/server_fnset/fail/missing_for.rs");
	t.compile_fail("tests/ui/server_fnset/fail/missing_name.rs");
	t.compile_fail("tests/ui/server_fnset/fail/unknown_key.rs");
	t.compile_fail("tests/ui/server_fnset/fail/unsafe_name.rs");
	#[cfg(feature = "model-server-fnset")]
	{
		t.compile_fail("tests/ui/server_fnset/fail/action_*.rs");
		t.compile_fail("tests/ui/server_fnset/fail/collection_lookup.rs");
		t.compile_fail("tests/ui/server_fnset/fail/duplicate_*.rs");
		t.compile_fail("tests/ui/server_fnset/fail/endpoint_collision.rs");
		t.compile_fail("tests/ui/server_fnset/fail/invalid_lookup.rs");
		t.compile_fail("tests/ui/server_fnset/fail/mismatched_for.rs");
		t.compile_fail("tests/ui/server_fnset/fail/missing_dto_mapping.rs");
		t.compile_fail("tests/ui/server_fnset/fail/non_unique_lookup.rs");
		t.compile_fail("tests/ui/server_fnset/fail/transactional_raw_connection.rs");
		t.compile_fail("tests/ui/server_fnset/fail/wrong_*.rs");
	}
}

#[cfg(feature = "model-server-fnset")]
#[test]
fn test_model_server_fnset_contract_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/server_fnset/model_contract/fail/*.rs");
}

#[cfg(not(feature = "model-server-fnset"))]
#[test]
fn test_model_server_fnset_feature_boundary_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/server_fnset/feature_boundary/fail/*.rs");
}

#[test]
fn test_client_form_choices_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/client_form/choices/pass/*.rs");
}

#[test]
fn test_client_form_choices_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/client_form/choices/fail/*.rs");
}

#[test]
fn test_client_form_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/client_form/pass/*.rs");
}

#[test]
fn test_client_form_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/client_form/fail/*.rs");
}

#[test]
fn test_wasm_server_api_macro_ui_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/wasm_server_api/pass/*.rs");
}

#[test]
fn test_wasm_server_api_macro_ui_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/wasm_server_api/fail/*.rs");
}

// Issues #5511 / #5577: React-parity hooks require a dependency mode as their
// second argument. These UI tests pin the public signatures and diagnostics:
// - `deps![...]` is explicit and statically checked inside `page!` bodies.
// - `deps_auto!()` is accepted only by effects, layout effects, and memos.
// - Legacy unit and tuple arguments are rejected by the type checker.
#[test]
fn test_hooks_dependency_modes_ui_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/hooks/pass/*.rs");
}

#[test]
fn test_hooks_dependency_modes_ui_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/hooks/fail/*.rs");
}

#[test]
fn test_from_request_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/from_request/pass/*.rs");
}

#[test]
fn test_from_request_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/from_request/fail/*.rs");
}

#[test]
fn test_page_props_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/page_props/pass/*.rs");
}

#[test]
fn test_page_props_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/page_props/fail/*.rs");
}

#[test]
fn test_component_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/component/pass/*.rs");
}

#[test]
fn test_component_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/component/fail/*.rs");
}

#[test]
fn test_layout_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/layout/pass/*.rs");
}

#[test]
fn test_layout_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/layout/fail/*.rs");
}

#[test]
fn test_loader_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/loader/pass/*.rs");
}

#[test]
fn test_loader_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/loader/fail/*.rs");
}

#[test]
fn test_client_page_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/client_page/pass/*.rs");
}

#[test]
fn test_client_page_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/client_page/fail/*.rs");
}
use rstest::rstest;

#[rstest]
fn test_style_macro_pass() {
	let cases = trybuild::TestCases::new();
	cases.pass("tests/ui/style/pass/*.rs");
}

#[rstest]
fn test_style_macro_fail() {
	let cases = trybuild::TestCases::new();
	cases.compile_fail("tests/ui/style/fail/*.rs");
}
