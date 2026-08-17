//! URL configuration for {{ project_name }} project (Pages).
//!
//! The routes function is the single project-level registration. Each
//! application exposes one url_patterns() aggregate for HTTP, WebSocket, gRPC,
//! and client routes; merge those values explicitly below.
//!
//! Module application example:
//!     let router = router.merge(crate::apps::chat::urls::url_patterns());
//!     let router = router.merge(crate::apps::accounts::urls::url_patterns());
//!
//! Workspace application example:
//!     let router = router.merge(chat::urls::url_patterns());
//!     let router = router.merge(accounts::urls::url_patterns());

use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> UnifiedRouter {
    let router = UnifiedRouter::new();

    // Add each module app explicitly (one merge per app):
    // `url_patterns()` is target-neutral; no server/client cfg branch is needed here.
    // let router = router
    //     .merge(crate::apps::notes::urls::url_patterns())
    //     .merge(crate::apps::accounts::urls::url_patterns());
    //
    // Add each workspace app explicitly (one merge per app):
    // let router = router
    //     .merge(notes::urls::url_patterns())
    //     .merge(accounts::urls::url_patterns());

    router
}
