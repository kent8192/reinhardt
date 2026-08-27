# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-formatter@v0.3.2...reinhardt-formatter@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(formatter)* format component style DSL

### Fixed

- *(formatter)* preserve custom property tokens
- *(release)* restore develop prerelease lifecycle
- *(formatter)* preserve match arm spacing

### Maintenance

- merge develop/0.4.0 into forward-merge branch
- merge latest main into develop forward-merge
- merge develop/0.4.0 into component-style branch

### Styling

- format component style fixtures

### Testing

- *(pages)* cover implicit page body captures
- *(formatter)* normalize component style fixtures

### Added

- *(formatter)* format component-scoped `style!` DSL bodies while preserving them across rustfmt

## [0.3.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-formatter@v0.3.12...reinhardt-formatter@v0.3.13) - 2026-08-27

### Fixed

- *(formatter)* stabilize direct page capture indentation

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-formatter@v0.3.0...reinhardt-formatter@v0.3.1) - 2026-07-04

### Fixed

- *(formatter)* wrap long page closure parameters
- *(formatter)* normalize wrapped closure indentation

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-formatter@v0.2.0...reinhardt-formatter@v0.3.0) - 2026-06-28

Stable release of `reinhardt-formatter` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(formatter)* add semantic page grammar nodes
- *(formatter)* rustfmt page expression islands
- Format safe Rust expression islands inside `page!` DSL macros with rustfmt
  while preserving unsupported islands unchanged.

### Fixed

- harden formatter temp file creation
- *(formatter)* handle reviewed page rustfmt islands

### Maintenance

- merge main into develop/0.3.0

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-formatter@v0.1.3...reinhardt-formatter@v0.2.0) - 2026-06-11

Stable release of `reinhardt-formatter` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Fixed

- split formatter from admin cli
- *(release)* publish reinhardt-formatter
