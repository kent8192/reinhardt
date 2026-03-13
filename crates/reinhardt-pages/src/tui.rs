//! TUI Rendering Backend for reinhardt-pages.
//!
//! This module provides a terminal-based rendering backend that maps
//! reinhardt-pages `Page` / `PageElement` trees to ratatui widgets,
//! enabling reinhardt-pages applications to render in terminal environments.
//!
//! ## Feature Gate
//!
//! This module requires the `tui` feature flag:
//!
//! ```toml
//! [dependencies]
//! reinhardt-pages = { version = "...", features = ["tui"] }
//! ```
//!
//! ## Architecture
//!
//! The TUI backend consists of three main components:
//!
//! - [`TuiRenderer`]: Converts `Page` trees into ratatui widget representations
//! - [`TuiApp`]: Runtime that manages terminal initialization, event loop, and rendering
//! - [`TuiElementMapper`]: Trait for customizing HTML element to TUI widget mapping

mod app;
mod mapper;
mod renderer;
mod style;
mod widget;

pub use app::{TuiApp, TuiAppBuilder};
pub use mapper::{DefaultElementMapper, TuiElementMapper};
pub use renderer::TuiRenderer;
pub use style::StyleConverter;
pub use widget::TuiWidget;
