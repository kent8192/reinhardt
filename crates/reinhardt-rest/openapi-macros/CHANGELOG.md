# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-rest-openapi-macros@v0.1.0) - 2026-03-08

### Fixed

- propagate parse errors and validate min/max constraints
- replace expect() with safe get_ident() handling in attribute parsing
- collapse nested if block in serde_attrs to satisfy clippy
- handle serde attributes and improve validation

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
