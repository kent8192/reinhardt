# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-routers-macros@v0.1.0-alpha.3...reinhardt-routers-macros@v0.1.0-rc.1) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-routers-macros@v0.1.0-alpha.2...reinhardt-routers-macros@v0.1.0-alpha.3) - 2026-02-21

### Security

- add compile-time validation for paths, SQL, and crate references
- fix path validation for ambiguous params and wildcards
- add input validation for route paths and SQL expressions

### Styling

- replace never-looping for with if-let per clippy::never_loop
- apply rustfmt formatting to workspace files

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-routers-macros@v0.1.0-alpha.1...reinhardt-routers-macros@v0.1.0-alpha.2) - 2026-02-03

### Other

- *(package)* replace version.workspace with explicit versions
