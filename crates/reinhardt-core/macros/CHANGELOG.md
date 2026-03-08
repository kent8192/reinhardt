# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-core-macros@v0.1.0) - 2026-03-08

### Fixed

- *(macros)* dereference extractor before validation in pre_validate
- *(macros)* replace skeleton tests with meaningful assertions in pre_validate
- *(macros)* add auto_increment param to field registration
- *(macros)* infer not_null from Rust Option type in field registration
- *(macros)* map DateTime to TimestampTz for timezone-aware columns
- *(release)* advance version to skip yanked alpha.3 and restore publish capability for dependents
