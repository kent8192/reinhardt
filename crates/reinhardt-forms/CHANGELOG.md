# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-forms@v0.1.0) - 2026-03-08

### Added

- add UrlValidator and SlugValidator for page/URL fields

### Changed

- Remove obsolete commented-out code from wizard module documentation

### Fixed

- remove develop/0.2.0 content accidentally merged via PR [[#1918](https://github.com/kent8192/reinhardt-web/issues/1918)](https://github.com/kent8192/reinhardt-web/issues/1918)
- *(release)* use path-only dev-dep for reinhardt-test in cyclic crates
- *(deps)* align workspace dependency versions
- enforce file size limits in form uploads (#558)
- replace panic with error handling in ModelForm::save (#560)
- escape user input in Widget::render_html to prevent XSS
- replace js-based validation with type-safe declarative rules
- remove SVG from default image extensions to prevent stored XSS

### Security

- sanitize validator errors and prevent password plaintext storage
- fix decimal leading zeros, IPv6 validation, and date year ambiguity
- fix input validation and resource limits across form fields
- fix XSS escaping, CSRF protection, and panic prevention

### Documentation

- add missing doc comments for public API modules and types

### Styling

- apply rustfmt after clippy auto-fix
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to workspace files

### Maintenance

- *(deps)* unify proptest versions to workspace dependency
- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

### Other

- resolve conflicts with origin/main
