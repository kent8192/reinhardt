# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-i18n@v0.1.0) - 2026-03-08

### Changed

- remove 8 unused dependencies from Cargo.toml

### Fixed

- *(release)* move reinhardt-test to optional dep in non-cyclic crates
- *(i18n)* remove Hungarian from no-plural language group
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

### Documentation

- add missing doc comments for public API modules and types

### Styling

- apply rustfmt to pre-existing formatting violations in 16 files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

### Reverted

- undo PR #219 version bumps for unpublished crates
