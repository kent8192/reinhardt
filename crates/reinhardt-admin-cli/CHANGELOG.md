# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.18](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.17...reinhardt-admin-cli@v0.1.0-alpha.18) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-pages, reinhardt-commands

## [0.1.0-alpha.17](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.16...reinhardt-admin-cli@v0.1.0-alpha.17) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.15...reinhardt-admin-cli@v0.1.0-alpha.16) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-pages, reinhardt-dentdelion, reinhardt-commands

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.14...reinhardt-admin-cli@v0.1.0-alpha.15) - 2026-02-21

### Fixed

- fix .env parsing, AST formatter, and file safety issues
- atomic file writes, preserve permissions, cleanup backups
- add recursion depth guard to AST formatter
- remove unused utility functions from utils module
- apply rustfmt formatting to utils module
- apply clippy fixes to utils module
- add error handling and type coercion safety
- add missing OpenOptionsExt import for secure backup creation
- fix key zeroing, file perms, and value redaction in admin-cli (#650, #656, #658)

### Security

- fix TOCTOU, silent errors, unsafe unwrap, backup file exposure, and DoS limits
- sanitize error messages to prevent information leakage
- add input validation, file size limits, and TOCTOU mitigations

### Changed

- add template_type validation and bound project root search

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
- fix clippy warnings and formatting in files merged from main
- apply formatting to files introduced by merge from main
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to workspace files

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.13...reinhardt-admin-cli@v0.1.0-alpha.14) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-pages, reinhardt-dentdelion, reinhardt-commands

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.12...reinhardt-admin-cli@v0.1.0-alpha.13) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-pages, reinhardt-dentdelion, reinhardt-commands

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.11...reinhardt-admin-cli@v0.1.0-alpha.12) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-pages, reinhardt-commands

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.10...reinhardt-admin-cli@v0.1.0-alpha.11) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-pages, reinhardt-dentdelion, reinhardt-commands

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.9...reinhardt-admin-cli@v0.1.0-alpha.10) - 2026-02-14

### Fixed

- *(release)* roll back unpublished crate versions after partial release failure

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.8...reinhardt-admin-cli@v0.1.0-alpha.9) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-commands

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.7...reinhardt-admin-cli@v0.1.0-alpha.8) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-commands

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.6...reinhardt-admin-cli@v0.1.0-alpha.7) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-commands, reinhardt-pages, reinhardt-dentdelion

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.5...reinhardt-admin-cli@v0.1.0-alpha.6) - 2026-02-07

### Other

- updated the following local packages: reinhardt-commands

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.4...reinhardt-admin-cli@v0.1.0-alpha.5) - 2026-02-07

### Other

- updated the following local packages: reinhardt-commands

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.3...reinhardt-admin-cli@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-pages, reinhardt-dentdelion, reinhardt-commands

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.2...reinhardt-admin-cli@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-pages, reinhardt-dentdelion, reinhardt-commands

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-alpha.1...reinhardt-admin-cli@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs

### Breaking Changes
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
- Initial release of `reinhardt-admin` CLI tool
- `startproject` command for scaffolding new Reinhardt projects
- `startapp` command for generating application modules
- `plugin` subcommands: install, remove, list, search, enable, disable, update, info
- `fmt` command for code formatting with rustfmt integration
- Verbose output support with `-v` flag
