# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-pages-ast@v0.1.0) - 2026-03-08

### Changed

- replace magic string with Option<Ident> for FormMacro name

### Fixed

- *(meta)* fix workspace inheritance and authors metadata
- replace unreachable!() with proper syn::Error in parse_if_node
- detect duplicate properties in form field parsing
- add max nesting depth to page parser
- add max nesting depth to SVG icon parser
- return Option from FormFieldProperty::name instead of panicking
- add reinhardt-manouche to workspace deps and address review comments

### Documentation

- add missing doc comments for public API modules and types

### Other

- Merge branch 'main' into refactor/extract-manouche-dsl
