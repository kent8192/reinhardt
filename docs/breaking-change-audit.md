# Breaking Change Audit

This document records the results of the 5-pass breaking change audit performed
for the RC release. The audit follows the methodology recommended by the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).

## Audit Methodology

The audit is performed in 5 passes, each focusing on a different category of
breaking change as defined by [RFC 1105](https://rust-lang.github.io/rfcs/1105-api-evolution.html).

### Pass 1: Deleted Items

Items removed from the public API that were present in the previous release.

| Item | Type | Status | Notes |
|------|------|--------|-------|
| — | — | No deletions found | Audit date: 2026-02-25 |

### Pass 2: Signature Changes

Changes to the signatures of existing public functions, methods, or types.

| Item | Change | Status | Notes |
|------|--------|--------|-------|
| — | — | No signature changes | Audit date: 2026-02-25 |

### Pass 3: Trait Changes

Changes to trait definitions, implementations, or required bounds.

| Item | Change | Status | Notes |
|------|--------|--------|-------|
| — | — | No trait breaking changes | Audit date: 2026-02-25 |

### Pass 4: Type Representation Changes

Changes to the layout or representation of types (e.g., added fields to
non-`#[non_exhaustive]` structs).

| Item | Change | Status | Notes |
|------|--------|--------|-------|
| — | — | Mitigated by `#[non_exhaustive]` | All public structs/enums now marked |

### Pass 5: Behavioral Changes

Changes to observable behavior that code depending on specific behavior may
rely on.

| Item | Change | Status | Notes |
|------|--------|--------|-------|
| — | — | No behavioral changes | Audit date: 2026-02-25 |

## CI Integration

Automated SemVer checking is performed by `cargo-semver-checks` on every PR
targeting `main`. See `.github/workflows/semver-check.yml`.

## Findings Summary

- No breaking changes detected in current RC audit
- `#[non_exhaustive]` added to all public error enums and config structs as
  preventative measure
- All public APIs maintain backward compatibility with 0.1.x baseline

## References

- [RFC 1105: API Evolution](https://rust-lang.github.io/rfcs/1105-api-evolution.html)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks)
