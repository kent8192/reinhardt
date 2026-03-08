# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-tasks@v0.1.0) - 2026-03-08

### Fixed

- *(release)* use path-only dev-dep for reinhardt-test in cyclic crates
- *(tasks)* implement weight-based ordering for Priority enum
- *(deps)* align dependency versions to workspace definitions
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

### Performance

- eliminate redundant get_task_data call

### Documentation

- add missing doc comments for public API modules and types

### Testing

- add webhook retry sleep regression test
- add regression tests for SQS lock scope, DAG cycle detection, and scheduler sleep
- apply rstest and AAA pattern to existing tests
- update scheduler integration tests for Arc API

### Styling

- apply workspace-wide formatting and clippy fixes
- apply workspace-wide formatting fixes
- apply rustfmt to reinhardt-tasks formatting

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- add explanatory comments to undocumented #[allow(...)] attributes

### Reverted

- undo PR #219 version bumps for unpublished crates

### Other

- resolve conflicts with origin/main
