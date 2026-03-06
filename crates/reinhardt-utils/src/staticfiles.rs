//! Static files and production utilities for Reinhardt

/// Cache control configuration and middleware.
pub mod caching;
/// CDN integration for static file serving.
pub mod cdn;
/// Static file system health checks.
pub mod checks;
/// Static file dependency resolution (e.g., CSS imports).
pub mod dependency_resolver;
/// Static file request handler.
pub mod handler;
/// Health check endpoints.
pub mod health;
/// Media file handling and storage.
pub mod media;
/// Static file serving metrics.
pub mod metrics;
/// Static file middleware for request processing.
pub mod middleware;
/// Static file path resolution.
pub mod path_resolver;
/// Static file processing (minification, fingerprinting).
pub mod processing;
/// Static file storage backends.
pub mod storage;
/// Template engine integration for static file URLs.
pub mod template_integration;

/// Development static file server with live reload.
#[cfg(feature = "dev-server")]
pub mod dev_server;

pub use caching::{CacheControlConfig, CacheControlMiddleware, CacheDirective, CachePolicy};
pub use cdn::{CdnConfig, CdnInvalidationRequest, CdnProvider, CdnPurgeHelper, CdnUrlGenerator};
pub use checks::{CheckLevel, CheckMessage, check_static_files_config};
pub use dependency_resolver::DependencyGraph;
pub use handler::{StaticError, StaticFile, StaticFileHandler, StaticResult};
pub use health::{
	CacheHealthCheck, DatabaseHealthCheck, HealthCheck, HealthCheckManager, HealthCheckResult,
	HealthReport, HealthStatus,
};
pub use media::{HasMedia, Media};
pub use metrics::{Metric, MetricsCollector, RequestMetrics, RequestTimer};
pub use middleware::{StaticFilesConfig as StaticMiddlewareConfig, StaticFilesMiddleware};
pub use path_resolver::PathResolver;
pub use storage::{
	FileSystemStorage, HashedFileStorage, Manifest, ManifestStaticFilesStorage, ManifestVersion,
	MemoryStorage, StaticFilesConfig, StaticFilesFinder, Storage, StorageRegistry,
};

#[cfg(feature = "s3")]
pub use storage::{S3Config, S3Storage};

#[cfg(feature = "azure")]
pub use storage::{AzureBlobConfig, AzureBlobStorage};

#[cfg(feature = "gcs")]
pub use storage::{GcsConfig, GcsStorage};

pub use template_integration::TemplateStaticConfig;

pub use processing::{ProcessingConfig, ProcessingPipeline, ProcessingResult, Processor};

#[cfg(feature = "dev-server")]
pub use dev_server::{
	AutoReload, AutoReloadBuilder, DevServerConfig, DevelopmentErrorHandler, FileWatcher,
	FileWatcherBuilder, ReloadEvent, WatchEvent,
};
