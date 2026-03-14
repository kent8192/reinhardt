# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.8...reinhardt-macros@v0.1.0-rc.9) - 2026-03-14

### Added

- feat!(macros): add #[derive(Validate)] proc macro for field-level validation

### Changed

- refactor!(macros): replace external validator crate in pre_validate codegen

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.4...reinhardt-macros@v0.1.0-rc.5) - 2026-03-07

### Fixed

- *(macros)* dereference extractor before validation in pre_validate
- *(macros)* replace skeleton tests with meaningful assertions in pre_validate

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.1...reinhardt-macros@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(macros)* add auto_increment param to field registration
- *(macros)* infer not_null from Rust Option type in field registration
- *(macros)* map DateTime to TimestampTz for timezone-aware columns

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-alpha.3...reinhardt-macros@v0.1.0-rc.1) - 2026-02-23

### Fixed

- *(release)* advance version to skip yanked alpha.3 and restore publish capability for dependents

## [0.1.0-alpha.3] - 2026-02-21 [YANKED]

This release was yanked shortly after publication. Use v0.1.0-alpha.4 instead.

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-alpha.1...reinhardt-macros@v0.1.0-alpha.2) - 2026-02-03

### Other

- *(package)* replace version.workspace with explicit versions
