# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.11...reinhardt-apps@v0.1.0-alpha.12) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- *(workspace)* remove unpublished reinhardt-settings-cli and fix stale references

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.10...reinhardt-apps@v0.1.0-alpha.11) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-conf

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.9...reinhardt-apps@v0.1.0-alpha.10) - 2026-02-21

### Fixed

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

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.8...reinhardt-apps@v0.1.0-alpha.9) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-conf

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.7...reinhardt-apps@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-conf

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.6...reinhardt-apps@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-conf

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.5...reinhardt-apps@v0.1.0-alpha.6) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-conf, reinhardt-di, reinhardt-server

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.4...reinhardt-apps@v0.1.0-alpha.5) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-core, reinhardt-conf, reinhardt-conf, reinhardt-http, reinhardt-di, reinhardt-server

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.3...reinhardt-apps@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di, reinhardt-conf, reinhardt-conf, reinhardt-server

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.2...reinhardt-apps@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-core, reinhardt-http, reinhardt-conf, reinhardt-conf, reinhardt-di, reinhardt-server

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-alpha.1...reinhardt-apps@v0.1.0-alpha.2) - 2026-02-03

### Other

- *(package)* replace version.workspace with explicit versions
