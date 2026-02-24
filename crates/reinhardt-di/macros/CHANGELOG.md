# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di-macros@v0.1.0-alpha.3...reinhardt-di-macros@v0.1.0-alpha.4) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di-macros@v0.1.0-alpha.2...reinhardt-di-macros@v0.1.0-alpha.3) - 2026-02-23

### Fixed

- *(release)* advance version to skip yanked alpha.2 and restore publish capability for dependents

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di-macros@v0.1.0-alpha.1...reinhardt-di-macros@v0.1.0-alpha.2) - 2026-02-21 [YANKED]

This release was yanked shortly after publication. Use v0.1.0-alpha.3 instead.

### Fixed

- remove undeclared tracing dependency from injectable macro output

### Security

- improve generated name hygiene, crate path diagnostics, and type path validation
- reject unknown macro arguments and unsupported scope attribute
- remove info leak and validate factory code generation
