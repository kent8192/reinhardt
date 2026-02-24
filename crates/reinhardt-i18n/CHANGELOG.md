# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.1.0-alpha.5...reinhardt-i18n@v0.1.0-alpha.6) - 2026-02-21

### Fixed

- handle special float values and add format string limit
- add input size limits to PO file parser
- add length limit to validate_locale()
- use try_borrow_mut in TranslationGuard::drop to prevent reentrant panic
- add comprehensive plural rules and fix negative number formatting
- replace mem::forget with proper guard handling (#713)
- prevent path traversal in CatalogLoader::load (#714)
- add plural index validation to prevent memory exhaustion
- add path traversal prevention with input validation
- roll back unpublished crate versions after partial release failure
- roll back unpublished crate versions and enable release_always
- revert unpublished crate versions to pre-release state

### Security

- apply validate_locale uniformly across all entry points

### Changed

- remove 8 unused dependencies from Cargo.toml

### Styling

- apply rustfmt to pre-existing formatting violations in 16 files

### Reverted

- undo PR #219 version bumps for unpublished crates
- undo release PR #215 version bumps

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.1.0-alpha.4...reinhardt-i18n@v0.1.0-alpha.5) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di, reinhardt-di

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.1.0-alpha.3...reinhardt-i18n@v0.1.0-alpha.4) - 2026-02-03

### Other

- updated the following local packages: reinhardt-di, reinhardt-di, reinhardt-test

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.1.0-alpha.2...reinhardt-i18n@v0.1.0-alpha.3) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections

<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.2] - 2026-01-23

### Changed

- Re-publish with correct repository URL (reinhardt-web)

## [0.1.0-alpha.1] - 2026-01-23 [YANKED]

### Added

- Initial crates.io release

