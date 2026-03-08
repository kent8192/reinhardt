# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-urls@v0.1.0) - 2026-03-08

### Changed

- remove incorrect dead_code annotations from proxy fields
- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

### Fixed

- *(urls)* accept case-insensitive UUIDs per RFC 4122
- *(urls)* correct UUID converter test expectations for case-insensitive validation
- *(urls)* convert path-type parameters to matchit catch-all syntax in RadixTree mode
- add memory-bounded eviction to LRU route cache
- bound LRU heap growth via periodic compaction
- prevent double substitution in UrlPattern::build_url
- handle lock poisoning and improve error handling in router and URL resolution
- replace Box::leak with Arc to prevent memory leak
- add path traversal prevention with input validation

### Security

- add compile-time validation for paths, SQL, and crate references
- fix path validation for ambiguous params and wildcards
- add input validation for route paths and SQL expressions
- add ReDoS prevention and input validation
- prevent path traversal and parameter injection

### Documentation

- document wildcard pattern cross-segment matching behavior

### Testing

- *(urls)* add coverage tests for LazyLoaded clone-based get and get_if_loaded

### Styling

- *(urls)* apply project formatting to pattern module
- apply rustfmt to pre-existing unformatted files
- replace never-looping for with if-let per clippy::never_loop
- apply rustfmt formatting to workspace files
- apply code formatting to security fix files

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

### Other

- resolve conflict with main in labels.yml
