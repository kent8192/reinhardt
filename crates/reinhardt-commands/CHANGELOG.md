# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.21](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.20...reinhardt-commands@v0.1.0-alpha.21) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-pages, reinhardt-test

## [0.1.0-alpha.20](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.19...reinhardt-commands@v0.1.0-alpha.20) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.19](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.18...reinhardt-commands@v0.1.0-alpha.19) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-apps, reinhardt-db, reinhardt-mail, reinhardt-middleware, reinhardt-rest, reinhardt-urls, reinhardt-pages, reinhardt-test, reinhardt-dentdelion, reinhardt-openapi

## [0.1.0-alpha.18](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.17...reinhardt-commands@v0.1.0-alpha.18) - 2026-02-21

### Fixed

- return Result instead of process::exit in library code
- propagate serialization errors from TemplateContext::insert
- add panic prevention for command registry and argument parsing
- remove map_err on non-Result OpenApiRouter::wrap return value
- return Result from OpenApiRouter::wrap instead of panicking
- prevent email header injection via address validation

### Security

- escape PO format characters and add checked arithmetic for MO offsets
- replace hardcoded default secret key with random generation
- redact sensitive values in error messages and env validation
- strengthen path traversal protection in runserver

### Changed

- remove unused media_root field from Settings
- replace unsafe pointer manipulation with Option pattern
- remove unused `middleware` string list from Settings
- remove unused `root_urlconf` field from Settings

### Styling

- apply formatting to files introduced by merge from main
- apply rustfmt to pre-existing formatting violations in 16 files
- apply rustfmt formatting to workspace files

## [0.1.0-alpha.17](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.16...reinhardt-commands@v0.1.0-alpha.17) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-middleware, reinhardt-rest, reinhardt-urls, reinhardt-pages, reinhardt-test, reinhardt-dentdelion, reinhardt-openapi

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.15...reinhardt-commands@v0.1.0-alpha.16) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-rest, reinhardt-conf, reinhardt-apps, reinhardt-db, reinhardt-mail, reinhardt-middleware, reinhardt-urls, reinhardt-pages, reinhardt-test, reinhardt-dentdelion, reinhardt-openapi

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.14...reinhardt-commands@v0.1.0-alpha.15) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-rest, reinhardt-urls, reinhardt-pages, reinhardt-test, reinhardt-openapi

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.13...reinhardt-commands@v0.1.0-alpha.14) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-apps, reinhardt-db, reinhardt-mail, reinhardt-middleware, reinhardt-rest, reinhardt-urls, reinhardt-pages, reinhardt-test, reinhardt-dentdelion, reinhardt-openapi

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.12...reinhardt-commands@v0.1.0-alpha.13) - 2026-02-14

### Fixed

- *(commands)* remove unused reinhardt-i18n dev-dependency
- *(release)* roll back unpublished crate versions after partial release failure

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.11...reinhardt-commands@v0.1.0-alpha.12) - 2026-02-12

### Fixed

- *(release)* roll back unpublished crate versions and enable release_always

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.10...reinhardt-commands@v0.1.0-alpha.11) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-test

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.9...reinhardt-commands@v0.1.0-alpha.10) - 2026-02-10

### Fixed

- *(ci)* remove version from reinhardt-test workspace dep to avoid cargo 1.84+ resolution failure
- *(release)* revert unpublished crate versions to pre-release state

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.8...reinhardt-commands@v0.1.0-alpha.9) - 2026-02-07

### Other

- Merge pull request #129 from kent8192/fix/issue-128-bug-runserver-uses-settingsdefault-instead-of-loading-from-settings-directory

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.7...reinhardt-commands@v0.1.0-alpha.8) - 2026-02-07

### Fixed

- add version to reinhardt-test workspace dependency for crates.io publishing

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.6...reinhardt-commands@v0.1.0-alpha.7) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils, reinhardt-di, reinhardt-rest, reinhardt-conf, reinhardt-server, reinhardt-apps, reinhardt-db, reinhardt-mail, reinhardt-middleware, reinhardt-urls, reinhardt-pages, reinhardt-test, reinhardt-dentdelion, reinhardt-openapi

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.5...reinhardt-commands@v0.1.0-alpha.6) - 2026-02-03

### Other

- updated the following local packages: reinhardt-pages, reinhardt-http, reinhardt-utils, reinhardt-conf, reinhardt-di, reinhardt-server, reinhardt-apps, reinhardt-db, reinhardt-mail, reinhardt-middleware, reinhardt-rest, reinhardt-urls, reinhardt-test, reinhardt-dentdelion, reinhardt-openapi

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-alpha.4...reinhardt-commands@v0.1.0-alpha.5) - 2026-02-03

### Other

- merge main into chore/release-plz-migration
- add release-plz migration markers to CHANGELOGs
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

## [0.1.0-alpha.4] - 2026-01-30

### Changed

- Version bump for publish workflow correction (no functional changes)

## [0.1.0-alpha.3] - 2026-01-29

### Changed

- Update imports for `reinhardt_utils::staticfiles` module rename (#114)

## [0.1.0-alpha.2] - 2026-01-28

### Changed
- Migrated welcome page rendering from Tera to reinhardt-pages SSR
- Added reinhardt-pages dependency

### Removed
- Removed welcome.tpl template (replaced by WelcomePage component)


## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

