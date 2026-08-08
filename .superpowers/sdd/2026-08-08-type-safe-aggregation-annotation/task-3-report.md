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

## Scope Notes

- The static aggregate API contains no raw or dynamic construction escape
  hatch.
- `NumericAggregateStorage` is doc-hidden and cannot be implemented outside
  the framework because its `DatabaseScalar` supertrait is sealed.
