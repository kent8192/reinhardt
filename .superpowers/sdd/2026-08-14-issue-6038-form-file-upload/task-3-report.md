# Task 3 Report: Typed Model-Form Server-Function Dispatch

## Changed files

- `crates/reinhardt-pages/src/form/model.rs`
  - Added hidden selection and dispatch contracts plus scalar JSON argument decoding.
  - Added a manual `Clone` implementation for `ModelFormState` so schema and policy types do not require `Clone` when dispatch snapshots form state.
- `crates/reinhardt-pages/src/form.rs`
  - Re-exported the hidden model-form dispatch contracts for macro expansion.
- `crates/reinhardt-pages/macros/src/server_fn.rs`
  - Generated named hidden argument markers and `ModelFormServerFn` implementations.
  - Multipart server functions dispatch scalar arguments through `json_argument` and file arguments through the existing typed file helpers.
  - JSON model-form server functions retain a single generated payload dispatch path.
- `crates/reinhardt-pages/macros/src/form/codegen.rs`
  - Generated a private explicit-fields selection type with exact positional name/count implementations.
  - Replaced direct payload invocation with the hidden marker dispatch contract on WASM.
  - Preserved native JSON fixture compatibility by not requiring the new marker contract outside WASM.
- `crates/reinhardt-pages/tests/model_form_multipart_wasm_compile.rs`
  - Added WASM compilation coverage for a scalar, required file, and optional file multipart submission.
- `crates/reinhardt-pages/tests/ui/form/model_multipart_support.rs`
  - Added shared multipart model/schema/server-function fixture support.
- `crates/reinhardt-pages/tests/ui/form/pass/model_multipart.rs`
  - Added the passing exact multipart selection fixture.
- `crates/reinhardt-pages/tests/ui/form/fail/model_multipart_{count_mismatch,order_mismatch,exclude}.{rs,stderr}`
  - Added compile-fail coverage for exact count, exact order/name, and unsupported multipart `exclude` selection.
- `crates/reinhardt-pages/tests/ui/form/fail/file_server_fn_{count_mismatch,name_mismatch,order_mismatch}.stderr`
  - Updated expected diagnostics because generated argument marker types now use actual parameter names.

## Decisions

- Explicit `fields` selections create a private selection marker that encodes count and positional field names. Multipart marker implementations require every encoded bound, so missing, extra, reversed, and renamed selections fail compilation.
- JSON model-form dispatch uses a hidden payload-selection trait, without count or name bounds. This preserves one-payload JSON continuity for both explicit `fields` and `exclude`.
- Multipart dispatch remains WASM-only. Native model-form expansion deliberately avoids naming the new dispatch contract so existing hand-written JSON marker fixtures continue to compile.
- Payload decoding errors are mapped to `ServerFnError::validation_with_message`, matching the existing form validation path.

## Commands and results

| Command | Result |
| --- | --- |
| `CARGO_BUILD_BUILD_DIR=/tmp/reinhardt-6038-task3-ui-build CARGO_TARGET_DIR=/tmp/reinhardt-6038-task3-ui-target RUSTC_WRAPPER= cargo test -p reinhardt-pages --test ui test_form_macro_pass -- --exact` | Passed. |
| `CARGO_BUILD_BUILD_DIR=/tmp/reinhardt-6038-task3-ui-build CARGO_TARGET_DIR=/tmp/reinhardt-6038-task3-ui-target RUSTC_WRAPPER= cargo test -p reinhardt-pages --test ui test_form_macro_fail -- --exact` | Passed: 1 test, 41 UI fixtures including the new multipart failures. |
| `CARGO_BUILD_BUILD_DIR=/tmp/reinhardt-6038-task3-wasm-build CARGO_TARGET_DIR=/tmp/reinhardt-6038-task3-wasm-target RUSTC_WRAPPER= cargo check -p reinhardt-pages --test model_form_multipart_wasm_compile --target wasm32-unknown-unknown` | Passed. |
| `cargo fmt --all -- --check` | Passed before the final interruption. |
| `git diff --check` | Passed immediately before staging. |
| `CARGO_BUILD_BUILD_DIR=/tmp/reinhardt-6038-task3-native-build CARGO_TARGET_DIR=/tmp/reinhardt-6038-task3-native-target RUSTC_WRAPPER= cargo check -p reinhardt-pages` | Interrupted during dependency compilation; no result recorded. |

## Concerns

- Broader native pages validation was intentionally interrupted at the request to finalize; it remains to be run in follow-up validation.
- Existing workspace patch warnings for unused Topiary patches appeared during UI checks and are unrelated to this task.
