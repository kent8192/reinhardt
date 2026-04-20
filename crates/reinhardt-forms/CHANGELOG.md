# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-rc.15...reinhardt-forms@v0.1.0-rc.16) - 2026-04-20

### Documentation

- *(forms)* update README example to unified validators syntax
- *(pages,forms)* clarify unified validators scope and runtime status

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-rc.14...reinhardt-forms@v0.1.0-rc.15) - 2026-03-29

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-rc.13...reinhardt-forms@v0.1.0-rc.14) - 2026-03-24

### Fixed

- *(forms)* add path traversal validation to file field
- address copilot review feedback and merge main
- *(pages,forms)* handle case-insensitive HTML tags and formset prefix collisions

### Performance

- *(pages,forms)* address Copilot review on allocation and prefix normalization

### Styling

- apply rustfmt formatting fixes

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-rc.8...reinhardt-forms@v0.1.0-rc.9) - 2026-03-15

### Fixed

- *(forms)* add missing Debug and Clone derives to form fields
- *(forms)* simplify OnceLock usage and extract regex patterns to constants

### Performance

- *(forms)* cache URL and email regex with LazyLock

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-rc.4...reinhardt-forms@v0.1.0-rc.5) - 2026-03-07

### Documentation

- add missing doc comments for public API modules and types

### Fixed

- remove develop/0.2.0 content accidentally merged via PR [[#1918](https://github.com/kent8192/reinhardt-web/issues/1918)](https://github.com/kent8192/reinhardt-web/issues/1918)

### Other

- resolve conflicts with origin/main

## [0.1.0-rc.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-rc.2...reinhardt-forms@v0.1.0-rc.3) - 2026-03-05

### Fixed

- *(release)* use path-only dev-dep for reinhardt-test in cyclic crates

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-rc.1...reinhardt-forms@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(deps)* align workspace dependency versions

### Maintenance

- *(deps)* unify proptest versions to workspace dependency

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-alpha.7...reinhardt-forms@v0.1.0-rc.1) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-alpha.6...reinhardt-forms@v0.1.0-alpha.7) - 2026-02-21

### Added

- add UrlValidator and SlugValidator for page/URL fields

### Fixed

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

### Styling

- apply rustfmt after clippy auto-fix
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to workspace files

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-alpha.5...reinhardt-forms@v0.1.0-alpha.6) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-alpha.4...reinhardt-forms@v0.1.0-alpha.5) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-alpha.3...reinhardt-forms@v0.1.0-alpha.4) - 2026-02-03

### Other

- merge main into chore/release-plz-migration
- add release-plz migration markers to CHANGELOGs
- N/A

### Added
- Work in progress features (not yet released)

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A


<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.3] - 2026-01-30

### Changed

- Version bump for publish workflow correction (no functional changes)

## [0.1.0-alpha.2] - 2026-01-29

### Changed

- Remove obsolete commented-out code from wizard module documentation
- Update package version from workspace reference to explicit version

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

