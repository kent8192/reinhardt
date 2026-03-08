# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-utils@v0.1.0) - 2026-03-08

### Added

- add path sanitization and input validation helpers
- add lock poisoning recovery utilities

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention
- **BREAKING**: Rename `r#static` module to `staticfiles` (#114)
- Module renamed from `reinhardt_utils::r#static` to `reinhardt_utils::staticfiles`
- Feature renamed from `static` to `staticfiles`
- Improves developer experience by eliminating raw identifier prefix
- **BREAKING**: Rename `r#static` module to `staticfiles` (#114)
- Module renamed from `reinhardt_utils::r#static` to `reinhardt_utils::staticfiles`
- Feature renamed from `static` to `staticfiles`
- Improves developer experience by eliminating raw identifier prefix

### Fixed

- *(utils)* capitalize only first character in capfirst function
- *(meta)* fix workspace inheritance and authors metadata
- *(staticfiles)* unify manifest.json format to use "paths" key
- prevent panic on truncation underflow and UTF-8 boundary
- add security feature dependency for strip_tags_safe
- escape HTML in linebreaks/linebreaksbr and fix strip_tags
- handle DST gap in make_aware_local without panic
- prevent UTF-8 slicing panic in repr_array and repr_object
- escape values in format_html to prevent XSS (#748)
- add path validation to all LocalStorage methods
- add path traversal prevention with input validation
- *(utils)* break circular publish dependency with reinhardt-test
- *(utils)* use fully qualified Result type in poll_until helpers

### Security

- add cancellation mechanism for auto-cleanup tasks
- recover from poisoned mutex/rwlock instead of panicking
- replace blocking KEYS with non-blocking SCAN+UNLINK
- replace recursive cleanup with bounded iterative loop
- fix XSS in error pages and media rendering, harden cache

### Documentation

- add missing doc comments for public API modules and types

### Testing

- add UTF-8 multibyte truncation boundary regression tests for #762

### Styling

- apply workspace-wide formatting fixes
- apply rustfmt to pre-existing formatting violations in 16 files
- apply rustfmt formatting to workspace files
- apply code formatting to security fix files

### Maintenance

- add explanatory comments to undocumented #[allow(...)] attributes

### Other

- resolve conflict with main (criterion version)
