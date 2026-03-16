# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-01-24

### Added
- Initial release of `reinhardt-storages`
- `StorageBackend` trait for unified storage API
- Local file system backend implementation (`LocalStorage`)
- Amazon S3 backend implementation (`S3Storage`)
- Configuration system with environment variable support
- Error types for storage operations
- Factory function for creating storage backends
- Presigned URL generation for S3
- Feature flags for optional backends (`s3`, `local`, `gcs`, `azure`)
- Integration tests for Local storage backend
- Comprehensive documentation and examples

### Features
- Async I/O using Tokio
- Type-safe configuration
- Support for path prefixes
- File metadata operations (size, modified time)

### Notes
- Google Cloud Storage and Azure Blob Storage backends are planned for future releases
