# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-graphql-macros@v0.1.0) - 2026-03-08

### Fixed

- emit errors on crate resolution failure instead of silent fallback
- emit compile error for invalid skip_if expressions
- propagate stream errors to GraphQL clients instead of dropping
- replace expect() with proper error handling in subscription macro (#814)

### Security

- add input validation and resource limits

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
