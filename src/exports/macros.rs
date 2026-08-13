//! Proc-macro and attribute macro re-exports.

#[cfg(all(feature = "core", native))]
pub use reinhardt_apps::{AppConfig, AppError, AppResult, Apps};

pub use reinhardt_macros::{AppConfig, app_config, installed_apps};

#[cfg(all(feature = "conf", native))]
pub use reinhardt_macros::settings;

#[cfg(all(feature = "core", native))]
pub use reinhardt_macros::HttpError;
pub use reinhardt_macros::{Model, ModelEnum, model};

pub use reinhardt_macros::Validate;
pub use reinhardt_macros::dto;
pub use reinhardt_macros::user;

#[cfg(all(feature = "database", native))]
pub use reinhardt_macros::collect_migrations;

#[cfg(native)]
pub use reinhardt_macros::{api_view, delete, get, patch, post, put};

#[cfg(all(feature = "websockets", native))]
pub use reinhardt_macros::websocket;

#[cfg(native)]
pub use reinhardt_macros::flatten_imports;
pub use reinhardt_macros::routes;
#[cfg(native)]
pub use reinhardt_macros::viewset;

#[cfg(all(feature = "admin", native))]
pub use reinhardt_macros::admin;

pub use reinhardt_core::apply_update::ApplyUpdate;
#[cfg(native)]
pub use reinhardt_macros::{ApplyUpdate as DeriveApplyUpdate, apply_update};
