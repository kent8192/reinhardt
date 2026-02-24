# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-ast@v0.1.0-alpha.5...reinhardt-pages-ast@v0.1.0-alpha.6) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-manouche

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-ast@v0.1.0-alpha.4...reinhardt-pages-ast@v0.1.0-alpha.5) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-manouche

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-ast@v0.1.0-alpha.3...reinhardt-pages-ast@v0.1.0-alpha.4) - 2026-02-21

### Fixed

- replace unreachable!() with proper syn::Error in parse_if_node
- detect duplicate properties in form field parsing
- add max nesting depth to page parser
- add max nesting depth to SVG icon parser
- return Option from FormFieldProperty::name instead of panicking

### Changed

- replace magic string with Option<Ident> for FormMacro name

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-ast@v0.1.0-alpha.2...reinhardt-pages-ast@v0.1.0-alpha.3) - 2026-02-05

### Fixed

- add reinhardt-manouche to workspace deps and address review comments

### Other

- Merge branch 'main' into refactor/extract-manouche-dsl

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-ast@v0.1.0-alpha.1...reinhardt-pages-ast@v0.1.0-alpha.2) - 2026-02-03

### Other

- *(package)* replace version.workspace with explicit versions
