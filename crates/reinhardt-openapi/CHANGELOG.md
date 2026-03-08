# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-openapi@v0.1.0) - 2026-03-08

### Added

- Initial release of `reinhardt-openapi` crate
- `OpenApiRouter` wrapper for automatic OpenAPI documentation endpoints
- Swagger UI endpoint at `/api/docs`
- Redoc UI endpoint at `/api/redoc`
- OpenAPI JSON endpoint at `/api/openapi.json`
- Handler and Router trait implementations for `OpenApiRouter`

### Changed

- extract shared OpenAPI route handling logic

### Fixed

- remove map_err on non-Result OpenApiRouter::wrap return value
- resolve clippy collapsible_if warnings after merge with main
- add enabled flag and optional auth guard for docs endpoints
- return Result from OpenApiRouter::wrap instead of panicking

### Security

- add security headers to documentation endpoints

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- *(testing)* add insta snapshot testing dependency across all crates

### Notes

- This crate was extracted from `reinhardt-rest` to resolve circular dependency issues
- See [Issue #23](https://github.com/kent8192/reinhardt-web/issues/23) for details
