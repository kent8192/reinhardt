# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-commands@v0.1.0) - 2026-03-08

### Changed

- remove unused media_root field from Settings
- replace unsafe pointer manipulation with Option pattern
- remove unused `middleware` string list from Settings
- remove unused `root_urlconf` field from Settings
- Update imports for `reinhardt_utils::staticfiles` module rename (#114)
- Migrated welcome page rendering from Tera to reinhardt-pages SSR
- Added reinhardt-pages dependency

### Removed

- Removed welcome.tpl template (replaced by WelcomePage component)

### Fixed

- *(commands)* correct project template compilation errors
- *(commands)* correct app template compilation errors
- *(deps)* align dependency versions to workspace definitions
- *(staticfiles)* unify manifest.json format to use "paths" key
- *(staticfiles)* use STATIC_URL in HTML template processing
- return Result instead of process::exit in library code
- propagate serialization errors from TemplateContext::insert
- add panic prevention for command registry and argument parsing
- remove map_err on non-Result OpenApiRouter::wrap return value
- return Result from OpenApiRouter::wrap instead of panicking
- prevent email header injection via address validation
- *(commands)* remove unused reinhardt-i18n dev-dependency

### Security

- escape PO format characters and add checked arithmetic for MO offsets
- replace hardcoded default secret key with random generation
- redact sensitive values in error messages and env validation
- strengthen path traversal protection in runserver

### Documentation

- add missing doc comments for public API modules and types

### Styling

- apply formatting to files introduced by merge from main
- apply rustfmt to pre-existing formatting violations in 16 files
- apply rustfmt formatting to workspace files

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
