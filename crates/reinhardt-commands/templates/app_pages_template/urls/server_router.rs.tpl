//! Server-side URL configuration for the {{ app_name }} application.
//!
//! This router is aggregated by url_patterns() and then merged by the
//! project-level config/urls.rs registration.
//!
//! Register HTTP endpoints and server-function markers here. The generated
//! function intentionally starts empty.

use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
    ServerRouter::new()
}
