# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-apps@v0.1.0) - 2026-03-08

### Fixed

- *(release)* use path-only dev-dep for reinhardt-test in cyclic crates
- *(meta)* fix workspace inheritance and authors metadata
- fix TOCTOU race in is_installed and add test isolation support
- detect duplicate apps in populate() instead of silently overwriting
- replace panic with Result in register_reverse_relation
- handle Mutex poisoning gracefully in Apps registry
- handle lock poisoning and remove Box::leak memory leak

### Security

- add regex pattern length limit and fix signal lock contention
- add path validation in AppConfig::with_path

### Styling

- apply formatting to files introduced by merge from main

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- *(workspace)* remove unpublished reinhardt-settings-cli and fix stale references
