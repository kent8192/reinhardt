# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.21](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.20...reinhardt-mail@v0.1.0-rc.21) - 2026-04-23

### Documentation

- add reinhardt-version-sync markers to all crate READMEs

## [0.1.0-rc.20](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.19...reinhardt-mail@v0.1.0-rc.20) - 2026-04-23

### Documentation

- *(mail)* fix unclosed code fence, wrong thread-safety claim, and stale versions

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.15...reinhardt-mail@v0.1.0-rc.16) - 2026-04-20

### Changed

- deduplicate utility functions across crates

### Fixed

- *(ci)* resolve format and clippy deprecated lint errors

### Maintenance

- upgrade workspace dependencies to latest versions
- *(build)* reduce tokio features and enable debug=1 profile for faster compilation

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.14...reinhardt-mail@v0.1.0-rc.15) - 2026-03-29

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.13...reinhardt-mail@v0.1.0-rc.14) - 2026-03-24

### Fixed

- *(mail)* validate pool configuration parameters
- address copilot review feedback and merge main
- *(mail)* use existing BackendError variant instead of adding new enum variant

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.4...reinhardt-mail@v0.1.0-rc.5) - 2026-03-07

### Documentation

- add missing doc comments for public API modules and types

### Other

- resolve conflicts with origin/main

## [0.1.0-rc.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.2...reinhardt-mail@v0.1.0-rc.3) - 2026-03-05

### Fixed

- *(release)* use path-only dev-dep for reinhardt-test in cyclic crates

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.1...reinhardt-mail@v0.1.0-rc.2) - 2026-03-04

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.11...reinhardt-mail@v0.1.0-rc.1) - 2026-02-23

### Maintenance

- *(workspace)* remove unpublished reinhardt-settings-cli and fix stale references

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.10...reinhardt-mail@v0.1.0-alpha.11) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-conf

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.9...reinhardt-mail@v0.1.0-alpha.10) - 2026-02-21

### Fixed

- document semaphore-based pool concurrency and add stress test
- validate header names against RFC 2822
- propagate config errors even when fail_silently is enabled
- add attachment rendering in dev backends and fix arbitrary header injection
- pin native-tls to =0.2.14 to fix build failure
- fix email validation and field access control (#512, #515, #517)
- enable proper TLS hostname verification in SMTP backend
- prevent email header injection via address validation

### Security

- add email length validation and credential zeroization
- fix HTML escaping, rate limiting, and validation

### Styling

- apply rustfmt to pre-existing unformatted files
- collapse nested if statements per clippy::collapsible_if
- apply rustfmt formatting to workspace files
- apply code formatting to security fix files

### Performance

- avoid unnecessary email body clone

### Maintenance

- add explanatory comments to undocumented #[allow(...)] attributes

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.8...reinhardt-mail@v0.1.0-alpha.9) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-conf

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.7...reinhardt-mail@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.6...reinhardt-mail@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.5...reinhardt-mail@v0.1.0-alpha.6) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.4...reinhardt-mail@v0.1.0-alpha.5) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-conf

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.3...reinhardt-mail@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-conf

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.2...reinhardt-mail@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-conf

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-alpha.1...reinhardt-mail@v0.1.0-alpha.2) - 2026-02-03

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

