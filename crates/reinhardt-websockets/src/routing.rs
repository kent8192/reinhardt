//! WebSocket routing — re-exported from `reinhardt-core::ws`.
//!
//! The foundational types (`WebSocketRoute`, `WebSocketRouter`, etc.) live in
//! `reinhardt-core::ws` so that `reinhardt-urls` can depend on them without
//! creating a circular dependency through `reinhardt-pages`.

pub use reinhardt_core::ws::{
    RouteError, RouteResult, WebSocketRoute, WebSocketRouter, clear_websocket_router,
    get_websocket_router, register_websocket_router, reverse_websocket_url,
};
