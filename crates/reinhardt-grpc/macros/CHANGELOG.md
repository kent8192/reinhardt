# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-grpc-macros@v0.1.0) - 2026-03-08

### Fixed

- *(meta)* fix workspace inheritance and authors metadata
- add async validation and fix impl name collision
- return generic errors and log details server-side
- emit compile error for unrecognized inject attribute options

### Security

- strengthen type checking in macro-generated code

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing formatting violations in 16 files

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
