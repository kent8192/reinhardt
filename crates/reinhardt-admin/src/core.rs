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
// Re-exports
pub use crate::types::{
	AdminAction, AdminActionOutcome, AdminActionRequest, AdminError, AdminResult,
	BulkDeleteRequest, BulkDeleteResponse, ColumnInfo, DashboardResponse, DetailResponse,
	ExportFormat as TypesExportFormat, FieldInfo, FieldType, FilterChoice, FilterInfo, FilterType,
	ImportResponse, ListQueryParams, ListResponse, ModelInfo, ModelPermission, MutationRequest,
	MutationResponse,
};
pub use database::{AdminDatabase, AdminDatabaseKey, AdminRecord};
/// Server-owned transaction passed to model admin action hooks.
pub type AdminActionTransaction = reinhardt_db::orm::AtomicTransaction;
pub use export::{CsvExporter, ExportBuilder, ExportConfig, ExportFormat, JsonExporter};
pub use import::{
	CsvImporter, ImportBuilder, ImportConfig, ImportError, ImportFormat, ImportResult, JsonImporter,
};
pub use model_admin::{AdminUser, ModelAdmin, ModelAdminConfig, ModelAdminConfigBuilder};
pub use router::{admin_csp_exempt_paths, admin_routes_with_di, admin_static_routes};
pub use site::{AdminSite, AdminSiteConfig, AdminSiteKey};
