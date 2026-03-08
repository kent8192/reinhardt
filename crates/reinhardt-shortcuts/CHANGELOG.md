# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-shortcuts@v0.1.0) - 2026-03-08

### Changed

- add configurable capacity limit to TemplateContext
- add security headers helper function

### Fixed

- *(release)* move reinhardt-test to optional dep in non-cyclic crates
- use HeaderValue::from_static for hardcoded header values
- fix data integrity in render_to_string and sanitize 404 errors
- prevent database error message leakage in HTTP response
- prevent URL validation bypass via From trait (#726)

### Security

- add XSS safety documentation and input sanitization for render_html
- prevent open redirect attacks

### Documentation

- add missing doc comments for public API modules and types

### Styling

- apply formatting to files introduced by merge from main

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
