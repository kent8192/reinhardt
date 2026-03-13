//! Hot Module Replacement (HMR) support for reinhardt-pages.
//!
//! Provides file watching and WebSocket-based browser notification for live
//! development reloading. CSS changes are applied without a full page reload,
//! while Rust source and template changes trigger a full reload.
//!
//! ## Features
//!
//! - File watching with debounce to avoid redundant reloads
//! - Change type classification (CSS hot-swap vs full reload)
//! - WebSocket server for push notifications to connected browsers
//! - Auto-injected client script for development builds
//!
//! ## Usage
//!
//! ```ignore
//! use reinhardt_pages::hmr::{HmrConfig, HmrServer};
//!
//! let config = HmrConfig::builder()
//!     .watch_path("src/")
//!     .watch_path("templates/")
//!     .debounce_ms(300)
//!     .ws_port(35729)
//!     .build();
//!
//! let server = HmrServer::new(config);
//! server.start().await;
//! ```

mod change_kind;
mod client_script;
mod config;
mod message;
mod server;
mod watcher;

pub use change_kind::ChangeKind;
pub use client_script::{HMR_CLIENT_SCRIPT, hmr_script_tag};
pub use config::{HmrConfig, HmrConfigBuilder};
pub use message::HmrMessage;
pub use server::HmrServer;
pub use watcher::FileWatcher;
