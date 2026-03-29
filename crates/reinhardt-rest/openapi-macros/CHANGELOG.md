# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-rc.14...reinhardt-openapi-macros@v0.1.0-rc.15) - 2026-03-29

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-rc.13...reinhardt-openapi-macros@v0.1.0-rc.14) - 2026-03-24

### Fixed

- *(deps)* update native-tls pin and use workspace versions in proc-macro crates

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-rc.11...reinhardt-openapi-macros@v0.1.0-rc.12) - 2026-03-18

### Added

- *(rest)* add container-level OpenAPI schema attributes

### Fixed

- *(rest)* address Copilot review feedback on OpenAPI annotations

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-alpha.4...reinhardt-openapi-macros@v0.1.0-rc.1) - 2026-02-21

### Fixed

- propagate parse errors and validate min/max constraints
- replace expect() with safe get_ident() handling in attribute parsing
- collapse nested if block in serde_attrs to satisfy clippy
- handle serde attributes and improve validation

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-alpha.3...reinhardt-openapi-macros@v0.1.0-alpha.4) - 2026-02-06

### Fixed

- break circular dependency between reinhardt-openapi-macros and reinhardt-rest

### Other

- Revert "Merge pull request #202 from kent8192/release-plz-2026-02-06T13-32-57Z"
- release

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-alpha.2...reinhardt-openapi-macros@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-rest

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-alpha.1...reinhardt-openapi-macros@v0.1.0-alpha.2) - 2026-02-03

### Other

- *(package)* replace version.workspace with explicit versions
