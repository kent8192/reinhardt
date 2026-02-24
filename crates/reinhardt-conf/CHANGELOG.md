# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.14...reinhardt-conf@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- *(workspace)* remove unpublished reinhardt-settings-cli and fix stale references

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.13...reinhardt-conf@v0.1.0-alpha.14) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.12...reinhardt-conf@v0.1.0-alpha.13) - 2026-02-21

### Fixed

- add database URL scheme validation before connection attempts
- fix .env parsing, AST formatter, and file safety issues
- document thread-safety invariant for env::set_var usage
- add missing media_root field in Settings::new
- fix key zeroing, file perms, and value redaction in admin-cli (#650, #656, #658)
- execute validation in validate command
- prevent encryption key exposure via CLI arguments
- prevent secret exposure in serialization
- use ManuallyDrop in into_inner to preserve ZeroizeOnDrop safety

### Security

- prevent duration underflow in rotation check and handle lock poisoning
- add input validation, file size limits, and TOCTOU mitigations
- redact sensitive values in error messages and env validation
- protect DatabaseConfig password and encode credentials in URLs

### Changed

- remove unnecessary async, glob imports, and strengthen validation
- extract secret types to always-available module
- change installed_apps and middleware defaults to empty vectors
- remove unused media_root field from Settings
- remove unused `middleware` string list from Settings
- remove unused `root_urlconf` field from Settings

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
- fix formatting after merge

### Documentation

- document planned-but-unimplemented settings fields
- wrap bare URL in backticks in azure provider doc comment

### Maintenance

- add SAFETY comments to unsafe blocks in secrets/providers/env.rs
- add SAFETY comments to unsafe blocks in sources.rs
- add SAFETY comments to unsafe blocks in profile.rs
- add SAFETY comments to unsafe blocks in env_loader.rs
- add SAFETY comments to unsafe blocks in testing.rs
- add SAFETY comments to unsafe blocks in env.rs

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.11...reinhardt-conf@v0.1.0-alpha.12) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-query

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.10...reinhardt-conf@v0.1.0-alpha.11) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.9...reinhardt-conf@v0.1.0-alpha.10) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.7...reinhardt-conf@v0.1.0-alpha.8) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.6...reinhardt-conf@v0.1.0-alpha.7) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.5...reinhardt-conf@v0.1.0-alpha.6) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-utils

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-alpha.4...reinhardt-conf@v0.1.0-alpha.5) - 2026-02-03

### Other

- merge main into chore/release-plz-migration
- add release-plz migration markers to CHANGELOGs

### Breaking Changes
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
- Update imports for `reinhardt_utils::staticfiles` module rename (#114)


## [0.1.0-alpha.3] - 2026-01-29 [YANKED]

**Note:** This version was yanked due to version skipping in the main crate (`reinhardt-web`). Use the latest available version instead.

### Changed

- Update imports for `reinhardt_utils::staticfiles` module rename (#114)

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

