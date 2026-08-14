# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-providers@v0.4.0-alpha.6...reinhardt-providers@v0.4.0-alpha.7) - 2026-08-14

### Added

- *(storage)* add conditional S3 object writes

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-providers@v0.3.2...reinhardt-providers@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle

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
