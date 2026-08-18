# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-providers@v0.4.0-alpha.6...reinhardt-providers@v0.4.0-alpha.7) - 2026-08-18

### Documentation

- *(providers)* clarify S3 client contracts

### Fixed

- *(providers)* preserve S3 presigned URL ordering

### Maintenance

- merge main into develop/0.4.0

### Testing

- *(providers)* cover AWS credential resolution
- *(providers)* make S3 signing deterministic
- *(providers)* cover S3 HTTP operations
- *(providers)* cover S3 failure responses
- *(providers)* assert exact S3 validation errors
- *(providers)* cover HEAD permission failures
- *(providers)* remove duplicate credential coverage
- *(providers)* stabilize review coverage fixtures
- *(providers)* exercise env guard through credential loading

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-providers@v0.3.2...reinhardt-providers@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle
## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-providers@v0.3.7...reinhardt-providers@v0.3.8) - 2026-08-16

### Maintenance

- update Cargo.toml dependencies

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-providers@v0.3.6...reinhardt-providers@v0.3.7) - 2026-08-12

### Documentation

- *(providers)* clarify S3 client contracts

### Fixed

- *(providers)* preserve S3 presigned URL ordering

### Testing

- *(providers)* cover AWS credential resolution
- *(providers)* make S3 signing deterministic
- *(providers)* cover S3 HTTP operations
- *(providers)* cover S3 failure responses
- *(providers)* assert exact S3 validation errors
- *(providers)* cover HEAD permission failures
- *(providers)* remove duplicate credential coverage
- *(providers)* stabilize review coverage fixtures
- *(providers)* exercise env guard through credential loading

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-providers@v0.2.0...reinhardt-providers@v0.3.0) - 2026-06-28

Stable release of `reinhardt-providers` for the Reinhardt 0.3.0 line. This
crate moves with the coordinated Reinhardt 0.3.0 release train.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- align crate release metadata with the Reinhardt 0.3.0 stable release train.

## [0.2.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-providers@v0.2.0) - 2026-06-11

Stable release of `reinhardt-providers` for the Reinhardt 0.2.0 line.

### Added

- *(providers)* add minimal S3 provider client

### Fixed

- *(providers)* preserve AWS credential chain
- *(providers)* address CodeRabbit review
- *(ci)* pin broken upstream transitive releases
