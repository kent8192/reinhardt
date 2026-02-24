# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-forms@v0.1.0-alpha.7...reinhardt-forms@v0.1.0-alpha.8) - 2026-02-23

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

