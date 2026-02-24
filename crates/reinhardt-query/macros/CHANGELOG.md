# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query-macros@v0.1.0-alpha.4...reinhardt-query-macros@v0.1.0-alpha.5) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query-macros@v0.1.0-alpha.3...reinhardt-query-macros@v0.1.0-alpha.4) - 2026-02-23

### Fixed

- *(release)* advance version to skip yanked alpha.3 and restore publish capability for dependents

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query-macros@v0.1.0-alpha.2...reinhardt-query-macros@v0.1.0-alpha.3) - 2026-02-21 [YANKED]

This release was yanked shortly after publication. Use v0.1.0-alpha.4 instead.

### Fixed

- add compile-time Debug assertion for derive(Iden)
- emit errors for invalid #[iden] attribute arguments
- replace write_str unwrap with expect documenting infallibility
- validate identifier names and handle enum variants with data

## [0.1.0-alpha.1](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-query-macros@v0.1.0-alpha.1) - 2026-02-14

### Added

- *(query)* add #[derive(Iden)] proc-macro crate

### Fixed

- *(macros)* proper closing delimiters and quote expansion (review [[#1](https://github.com/kent8192/reinhardt-web/issues/1)](https://github.com/kent8192/reinhardt-web/issues/1))
- *(macros)* fix type mismatches and generate Iden trait impl in derive macro
- *(query)* add Table variant special handling in Iden derive macro
- *(query)* add Meta::List support to Iden derive macro attribute parsing
- *(query)* read iden attribute from struct-level instead of first field

### Styling

- *(query)* format Iden derive macro code
