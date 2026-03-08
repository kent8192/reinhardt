# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-admin-cli@v0.1.0) - 2026-03-08

### Added

- Initial release of `reinhardt-admin` CLI tool
- `startproject` command for scaffolding new Reinhardt projects
- `startapp` command for generating application modules
- `plugin` subcommands: install, remove, list, search, enable, disable, update, info
- `fmt` command for code formatting with rustfmt integration
- Verbose output support with `-v` flag

### Changed

- add template_type validation and bound project root search

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

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
- fix clippy warnings and formatting in files merged from main
- apply formatting to files introduced by merge from main
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to workspace files

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
