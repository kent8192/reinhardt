# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.19](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.18...reinhardt-pages@v0.1.0-alpha.19) - 2026-02-24

### Fixed

- correct repository URLs from reinhardt-rs to reinhardt-web

## [0.1.0-alpha.18](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.17...reinhardt-pages@v0.1.0-alpha.18) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.17](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.16...reinhardt-pages@v0.1.0-alpha.17) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-pages-ast, reinhardt-pages-macros, reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.15...reinhardt-pages@v0.1.0-alpha.16) - 2026-02-21

### Fixed

- store WebSocket closures in handle instead of leaking via forget()
- replace unreachable!() with proper syn::Error in parse_if_node
- reject non-boolean values for disabled/readonly/autofocus
- reject whitespace in server_fn endpoint paths
- add missing input type image and form method dialog
- detect duplicate properties in form field parsing
- replace direct indexing with safe .first() access
- escape field names and media paths (#594, #595)
- escape auth data in JSON output to prevent XSS (#586)
- validate img src URLs and wrapper tag names
- add tag name allowlist for wrapper and icon elements
- validate img src against dangerous URL schemes
- add max nesting depth to page parser
- add max nesting depth to SVG icon parser
- emit compile error for unknown codec instead of silent fallback
- replace expect() panics with compile errors in head.rs
- fix link tag as_ attribute code generation
- emit compile error for unsupported form-level validators
- add required attributes to allowed_attrs for track, param, data
- return Option from FormFieldProperty::name instead of panicking
- add authentication and authorization enforcement to all endpoints

### Security

- replace panicking unwrap calls with proper error handling
- replace silent Click fallback for unknown event types
- add constant-time CSRF token verification
- add URL scheme and path validation for forms and head
- add input size limit to HTML minification to prevent DoS
- prevent open redirect attacks
- escape HTML characters in SSR state JSON to prevent XSS

### Changed

- replace magic string with Option<Ident> for FormMacro name
- extract duplicated form ID and action string generation
- remove duplicate img required attribute validation

### Styling

- apply workspace-wide formatting fixes
- apply formatting to files introduced by merge from main
- fix rustfmt formatting in renderer.rs
- fix formatting issues

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.14...reinhardt-pages@v0.1.0-alpha.15) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-pages-macros, reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.13...reinhardt-pages@v0.1.0-alpha.14) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.12...reinhardt-pages@v0.1.0-alpha.13) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.11...reinhardt-pages@v0.1.0-alpha.12) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.10...reinhardt-pages@v0.1.0-alpha.11) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.8...reinhardt-pages@v0.1.0-alpha.9) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

### Reverted

- undo unintended visibility and formatting changes

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.7...reinhardt-pages@v0.1.0-alpha.8) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.6...reinhardt-pages@v0.1.0-alpha.7) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils, reinhardt-di, reinhardt-server, reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.5...reinhardt-pages@v0.1.0-alpha.6) - 2026-02-03

### Other

- Merge pull request #111 from kent8192/fix/issue-81-bug-reinhardt-pages-wasm-build-fails-due-to-tokiomio-server-side-dependencies

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-alpha.4...reinhardt-pages@v0.1.0-alpha.5) - 2026-02-03

### Fixed

- *(ci)* remove proptest regression files from git tracking

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

## [0.1.0-alpha.4] - 2026-01-30

### Changed

- Re-release of 0.1.0-alpha.3 content after version correction
- Update imports for `reinhardt_utils::staticfiles` module rename (#114)


## [0.1.0-alpha.3] - 2026-01-29 [YANKED]

**Note:** This version was yanked due to version skipping in the main crate (`reinhardt-web`). Use the latest available version instead.

### Changed

- Update imports for `reinhardt_utils::staticfiles` module rename (#114)


## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

