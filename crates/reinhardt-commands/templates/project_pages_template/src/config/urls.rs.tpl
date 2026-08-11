//! URL configuration for {{ project_name }} project (Pages).
//!
//! The routes function is the single project-level registration. Each
//! application exposes one url_patterns() aggregate for HTTP, WebSocket, gRPC,
//! and client routes; merge those values explicitly below.
//!
//! Module application example:
//!     let router = router.merge(crate::apps::chat::urls::url_patterns());
//!
//! Workspace application example:
//!     let router = router.merge(chat::url_patterns());

use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> UnifiedRouter {
    let router = UnifiedRouter::new();

    // Add each module app explicitly:
    // let router = router.merge(crate::apps::your_app::urls::url_patterns());
    //
    // Add each workspace app explicitly:
    // let router = router.merge(your_app::url_patterns());

    router
}
