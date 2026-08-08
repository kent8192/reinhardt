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
- Added structured operand-aggregate `DISTINCT` state. `COUNT(*)` remains the
  separate operand-free node; because the approved public return signatures use
  the same `TypedExpression<_, _, AggregateKind>` type, `distinct()` preserves
  that signature and rejects `COUNT(*)` at runtime. `try_distinct()` exposes
  the validation error for callers whose aggregate source is not statically
  known.
- Restored direct `TypedPredicate` and `HavingPredicate` comparison return
  values so existing filter callers do not receive an unexpected `Result`.
- Added the requested trybuild harness, three pass fixtures, and four operand
  rejection fixtures.

## Validation

- `cargo check --target-dir /tmp/reinhardt-issue-5811-task3-check -p reinhardt-db --all-features` — PASS, no Task 3 warnings.
- `cargo fmt --check --all` — PASS.
- `git diff --check` — PASS.
- `cargo test --target-dir /tmp/reinhardt-issue-5811-task3-target -p reinhardt-db --test typed_aggregation_ui --all-features` — compile phase completed, but the first pass fixture's `trybuild000` child remained asleep in macOS `_dyld_start` before Rust `main` (CPU 0%). It was terminated as a Task 3-only process after diagnosis. No Task 3 compile diagnostic was emitted.
- `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/reinhardt-issue-5811-task3-trybuild cargo test --target-dir /tmp/reinhardt-issue-5811-task3-trybuild -p reinhardt-db --test typed_aggregation_ui --no-default-features --features orm -- --nocapture` — the outer test build and trybuild support crate completed. The first pass fixture (`custom_field.rs`) then entered its own `trybuild000` Cargo build. Its macOS build-script children (including `icu_normalizer_data` and `icu_properties_data`) repeatedly remained in `_dyld_start` with CPU 0% before Rust `main`; only the Task 3 parent/child process tree was terminated. Therefore the command did not reach a natural exit (SIGTERM, exit 143-equivalent), emitted no Task 3 API diagnostic, and produced no `.stderr` snapshots.
- `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/reinhardt-issue-5811-task3-trybuild cargo check --target-dir /tmp/reinhardt-issue-5811-task3-trybuild -p reinhardt-db --test typed_aggregation_ui --no-default-features --features orm` — the focused check began normal dependency checking, but its `rustls` build script also remained in `_dyld_start` with CPU 0% for more than 90 seconds before Rust `main`. The Task 3-only process was terminated, so it did not reach a natural exit (SIGTERM, exit 143-equivalent).
- `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/reinhardt-issue-5811-task3-fix-test cargo test --target-dir /tmp/reinhardt-issue-5811-task3-fix-test -p reinhardt-db --lib query_fields::expression --all-features --no-run` — the captured log contains only `Compiling reinhardt-core` and `Compiling reinhardt-macros`; it has no exit-status footer. The process was later absent, so its natural-exit versus termination status cannot be established from the log.
- Direct `rustc --cfg trybuild --verbose` checks against the already-built Task 3 ORM artifact — all four reject fixtures failed with their intended sealed operand contracts. The reviewed, Trybuild-normalized diagnostics are committed as the four `.stderr` snapshots.

## Trybuild Environment Finding

The limited-feature retry reproduced the macOS dyld child-launch symptom inside
trybuild fixture compilation, so the evidence does not support the initial
all-features pressure hypothesis. The failure occurs before fixture source is
compiled: `custom_field.rs` did not pass or fail and no reject fixture was
reached by the harness. The static aggregate APIs remain covered by the
completed all-feature crate check. The four reviewed snapshots use Trybuild's
standard path normalization and were confirmed from direct compiler output;
the normal harness still needs one healthy child-launch environment to exercise
both pass fixtures and saved snapshots end to end.

The snapshot generation command is deterministic once child build scripts can
launch: run `TRYBUILD=overwrite cargo test -p reinhardt-db --test
typed_aggregation_ui --no-default-features --features orm`, review the four
generated files under `tests/ui/typed_aggregation/fail/`, then rerun the same
command without `TRYBUILD=overwrite`.

## Scope Notes

- The static aggregate API contains no raw or dynamic construction escape
  hatch.
- `NumericAggregateStorage` is doc-hidden and cannot be implemented outside
  the framework because its `DatabaseScalar` supertrait is sealed.
- The `COUNT(*)` distinct handling is a signature-preserving runtime guard:
  the approved constructors all return `TypedExpression<_, _, AggregateKind>`,
  so Rust cannot expose `distinct()` for operand nodes while omitting it from
  `count_all()` without changing those public return types. `CountAll` retains
  no distinct state; `try_distinct()` reports the validation error and
  `distinct()` panics with the same reason.
