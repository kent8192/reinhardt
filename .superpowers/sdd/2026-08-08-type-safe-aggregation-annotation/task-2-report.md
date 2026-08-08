# Task 2 Report: Typed Expression Kinds, Nodes, Labels, and Numeric Opt-In

## Delivered

- Added sealed scalar and aggregate expression-kind markers with the complete
  composition matrix.
- Replaced the typed-expression raw SQL payload with structured nodes and join
  requirements. Nodes retain root and related column metadata, literals,
  aggregates, arithmetic, CASE, COALESCE, and the crate-private existing
  `SimpleExpr` compatibility node.
- Added validated labels that erase only the expression result type, retain the
  model and expression kind, and reject invalid SQL identifiers before
  allocation.
- Added numeric aggregate opt-in for the supported scalar field types and
  nullable variants, with private storage output metadata.
- Added scalar arithmetic, literal, COALESCE, and CASE composition APIs.
- Updated `QuerySet::annotate_expr` and `QuerySet::select_expr` to consume the
  new compiled expression accessor. This was the minimal compatibility change
  required because the Task 2 brief's field map omitted the two existing direct
  `.expr` consumers in `orm/query.rs`.
- Updated three pgvector trybuild stderr snapshots following Task 1's generated
  support-type naming change. The failing diagnostics remained the same type
  safety failures; only `support::Document` rendering changed to `Document`.

## Test-first Evidence

- Added label validation tests before implementation and observed the expected
  missing-API compile failure.
- Added structural de-duplication assertions for identical expressions and
  distinct physical root columns.

## Validation

- `cargo test --target-dir /tmp/reinhardt-issue-5811-task2-target -p reinhardt-db --lib query_fields::expression --all-features` — PASS (6 tests).
- `cargo test --target-dir /tmp/reinhardt-issue-5811-task2-target -p reinhardt-db --lib query_fields --all-features` — PASS (18 tests).
- `cargo test --target-dir /tmp/reinhardt-issue-5811-task2-target -p reinhardt-db --test vector_expression --features pgvector` — PASS after snapshot refresh.
- `cargo check --target-dir /tmp/reinhardt-issue-5811-task2-target -p reinhardt-db --all-features` — PASS.
- `cargo fmt --check` and `git diff --check` — PASS.

The validation commands emit pre-existing warnings from unrelated migration,
model-macro, transaction, and backend code; Task 2 introduces no new compiler
warnings in the final checks.

## Scope Notes

- No docs, plan, or specification files were changed; this report is the
  requested Task 2 handoff artifact.
- Related-column joins are retained structurally in `JoinRequirements`; the
  subsequent annotation projection task is responsible for consuming those
  requirements when building query joins.

## Review Fixes

- Restricted `TypedExpression` field conversions to
  `FieldRef<_, _, GeneratedModelField>` and
  `RelatedFieldRef<_, _, _, GeneratedRelatedField>`. Safe, manually constructed
  unverified origins can no longer enter typed-expression operands. The
  distinct-column fixture now supplies generated-marker proofs instead of
  `FieldRef::new`.
- Added structural comparison for `ExpressionNode::Case`, comparing the
  predicate debug representation and recursively comparing result and optional
  otherwise branches. Identical CASE expressions now deduplicate.
- Added `identical_case_nodes_are_deduplicated` with a strict single-node
  assertion.

## Review-Fix Validation

- `cargo test --target-dir /tmp/reinhardt-issue-5811-task2-target -p reinhardt-db --lib query_fields::expression::tests::identical_case_nodes_are_deduplicated --all-features -- --exact --nocapture` — exit code 0 and `test result: ok`; its filter is missing the `orm::` module prefix, so Cargo reported 0 tests.
- `/Volumes/cache/cargo-build/79/3d926a951755d3/debug/deps/reinhardt_db-3a0602e3a1f4adeb orm::query_fields::expression::tests::identical_case_nodes_are_deduplicated --exact --nocapture` — PASS: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3520 filtered out`.
- `cargo fmt --check` — PASS.
- `git diff --check` — PASS.
