# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.1.0-rc.15...reinhardt-grpc-macros@v0.1.0-rc.16) - 2026-04-14

### Added

- *(di)* [**breaking**] deprecate Injected<T> in favor of Depends<T> and remove auto-Clone

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.1.0-rc.14...reinhardt-grpc-macros@v0.1.0-rc.15) - 2026-03-29

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.1.0-rc.13...reinhardt-grpc-macros@v0.1.0-rc.14) - 2026-03-24

### Fixed

- *(deps)* update native-tls pin and use workspace versions in proc-macro crates
- *(reinhardt-grpc)* fork DI context per-request in gRPC handler macros

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.1.0-rc.1...reinhardt-grpc-macros@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(meta)* fix workspace inheritance and authors metadata

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.1.0-alpha.2...reinhardt-grpc-macros@v0.1.0-rc.1) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.1.0-alpha.1...reinhardt-grpc-macros@v0.1.0-alpha.2) - 2026-02-21

### Fixed

- add async validation and fix impl name collision
- return generic errors and log details server-side
- emit compile error for unrecognized inject attribute options

### Security

- strengthen type checking in macro-generated code

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing formatting violations in 16 files
