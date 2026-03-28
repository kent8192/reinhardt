//! # Reinhardt Deploy Engine
//!
//! Vercel-inspired deployment engine for the Reinhardt web framework.
//! Provides zero-configuration deployment with automatic infrastructure
//! detection, Terraform-backed provisioning, and multi-provider support.
//!
//! ## Architecture
//!
//! The deploy engine follows a pipeline architecture:
//!
//! 1. **Configuration** ([`config`]) — Parses `deploy.toml` for deployment settings
//! 2. **Detection** ([`detection`]) — 3-layer feature detection (feature flags,
//!    code analysis, interactive wizard)
//! 3. **Pre-flight** ([`checks`]) — Validates required tools (Terraform, Docker,
//!    provider CLIs)
//! 4. **Build** ([`build`]) — Generates Dockerfiles and builds container images
//! 5. **Generate** ([`terraform`], [`providers`]) — Produces provider-specific
//!    Terraform HCL
//! 6. **Plan/Apply** ([`pipeline`]) — Orchestrates the deployment pipeline
//! 7. **Report** ([`report`]) — Generates deployment reports in multiple formats
//!
//! ## Supported Providers
//!
//! | Provider | Module | Infrastructure |
//! |----------|--------|----------------|
//! | Docker | [`providers::docker`] | Local Docker Compose |
//! | AWS | [`providers::aws`] | ECS Fargate + RDS + ElastiCache |
//! | GCP | [`providers::gcp`] | Cloud Run + Cloud SQL + Memorystore |
//! | Fly.io | [`providers::fly_io`] | Fly Machines + Fly Postgres |
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use reinhardt_deploy::config::DeployConfig;
//! use reinhardt_deploy::providers::create_provider;
//!
//! // Load configuration from deploy.toml or use defaults
//! let config = DeployConfig::load_or_default(std::path::Path::new(".")).unwrap();
//!
//! // Create a provider and generate infrastructure files
//! let provider = create_provider(config.provider.provider_type.clone());
//! let hcl_files = provider.generate_hcl(&config).unwrap();
//!
//! for (filename, content) in &hcl_files {
//!     println!("{filename}: {} bytes", content.len());
//! }
//! ```
//!
//! ## Additional Features
//!
//! - **Cost Estimation** ([`cost`]) — Monthly cost estimates per provider
//! - **Preview Environments** ([`preview`]) — Per-PR preview deployments with TTL
//! - **CI/CD Generation** ([`ci`]) — GitHub Actions workflow generation
//! - **Rollback** ([`rollback`]) — Deployment history tracking and rollback support
//! - **Plan Diff** ([`report`]) — Idempotency verification between deployment plans
//! - **Initialization** ([`init`]) — Interactive `deploy.toml` generation

/// Docker image building and Dockerfile generation.
///
/// Provides multi-stage Dockerfile generation with support for static asset
/// compilation and optimized layer caching.
pub mod build;

/// Pre-flight checks for required tools and services.
///
/// Validates that Terraform, Docker, and provider-specific CLI tools are
/// installed and accessible before deployment begins.
pub mod checks;

/// CI/CD workflow generation.
///
/// Generates GitHub Actions workflow files for automated deployment pipelines,
/// including preview deployments and production releases.
pub mod ci;

/// Deployment configuration.
///
/// Parses `deploy.toml` files and provides typed configuration for all
/// deployment aspects including provider selection, instance sizing, database
/// setup, and environment-specific overrides.
pub mod config;

/// Monthly cost estimation engine.
///
/// Calculates estimated monthly infrastructure costs based on provider-specific
/// pricing tables, instance sizes, and enabled features.
pub mod cost;

/// Feature detection engine.
///
/// Implements a 3-layer detection strategy: Cargo feature flags, source code
/// analysis, and an interactive wizard for features that cannot be
/// auto-detected.
pub mod detection;

/// Error types for the deploy engine.
///
/// Defines [`DeployError`] for all failure modes and the [`DeployResult`]
/// type alias used throughout the crate.
pub mod error;

/// Deploy initialization.
///
/// Generates and writes `deploy.toml` configuration files with detected
/// feature settings and user-selected provider options.
pub mod init;

/// Pipeline orchestration.
///
/// Manages the deployment pipeline stages (detect, build, generate, plan,
/// apply) with result tracking and stage-level error handling.
pub mod pipeline;

/// Preview deployments.
///
/// Supports per-PR preview environments with configurable TTL, enabling
/// ephemeral deployment targets for pull request review.
pub mod preview;

/// Cloud provider implementations.
///
/// Contains provider-specific logic for Docker, AWS, GCP, and Fly.io,
/// including Terraform HCL generation, instance size mapping, and
/// pre-flight checks. See [`providers::create_provider`] for the
/// factory function.
pub mod providers;

/// Deployment reports.
///
/// Generates deployment reports in human-readable, JSON, and Markdown
/// formats. Includes plan diff functionality for verifying idempotency
/// between successive deployment plans.
pub mod report;

/// Deployment history and rollback.
///
/// Tracks deployment versions and provides rollback support for reverting
/// to previously deployed configurations.
pub mod rollback;

/// Terraform integration.
///
/// Handles HCL file generation, workspace management, and Terraform
/// command orchestration (`init`, `plan`, `apply`, `destroy`).
pub mod terraform;

pub use config::DeployConfig;
pub use error::{DeployError, DeployResult};
