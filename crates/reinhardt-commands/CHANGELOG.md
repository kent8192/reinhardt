# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.15...reinhardt-commands@v0.1.0-rc.16) - 2026-04-19

### Added

- *(commands)* add createsuperuser as built-in management command
- *(commands)* add requires_database() and auto-init ORM dispatch
- *(auth)* auto-register SuperuserCreator via inventory for #[user(full = true)] + #[model] types
- *(commands)* auto-build WASM when runserver --with-pages is used
- *(urls)* [**breaking**] support async functions in #[routes] macro
- *(commands)* add RunserverHook for concurrent service startup and pre-listen validation
- *(commands)* reject reinhardt_ prefixed project and app names
- *(macros)* [**breaking**] extend #[url_patterns] for viewset/mount, add #[viewset] macro, deprecate named variants
- *(macros)* add #[export_endpoints] attribute for multi-file view modules
- *(commands)* startapp appends entry to installed_apps! block

### Changed

- *(commands)* extract initialize_orm_database() shared function
- *(commands)* simplify run_server() ORM block to DI-only

### Deprecated

- *(macros)* rename define_views! to flatten_imports! and deprecate old name
- *(macros)* fix comment and add flatten_imports! example to templates

### Fixed

- *(commands)* remove bare DB connect from createsuperuser
- *(commands)* integrate file-based fallback into makemigrations
- *(admin)* resolve CI failures blocking release-plz PR [[#3236](https://github.com/kent8192/reinhardt-web/issues/3236)](https://github.com/kent8192/reinhardt-web/issues/3236)
- *(test)* use recursive file count in collectstatic tests
- *(test)* skip hidden files in collectstatic test helper
- *(commands)* exit with error when --with-pages WASM build fails
- *(commands)* ensure admin vendor assets are collected to STATIC_ROOT
- *(commands)* resolve WASM artifact path relative to workspace root
- *(tests)* fix makemigrations and admin create test regressions
- *(commands)* address Copilot review feedback on RunserverHook
- *(macros)* [**breaking**] replace `#[export_endpoints]` with `define_views!` for stable Rust support
- *(ci)* resolve fmt and clippy violations
- *(commands)* inject project_crate_name in workspace app context and update AppLabel doctest

### Maintenance

- upgrade workspace dependencies to latest versions
- *(templates)* update scaffold to typed #[url_patterns]

### Other

- resolve conflict with main (deduplicate tests)
- resolve conflict with main in createsuperuser error message
- Detect workspace target_dir via cargo metadata in WASM builder
- Validate project directory before running makemigrations

### Styling

- *(commands)* fix formatting in createsuperuser deprecation warning
- apply cargo make auto-fix (clippy + fmt)

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.14...reinhardt-commands@v0.1.0-rc.15) - 2026-03-29

### Added

- *(examples)* remove manual WASM scripts for auto-injection

### Documentation

- update rust version references from 1.91.1 to 1.94.1

### Fixed

- *(admin)* add deferred DI registration to bridge route-server scope gap
- *(di)* register DatabaseConnection in user-provided DI context

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.13...reinhardt-commands@v0.1.0-rc.14) - 2026-03-24

### Added

- *(commands)* add --index CLI option to runserver command
- *(commands)* integrate --index into runserver execution and autoreload
- *(commands)* add --index option to collectstatic for index.html source

### Changed

- *(commands)* update project templates to use ProjectSettings
- *(templates)* remove index.html copy from wasm-finalize tasks

### Fixed

- *(deps)* consolidate colored and criterion versions to workspace dependencies
- *(middleware)* update Settings field access for CoreSettings restructuring
- suppress deprecated Settings warnings and fix unreachable pub visibility
- address Copilot review feedback
- *(ci)* resolve docs.rs and semver CI failures
- address Copilot review feedback for PR [[#2874](https://github.com/kent8192/reinhardt-web/issues/2874)](https://github.com/kent8192/reinhardt-web/issues/2874)
- *(commands)* use CLI parsing for non-exhaustive enum variants in integration tests
- *(commands)* use helper function to construct non_exhaustive Collectstatic in tests
- *(test)* account for auto-discovered app static files in backward compatibility test

### Other

- resolve conflict with main in middleware.rs

### Styling

- *(commands)* fix formatting and clippy warnings in staticfiles changes

### Testing

- *(staticfiles)* add comprehensive test coverage for index file separation

## [0.1.0-rc.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.12...reinhardt-commands@v0.1.0-rc.13) - 2026-03-18

### Added

- *(commands)* extend InfraSignals with gRPC, storage, mail, session, graphql, admin, i18n detection

### Fixed

- *(commands)* align mail and session detection with workspace feature names

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.11...reinhardt-commands@v0.1.0-rc.12) - 2026-03-18

### Added

- *(commands)* add --merge option to makemigrations command
- *(commands)* add introspect management command

### Deprecated

- *(conf)* mark Settings.installed_apps and related methods as deprecated

### Fixed

- *(commands)* address Copilot review feedback on introspect command

### Testing

- add tests for makemigrations --merge option

## [0.1.0-rc.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.10...reinhardt-commands@v0.1.0-rc.11) - 2026-03-16

### Documentation

- *(reinhardt-commands)* document --force-empty-state flag and linkme dependency

### Fixed

- *(reinhardt-commands)* add mysql branch to migrate command connection logic

## [0.1.0-rc.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.9...reinhardt-commands@v0.1.0-rc.10) - 2026-03-15

### Fixed

- *(commands)* propagate openapi-router feature to reinhardt-commands
- *(commands)* gate docs banner on openapi-router feature

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.8...reinhardt-commands@v0.1.0-rc.9) - 2026-03-15

### Fixed

- *(commands)* add features section and fix router types in templates
- *(commands)* remove redundant features section from restful template

### Styling

- add explanatory comments to remaining #[allow(dead_code)] attributes

## [0.1.0-rc.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.7...reinhardt-commands@v0.1.0-rc.8) - 2026-03-12

### Fixed

- *(commands)* generate app module as {name}.rs per Rust 2024 Edition convention
- *(commands)* only rename lib.rs for default app location, not custom targets
- collapse nested if statements in start_commands to fix clippy lint
- *(commands)* update startapp test assertions for Rust 2024 module paths

## [0.1.0-rc.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.6...reinhardt-commands@v0.1.0-rc.7) - 2026-03-11

### Changed

- *(commands)* rename template cfg attrs to cfg_aliases and add missing module roots

### Fixed

- *(commands)* add missing middleware, root_urlconf, media_root defaults to runserver and collectstatic

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.4...reinhardt-commands@v0.1.0-rc.5) - 2026-03-07

### Documentation

- add missing doc comments for public API modules and types

## [0.1.0-rc.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.2...reinhardt-commands@v0.1.0-rc.3) - 2026-03-04

### Fixed

- *(commands)* correct project template compilation errors
- *(commands)* correct app template compilation errors

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.1...reinhardt-commands@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(deps)* align dependency versions to workspace definitions
- *(staticfiles)* unify manifest.json format to use "paths" key
- *(staticfiles)* use STATIC_URL in HTML template processing

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

