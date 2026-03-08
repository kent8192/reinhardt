# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-pages@v0.1.0) - 2026-03-08

### Changed

- replace magic string with Option<Ident> for FormMacro name
- extract duplicated form ID and action string generation
- remove duplicate img required attribute validation
- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention
- Update imports for `reinhardt_utils::staticfiles` module rename (#114)
- Update imports for `reinhardt_utils::staticfiles` module rename (#114)

### Fixed

- *(pages)* use dynamic year in SelectDateWidget instead of hardcoded 2025
- remove develop/0.2.0 content accidentally merged via PR [[#1918](https://github.com/kent8192/reinhardt-web/issues/1918)](https://github.com/kent8192/reinhardt-web/issues/1918)
- restore non-crate develop/0.2.0 changes that are harmless or beneficial
- *(pages)* add explanatory comments to #[allow(dead_code)]
- correct repository URLs from reinhardt-rs to reinhardt-web
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

### Styling

- apply workspace-wide formatting fixes
- apply formatting to files introduced by merge from main
- fix rustfmt formatting in renderer.rs
- fix formatting issues

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

### Other

- resolve conflicts with origin/main
