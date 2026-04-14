# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-routers-macros@v0.1.0-rc.15...reinhardt-routers-macros@v0.1.0-rc.16) - 2026-04-14

### Maintenance

- update Cargo.toml dependencies

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-routers-macros@v0.1.0-rc.14...reinhardt-routers-macros@v0.1.0-rc.15) - 2026-03-29

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-routers-macros@v0.1.0-rc.13...reinhardt-routers-macros@v0.1.0-rc.14) - 2026-03-24

### Documentation

- *(crates)* update version references from 0.1.0-alpha.1 to 0.1.0-rc.13 across all READMEs

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
