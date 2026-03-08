# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-grpc@v0.1.0) - 2026-03-08

### Changed

- use Cow<str> to reduce allocations and improve test messages

### Fixed

- add async validation and fix impl name collision
- return generic errors and log details server-side
- emit compile error for unrecognized inject attribute options
- roll back unpublished crate versions after partial release failure
- roll back unpublished crate versions and enable release_always

### Security

- add request timeout, connection limits, and tower integration docs
- strengthen type checking in macro-generated code
- add protobuf depth limits and sanitize error messages
- add default message size limit

### Documentation

- add missing doc comments for public API modules and types

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing formatting violations in 16 files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- replace Japanese comments with English in proto type tests
- *(clippy)* add deny lints for todo/unimplemented/dbg_macro
