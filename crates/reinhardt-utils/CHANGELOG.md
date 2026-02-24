# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.0-alpha.10...reinhardt-utils@v0.1.0-alpha.11) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-http

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.0-alpha.9...reinhardt-utils@v0.1.0-alpha.10) - 2026-02-21

### Added

- add path sanitization and input validation helpers
- add lock poisoning recovery utilities

### Fixed

- prevent panic on truncation underflow and UTF-8 boundary
- add security feature dependency for strip_tags_safe
- escape HTML in linebreaks/linebreaksbr and fix strip_tags
- handle DST gap in make_aware_local without panic
- prevent UTF-8 slicing panic in repr_array and repr_object
- escape values in format_html to prevent XSS (#748)
- add path validation to all LocalStorage methods
- add path traversal prevention with input validation

### Security

- add cancellation mechanism for auto-cleanup tasks
- recover from poisoned mutex/rwlock instead of panicking
- replace blocking KEYS with non-blocking SCAN+UNLINK
- replace recursive cleanup with bounded iterative loop
- fix XSS in error pages and media rendering, harden cache

### Styling

- apply workspace-wide formatting fixes
- apply rustfmt to pre-existing formatting violations in 16 files
- apply rustfmt formatting to workspace files
- apply code formatting to security fix files

### Testing

- add UTF-8 multibyte truncation boundary regression tests for #762

### Maintenance

- add explanatory comments to undocumented #[allow(...)] attributes

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.0-alpha.8...reinhardt-utils@v0.1.0-alpha.9) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.0-alpha.7...reinhardt-utils@v0.1.0-alpha.8) - 2026-02-08

### Fixed

- *(utils)* break circular publish dependency with reinhardt-test
- *(utils)* use fully qualified Result type in poll_until helpers

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.0-alpha.6...reinhardt-utils@v0.1.0-alpha.7) - 2026-02-06

### Fixed

- *(utils)* remove unused dev-dependencies to break circular publish chain

### Other

- Revert "Merge pull request #202 from kent8192/release-plz-2026-02-06T13-32-57Z"
- release

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.0-alpha.5...reinhardt-utils@v0.1.0-alpha.6) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.0-alpha.4...reinhardt-utils@v0.1.0-alpha.5) - 2026-02-03

### Other

- merge main into chore/release-plz-migration
- add release-plz migration markers to CHANGELOGs
- N/A

### Added
- Work in progress features (not yet released)

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A


<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.4] - 2026-01-30

### Changed

- Re-release of 0.1.0-alpha.3 content after version correction
- **BREAKING**: Rename `r#static` module to `staticfiles` (#114)
  - Module renamed from `reinhardt_utils::r#static` to `reinhardt_utils::staticfiles`
  - Feature renamed from `static` to `staticfiles`
  - Improves developer experience by eliminating raw identifier prefix


## [0.1.0-alpha.3] - 2026-01-29 [YANKED]

**Note:** This version was yanked due to version skipping in the main crate (`reinhardt-web`). Use the latest available version instead.

### Changed

- **BREAKING**: Rename `r#static` module to `staticfiles` (#114)
  - Module renamed from `reinhardt_utils::r#static` to `reinhardt_utils::staticfiles`
  - Feature renamed from `static` to `staticfiles`
  - Improves developer experience by eliminating raw identifier prefix


## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

