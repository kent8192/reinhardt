# Task 6 report

## Status

Completed consumer-process coverage for clean, violating, and broken
applications using one materialized Cargo fixture.

## Coverage

- Clean application exits successfully with the exact `Verification passed.`
  summary.
- Violating application reports schema, authorization, and settings findings;
  excludes the unmounted endpoint and redacts the secret sentinel and dynamic
  map key.
- Repeated violating runs produce identical stdout and stderr.
- A deliberately broken consumer source makes the nested Cargo check fail and
  prevents framework findings from being emitted.

## Verification

- `CARGO_HOME=/tmp/reinhardt-cargo-home CARGO_TARGET_DIR=/tmp/reinhardt-task6-test CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p reinhardt-commands --features contract --test contract_verify_consumer -- --exact consumer_processes_cover_clean_violating_and_cargo_failure --nocapture`: 1 test passed in 87.96s.
- `cargo fmt --all` and `git diff --check`: passed.

The test propagates the active Rust toolchain and uses isolated nested Cargo
targets so the replay observes the same compiler context as the consumer.
