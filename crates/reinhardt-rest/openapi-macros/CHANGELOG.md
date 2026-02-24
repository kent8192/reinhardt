# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-alpha.4...reinhardt-openapi-macros@v0.1.0-alpha.5) - 2026-02-21

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
