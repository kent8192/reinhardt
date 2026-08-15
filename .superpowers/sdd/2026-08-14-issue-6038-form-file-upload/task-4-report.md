# Task 4 Report: Model Form File Upload Controls

## Status

DONE_WITH_CONCERNS

## Implementation

- Model forms containing `File` or `Image` descriptors now render with
  `enctype="multipart/form-data"`.
- `Image` descriptors render an exact file input with `accept="image/*"`.
- Model file controls reuse the ordinary file-input change-event reader. The
  first selected browser `web_sys::File` is stored through
  `ModelFormState::set_file`; no JSON path or string surrogate is introduced.
- The generated submit snapshot continues to handle scalar values only. File
  arguments are supplied by the existing typed model-form dispatch from the
  selected file state.
- Failed submits retain selected file inputs and scalar DOM values. A successful
  generated submit clears selected file state and clears only file inputs found
  in the mounted form. The generated reset handler clears selected file state;
  native form reset clears the corresponding DOM controls.
- Ordinary file input bindings and model-form file controls share the same
  browser `ChangeEvent::files` extraction helper.

## Focused Coverage

Added `model_form_file_upload_wasm_test` with a browser fetch stub. It asserts:

- form multipart encoding, exact file input types, and `image/*` acceptance;
- first selected `web_sys::File` identity reaches multipart `FormData` for both
  required and optional file arguments;
- scalar form data remains JSON encoded;
- a failed request leaves the selected DOM files and scalar value unchanged;
- exactly one server-function request is emitted for the submit.

The existing native `server_fn_native_handler_tests` harness contains a typed
multipart route and tests scalar-plus-file handling, absent optional files,
empty required files, and empty optional files. It is the narrow transport
contract for this task; no new route was needed.

## Validation

Shared `/Volumes/cache` was full, so all successful Cargo commands used
isolated `/tmp` target and build directories.

Passed:

```text
cargo fmt --all

CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
CHROMEDRIVER="$(command -v chromedriver)" \
WASM_BINDGEN_TEST_ONLY_WEB=1 \
cargo --config 'build.build-dir="/tmp/reinhardt-6038-task4-build.fQu62M"' \
  test --target-dir /tmp/reinhardt-6038-task4-target.hhHzKs \
  -p reinhardt-pages --target wasm32-unknown-unknown --features testing \
  --test model_form_file_upload_wasm_test
# 1 passed; 0 failed

cargo --config 'build.build-dir="/tmp/reinhardt-6038-task4-build-host"' \
  check --target-dir /tmp/reinhardt-6038-task4-target-host \
  -p reinhardt-pages-macros
# passed

git diff --check
# passed
```

Not completed:

```text
cargo --config 'build.build-dir="/tmp/reinhardt-6038-task4-build-host"' \
  test --target-dir /tmp/reinhardt-6038-task4-target-host \
  -p reinhardt-pages --test server_fn_native_handler_tests multipart_
```

The native command began a cold isolated dependency build but did not complete.
It therefore has no
pass/fail result in this task.

## Concerns

- The focused browser test validates render, exact selected file transport,
  failed-submit retention, and one-request behavior. It does not execute a
  stable end-to-end assertion of the generated success/reset DOM lifecycle:
  the current reactive test harness replaces mounted nodes across the async
  success transition, making retained `HtmlInputElement` handles stale. The
  generated success/reset code is implemented, but that browser lifecycle
  assertion remains a follow-up test gap.
- No native HTTP integration result is recorded because its command did not
  complete; the existing typed multipart handler tests remain the
  intended integration contract.
