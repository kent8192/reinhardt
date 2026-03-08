# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-urls-routers-macros@v0.1.0) - 2026-03-08

### Security

- add compile-time validation for paths, SQL, and crate references
- fix path validation for ambiguous params and wildcards
- add input validation for route paths and SQL expressions

### Styling

- replace never-looping for with if-let per clippy::never_loop
- apply rustfmt formatting to workspace files

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
