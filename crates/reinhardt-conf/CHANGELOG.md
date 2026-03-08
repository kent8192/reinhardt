# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-conf@v0.1.0) - 2026-03-08

### Changed

- remove unnecessary async, glob imports, and strengthen validation
- extract secret types to always-available module
- change installed_apps and middleware defaults to empty vectors
- remove unused media_root field from Settings
- remove unused `middleware` string list from Settings
- remove unused `root_urlconf` field from Settings
- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention
- Update imports for `reinhardt_utils::staticfiles` module rename (#114)
- Update imports for `reinhardt_utils::staticfiles` module rename (#114)

### Fixed

- *(conf)* replace parking_lot::Mutex with tokio::sync::Mutex in DynamicSettings hot-reload
- *(deps)* align workspace dependency versions
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

### Documentation

- document planned-but-unimplemented settings fields
- wrap bare URL in backticks in azure provider doc comment

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
- fix formatting after merge

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- *(workspace)* remove unpublished reinhardt-settings-cli and fix stale references
- add SAFETY comments to unsafe blocks in secrets/providers/env.rs
- add SAFETY comments to unsafe blocks in sources.rs
- add SAFETY comments to unsafe blocks in profile.rs
- add SAFETY comments to unsafe blocks in env_loader.rs
- add SAFETY comments to unsafe blocks in testing.rs
- add SAFETY comments to unsafe blocks in env.rs

### Other

- resolve conflict with main (criterion version)
