# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-views@v0.1.0) - 2026-03-08

### Fixed

- merge main and resolve CI issues
- *(views)* replace std RwLock with parking_lot to prevent poisoning panics
- *(views)* add RefUnwindSafe impl for ViewSetHandler
- *(views)* remove unsafe keyword from RefUnwindSafe impl
- replace Box::leak with Arc to prevent memory leak
- escape user input to prevent XSS
- *(views)* move tests to integration crate to break circular publish chain

### Styling

- apply formatting to migrated test files and modified source files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
