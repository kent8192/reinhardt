# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.13...reinhardt-testkit@v0.1.0-rc.14) - 2026-03-20

### Fixed

- *(testkit)* unify PostgreSQL version, add pool close, and cleanup backoff

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.11...reinhardt-testkit@v0.1.0-rc.12) - 2026-03-18

### Added

- *(testkit)* add postgres_with_migrations_from_dir helper using FilesystemSource

### Deprecated

- *(testkit)* deprecate global_registry-based migration fixtures

### Documentation

- *(macros,testkit)* use backticks for cross-crate intra-doc links

### Styling

- *(testkit)* apply auto-fix formatting to fixtures re-export

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.8...reinhardt-testkit@v0.1.0-rc.9) - 2026-03-15

### Styling

- add explanatory comments to remaining #[allow(dead_code)] attributes
