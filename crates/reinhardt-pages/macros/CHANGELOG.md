# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-pages-macros@v0.1.0) - 2026-03-08

### Added

- Initial release of `reinhardt-pages-macros` crate
- `page!` macro for anonymous component DSL with closure-style props
- Support for 70+ HTML elements with compile-time validation
- Event handlers using `@event: handler` syntax
- Conditional rendering with `if`/`else` and list rendering with `for` loops
- Reactive features with `watch` blocks for Signal-dependent rendering
- Component calls with named arguments
- Accessibility validation (img alt, button labels)
- Security validation (XSS prevention for URL attributes)
- `head!` macro for HTML head section DSL
- Support for title, meta, link, script, and style elements
- SSR metadata injection support
- `form!` macro for type-safe forms with reactive bindings
- Multiple field types: CharField, TextField, EmailField, IntegerField, etc.
- Widget customization: TextInput, PasswordInput, Select, RadioSelect, etc.
- Built-in validation (required, min/max length, pattern)
- Server-side and client-side validators
- UI state management (loading, error, success)
- Two-way Signal binding
- Computed values with `derived` block
- Field groups with `FieldGroup`
- Custom wrapper elements and SVG icon support
- Slots for custom content injection
- CSRF protection for non-GET methods
- Initial value loading with `initial_loader`
- Dynamic choice loading with `choices_loader`
- `#[server_fn]` attribute macro for Server Functions (RPC)
- WASM client stub generation
- Server-side handler generation
- Custom endpoint paths
- Codec selection (json, url, msgpack)
- Dependency injection support with `#[reinhardt::inject]`

### Changed

- replace magic string with Option<Ident> for FormMacro name
- extract duplicated form ID and action string generation
- remove duplicate img required attribute validation
- update references for flattened examples structure
- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

### Fixed

- *(meta)* fix workspace inheritance and authors metadata
- reject non-boolean values for disabled/readonly/autofocus
- reject whitespace in server_fn endpoint paths
- add missing input type image and form method dialog
- replace direct indexing with safe .first() access
- validate img src URLs and wrapper tag names
- add tag name allowlist for wrapper and icon elements
- validate img src against dangerous URL schemes
- emit compile error for unknown codec instead of silent fallback
- replace expect() panics with compile errors in head.rs
- fix link tag as_ attribute code generation
- emit compile error for unsupported form-level validators
- add required attributes to allowed_attrs for track, param, data
- add reinhardt-manouche to workspace deps and address review comments

### Security

- add URL scheme and path validation for forms and head

### Styling

- apply rustfmt to pre-existing unformatted files
- apply formatting to files introduced by merge from main

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

### Other

- Merge branch 'main' into refactor/extract-manouche-dsl
