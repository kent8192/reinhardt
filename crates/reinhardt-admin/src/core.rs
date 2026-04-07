//! Core logic for Reinhardt admin panel
//!
//! This crate provides the core business logic for the admin panel,
//! including:
//! - ModelAdmin trait and configuration
//! - AdminSite registry
//! - Database operations
//! - Import/Export functionality

pub mod database;
pub mod export;
pub mod import;
pub mod model_admin;
pub mod router;
pub mod site;
#[cfg(server)]
pub mod vendor;

// Re-exports
pub use crate::types::{
	AdminError, AdminResult, BulkDeleteRequest, BulkDeleteResponse, ColumnInfo, DashboardResponse,
	DetailResponse, ExportFormat as TypesExportFormat, FieldInfo, FieldType, FilterChoice,
	FilterInfo, FilterType, ImportResponse, ListQueryParams, ListResponse, ModelInfo,
	MutationRequest, MutationResponse,
};
pub use database::{AdminDatabase, AdminRecord};
pub use export::{CsvExporter, ExportBuilder, ExportConfig, ExportFormat, JsonExporter};
pub use import::{
	CsvImporter, ImportBuilder, ImportConfig, ImportError, ImportFormat, ImportResult, JsonImporter,
};
pub use model_admin::{AdminUser, ModelAdmin, ModelAdminConfig, ModelAdminConfigBuilder};
pub use router::{admin_csp_exempt_paths, admin_routes_with_di, admin_static_routes};
pub use site::{AdminSite, AdminSiteConfig};
