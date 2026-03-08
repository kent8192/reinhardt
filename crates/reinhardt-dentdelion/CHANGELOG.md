# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-dentdelion@v0.1.0) - 2026-03-08

### Added

- Initial release of the plugin system for Reinhardt framework
- Plugin trait for defining reusable framework extensions
- Plugin manifest and metadata support
- Plugin loading and initialization infrastructure

### Changed

- share reqwest::Client across HostState instances
- add #[non_exhaustive] to ColumnType and TsError enums

### Fixed

- acquire multiple locks simultaneously to prevent TOCTOU
- prevent silent failures in WASM config and plugin metadata
- replace hardcoded placeholder email in crates.io User-Agent
- replace panicking unwrap/expect calls with safe alternatives
- remove unsafe Send/Sync impl from TsRuntime
- correct HostState clone and topological sort in dentdelion (#682, #683)
- escape script tags in hydration to prevent XSS
- add SQL validation for WASM plugin queries
- add security controls to render_component
- add SSRF prevention with URL validation in WASM host

### Security

- validate plugin names to prevent path traversal and log injection
- add resource limits for JS execution, event subscriptions, and plugin disable

### Documentation

- document validate_component_path security rationale
- document is_valid_wasm magic byte validation scope

### Styling

- apply formatting to files introduced by merge from main
- apply rustfmt to crates_io module
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to wasm module files
- apply code formatting to security fix files

### Maintenance

- *(deps)* downgrade wasmtime to 36.0.6 to fix security advisories
- *(testing)* add insta snapshot testing dependency across all crates
- upgrade remaining crates from edition 2021 to 2024
