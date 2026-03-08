# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-dispatch@v0.1.0) - 2026-03-08

### Fixed

- fix dead code, default handler, and lost request context
- log signal send errors instead of silently discarding
- replace lock unwrap with poison error recovery

### Security

- add configurable middleware chain depth limit
- add content-type and nosniff headers to error responses
- prevent information disclosure in exception handler

### Styling

- apply rustfmt to pre-existing unformatted files
- apply rustfmt to pre-existing formatting violations in 16 files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
