# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc@v0.1.0-alpha.6...reinhardt-grpc@v0.1.0-alpha.7) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc@v0.1.0-alpha.5...reinhardt-grpc@v0.1.0-alpha.6) - 2026-02-21

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

### Changed

- use Cow<str> to reduce allocations and improve test messages

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing formatting violations in 16 files

### Maintenance

- replace Japanese comments with English in proto type tests

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc@v0.1.0-alpha.4...reinhardt-grpc@v0.1.0-alpha.5) - 2026-02-10

### Maintenance

- *(clippy)* add deny lints for todo/unimplemented/dbg_macro

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc@v0.1.0-alpha.3...reinhardt-grpc@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc@v0.1.0-alpha.2...reinhardt-grpc@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-di

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc@v0.1.0-alpha.1...reinhardt-grpc@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections
- *(changelog)* add missing 0.1.0-alpha.1 release entries
- *(package)* replace version.workspace with explicit versions
- N/A

### Added
- Work in progress features (not yet released)

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A


<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

