//! ViewSet handler implementations.
//!
//! This module is split by responsibility:
//!
//! - `view_set_handler` — `ViewSetHandler`: HTTP-method-to-action mapping
//!   and dispatch to a [`crate::ViewSet`], including DRF-compatible
//!   `kwargs`/`args` attribute handling and `405 Method Not Allowed`
//!   response shaping with a populated `Allow` header.
//! - `model_view_set_handler` — `ModelViewSetHandler`: Django REST
//!   Framework-style CRUD handler (list / retrieve / create / update /
//!   destroy) with permission checks, optional pagination, and response
//!   rendering.
//! - `error` — `ViewError` and its conversion into
//!   `reinhardt_core::exception::Error`.
//!
//! The public surface (`ViewSetHandler`, `ModelViewSetHandler`,
//! `ViewError`) is re-exported below to preserve the existing
//! `crate::viewsets::handler::*` import paths.

pub mod error;
pub mod model_view_set_handler;
pub mod view_set_handler;

pub use error::ViewError;
pub use model_view_set_handler::{ModelViewSetHandler, QuerySetProvider};
pub use view_set_handler::ViewSetHandler;
