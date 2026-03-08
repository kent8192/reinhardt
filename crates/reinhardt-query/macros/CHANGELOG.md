# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-query-macros@v0.1.0) - 2026-03-08

### Added

- *(query)* add #[derive(Iden)] proc-macro crate

### Fixed

- *(meta)* fix workspace inheritance and authors metadata
- *(release)* advance version to skip yanked alpha.3 and restore publish capability for dependents
- add compile-time Debug assertion for derive(Iden)
- emit errors for invalid #[iden] attribute arguments
- replace write_str unwrap with expect documenting infallibility
- validate identifier names and handle enum variants with data
- *(macros)* proper closing delimiters and quote expansion (review [[#1](https://github.com/kent8192/reinhardt-web/issues/1)](https://github.com/kent8192/reinhardt-web/issues/1))
- *(macros)* fix type mismatches and generate Iden trait impl in derive macro
- *(query)* add Table variant special handling in Iden derive macro
- *(query)* add Meta::List support to Iden derive macro attribute parsing
- *(query)* read iden attribute from struct-level instead of first field

### Styling

- *(query)* format Iden derive macro code

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
