# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.13...reinhardt-dentdelion@v0.1.0-alpha.14) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.12...reinhardt-dentdelion@v0.1.0-alpha.13) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.11...reinhardt-dentdelion@v0.1.0-alpha.12) - 2026-02-21

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

### Changed

- share reqwest::Client across HostState instances
- add #[non_exhaustive] to ColumnType and TsError enums

### Styling

- apply formatting to files introduced by merge from main
- apply rustfmt to crates_io module
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to wasm module files
- apply code formatting to security fix files

### Documentation

- document validate_component_path security rationale
- document is_valid_wasm magic byte validation scope

### Maintenance

- upgrade remaining crates from edition 2021 to 2024

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.10...reinhardt-dentdelion@v0.1.0-alpha.11) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.9...reinhardt-dentdelion@v0.1.0-alpha.10) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.8...reinhardt-dentdelion@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.7...reinhardt-dentdelion@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.6...reinhardt-dentdelion@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.5...reinhardt-dentdelion@v0.1.0-alpha.6) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.4...reinhardt-dentdelion@v0.1.0-alpha.5) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.3...reinhardt-dentdelion@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.2...reinhardt-dentdelion@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-db

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-alpha.1...reinhardt-dentdelion@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs

<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial release of the plugin system for Reinhardt framework
- Plugin trait for defining reusable framework extensions
- Plugin manifest and metadata support
- Plugin loading and initialization infrastructure
