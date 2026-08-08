# Task 3 Report: Static Typed Aggregate Constructors

## Delivered

- Added `orm::func` with static `count_all`, `count`, `sum`, `avg`, `min`, and
  `max` constructors plus scalar `literal`, `coalesce`, and `case_when`
  wrappers.
- Added public sealed aggregate operand contracts. Only generated root fields,
  generated related fields, and generated relation paths can satisfy them.
- Promoted the Task 2 numeric storage result mapping to a doc-hidden public
  trait. This lets an application opt a custom `DatabaseField<Storage = i64>`
  into `NumericAggregateField` without duplicating type mapping logic.
- Added structured `CountAll` support and converts relation-path `COUNT` joins
  to left joins over the target primary-key column.
- Added scalar `TypedPredicate` and aggregate `HavingPredicate` comparisons.
  Each comparison encodes its right side through the left result type's
  `DatabaseField` contract.
- Added the requested trybuild harness, three pass fixtures, and four operand
  rejection fixtures.

## Validation

- `cargo check --target-dir /tmp/reinhardt-issue-5811-task3-check -p reinhardt-db --all-features` — PASS, no Task 3 warnings.
- `cargo fmt --check --all` — PASS.
- `git diff --check` — PASS.
- `cargo test --target-dir /tmp/reinhardt-issue-5811-task3-target -p reinhardt-db --test typed_aggregation_ui --all-features` — compile phase completed, but the first pass fixture's `trybuild000` child remained asleep in macOS `_dyld_start` before Rust `main` (CPU 0%). It was terminated as a Task 3-only process after diagnosis. No Task 3 compile diagnostic was emitted, so reviewed `.stderr` snapshots still require a healthy trybuild child-launch environment.
- `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/reinhardt-issue-5811-task3-trybuild cargo test --target-dir /tmp/reinhardt-issue-5811-task3-trybuild -p reinhardt-db --test typed_aggregation_ui --no-default-features --features orm -- --nocapture` — the outer test build and trybuild support crate completed. The first pass fixture (`custom_field.rs`) then entered its own `trybuild000` Cargo build. Its macOS build-script children (including `icu_normalizer_data` and `icu_properties_data`) repeatedly remained in `_dyld_start` with CPU 0% before Rust `main`; only the Task 3 parent/child process tree was terminated. Therefore the command did not reach a natural exit (SIGTERM, exit 143-equivalent), emitted no Task 3 API diagnostic, and produced no `.stderr` snapshots.
- `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/reinhardt-issue-5811-task3-trybuild cargo check --target-dir /tmp/reinhardt-issue-5811-task3-trybuild -p reinhardt-db --test typed_aggregation_ui --no-default-features --features orm` — the focused check began normal dependency checking, but its `rustls` build script also remained in `_dyld_start` with CPU 0% for more than 90 seconds before Rust `main`. The Task 3-only process was terminated, so it did not reach a natural exit (SIGTERM, exit 143-equivalent).

## Trybuild Environment Finding

The limited-feature retry reproduced the macOS dyld child-launch symptom inside
trybuild fixture compilation, so the evidence does not support the initial
all-features pressure hypothesis. The failure occurs before fixture source is
compiled: `custom_field.rs` did not pass or fail, no reject fixture was reached,
and no expected-error snapshot was generated. The static aggregate APIs remain
covered by the completed all-feature crate check, but trybuild snapshots must be
reviewed and committed in an environment where child build scripts can reach
Rust `main`.

## Scope Notes

- The static aggregate API contains no raw or dynamic construction escape
  hatch.
- `NumericAggregateStorage` is doc-hidden and cannot be implemented outside
  the framework because its `DatabaseScalar` supertrait is sealed.
