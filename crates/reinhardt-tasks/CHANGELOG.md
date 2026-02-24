# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-tasks@v0.1.0-alpha.3...reinhardt-tasks@v0.1.0-alpha.4) - 2026-02-21

### Fixed

- replace println!/eprintln! with structured logging macros
- fix TTL truncation and RetryStrategy multiplier validation
- enforce concurrency limit using tokio Semaphore
- delegate task to backend in TaskQueue::enqueue
- prevent panic on integer underflow, zero-weight division, and duration overflow
- update scheduler size assertion to match current struct layout
- add SSRF protection for webhook URLs
- use Redis MULTI/EXEC transaction for atomic enqueue
- add async task execution and shutdown mechanism to Scheduler
- move PriorityTaskQueue counter to instance field
- remove SQS receipt_handle after successful message deletion
- propagate RabbitMQ metadata update errors instead of silently discarding

### Security

- add resource limits and prevent busy loops in task subsystem

### Styling

- apply workspace-wide formatting and clippy fixes
- apply workspace-wide formatting fixes
- apply rustfmt to reinhardt-tasks formatting

### Performance

- eliminate redundant get_task_data call

### Testing

- add webhook retry sleep regression test
- add regression tests for SQS lock scope, DAG cycle detection, and scheduler sleep
- apply rstest and AAA pattern to existing tests
- update scheduler integration tests for Arc API

### Maintenance

- add explanatory comments to undocumented #[allow(...)] attributes

### Reverted

- undo PR #219 version bumps for unpublished crates
- undo release PR #215 version bumps

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-tasks@v0.1.0-alpha.2...reinhardt-tasks@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-test

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-tasks@v0.1.0-alpha.1...reinhardt-tasks@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections
- *(changelog)* add missing 0.1.0-alpha.1 release entries
- *(package)* replace version.workspace with explicit versions
- N/A

### Added
- Work in progress features (not yet released)

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A


<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

